/* lysh eval.h —— 「候选小屋 → 最终结果」的**库级产品入口**。
 *
 * 这是给 Java GUI（经 JNI）和 CLI 共用的那一层：给一个世界种子和一个小屋方块坐标
 * (hutX, hutZ)，算出
 *   · 朝向（footprint 形状 7(x)×9(z) 还是 9(x)×7(z)）
 *   · footprint 的**精确平均高度**（阶段 2 的 `interpolated` 三线性插值高度图）
 *   · 含水层判定：整片 footprint 在 y=62 是否全被灌满（flooded）—— 对应「洞穴被水
 *     灌满到海平面 ⇒ 小屋浮在水面」那一类高 Y 结果
 *   · 是否满足调用方给的 max_y
 *
 * 本文件**只**是编排层：所有数学都在 src/phase2.c / src/terrain.c / src/aquifer.c，
 * 朝向配方在 src/eval.c 的 lysh_hut_orientation()（唯一一份，工具与 JNI 都调它）。
 *
 * ── 朝向配方（唯一经过实测的 11/11 配方）──────────────────────────────
 *   r = WorldgenRandom(new LegacyRandomSource(0));
 *   r.setLargeFeatureSeed(worldSeed, hutX/16, hutZ/16);
 *   dir = r.nextInt(4);                 // 0=N 1=E 2=S 3=W
 *   axisZ = (dir == 0 || dir == 2);     // axisZ -> 7(x)×9(z)，否则 9(x)×7(z)
 *   原点 = (hutX, hutZ)，不做偏移
 * 证据：11 个真实 SwampHutPiece.boundingBox 全部吻合（README §2.5.5）。
 *
 * ── 高度语义（实测，见 README §2.5.4）─────────────────────────────────
 *   avg_y = footprint 全 63 列的 lysh_phase2_height(x,z) 之和 / 63（C 的整数除法，
 *           向零截断 —— 这就是 MC `updateAverageGroundHeight` 里的 `sum / n`）。
 *   该值在 11 个已知小屋与 26.1.2 oracle 上 **11/11 相等**。
 *
 *   ⚠️ Java 生产代码的 `ceil(sum/63 + 1)` **不是** off-by-one bug：
 *      Java 每列累加的是**方块 Y**（比 `getBaseHeight` 小 1），所以
 *      `ceil(sum_block/63 + 1)` 与 `sum(getBaseHeight)/63` 描述的是**同一个量**；
 *      负数段 `ceil` 与向零截断恒等 ⇒ 低 Y 结果不会偏移。
 *      （早期版本此处误写为"Java 在 11 个点上 0/11、相差 1"，已作废。）
 *
 * ── 怎么用 ────────────────────────────────────────────────────────────
 *   lysh_search_ctx ctx;
 *   lysh_search_ctx_init(&ctx, world_seed, opts, max_height);
 *   lysh_hut_result r = lysh_eval_hut(&ctx, hut_x, hut_z, max_y);
 *   ... r.ok / r.avg_y / r.flooded ...
 *   lysh_search_ctx_free(&ctx);
 *
 * `lysh_search_ctx` 会同时初始化阶段 1 与阶段 2（阶段 2 的初始化约几毫秒，
 * 每条线程一份，**不要在热循环里反复 init**）。一个 ctx 不是线程安全的：
 * 它带可变缓存（含水层网格 / cell 插值格），每条线程要用自己的 ctx。
 * 也**不要**在多个线程间共享同一个 ctx。
 *
 * 需要"同一种子扫很多个带"时（Java 多种子模式就是），用文件末尾的
 * `lysh_scan_session`：ctx 只在 open 时建一次，之后每个带复用，
 * 连明细回放也不再新建 ctx。**不要**在带循环里反复调 lysh_grade_scan。
 */
#ifndef LYSH_EVAL_H
#define LYSH_EVAL_H

#include <stdint.h>

#include "phase1.h"
#include "phase2.h"
#include "biome.h"

#ifdef __cplusplus
extern "C" {
#endif

/* 单候选的完整结果。字段全部由 C 核心写入，调用方只读。 */
typedef struct {
    int hut_x, hut_z;     /* 输入回显（小屋原点，= chunkX*16 / chunkZ*16） */
    int rx, rz;           /* 该小屋所属的区域格（hutX/16 / 32 的 floorDiv） */
    int dir;              /* setLargeFeatureSeed 之后第一个 nextInt(4)：0=N 1=E 2=S 3=W */
    int axis_z;           /* 1 → footprint 7(x)×9(z)；0 → 9(x)×7(z) */
    int size_x, size_z;   /* footprint 的 x/z 尺寸（7/9 或 9/7） */

    int avg_y;            /* footprint 平均高度 = sum(h)/63（整数除法，向零截断） */
    long long sum_h;      /* 63 列高度之和（诊断用；avg_y = sum_h / 63） */
    int wet_columns;      /* footprint 里 computeFluid(x,62,z).at(62)==WATER 的列数 */
    int flooded;          /* 1 ⇔ wet_columns == 63（整片灌满 ⇒ 该小屋属于高 Y 那类） */
    int all_dry;          /* 1 ⇔ wet_columns == 0 */

    int ok;               /* 1 ⇔ 通过判定：biome_ok && avg_y <= max_y && !flooded */
    int reject;           /* ok=0 时给出原因，见下面的 LYSH_HUT_REJECT_* */

    /* ---- 真实群系门（src/biome.c；MC 的 Structure.isValidBiome）----
     * 阶段 1 的气候门只是旧 Java 程序的近似，不等价于 MC 的群系判定；少这一门时
     * 全图会多报 4 个候选（它们其实是 dripstone_caves）。详见 biome.h。 */    int biome_ok;         /* 1 = chunk 中心列地表判定出的群系就是 swamp */
    int biome_winner;     /* 参数树给出的群系序号（cubiomes biomes.h 序数；6 = swamp） */
    int biome_qx, biome_qy, biome_qz;   /* 判定用的 quart 坐标 */
    int biome_occ_y;      /* getFirstOccupiedHeight(WORLD_SURFACE_WG) = baseHeight - 1 */
    long long biome_t[6]; /* 量化后的 (temperature,humidity,continentalness,erosion,depth,weirdness) */
} lysh_hut_result;

enum {
    LYSH_HUT_OK          = 0,
    LYSH_HUT_REJECT_Y    = 1,   /* avg_y > max_y */
    LYSH_HUT_REJECT_FLOOD= 2,   /* 整片 footprint 在 y=62 被水灌满 */
    LYSH_HUT_REJECT_BIOME= 3,   /* chunk 中心列地表不是沼泽系群系（真实群系判定） */
};

/* JNI 桥用的扁平「成绩单」：一次调用拿到结果 + 统计 + 耗时。
 * `hits` 是**调用方提供**的 int 缓冲（可为 NULL = 不要明细），布局见
 * grade_fill_hit（eval.c）：每条 12 个 int，
 *   [hutX, hutZ, rx, rz, dir, avg_y, flooded, ok, wet_columns, size_x, size_z, reject] */
#define LYSH_HUT_GRADE_INTS 12
typedef struct {
    int       *hits;            /* 调用方缓冲；NULL = 只统计 */
    int        hits_cap;        /* 可写条数（= 缓冲 int 个数 / LYSH_HUT_GRADE_INTS） */
    int        hits_written;    /* 实际写入条数（由 C 填） */
    long long  scanned;         /* 本次调用扫过的区域格数（lysh_grade_scan 填） */
    long long  evaluated;       /* 这次调用评估的候选数（单候选时 = 1） */
    long long  accepted;        /* ok == 1 的个数 */
    long long  rejected_y;      /* 因 avg_y > max_y 被拒 */
    long long  rejected_flood;  /* 因整片灌满被拒 */
    long long  rejected_biome;  /* 因真实群系不是沼泽被拒（src/biome.c） */
    double     ms;              /* 本次调用的墙钟耗时（毫秒） */
    double     p2_ms;           /* 其中**阶段 2 钩子**的累计墙钟（毫秒；多线程则各线程之和） */
} lysh_hut_grade;

/* 搜索上下文：阶段 1 + 阶段 2 的全部预计算状态。
 * `max_height` 是阶段 1 的 maxHeight（生产代码把 -50/-54 夹到 -50；CLI 已经夹好）。 */
typedef struct {
    uint64_t seed;
    lysh_phase1_opts opts;
    int max_height;
    int large_biomes;           /* 冗余于 opts.large_biomes，方便读 */

    lysh_phase1 p1;
    lysh_phase2 p2;
    lysh_biome  biome;          /* 真实群系门要的温度/植被噪声（src/biome.c） */
    int inited;

    /* 阶段 2 钩子的**每线程私有**挂载点（见 search.h 的 phase2_ctx_init）。
     * 每个 worker 一个 ctx，所以这里天然是线程私有的；钩子把它的写游标放这里，
     * 就绝不会跨线程共享可写状态。 */
    void *p2_slot;
    int   p2_thread_index;
} lysh_search_ctx;

/* 初始化 / 释放（free 可重复调用）。 */
void lysh_search_ctx_init(lysh_search_ctx *ctx, uint64_t world_seed,
                          const lysh_phase1_opts *opts, int max_height);
void lysh_search_ctx_free(lysh_search_ctx *ctx);

/* 朝向：唯一一份配方（工具 hutdir_test / CLI / JNI 都调它，不要再各抄一遍）。 */
int lysh_hut_orientation(uint64_t world_seed, int hut_x, int hut_z);

/* 单候选完整评估。`max_y` 是**最终 Y 的门槛**（判据 avg_y <= max_y）。
 * 传 INT_MAX 表示"不设 Y 门槛"（仍然会做灌水判定）。 */
lysh_hut_result lysh_eval_hut(lysh_search_ctx *ctx, int hut_x, int hut_z, int max_y);

/* 与 Java 产品同一口径的一行：`/tp %d %.0f %d`（Y = footprint 平均高度）。
 * 返回指向内部 static 缓冲区的指针 —— 单线程用；需要保存请自己 strcpy。 */
const char *lysh_hut_tp_line(const lysh_hut_result *r);

/* ------------------------------------------------------------------ *
 * 扁平成绩单 API（JNI 专用；也方便 C 侧的基准/验收复用）
 *
 * 语义与 lysh_eval_hut 完全一致 —— 它内部就是循环调用它，不含任何新判定。
 * `max_y` 同 lysh_eval_hut。明细缓冲区由调用方给：`grade.hits` / `grade.hits_cap`
 * 在调用前填好（`hits` 可为 NULL = 只要统计）；返回时 `hits` / `hits_cap` 原样带回，
 * `hits_written` 是实际写入条数。
 *
 * lysh_grade_scan：对 [rx0,rx1)×[rz0,rz1) 里**通过阶段 1 的**候选逐个评估
 *                  （阶段 1 的那一步与 lysh scan 完全是同一条代码路径）。
 *                  `evaluated` 计的是阶段 1 幸存数，不是区域格数。
 *                  现在它是「开会话 → 扫一个带 → 关会话」的薄包装（见下面
 *                  lysh_scan_session），签名与可观察行为都没变。
 *
 * （历史上有过一个"对候选数组逐个评估"的 lysh_grade_huts：它既没有产品调用方，
 *   归档工具也只用了 lysh_grade_scan，已删除 —— 不要再加回来。）
 * ------------------------------------------------------------------ */
void lysh_grade_scan(uint64_t world_seed, const lysh_phase1_opts *opts, int max_height,
                     int rx0, int rx1, int rz0, int rz1, int salt, int threads,
                     int max_y,
                     lysh_hut_grade *grade);

/* ------------------------------------------------------------------ *
 * 扫描会话（性能修复：一个种子只初始化一次噪声栈）
 *
 * 背景（实测）：Java 多种子模式把一个种子的扫描切成 MAX_SCAN_BANDS = 256 个 Z 带，
 * 每带一次 JNI 调用。而老的 lysh_grade_scan 每次都重新建/销毁 worker ctx
 * （= 重跑整条噪声初始化），还要为**明细回放**再建一次。实测 65,536 格：
 *   256 带 → 95.4 ms/种子；128 带（同面积）→ 50.1 ms/种子；纯计算只要 ~7.5 ms。
 * 成本因此只跟**带数**走（每带 2 次初始化 ≈ 0.19 ms），跟面积无关。
 *
 * 会话把这层代价挪到"每个种子一次"：
 *   · open   —— 建好 threads 个 worker ctx（每个恰好一次 lysh_search_ctx_init），
 *               记下 seed / opts / max_height / salt / threads；失败返回 NULL。
 *   · band   —— 扫 [rx0,rx1)×[rz0,rz1) 这一个带，复用上面那批 ctx，填 *grade
 *               的字段与命中布局与 lysh_grade_scan 完全一致；
 *               `grade->hits` / `hits_cap` 由调用方给（每个带可以不一样）。
 *   · free   —— 销毁全部（可传 NULL，可在任意多个 band 之后调用）。
 *
 * ⚠️ 明细回放**不新建 ctx**：回放发生在 worker 全部 join 之后（单线程），
 *    直接复用 ctx[0]（第一个 worker 的 ctx）。这是"threads=1 时全程恰好一次
 *    lysh_search_ctx_init"这条硬要求的最后一环。
 *
 * 线程模型与 lysh_grade_scan 一致：threads == 1 在调用线程里内联跑；
 * threads > 1 仍然"每带 spawn/join"，没有常驻线程池。
 * ------------------------------------------------------------------ */
typedef struct lysh_scan_session lysh_scan_session;

lysh_scan_session *lysh_scan_session_open(uint64_t world_seed,
                                          const lysh_phase1_opts *opts,
                                          int max_height, int salt, int threads);
void lysh_scan_session_free(lysh_scan_session *s);

void lysh_scan_session_band(lysh_scan_session *s,
                            int rx0, int rx1, int rz0, int rz1,
                            int max_y, lysh_hut_grade *grade);

#ifdef __cplusplus
}
#endif

#endif /* LYSH_EVAL_H */
