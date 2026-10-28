/* lysh eval.c —— 见 eval.h。产品路径的唯一编排层。
 *
 * 这里**不实现任何世界生成数学**，只做：
 *   1. 朝向（唯一一份配方，见 eval.h 顶部）
 *   2. footprint 63 列的 avg_y 与灌水判定（调用 phase2 / aquifer 的既有函数）
 *   3. 门槛比较
 * 所以它不可能与已被对拍过的阶段 1/阶段 2 产生分歧。
 */
#include "eval.h"

#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "column_top.h"
#include "rng.h"
#include "search.h"
#include "structure.h"

#if defined(_WIN32)
#  define WIN32_LEAN_AND_MEAN
#  include <windows.h>
#else
#  include <time.h>
#endif

static double eval_now_ms(void) {
#if defined(_WIN32)
    return (double)GetTickCount64();
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1.0e6;
#endif
}

/* ------------------------------------------------------------------ */
/* 朝向                                                                */
/* ------------------------------------------------------------------ */
int lysh_hut_orientation(uint64_t world_seed, int hut_x, int hut_z) {
    /* r = WorldgenRandom(new LegacyRandomSource(0));
     * r.setLargeFeatureSeed(worldSeed, chunkX, chunkZ);   // = 两次 nextLong 后异或
     * dir = r.nextInt(4);                                 // 0=N 1=E 2=S 3=W
     * 播种部分与结构放置用的 lysh_set_carver_seed 相同（同一个 s）；
     * 区别只在第一次抽样用 nextInt(4) 而不是 nextFloat()。 */
    lysh_lcg r;
    lysh_set_carver_seed(&r, world_seed, hut_x >> 4, hut_z >> 4);
    return lysh_lcg_next_int_bound(&r, 4);
}

/* ------------------------------------------------------------------ */
/* ctx                                                                 */
/* ------------------------------------------------------------------ */
/* 含水层的列顶回调：user 指向 ctx->p2.terrain（`preliminarySurfaceLevel`）。 */
static int eval_column_top_cb(void *user, int x, int z) {
    return lysh_preliminary_surface_level((const lysh_terrain *)user, x, z);
}

/* 群系门的列顶回调：user 指向 ctx->p2，给出 **WORLD_SURFACE_WG** 高度。
 * 口径（见 biome.h 的取证）：getFirstOccupiedHeight = getBaseHeight(WORLD_SURFACE_WG) - 1，
 * 而 lysh_phase2_height_pre_carver 就是那个 NOT_AIR 谓词的 getBaseHeight（返回 y+1）。
 * ⚠️ 这里必须用**雕刻前**的口径：`getBaseHeight` 只吃 RandomState 的密度函数
 * （`iterateNoiseColumn`，没有 ChunkAccess），看不到 CARVERS（见 carver.h 顶部）。
 *     footprint 的 63 列则相反，读的是 FEATURES 开头重量过的 POST-carver 高度图。 */
static int eval_biome_height_cb(void *user, int x, int z) {
    return lysh_phase2_height_pre_carver((lysh_phase2 *)user, x, z);
}

void lysh_search_ctx_init(lysh_search_ctx *ctx, uint64_t world_seed,
                          const lysh_phase1_opts *opts, int max_height) {
    memset(ctx, 0, sizeof(*ctx));
    ctx->seed = world_seed;
    if (opts) ctx->opts = *opts;
    ctx->max_height = max_height;
    ctx->large_biomes = ctx->opts.large_biomes;

    lysh_phase1_init(&ctx->p1, world_seed, &ctx->opts);

    lysh_phase2_init(&ctx->p2, world_seed, ctx->large_biomes);
    /* 含水层的 skipSamplingAboveY 需要列顶回调；这里把它接上（与 hut_judge.c /
     * dencol_diff.c 完全一致）。user 是 terrain 本身，phase2 生命周期内有效。 */
    lysh_phase2_set_column_top(&ctx->p2, eval_column_top_cb, &ctx->p2.terrain);
    /* CARVERS 层对**所有版本**都打开（26.2 / 1.21 / 1.20.1 / 1.19.2 / 1.18.2）。
     * 依据（逐字节对过，见 carver.h 顶部）：
     *   · **播种与版本无关**：1.18.2 的 `cuv.c(JII)`、1.19.2 的 `dbo.c(JII)`、
     *     1.20 的 `dij.c(JII)`、1.21 的 `dzx.c(JII)`、26.1.2 的
     *     `WorldgenRandom.setLargeFeatureSeed(JII)` 是**逐条相同的字节码**
     *     （setSeed(seed); l=nextLong(); m=nextLong();
     *      setSeed(chunkX*l ^ chunkZ*m ^ seed)，**没有** `| 1L`），
     *     调用点也都是 `setLargeFeatureSeed(seed + carverIndex, ox, oz)` 落在
     *     硬编码的 -8..8 双循环里。所以旧版分支没有"另一套播种"。
     *   · **配置在 1.20~26.1.2 完全一致**（从官方 client jar 里取出的
     *     data/minecraft/worldgen/configured_carver/{cave,cave_extra_underground,canyon}.json
     *     逐字段相同：0.15 / 0.07 / 0.01，y 范围 above_bottom(8)..180、..47、10..67）。
     *     ⚠️ **1.18.2 与 1.19.2 的 configured_carver 在本地无法验证**：那两个版本的
     *     client jar 不带 worldgen 数据（数据在 server jar 里），本机没有对应的
     *     server jar，也不能联网下载。因此本层对 1.18.2/1.19.2 用的是**同一套
     *     26.1.2 配置**（cubiomes 的独立实现 `c_cave_118` 也恰好是这些值，
     *     即 1.18 起未变；若某天拿到那两个 jar，必须重新逐字段核对）。 */
    lysh_phase2_set_carvers(&ctx->p2, 1);
    /* 真实群系门只要两个噪声（温度 / 植被）加一棵参数树；NORMAL 用
     * minecraft:temperature / minecraft:vegetation，LARGE_BIOMES 用 _large 变体。
     * 参数树必须按版本选（1.18.2 没有 mangrove_swamp，见 biome.h）。 */
    int tree_sel = ctx->opts.mc_1_18_2 ? LYSH_BIOME_TREE_1_18
                 : (ctx->opts.pre_26_2 ? LYSH_BIOME_TREE_1_21_5 : LYSH_BIOME_TREE_26_2);
    lysh_biome_init(&ctx->biome, world_seed, ctx->large_biomes, tree_sel);
    ctx->inited = 1;
}

void lysh_search_ctx_free(lysh_search_ctx *ctx) {
    if (!ctx) return;
    if (ctx->inited) lysh_phase2_free(&ctx->p2);
    ctx->inited = 0;
}

/* ------------------------------------------------------------------ */
/* 单候选评估                                                          */
/* ------------------------------------------------------------------ */
lysh_hut_result lysh_eval_hut(lysh_search_ctx *ctx, int hut_x, int hut_z, int max_y) {
    lysh_hut_result r;
    memset(&r, 0, sizeof(r));
    r.hut_x = hut_x;
    r.hut_z = hut_z;
    /* 区域格：生产代码是 hutX/32（regionX*32+j 的逆），负数用 floorDiv */
    r.rx = hut_x / 32; if (hut_x < 0 && hut_x % 32) r.rx--;
    r.rz = hut_z / 32; if (hut_z < 0 && hut_z % 32) r.rz--;

    r.dir = lysh_hut_orientation(ctx->seed, hut_x, hut_z);
    r.axis_z = (r.dir == 0 || r.dir == 2);
    r.size_x = r.axis_z ? 7 : 9;
    r.size_z = r.axis_z ? 9 : 7;

    /* ---- 真实群系门（游戏里的顺序：isValidBiome 先于放置）----
     * chunk 中心列地表是不是沼泽系群系。阶段 1 的气候门只是近似，少了这一门会多报
     * 候选（实测全图 4 个）。成本只有 7593×6×2 次整数运算，远小于下面 63 列的阶段 2，
     * 而阶段 1 幸存者本来就极少（全图 15 个），所以这里照旧把 footprint 也算出来，
     * 让 `lysh hut` 能同时给出两组证据。 */
    lysh_biome_result br;
    lysh_swamp_hut_biome_ok(&ctx->biome, &ctx->p2.terrain, hut_x, hut_z,
                            eval_biome_height_cb, &ctx->p2, &br);
    r.biome_ok = br.ok;
    r.biome_winner = br.winner;
    r.biome_qx = br.target.qx;
    r.biome_qy = br.target.qy;
    r.biome_qz = br.target.qz;
    r.biome_occ_y = br.target.occ_y;
    for (int i = 0; i < 6; i++) r.biome_t[i] = br.target.t[i];

    /* ---- y=62 的含水层判定：**先算**，因为它与雕刻器无关 ----
     * 判定口径（README / hut_judge.c）：computeFluid(x,62,z).at(62)==WATER
     * —— 直接问 aquifer，不经过密度，避免密度误差污染判定。
     * computeFluid 只吃 column_top 回调 + 岩浆噪声，雕刻器既不读也不写含水层网格，
     * 所以这个判定是先算后算都一样（这正是下面那条提前短路的正确性来源）。 */
    int wet = 0;
    for (int dx = 0; dx < r.size_x; dx++) {
        for (int dz = 0; dz < r.size_z; dz++) {
            int x = hut_x + dx, z = hut_z + dz;
            int level;
            lysh_block_state type;
            lysh_aquifer_compute_fluid(&ctx->p2.aquifer, x, 62, z, &level, &type);
            if (type == LYSH_BLOCK_WATER && 62 < level) wet++;
        }
    }
    r.wet_columns = wet;
    r.flooded = (wet == 63);
    r.all_dry = (wet == 0);

    /* ---- 63 列的精确高度 ----
     * ⚠️ 口径：小屋读的 `getHeightmapPos(MOTION_BLOCKING_NO_LEAVES)` 是 **CARVERS
     *    之后**的高度（FEATURES 开头 primeHeightmaps 从活方块重量），所以走
     *    `lysh_phase2_height`（= 雕刻后），见 carver.h。
     *
     * 提前短路（避开整层雕刻）：
     *   · 逐列 post 列顶 <= pre 列顶：pre 口径已经把含水层的水算进去了（`block_at`
     *     = 密度 + 含水层），而 carveBlock 只在**可替换**（实心/水）的格子上写；
     *     列顶之上的格子恒为空气、不可替换 ⇒ 那里写不进东西 ⇒ 高度只会降不会升。
     *     所以 **Y 门绝不能用来跳过雕刻** —— pre 高于门槛的候选可能被雕到门槛之下，
     *     必须照雕（原本就是这个行为，没动）。
     *   · 灌水判定与雕刻无关（见上），所以"整片 63 列在 y=62 已经灌满"的候选可以先判掉。
     * guard（阈值 **64**）：只要某列**雕刻前的列顶方块 >= 64**（即 `pre_h - 1 >= 64`），
     *     就认为该列顶部落在雕刻器能改写的高度里，雕刻可能削低列顶 ⇒ 不短路、照常雕刻。
     *     63 列的列顶方块全部 <= 63 时才短路（海平面水层顶在 y=62，水柱最高顶到 y=63）。
     * 阈值由 62 提到 64（业主决定）：62 会把"列顶方块正好在 y=62"的灌水候选也判成不可短路，
     *     实测 100M 格扫描里 28 个灌水候选因此**一次都没触发**短路（0/28）；
     *     提到 64 后同一批里 18/28 走短路，阶段 2 的 CPU 从 219 ms 降到 155 ms。
     * 取舍（有意为之）：走短路的候选不雕刻，它报告的 sum_h/avg_y 因此是**雕刻前**的，
     *    reject 也一律记 FLOOD（老口径下"pre 也高于门槛"的那批会记成 Y）。
     *    两类结果都已被拒绝、ACCEPT 集合**完全不变**，只有诊断口径移动。 */
    long long sum = 0;
    int carve_skipped = 0;
    if (r.flooded) {
        /* guard 有**早退**：只要有一列雕刻前列顶方块 >= 64 就立刻放弃短路（后面的列不用再看）。
         * 不早退的话，每次都要多付一整遍 63 列雕刻前高度扫描（~5 ms）—— 短路候选只付这一遍，
         * 不再走整层雕刻。 */
        int blocked = 0;                        /* 1 = 有列顶方块 >= 64 */
        long long sum_pre = 0;
        for (int dx = 0; dx < r.size_x && !blocked; dx++) {
            for (int dz = 0; dz < r.size_z; dz++) {
                int hp = lysh_phase2_height_pre_carver(&ctx->p2, hut_x + dx, hut_z + dz);
                sum_pre += hp;
                if (hp - 1 >= 64) { blocked = 1; break; }
            }
        }
        if (!blocked) {                         /* 63 列列顶方块全 <= 63：短路 */
            sum = sum_pre;
            carve_skipped = 1;
        }
    }
    if (!carve_skipped) {
        for (int dx = 0; dx < r.size_x; dx++) {
            for (int dz = 0; dz < r.size_z; dz++) {
                sum += lysh_phase2_height(&ctx->p2, hut_x + dx, hut_z + dz);
            }
        }
    }
    r.sum_h = sum;
    r.avg_y = (int)(sum / 63);      /* C 的整数除法：向零截断（= oracle 的口径） */

    if (!r.biome_ok)     { r.ok = 0; r.reject = LYSH_HUT_REJECT_BIOME; }
    else if (r.flooded)  { r.ok = 0; r.reject = LYSH_HUT_REJECT_FLOOD; }
    else if (r.avg_y > max_y) { r.ok = 0; r.reject = LYSH_HUT_REJECT_Y; }
    else                 { r.ok = 1; r.reject = LYSH_HUT_OK; }
    return r;
}

/* 与 Java 产品 `SearchCoords.Result.toString()` 完全同一口径的一行。
 * avg_y 在 26.1.2 的语义下就是 footprint 平均高度，`%.0f` 对整数不改变数值，
 * 所以这里直接按整数打印（Java 侧还有 -1 的偏移，见 README §2.5.6）。 */
const char *lysh_hut_tp_line(const lysh_hut_result *r) {
    static char buf[96];
    snprintf(buf, sizeof(buf), "/tp %d %.0f %d", r->hut_x, (double)r->avg_y, r->hut_z);
    return buf;
}

/* ------------------------------------------------------------------ */
/* 扁平成绩单                                                          */
/* ------------------------------------------------------------------ */
static void grade_fill_hit(int *dst, const lysh_hut_result *r) {
    dst[0] = r->hut_x;
    dst[1] = r->hut_z;
    dst[2] = r->rx;
    dst[3] = r->rz;
    dst[4] = r->dir;
    dst[5] = r->avg_y;
    dst[6] = r->flooded;
    dst[7] = r->ok;
    dst[8] = r->wet_columns;
    dst[9] = r->size_x;
    dst[10] = r->size_z;
    dst[11] = r->reject;
}

/* 阶段 1 幸存者钩子：把阶段 2 的评估塞进 lysh_scan_rect 的 worker 里，
 * 这样"阶段 1 之后再做阶段 2"就是内核里的同一条路径，CLI 与 JNI 都只调它。
 *
 * ⚠️ 最要紧的一点：钩子是**多线程并发**调用的（每个 worker 一个线程）。
 *    `lysh_hut_grade` 里的 accepted/evaluated 是必须原子累加的计数器，
 *    而 `hits` 还需要一个"写到哪了"的游标 —— 直接在共享结构上写就是数据竞争，
 *    会静默产生"统计说 0、明细有 1"这种自相矛盾的结果（实测踩过）。
 *    所以：
 *      ① worker 线程里只往 **ctx 上挂的私有 slot** 记账 + 存一份"决定"；
 *      ② 扫完之后**单线程**按 slot 顺序回放这些决定、填明细。
 *    回放是纯读取 + 一次 lysh_eval_hut（同一个 ctx 完全确定性），所以结果
 *    与在 worker 里直接填完全一致，但没有任何并发写。 */
typedef struct {
    int hut_x, hut_z;           /* 该 worker 决定过的候选（按评估顺序） */
    int ok;                     /* 1 = 通过（只有通过的要回放） */
} grade_decision_t;

#define GRADE_SLOT_MAX_DECISIONS 1024

typedef struct {
    int max_y;
    int n_decisions;            /* 每线程：记下的决定数（含被拒的，回放时只填 ok） */
    int n_ok;                   /* 每线程：通过数（= 需要回放的条数） */
    long long evaluated;        /* 每线程：评估数 */
    long long rejected_y, rejected_flood, rejected_biome;
    grade_decision_t decisions[GRADE_SLOT_MAX_DECISIONS];
} grade_slot_t;

typedef struct {
    int max_y;
    int nthreads;
    grade_slot_t *slots;
    lysh_hut_grade shared;      /* 只读：hits / hits_cap 由调用方给 */
} grade_scan_hook_t;

/* 每个 worker 第一次需要阶段 2 时调用：给它的 ctx 挂一份私有 slot */
static void grade_scan_ctx_init(void *user, lysh_search_ctx *ctx, int thread_index) {
    grade_scan_hook_t *h = (grade_scan_hook_t *)user;
    if (thread_index < 0 || thread_index >= h->nthreads) return;
    grade_slot_t *s = &h->slots[thread_index];
    memset(s, 0, sizeof(*s));
    s->max_y = h->max_y;
    ctx->p2_slot = s;
}

static int grade_scan_on_survivor(void *user, lysh_search_ctx *ctx,
                                  int rx, int rz, int hut_x, int hut_z) {
    grade_scan_hook_t *h = (grade_scan_hook_t *)user;
    (void)rx; (void)rz;
    grade_slot_t *s = (grade_slot_t *)ctx->p2_slot;
    if (!s) {
        /* 没有 slot（不该发生）：退回只判定、不记明细 */
        lysh_hut_result r0 = lysh_eval_hut(ctx, hut_x, hut_z, h->max_y);
        return r0.ok;
    }
    lysh_hut_result r = lysh_eval_hut(ctx, hut_x, hut_z, s->max_y);
    s->evaluated++;
    if (r.ok) s->n_ok++;
    else if (r.reject == LYSH_HUT_REJECT_BIOME) s->rejected_biome++;
    else if (r.reject == LYSH_HUT_REJECT_Y) s->rejected_y++;
    else if (r.reject == LYSH_HUT_REJECT_FLOOD) s->rejected_flood++;

    /* 记决定（只有通过的需要回放；slot 满了就丢掉明细但保留计数） */
    if (r.ok && s->n_decisions < GRADE_SLOT_MAX_DECISIONS) {
        s->decisions[s->n_decisions].hut_x = hut_x;
        s->decisions[s->n_decisions].hut_z = hut_z;
        s->decisions[s->n_decisions].ok = 1;
        s->n_decisions++;
    }
    return r.ok;
}

/* ------------------------------------------------------------------ */
/* 扫描会话（见 eval.h 末尾）                                          */
/* ------------------------------------------------------------------ */
struct lysh_scan_session {
    uint64_t seed;
    lysh_phase1_opts opts;
    int max_height;
    int salt;
    int threads;                /* 已解析的 worker 数（>= 1） */
    int nctx;
    lysh_search_ctx **ctx;      /* 每个 worker 一份；open 时建好，跨 band 复用 */
};

lysh_scan_session *lysh_scan_session_open(uint64_t world_seed,
                                          const lysh_phase1_opts *opts,
                                          int max_height, int salt, int threads) {
    int want = threads > 0 ? threads : lysh_cpu_count();
    if (want < 1) want = 1;

    lysh_scan_session *s = (lysh_scan_session *)calloc(1, sizeof(*s));
    if (!s) return NULL;
    s->ctx = (lysh_search_ctx **)calloc((size_t)want, sizeof(*s->ctx));
    if (!s->ctx) { free(s); return NULL; }

    s->seed = world_seed;
    if (opts) s->opts = *opts;
    s->max_height = max_height;
    s->salt = salt ? salt : LYSH_SWAMP_HUT_SALT;
    s->threads = want;
    s->nctx = want;

    /* ⚠️ 会话存在的唯一理由：每个 worker 的噪声栈**只在这里初始化一次**。
     * 之后无论扫多少个带，都不会再付一次初始化的代价。 */
    for (int i = 0; i < want; i++) {
        s->ctx[i] = (lysh_search_ctx *)calloc(1, sizeof(*s->ctx[i]));
        if (!s->ctx[i]) { lysh_scan_session_free(s); return NULL; }
        lysh_search_ctx_init(s->ctx[i], world_seed, &s->opts, max_height);
    }
    return s;
}

void lysh_scan_session_free(lysh_scan_session *s) {
    if (!s) return;
    if (s->ctx) {
        for (int i = 0; i < s->nctx; i++) {
            if (!s->ctx[i]) continue;
            lysh_search_ctx_free(s->ctx[i]);
            free(s->ctx[i]);
        }
        free(s->ctx);
    }
    free(s);
}

/* 用一个会话已建好的 ctx 扫一个带，填 *grade（语义与老的 lysh_grade_scan 逐字段一致）。 */
void lysh_scan_session_band(lysh_scan_session *s,
                            int rx0, int rx1, int rz0, int rz1,
                            int max_y, lysh_hut_grade *grade) {
    if (!s) { if (grade) memset(grade, 0, sizeof(*grade)); return; }

    grade_scan_hook_t h;
    memset(&h, 0, sizeof(h));
    h.max_y = max_y;
    if (grade) {
        h.shared.hits = grade->hits;
        h.shared.hits_cap = grade->hits_cap;
    }

    /* 每个带开头清掉上一带挂上的每线程游标：slot 数组在本带末尾就释放了，
     * 而池里的 ctx 会活到下一个带，不清就是悬垂指针。 */
    for (int i = 0; i < s->nctx; i++) s->ctx[i]->p2_slot = NULL;

    lysh_scan_opts o;
    memset(&o, 0, sizeof(o));
    o.seed = s->seed;
    o.opts = s->opts;
    o.max_height = s->max_height;
    o.salt = s->salt;
    o.threads = s->threads;
    o.phase2_hook = grade_scan_on_survivor;
    o.phase2_user = &h;
    /* 预建的 worker ctx：worker 不再自己 init，也不释放（生命周期归本会话）。 */
    o.phase2_ctx_pool = s->ctx;

    /* worker 数由 search.c 决定（= min(请求/核数, 行数)）；这里给足 slot。
     * 池子按 s->threads 建，所以下标一定落在池内。 */
    int want = s->threads;
    long long rows = (long long)rz1 - rz0;
    if ((long long)want > rows) want = (int)rows;
    if (want < 1) want = 1;
    h.nthreads = want;
    h.slots = (grade_slot_t *)calloc((size_t)want, sizeof(grade_slot_t));
    if (!h.slots) {                     /* 内存不够：退回只统计 */
        h.nthreads = 0;
        o.phase2_ctx_init = NULL;
    } else {
        o.phase2_ctx_init = grade_scan_ctx_init;
    }

    lysh_scan_result res;
    double t0 = eval_now_ms();
    if (lysh_scan_rect(&o, rx0, rx1, rz0, rz1, &res) != 0) {
        free(h.slots);
        if (grade) memset(grade, 0, sizeof(*grade));
        return;
    }
    double p2_cpu_ms = res.p2_seconds * 1000.0;
    long long scanned = res.scanned;

    /* 单线程合并各 slot：计数直接相加；明细按 slot 顺序回放。 */
    lysh_hut_grade merged;
    memset(&merged, 0, sizeof(merged));
    merged.ms = eval_now_ms() - t0;
    merged.p2_ms = p2_cpu_ms;
    merged.scanned = scanned;
    merged.hits = h.shared.hits;
    merged.hits_cap = h.shared.hits_cap;

    for (int t = 0; t < h.nthreads; t++) {
        grade_slot_t *sl = &h.slots[t];
        merged.evaluated += sl->evaluated;
        merged.accepted += sl->n_ok;
        merged.rejected_y += sl->rejected_y;
        merged.rejected_flood += sl->rejected_flood;
        merged.rejected_biome += sl->rejected_biome;
    }

    /* 回放：把每个 slot 记下的"通过"决定重新算一遍（确定性完全一样），
     * 顺序写进调用方的缓冲 —— 单线程，无竞争。
     * ⚠️ 这里**不新建 ctx**：worker 已全部 join，直接用 ctx[0]（第一个 worker 的
     *    ctx）即可 —— 一个种子因此只初始化一次噪声栈。 */
    int out_n = 0;
    if (merged.hits && merged.hits_cap > 0 && merged.accepted > 0) {
        lysh_search_ctx *rctx = s->ctx[0];
        for (int t = 0; t < h.nthreads && out_n < merged.hits_cap; t++) {
            grade_slot_t *sl = &h.slots[t];
            for (int i = 0; i < sl->n_decisions && out_n < merged.hits_cap; i++) {
                if (!sl->decisions[i].ok) continue;
                lysh_hut_result r = lysh_eval_hut(rctx, sl->decisions[i].hut_x,
                                                  sl->decisions[i].hut_z, max_y);
                if (!r.ok) continue;        /* 理论上不会发生；发生了就不填 */
                grade_fill_hit(merged.hits + (size_t)out_n * LYSH_HUT_GRADE_INTS, &r);
                out_n++;
            }
        }
    }
    merged.hits_written = out_n;

    free(h.slots);
    if (grade) *grade = merged;
}

/* 便捷入口：一个种子的**一个**带。= 开会话 → 扫一带 → 关会话。
 * 签名与可观察行为与历史上那次"直接扫"完全一致（CLI / JNI 都还在用它）。 */
void lysh_grade_scan(uint64_t world_seed, const lysh_phase1_opts *opts, int max_height,
                     int rx0, int rx1, int rz0, int rz1, int salt, int threads,
                     int max_y,
                     lysh_hut_grade *grade) {
    lysh_scan_session *s = lysh_scan_session_open(world_seed, opts, max_height, salt, threads);
    if (!s) {
        if (grade) memset(grade, 0, sizeof(*grade));
        return;
    }
    lysh_scan_session_band(s, rx0, rx1, rz0, rz1, max_y, grade);
    lysh_scan_session_free(s);
}
