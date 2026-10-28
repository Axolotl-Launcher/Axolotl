/* ⛔ 版本归属（见 README §0）：本文件实现的是 **1.18 的地形语义**，默认无效。
 * 只有显式选择版本 `1.18.2` 时才作为该分支的实现保留。
 * 1.21+ 的权威是客户端 jar 的 worldgen JSON（含显式的 `interpolated` / `squeeze` /
 * `quarter_negative` 等计算图节点），**不是**本文件所依据的 1.18.1 字节码。
 *
 * 另：本实现**未做 MC 的三线性插值**，实测 200 点错 86 个（差值 1~2 格）。
 * 1.21 的 JSON 里 `interpolated` 是显式节点，重写时按节点实现即可。
 */

/* lysh phase2.h —— M3-L6：从 (x, z) 得到高度图 Y（`sampleHeightmap`）。
 *
 * 这是**产品真正用的那一步**：低 Y 小屋的 Y 就是这里出来的。
 *
 * 取证：`_archive/dev_scratch_20260921.zip -> net.minecraft.world.gen.NoiseChunkGenerator.txt:238-508`
 *
 * ```java
 * public int getHeight(int x, int z, Heightmap.Type type, HeightLimitView world) {
 *     int minY = max(cfg.minimumY(), world.getBottomY());
 *     int maxY = min(cfg.minimumY() + cfg.height(), world.getTopY());
 *     int minCellY = floorDiv(minY, vbs);
 *     int cellCount = floorDiv(maxY - minY, vbs);
 *     if (cellCount <= 0) return world.getBottomY();
 *     return sampleHeightmap(x, z, null, type.getBlockPredicate(), minCellY, cellCount)
 *              .orElse(world.getBottomY());
 * }
 *
 * private OptionalInt sampleHeightmap(x, z, out, pred, minCellY, cellCount) {
 *     int hbs = cfg.horizontalBlockSize();   // 4
 *     int vbs = cfg.verticalBlockSize();     // 8
 *     int cellX = floorDiv(x, hbs), cellZ = floorDiv(z, hbs);
 *     double fracX = floorMod(x, hbs) / (double) hbs;
 *     double fracZ = floorMod(z, hbs) / (double) hbs;
 *     ChunkNoiseSampler s = ChunkNoiseSampler.create(cellX*hbs, cellZ*hbs,
 *                                minCellY, cellCount, ncs, settings, fluidLevelSampler);
 *     ...
 *     for (int i = cellCount - 1; i >= 0; i--) {
 *         for (int j = vbs - 1; j >= 0; j--) {
 *             int y = (minCellY + i) * vbs + j;
 *             BlockState st = blockStateSampler.apply(s, x, y, z);
 *             BlockState v = (st == null) ? defaultBlock : st;   // ★ null → 石头
 *             if (pred.test(v)) return OptionalInt.of(y + 1);    // ★ 返回值是 y+1
 *         }
 *     }
 *     return empty();
 * }
 * ```
 *
 * ⚠️ 关键语义：`Heightmap.Type.WORLD_SURFACE_WG` 的谓词是 `NOT_AIR`，
 * 而 **水不是空气 → 水算"表面"**。
 * 实测（seed -143551518615525778，300 点）：命中处 **water=165 / stone=135**，
 * 也就是说 **55% 的高度由含水层（水）决定**，不是密度。
 * 这正是"95% 充水洞穴 → 小屋浮在水面（高 Y）"的机制来源。
 *
 * ⚠️ 本层需要含水层（L5）。`method_39900`（列顶 Y）由外部注入回调，
 *    因为它是 L5/L6 交界处唯一还没单独对拍通过的一环。
 */
#ifndef LYSH_PHASE2_H
#define LYSH_PHASE2_H

#include <stdint.h>

#include "aquifer.h"
#include "density.h"
#include "interp_noise.h"
#include "terrain.h"

/* 雕刻器结果缓存（每个目标 chunk 98304 字节；定义在 phase2.c）。
 * 用不完整类型 + 指针，避免把这个大数组塞进 lysh_phase2 / 调用方栈。 */
typedef struct lysh_carve_cache lysh_carve_cache;

typedef struct {
    int horizontal_block_size;   /* 4 */
    int vertical_block_size;     /* 8 */
    int minimum_block_y;         /* -8（cell） */
    int vertical_block_count;    /* 48 */
    int minimum_y;               /* -64（方块） */
    int height;                  /* 384 */
    int sea_level;               /* 63 */

    lysh_terrain terrain;
    lysh_density density;
    lysh_interp_noise interp;    /* L4：base_3d_noise */
    lysh_aquifer aquifer;

    int aq_chunk_x, aq_chunk_z;  /* 当前含水层网格对应的 chunk（惰性重建） */

    /* ---- CARVERS 阶段的雕刻器（26.1.2 语义；见 carver.h）----
     * 小屋 Y 是**雕刻之后**的高度图（FEATURES 阶段会 primeHeightmaps 重量），
     * 所以 `lysh_phase2_height` 默认走雕刻后的状态。 */
    uint64_t world_seed;
    lysh_carve_cache *carve;     /* 惰性填；分配失败时为 NULL（退回雕刻前的高度） */
    int carve_enabled;           /* 默认 1；**所有版本**都用（见 eval.c 的版本无关证据） */

    /* ---- `interpolated` 网格缓存（26.1.2 的 4(x) × 8(y) × 4(z) cell）----
     * 每个 cell 需要 4 个角点列，每列 49 个 cell-Y 取值
     * （y = minimum_y + i * vertical_block_size，i = 0..48）。
     *
     * ⚠️ 缓存粒度是**一个 chunk 的角点列网格**（5×5 = 25 列），不是单个 cell：
     *    雕刻器按椭球遍历目标 chunk，x/z 在 cell 之间来回跳，单 cell 缓存会被
     *    反复重建（每个 cell 要重算 4 个角点列 × 49 个 cell-Y）。一个 chunk = 4×4 个
     *    cell，相邻 cell 共用角点，所以 25 列就够；`cell_col_valid` 是 25 位有效掩码，
     *    按需算列（`index = gx * 5 + gz`，gx/gz ∈ 0..4，格点坐标 = chunk 内 cell 偏移）。
     *    实测（`lysh scan` 100M 格 8 线程）这一项把 phase-2 每候选从 137.6 ms 压到
     *    44 ms 量级（见 README / 交付报告），因为雕刻阶段的密度求值不再被 cell 重建淹没。 */
    int cell_col_x, cell_col_z;          /* 角点列网格左下角的 cell 坐标（4 对齐） */
    unsigned int cell_col_valid;         /* 25 位掩码：bit(gx*5+gz) = 该角点列已算 */
    double cell_col_main[25][49];
    double cell_col_noodle[3][25][49];

    int inited;
} lysh_phase2;

/* 初始化（由 worldSeed 推出全部噪声）。 */
void lysh_phase2_init(lysh_phase2 *p2, uint64_t world_seed, int large_biomes);

void lysh_phase2_free(lysh_phase2 *p2);

/* MC 的 `getHeight(x, z, WORLD_SURFACE_WG, world).orElse(bottomY)`。
 * 返回 [bottom_y, maximum_y] 内的高度；无命中返回 bottom_y。
 *
 * ⚠️ 口径（见 carver.h 顶部）：小屋读的 `getHeightmapPos(MOTION_BLOCKING_NO_LEAVES)`
 *    是 **CARVERS 之后**的方块状态（FEATURES 开头 `primeHeightmaps` 重量），
 *    所以**本函数走雕刻后的状态**（`carve_enabled` 打开时）。 */
int lysh_phase2_height(lysh_phase2 *p2, int x, int z);

/* 同一个函数的**雕刻前**口径（= 纯密度 + 含水层）。
 * 群系门要用它：`Structure.onTopOfChunkCenter` → `ChunkGenerator.getFirstOccupiedHeight`
 * → `getBaseHeight` → `iterateNoiseColumn`，**只吃 RandomState 的密度函数**，
 * 拿不到 ChunkAccess ⇒ 看不到雕刻（bytecode 取证见 carver_invoke_spec §5.1）。 */
int lysh_phase2_height_pre_carver(lysh_phase2 *p2, int x, int z);

/* 开关雕刻层（默认开，**所有版本都用**）。曾经只对 26.2 打开，理由是"旧版用另一套
 * 播种（`setCarverSeed` + `| 1L`）"——**这个理由已被字节码证伪**：1.18.2 的 `cuv.c(JII)`、
 * 1.19.2 的 `dbo.c(JII)`、1.20 的 `dij.c(JII)`、1.21 的 `dzx.c(JII)` 与 26.1.2 的
 * `WorldgenRandom.setLargeFeatureSeed(JII)` 是逐条相同的字节码（都没有 `| 1L`），
 * 调用点也都是 `setLargeFeatureSeed(seed + carverIndex, ox, oz)` 落在 -8..8 双循环里；
 * configured_carver 的 JSON 在 1.20~26.1.2 之间逐字段相同（1.18.2/1.19.2 的
 * client jar 不带 worldgen 数据，本地无法核对，见 eval.c 的 caveat）。 */
void lysh_phase2_set_carvers(lysh_phase2 *p2, int enable);

/* 26.1.2 的 `final_density`（**含水层拿到的那个**）：
 *   min( squeeze(0.64 * interpolated(sloped_cheese+caves+slides)), noodle )
 * 也就是 `NoiseChunk.getInterpolatedDensity()`。
 * y 落在 [-64, 319] 之外时退回逐点求值（网格覆盖不到）。 */
double lysh_phase2_density_at(lysh_phase2 *p2, int x, int y, int z);

/* 非插值的点值（`interpolated` 节点的**内容**，= 旧的 1.18 `sampleNoiseColumn`）。
 * 只为对拍/诊断保留，不参与产品路径。 */
double lysh_phase2_density_raw_at(lysh_phase2 *p2, int x, int y, int z);

/* 单个 (x,y,z) 的方块状态（含水层判定后）。
 * 返回 LYSH_BLOCK_NULL 表示实心（调用方回退 defaultBlock = 石头）。
 * ⚠️ **雕刻前**口径：这是 NOISE 阶段的结果，不含 CARVERS。雕刻后的版本只经由
 *    `lysh_phase2_height` 暴露（它也是雕刻器 carver.c 的 base_fn）。 */
lysh_block_state lysh_phase2_block_at(lysh_phase2 *p2, int x, int y, int z);

/* 供测试：直接注入列顶 Y 回调 */
void lysh_phase2_set_column_top(lysh_phase2 *p2, lysh_aquifer_column_top_fn fn, void *user);

/* 保证含水层网格对应 (x,z) 所在的 chunk（惰性重建；block_at 内部会自己调）。 */
void lysh_phase2_ensure_chunk(lysh_phase2 *p2, int x, int z);

#endif /* LYSH_PHASE2_H */
