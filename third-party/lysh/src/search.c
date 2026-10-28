/* lysh search.c —— 见 search.h。
 *
 * 这是什么：阶段 1 的多线程区域扫描 + 可选的阶段 2 收尾钩子。
 *   · 每个 worker 线程私有：一个 lysh_phase1、一个 lysh_scan_result（只有统计）；
 *     挂了阶段 2 钩子时再加一个 **lysh_search_ctx**（阶段 2 的全部预计算，惰性建）。
 *     如果 `lysh_scan_opts.phase2_ctx_pool` 给了预建池（eval.c 的扫描会话就是这么用的），
 *     worker 直接拿池里那一份，**不**自建也不销毁 —— 这样一个种子的噪声栈只初始化一次。
 *     线程模型：nthreads == 1 时在调用线程里内联跑，不建线程；> 1 时每带 spawn/join。
 *   · 钩子是**每线程串行**调用的，但线程之间并发 —— 钩子只许写
 *     ctx->p2_slot 这一级别的每线程私有状态（见 search.h 的 phase2_ctx_init
 *     和 README §6.12）。跨线程共享的可写状态一律禁止。
 *
 * 怎么跑：它是库代码，被 src/eval.c（产品入口）与 src/main.c 调用。
 *   CLI:     lysh scan --seed <long> --regions N
 *   回归：   `lysh scan` 的 `[funnel]` / 阶段 2 统计 + 11 个已知小屋的 Y
 *            （归档里的 phase1_full_* / hut11_e2e 工具见 _archive/verify_tools_*.zip）
 */
#include "search.h"

#include <stdlib.h>
#include <string.h>

#include "eval.h"           /* lysh_search_ctx：阶段 2 的钩子持有它 */
#include "structure.h"

#if defined(_WIN32)
#  define WIN32_LEAN_AND_MEAN
#  include <windows.h>
#else
#  include <pthread.h>
#  include <time.h>
#  include <unistd.h>
#endif

static double lysh_now_ms(void) {
#if defined(_WIN32)
    return (double)GetTickCount64();
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1.0e6;
#endif
}

/* ------------------------------------------------------------------ */
/* 极简线程抽象（只需要"创建 + 等待"两个原语，避免引入依赖）              */
/* ------------------------------------------------------------------ */
typedef struct {
    void (*fn)(void *);
    void *arg;
#if defined(_WIN32)
    HANDLE handle;
#else
    pthread_t handle;
#endif
} lysh_thread;

#if defined(_WIN32)
static DWORD WINAPI thread_trampoline(LPVOID p) {
    lysh_thread *t = (lysh_thread *)p;
    t->fn(t->arg);
    return 0;
}
static int thread_start(lysh_thread *t, void (*fn)(void *), void *arg) {
    t->fn = fn;
    t->arg = arg;
    t->handle = CreateThread(NULL, 0, thread_trampoline, t, 0, NULL);
    return t->handle ? 0 : -1;
}
static void thread_join(lysh_thread *t) {
    if (t->handle) { WaitForSingleObject(t->handle, INFINITE); CloseHandle(t->handle); t->handle = NULL; }
}
int lysh_cpu_count(void) {
    SYSTEM_INFO si;
    GetSystemInfo(&si);
    return (int)si.dwNumberOfProcessors;
}
#else
static void *thread_trampoline(void *p) {
    lysh_thread *t = (lysh_thread *)p;
    t->fn(t->arg);
    return NULL;
}
static int thread_start(lysh_thread *t, void (*fn)(void *), void *arg) {
    t->fn = fn;
    t->arg = arg;
    return pthread_create(&t->handle, NULL, thread_trampoline, t) == 0 ? 0 : -1;
}
static void thread_join(lysh_thread *t) { pthread_join(t->handle, NULL); }
int lysh_cpu_count(void) {
    long n = sysconf(_SC_NPROCESSORS_ONLN);
    return n > 0 ? (int)n : 1;
}
#endif

/* ------------------------------------------------------------------ */
/* 结果容器                                                            */
/* ------------------------------------------------------------------ */
void lysh_scan_result_init(lysh_scan_result *res) {
    memset(res, 0, sizeof(*res));
}

/* 把一次子扫描的统计并进 res（lysh_scan_linear 用它拼"整行 + 尾行"）。 */
static void lysh_scan_result_add(lysh_scan_result *res, const lysh_scan_result *part) {
    res->scanned += part->scanned;
    res->accepted += part->accepted;
    res->reached_ladder += part->reached_ladder;
    res->p2_seconds += part->p2_seconds;
    for (int i = 0; i < LYSH_P1_REJECT_KIND_N; i++) res->reject_hist[i] += part->reject_hist[i];
    for (int i = 0; i < LYSH_TIER_MAX; i++)        res->tier_hist[i]   += part->tier_hist[i];
}

/* ------------------------------------------------------------------ */
/* 工作线程                                                            */
/* ------------------------------------------------------------------ */
typedef struct {
    const lysh_scan_opts *o;
    int rx0, rx1, rz0, rz1;
    int salt;
    int index;                  /* worker 下标（阶段 2 的每线程挂载点要用） */
    lysh_scan_result local;     /* 线程私有统计（免锁） */
    lysh_search_ctx *ctx;       /* 线程私有：阶段 2 需要（惰性建，或取自 phase2_ctx_pool） */
    int ctx_ok;
} worker_t;

static void worker_run(void *arg) {
    worker_t *w = (worker_t *)arg;
    const lysh_scan_opts *o = w->o;

    /* 阶段 1 状态：给了预建池（= eval.c 的扫描会话）就复用本 worker 那一份 ctx 的
     * `p1` —— 它在 lysh_search_ctx_init 里已经初始化过了，所以一个种子在整个带循环里
     * 只付一次阶段 1 噪声初始化（实测 66 us/次，256 带就是 17 ms，是修完之后剩下的
     * 主要每带开销）。为什么复用是**逐位等价**的：
     *   · lysh_phase1_init(seed, opts) 是确定性的，池里的 p1 与现建的一模一样；
     *   · 扫描期只读 —— lysh_phase1_check_ex 的形参就是 `const lysh_phase1 *`
     *     （phase1.h），编译期即保证不可能改写；
     *   · 每个 worker 只碰 pool[index] 那一份，隔离性与"本线程局部变量"完全相同。
     * 没有池时（CLI `lysh scan` 的第一遍等）保持老行为：本线程现建一份。 */
    lysh_phase1 local_p1;
    lysh_phase1 *p1 = &local_p1;
    if (o->phase2_ctx_pool) {
        p1 = &o->phase2_ctx_pool[w->index]->p1;
    } else {
        lysh_phase1_init(p1, o->seed, &o->opts);
    }

    lysh_phase1_result r;
    for (int rz = w->rz0; rz < w->rz1; rz++) {
        for (int rx = w->rx0; rx < w->rx1; rx++) {
            lysh_chunkpos pos = lysh_swamp_hut_in_region(o->seed, rx, rz, w->salt);
            int hut_x = pos.chunkX * 16;
            int hut_z = pos.chunkZ * 16;

            int accept = lysh_phase1_check_ex(p1, &o->opts, hut_x, hut_z, o->max_height,
                                              &r, w->local.tier_hist);

            w->local.scanned++;
            if (r.reject >= 0 && r.reject < LYSH_P1_REJECT_KIND_N) w->local.reject_hist[r.reject]++;
            if (r.reject > LYSH_P1_REJECT_RIDGE) w->local.reached_ladder++;
            if (!accept) continue;

            w->local.accepted++;

            /* 没有阶段 2 钩子 = 只要阶段 1 的 funnel 数字：连 ctx 都不建。
             * （`lysh scan` 的第一遍就是这一条；真正的阶段 2 由 lysh_grade_scan
             *   单线程回放，明细也只有那一条出口。） */
            if (!o->phase2_hook) continue;

            /* ---- 阶段 2：每个幸存者过一遍评估钩子 ---- */
            if (!w->ctx_ok) {
                if (o->phase2_ctx_pool) {
                    /* 预建池（扫描会话）：worker 不自建、不销毁 —— 一个种子
                     * 的噪声栈只在 lysh_scan_session_open 里初始化一次。 */
                    w->ctx = o->phase2_ctx_pool[w->index];
                } else {
                    w->ctx = (lysh_search_ctx *)calloc(1, sizeof(lysh_search_ctx));
                    if (w->ctx) lysh_search_ctx_init(w->ctx, o->seed, &o->opts, o->max_height);
                }
                if (w->ctx) {
                    /* 钩子在这里挂上自己的**每线程**私有游标（无锁的关键） */
                    if (o->phase2_ctx_init) o->phase2_ctx_init(o->phase2_user, w->ctx, w->index);
                    w->ctx_ok = 1;
                }
            }
            if (!w->ctx_ok) continue;   /* 内存不够：钩子拿不到 ctx，只能跳过（钩子自己记账） */

            double t0 = lysh_now_ms();
            /* 返回值只是"这个候选过了阶段 2"的示意；权威的统计与明细一律由钩子自己
             * 负责（它持有调用方给的 output slot）—— 见 eval.c 的 grade_scan_on_survivor。 */
            (void)o->phase2_hook(o->phase2_user, w->ctx, rx, rz, hut_x, hut_z);
            w->local.p2_seconds += (lysh_now_ms() - t0) / 1000.0;
        }
    }
}

/* ------------------------------------------------------------------ */

int lysh_scan_rect(const lysh_scan_opts *o,
                   int rx0, int rx1, int rz0, int rz1,
                   lysh_scan_result *res) {
    if (!o || !res || rx1 <= rx0 || rz1 <= rz0) return 1;

    lysh_scan_result_init(res);
    int nthreads = o->threads > 0 ? o->threads : lysh_cpu_count();
    long long rows = (long long)rz1 - rz0;
    if ((long long)nthreads > rows) nthreads = (int)rows;
    if (nthreads < 1) nthreads = 1;

    worker_t *ws = (worker_t *)calloc((size_t)nthreads, sizeof(worker_t));
    lysh_thread *ts = (lysh_thread *)calloc((size_t)nthreads, sizeof(lysh_thread));
    int *spawned = (int *)calloc((size_t)nthreads, sizeof(int));
    if (!ws || !ts || !spawned) { free(ws); free(ts); free(spawned); return 2; }

    double t_start = lysh_now_ms();

    for (int t = 0; t < nthreads; t++) {
        /* 按行静态分块；阶段 1 的代价几乎只取决于 erosion 门，行与行之间差异很小 */
        int r0 = rz0 + (int)(rows * t / nthreads);
        int r1 = rz0 + (int)(rows * (t + 1) / nthreads);
        ws[t].o = o;
        ws[t].rx0 = rx0; ws[t].rx1 = rx1;
        ws[t].rz0 = r0;  ws[t].rz1 = r1;
        ws[t].index = t;
        ws[t].salt = o->salt ? o->salt : LYSH_SWAMP_HUT_SALT;
        lysh_scan_result_init(&ws[t].local);
    }
    if (nthreads == 1) {
        /* 单线程：不建线程，直接在调用线程里跑完（扫描会话的 threads == 1 路径）。
         * 这样"一个种子一次初始化"就是字面意义上的一次，且带循环里没有线程开销。 */
        worker_run(&ws[0]);
    } else {
        for (int t = 0; t < nthreads; t++) {
            if (thread_start(&ts[t], worker_run, &ws[t]) != 0) {
                worker_run(&ws[t]);          /* 起不来就在本线程跑掉，保证结果正确 */
            } else {
                spawned[t] = 1;
            }
        }
    }
    for (int t = 0; t < nthreads; t++) {
        if (spawned[t]) thread_join(&ts[t]);
        res->scanned += ws[t].local.scanned;
        res->accepted += ws[t].local.accepted;
        res->reached_ladder += ws[t].local.reached_ladder;
        res->p2_seconds += ws[t].local.p2_seconds;
        for (int i = 0; i < LYSH_P1_REJECT_KIND_N; i++) {
            res->reject_hist[i] += ws[t].local.reject_hist[i];
        }
        for (int i = 0; i < LYSH_TIER_MAX; i++) {
            res->tier_hist[i] += ws[t].local.tier_hist[i];
        }
        if (ws[t].ctx && !o->phase2_ctx_pool) {   /* 阶段 2 的线程私有 ctx（池里的不归这里管） */
            lysh_search_ctx_free(ws[t].ctx);
            free(ws[t].ctx);
        }
        ws[t].ctx = NULL;
    }

    res->seconds = (lysh_now_ms() - t_start) / 1000.0;

    free(ws);
    free(ts);
    free(spawned);
    return 0;
}

int lysh_scan_linear(const lysh_scan_opts *o, long long n, lysh_scan_result *res) {
    if (n <= 0) return 1;
    /* 与归档工具 Phase1FullDump.java 相同的线性枚举：rx = i%6000, rz = (i/6000)%6000。
     * 用矩形扫描实现时，"整行 + 尾行"两段合起来正好是同一批格子且顺序一致。 */
    long long full_rows = n / 6000;
    int tail = (int)(n % 6000);
    double t_start = lysh_now_ms();

    lysh_scan_result_init(res);

    if (full_rows > 0) {
        lysh_scan_result whole;
        if (lysh_scan_rect(o, 0, 6000, 0, (int)full_rows, &whole)) return 1;
        lysh_scan_result_add(res, &whole);
    }
    if (tail > 0) {
        lysh_scan_result t2;
        if (lysh_scan_rect(o, 0, tail, (int)full_rows, (int)full_rows + 1, &t2)) return 1;
        lysh_scan_result_add(res, &t2);
    }
    res->seconds = (lysh_now_ms() - t_start) / 1000.0;
    return 0;
}
