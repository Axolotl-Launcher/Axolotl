/* lysh aquifer.h —— MC **26.1.2** 的含水层 `net.minecraft.world.level.levelgen.Aquifer`
 * （impl：`Aquifer$NoiseBasedAquifer`）。
 *
 * ⛔ 版本归属（见 README §0）：**旧版实现的是 1.18 的 `AquiferSampler.Impl`，已作废。**
 *    26.1.2 的类名/结构都变了，逐条差异（全部来自 `javap -p -c`，见
 *    `_archive/dev_scratch_20260921.zip -> test/aquifer_javap.txt`）：
 *
 *     1.18 `AquiferSampler.Impl#getFluidLevel(x,y,z) -> FluidLevel`
 *     26.1.2 `NoiseBasedAquifer#computeFluid(x,y,z) -> FluidStatus`
 *            `FluidStatus{fluidLevel:int, fluidType:BlockState}`
 *            `FluidStatus.at(y) = y < fluidLevel ? fluidType : AIR`
 *
 *     1.18 `Impl#apply(x,y,z,d1,d2)`（d1 = 原始密度，`d1 <= -64` 直接走全局 picker）
 *     26.1.2 `Aquifer#computeSubstance(ctx, density)` —— **只有一个密度参数**，
 *            没有 `d1 <= -64` 那一支；取而代之的是
 *            `if (y > skipSamplingAboveY) return globalStatus.at(y)`。
 *
 *     1.18 取最近 **3** 个锚点；26.1.2 取最近 **4** 个（`d0..d3`）。
 *            并且插入判据是 `dist <= d[k]`（**含等号**），不是 `<`。
 *
 *     1.18 压力合成 `2 * m01 * max(e1, max(e2*m02, e3*m12))` 再与密度相加后判一次。
 *     26.1.2 是**逐项**判：`density + s01*e1 > 0` → null；`density + s01*s02*e2 > 0` → null；
 *            `density + s01*s12*e3 > 0` → null。没有那个 `2*` 外层因子。
 *            而 `calculatePressure` 内部返回的是 `2.0 * (barrier + o)`（**多了 2 倍**）。
 *
 *     `FLOWING_UPDATE_SIMULARITY = similarity(10*10, 12*12) = 1 - 44/25 = -0.76`
 *     （1.18 里那个 0.92 的常量在 26.1.2 已不存在）。
 *
 *     `similarity(a,b) = 1.0 - (b - a) / 25.0`（**不对称**；调用处恒有 a <= b）。
 *
 * 关键结构（都不能"顺手化简"）：
 *   · `aquiferCache[]` / `aquiferLocationCache[]` 的 per-chunk 记忆化（位置缓存初值 Long.MAX_VALUE）；
 *   · `SURFACE_SAMPLING_OFFSETS_IN_CHUNKS` 13 项，**(0,0) 排在第一个**（1.18 里排在中间）；
 *   · `minCellTopY` 用的是 `preliminarySurfaceLevel` 的**原始值**，
 *     而喂给 fluid picker / 判空的是 `adjustSurfaceLevel(top) = top + 8`；
 *   · `calculatePressure` 里的 `barrierAcc` 是 e1/e2/e3 **共享**的 MutableDouble(NaN)；
 *   · `computeSurfaceLevel` 先做 `OverworldBiomeBuilder.isDeepDarkRegion(erosion, depth, ctx)`，
 *     命中则 surfaceLevel = `DimensionType.WAY_BELOW_MIN_Y`（=-32512，语义"此处没有水"）。
 *
 * 随机锚点：26.1.2 用 `RandomState.aquiferRandom()`：
 *   `Xoroshiro(seed).forkPositional()`  →  `fromHashOf("minecraft:aquifer")`
 *   →  `.forkPositional()`  →  `at(x,y,z) = Xoroshiro(Mth.getSeed(x,y,z) ^ lo, hi)`
 * （`forkPositional()` 会**吃掉两个 nextLong()**，这个不能漏。）
 */
#ifndef LYSH_AQUIFER_H
#define LYSH_AQUIFER_H

#include <stdint.h>

#include "density.h"
#include "noise.h"
#include "rng.h"
#include "terrain.h"

/* 含水层返回的方块状态（NULL = 实心，调用方回退 defaultBlock） */
typedef enum {
    LYSH_BLOCK_NULL = 0,   /* 实心 */
    LYSH_BLOCK_AIR,
    LYSH_BLOCK_WATER,
    LYSH_BLOCK_LAVA
} lysh_block_state;

/* 全局流体 picker（`NoiseBasedChunkGenerator` 里那个 lambda）。
 * overworld: `y < min(-54, seaLevel=63) = -54` → 岩浆层 -54；否则水层 63。 */
typedef struct {
    int lava_y;          /* -54 */
    int water_y;         /* seaLevel = 63 */
    int lava_cut;        /* min(lava_y, water_y) = -54 */
} lysh_fluid_picker;

/* 列顶 Y 的宿主回调：返回 `NoiseChunk.preliminarySurfaceLevel(x, z)`
 * （= 26.1.2 的 `find_top_surface` 结果；无命中返回下界 -64）。 */
typedef int (*lysh_aquifer_column_top_fn)(void *user, int x, int z);

typedef struct {
    /* --- 常量 --- */
    int x_spacing, y_spacing, z_spacing;   /* 16, 12, 16 */

    /* --- 网格（per chunk；字段名对齐 26.1.2） --- */
    int min_grid_x, min_grid_y, min_grid_z;
    int grid_size_x, grid_size_y, grid_size_z;
    int grid_len;

    /* --- 记忆化 --- */
    int32_t *fluid_level;             /* aquiferCache[i].fluidLevel */
    uint8_t *fluid_type;              /* aquiferCache[i].fluidType（lysh_block_state） */
    uint8_t *cache_valid;             /* aquiferCache[i] != null */
    int64_t *location;                /* aquiferLocationCache[i]（BlockPos.asLong） */

    /* --- positionalRandomFactory（见头部说明的派生链） --- */
    uint64_t rand_lo, rand_hi;

    /* --- 四个含水层噪声（`minecraft:noise` 节点；scale 见 .c） --- */
    lysh_dblnoise barrier;        /* xz=1.0, y=0.5   */
    lysh_dblnoise floodedness;    /* xz=1.0, y=0.67  */
    lysh_dblnoise spread;         /* xz=1.0, y=0.7142857142857143 */
    lysh_dblnoise lava;           /* xz=1.0, y=1.0   */

    /* --- 全局流体 --- */
    lysh_fluid_picker picker;

    /* --- deep-dark 判定（`isDeepDarkRegion(erosion, depth, ctx)`）用的地形样条 --- */
    const lysh_terrain *terrain;

    /* --- settings --- */
    int min_y;                    /* NoiseGeneratorSettings.minY()  = -64 */
    int height;                   /* NoiseGeneratorSettings.height() = 384 */
    int skip_sampling_above_y;    /* ctor 里由 maxPreliminarySurfaceLevel 推出 */

    /* --- 列顶 Y 回调 --- */
    lysh_aquifer_column_top_fn column_top;
    void *column_top_user;

    /* --- 上一次 computeSubstance 的 shouldScheduleFluidUpdate --- */
    int should_schedule_fluid_update;

    int inited;
} lysh_aquifer;

/* 由 worldSeed 推出 4 个含水层噪声 + aquiferRandom。
 * terrain 会被保存（deep-dark 判定要用）；min_y/height 来自 noise_settings。 */
void lysh_aquifer_init(lysh_aquifer *aq, uint64_t world_seed,
                       const lysh_terrain *terrain, int min_y, int height);

/* 为 chunk (chunk_x, chunk_z) 建网格 —— 等价于 26.1.2 的 ctor 里那段
 * minGridX/Y/Z + gridSizeX/Y/Z + skipSamplingAboveY 的计算。
 * 需要 column_top 回调已经设好（skipSamplingAboveY 要扫一遍列顶）。 */
void lysh_aquifer_begin_chunk(lysh_aquifer *aq, int chunk_x, int chunk_z);

void lysh_aquifer_free(lysh_aquifer *aq);

/* ---- 查询 ---- */

/* `NoiseBasedAquifer.computeFluid(x,y,z)`。
 * out_level / out_type 就是 `FluidStatus` 的两个字段；
 * 方块状态 = `lysh_aquifer_level_block_at(level, type, y)`。 */
void lysh_aquifer_compute_fluid(lysh_aquifer *aq, int x, int y, int z,
                                int *out_level, lysh_block_state *out_type);

/* `FluidStatus.at(y)`：at_y < level ? type : AIR */
lysh_block_state lysh_aquifer_level_block_at(int level, lysh_block_state type, int at_y);

/* `Aquifer.computeSubstance(ctx, density)`。返回 LYSH_BLOCK_NULL 表示实心。 */
lysh_block_state lysh_aquifer_compute_substance(lysh_aquifer *aq, int x, int y, int z,
                                                double density);

#endif /* LYSH_AQUIFER_H */
