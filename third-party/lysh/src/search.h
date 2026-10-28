/* lysh search.h —— 阶段 1 的区域扫描（多线程）+ 可选的阶段 2 收尾。
 *
 * 这是阶段 1 的"产品级"入口：给一个矩形区域范围，枚举所有区域格，
 * 用 lysh_swamp_hut_in_region 求出候选小屋位置，跑完整的 lysh_phase1_check_ex，
 * 统计每一门淘汰了多少格。
 *
 * ⭐ 阶段 2 通过 `lysh_scan_opts.phase2_hook` 接上：每个 worker 线程自己持有一个
 * `lysh_search_ctx`（= 阶段 1 + 阶段 2 的全部预计算），对**每个阶段 1 幸存者**调用
 * 调用方给的钩子。钩子是**按线程串行**调用的，每个线程只碰自己的 ctx，所以钩子里的
 * ctx 不需要加锁；但钩子必须自己保证"只碰传进来的 ctx"。
 *
 * ⚠️ `phase2_hook` 是**唯一**的阶段 2 开关：为 NULL 时 worker 只累加阶段 1 统计，
 *    连 phase-2 的 ctx 都不会建（`lysh scan` 的第一遍就是这个用法 —— 它只要 funnel
 *    数字，真正的阶段 2 明细由 lysh_grade_scan 单线程回放，见 eval.c）。
 *
 * 区域枚举顺序刻意与归档工具的 Phase1FullDump.java 一致（rz 外层、rx 内层）——
 * 见 `_archive/verify_tools_*.zip`。
 */
#ifndef LYSH_SEARCH_H
#define LYSH_SEARCH_H

#include <stdint.h>

#include "eval.h"       /* lysh_search_ctx（阶段 2 的钩子要用它的完整定义） */
#include "phase1.h"

typedef struct {
    uint64_t seed;
    lysh_phase1_opts opts;
    int max_height;     /* 阶段 1 用的 maxHeight（选 -54 时生产代码传 -50） */
    int salt;           /* 0 → 用 LYSH_SWAMP_HUT_SALT */
    int threads;        /* <= 0 → 取 CPU 核数 */

    /* ---- 阶段 2（可选；NULL = 不跑阶段 2）----
     * 每个幸存者调一次 phase2_hook，返回值非 0 计入 p2_seconds/p2_accepted 之外的
     * 统计由钩子自己负责。
     *
     * ⚠️ 钩子在**每个 worker 线程**里被调用（串行，但线程之间并发）。
     *    `phase2_ctx_init` 会为每个 worker 的 ctx 调一次 —— 钩子必须用它把
     *    **每线程私有的**写游标/计数器挂到 ctx 上（`ctx->p2_slot`），
     *    绝不能直接往 `phase2_user` 指向的共享结构里写（那是数据竞争，
     *    会得到"统计与明细互相矛盾"的静默错误）。 */
    int (*phase2_hook)(void *user, lysh_search_ctx *ctx,
                       int rx, int rz, int hut_x, int hut_z);
    void *phase2_user;
    void (*phase2_ctx_init)(void *user, lysh_search_ctx *ctx, int thread_index);

    /* ---- 预建的 worker ctx 池（可选；唯一的使用者是 eval.c 的扫描会话）----
     * 非 NULL 时长度为 `threads`：worker i 直接拿 pool[i]，**不**自己
     * lysh_search_ctx_init、也**不**释放（生命周期由调用方 / 会话负责）。
     * 它同时提供阶段 1 与阶段 2 的状态：worker 的阶段 1 检查直接用
     * `pool[i]->p1`（扫描期只读，见 phase1.h），所以阶段 1 的噪声初始化也
     * 一个种子只做一次。
     * 这是"一个种子只初始化一次噪声栈"的关键：同一个池被多个带反复复用。
     * 为 NULL 时保持老行为（每个 worker 惰性建阶段 2 ctx，阶段 1 现建）。 */
    lysh_search_ctx **phase2_ctx_pool;
} lysh_scan_opts;

/* 扫描统计。**不含命中明细** —— 明细只有一条出口：阶段 2 的
 * `lysh_hut_grade.hits`（`lysh_grade_scan` 单线程回放后一次填好，见 eval.c）。
 * 这条纪律是 2026-xx 那次单遍修复定下来的，不要再往这里加 hit 列表。 */
typedef struct {
    long long scanned;
    long long accepted;
    long long reached_ladder;                            /* 过了气候门、进入洞穴梯子 */
    long long reject_hist[LYSH_P1_REJECT_KIND_N];
    /* erosion 门分层提前退出的档位命中分布（每格最多 +1）。用于报告"多少格死在第几档"。 */
    long long tier_hist[LYSH_TIER_MAX];
    double seconds;

    /* ---- 阶段 2（没有 phase2_hook 时恒为 0）---- */
    double p2_seconds;           /* 阶段 2 累计墙钟（各线程之和，不是墙钟时长） */
} lysh_scan_result;

void lysh_scan_result_init(lysh_scan_result *res);

/* 扫描 [rz0, rz1) × [rx0, rx1)。返回 0 成功，非 0 表示参数非法。 */
int lysh_scan_rect(const lysh_scan_opts *o,
                   int rx0, int rx1, int rz0, int rz1,
                   lysh_scan_result *res);


/* 扫描前 n 个区域格，枚举顺序与 Phase1FullDump.java 相同（i → rx = i%6000, rz = i/6000） */
int lysh_scan_linear(const lysh_scan_opts *o, long long n, lysh_scan_result *res);

int lysh_cpu_count(void);

#endif /* LYSH_SEARCH_H */
