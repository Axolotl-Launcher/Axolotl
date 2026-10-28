/* lysh aquifer.c —— 见 aquifer.h 顶部的 26.1.2 逐条差异与取证说明。
 *
 * 表达式结构逐条照抄 `Aquifer$NoiseBasedAquifer` 的字节码（javap -p -c），
 * 不做任何浮点"化简"、不合并判断、不改判据的等号方向。
 */
#include "aquifer.h"

#include <math.h>
#include <stdlib.h>
#include <string.h>

#include "column_top.h"
#include "mcmath.h"

/* 哨兵：aquiferLocationCache 的未计算值（= Long.MAX_VALUE） */
#define LYSH_POS_UNSET INT64_MAX
/* DimensionType.WAY_BELOW_MIN_Y —— "此处没有水"（实测 -32512） */
#define LYSH_NO_FLUID_Y (-32512)

/* OverworldBiomeBuilder.isDeepDarkRegion 的两个阈值（javap：float 常量拓宽成 double） */
#define LYSH_DEEP_DARK_EROSION (-0.22499999403953552)
#define LYSH_DEEP_DARK_DEPTH   (0.8999999761581421)

/* FLOWING_UPDATE_SIMULARITY = similarity(Mth.square(10), Mth.square(12))
 *                           = 1.0 - (144 - 100)/25.0 = -0.76  */
#define LYSH_FLOWING_UPDATE_SIMULARITY (-0.76)

/* ---------------- Java 语义的小工具 ---------------- */
static inline int floor_div(int a, int b) {
    int q = a / b, r = a % b;
    if (r != 0 && ((r < 0) != (b < 0))) q--;
    return q;
}

/* `gridX(v) = v >> 4`（算术右移，等价 floorDiv 16） */
static inline int grid_x(int v) { return v >> 4; }
static inline int grid_z(int v) { return v >> 4; }
static inline int grid_y(int v) { return floor_div(v, 12); }
static inline int from_grid_x(int i, int j) { return (i << 4) + j; }
static inline int from_grid_z(int i, int j) { return (i << 4) + j; }
static inline int from_grid_y(int i, int j) { return i * 12 + j; }

/* DensityFunctions.yClampedGradient(fromY, toY, fromValue, toValue) */
static inline double y_clamped_gradient(double y, double from_y, double to_y,
                                        double from_value, double to_value) {
    double t = mc_clamp((y - from_y) / (to_y - from_y), 0.0, 1.0);
    return mc_lerp(t, from_value, to_value);
}

/* NoiseBasedAquifer.similarity(a, b) = 1.0 - (b - a)/25.0（**不对称**） */
static inline double similarity(int a, int b) {
    return 1.0 - (double)(b - a) / 25.0;
}

/* ---------------- index / BlockPos ---------------- */

static inline int aq_index(const lysh_aquifer *aq, int gx, int gy, int gz) {
    /* 与 26.1.2 的 index() 逐字一致：
     *   ((y - minGridY) * gridSizeZ + (z - minGridZ)) * gridSizeX + (x - minGridX)
     * MC 没有边界检查（网格尺寸保证覆盖本 chunk 的邻域），但这里保留一个
     * "-1 = 越界" 哨兵，免得 dump 里的越界点把内存踩坏。 */
    int dx = gx - aq->min_grid_x, dy = gy - aq->min_grid_y, dz = gz - aq->min_grid_z;
    if (dx < 0 || dx >= aq->grid_size_x) return -1;
    if (dy < 0 || dy >= aq->grid_size_y) return -1;
    if (dz < 0 || dz >= aq->grid_size_z) return -1;
    return (dy * aq->grid_size_z + dz) * aq->grid_size_x + dx;
}

/* BlockPos.asLong / unpack：26 位每轴 + y 低 12 位。
 *
 * ⚠️ 解包的移位量必须与 MC 逐字一致（javap `BlockPos.getX/getY/getZ`）：
 *     PACKED_HORIZONTAL_LENGTH = 26, X_OFFSET = 38, Z_OFFSET = 12, Y 在低 12 位。
 *     getX = (v << 0) >> 38; getY = (v << 52) >> 52; getZ = (v << 26) >> 38
 * 早期版本写成 `(v << 26) >> 52` / `(v << 12) >> 38`，**解出来的 y 恒为 0、z 也错**，
 * 于是 `getAquiferStatus` 永远在 y=0 上求值（水位 63 → 到处判成水）。 */
static inline int64_t blockpos_as_long(int x, int y, int z) {
    return ((int64_t)(x & 0x3FFFFFF) << 38) | ((int64_t)(z & 0x3FFFFFF) << 12)
           | (int64_t)(y & 0xFFF);
}
static inline int blockpos_x(int64_t v) { return (int)(v >> 38); }
static inline int blockpos_y(int64_t v) { return (int)((v << 52) >> 52); }
static inline int blockpos_z(int64_t v) { return (int)((v << 26) >> 38); }

/* ---------------- 全局流体 picker ---------------- */

static void global_fluid(const lysh_aquifer *aq, int y,
                         int *out_level, lysh_block_state *out_type) {
    if (y < aq->picker.lava_cut) {
        *out_level = aq->picker.lava_y;
        *out_type = LYSH_BLOCK_LAVA;
    } else {
        *out_level = aq->picker.water_y;
        *out_type = LYSH_BLOCK_WATER;
    }
}

lysh_block_state lysh_aquifer_level_block_at(int level, lysh_block_state type, int at_y) {
    return at_y < level ? type : LYSH_BLOCK_AIR;
}

/* ---------------- 初始化 ---------------- */

void lysh_aquifer_init(lysh_aquifer *aq, uint64_t world_seed,
                       const lysh_terrain *terrain, int min_y, int height) {
    memset(aq, 0, sizeof(*aq));
    aq->x_spacing = 16;
    aq->y_spacing = 12;
    aq->z_spacing = 16;

    aq->picker.lava_y = -54;
    aq->picker.water_y = 63;          /* overworld sea_level */
    aq->picker.lava_cut = -54;        /* min(-54, 63) */

    aq->min_y = min_y;
    aq->height = height;
    aq->terrain = terrain;

    /* 4 个含水层噪声：`minecraft:noise` 节点，由 worldSeed + identifier 派生。
     * scale 见 overworld.json：barrier(1.0,0.5) floodedness(1.0,0.67)
     * spread(1.0,0.7142857142857143) lava(1.0,1.0)。 */
    lysh_dblnoise_from_seed_id(&aq->barrier, world_seed, "minecraft:aquifer_barrier");
    lysh_dblnoise_from_seed_id(&aq->floodedness, world_seed, "minecraft:aquifer_fluid_level_floodedness");
    lysh_dblnoise_from_seed_id(&aq->spread, world_seed, "minecraft:aquifer_fluid_level_spread");
    lysh_dblnoise_from_seed_id(&aq->lava, world_seed, "minecraft:aquifer_lava");

    /* `RandomState.aquiferRandom()`（javap 确认）：
     *   random          = Xoroshiro(seed).forkPositional()          ← 吃掉 2 个 nextLong
     *   aquiferRng      = random.fromHashOf("minecraft:aquifer")    ← MD5 哈希 XOR
     *   aquiferRandom   = aquiferRng.forkPositional()               ← 再吃 2 个 nextLong */
    xoroshiro_t rng;
    lysh_xr_init(&rng, world_seed);
    xoroshiro_t base;
    base.lo = lysh_xr_next_long(&rng);
    base.hi = lysh_xr_next_long(&rng);
    xoroshiro_t aqrng;
    lysh_xr_create_random(&aqrng, &base, "minecraft:aquifer");
    aq->rand_lo = lysh_xr_next_long(&aqrng);
    aq->rand_hi = lysh_xr_next_long(&aqrng);

    aq->inited = 1;
}

static void aq_alloc(lysh_aquifer *aq) {
    int n = aq->grid_size_x * aq->grid_size_y * aq->grid_size_z;
    aq->grid_len = n;

    free(aq->fluid_level);
    free(aq->fluid_type);
    free(aq->cache_valid);
    free(aq->location);
    aq->fluid_level = (int32_t *)malloc(sizeof(int32_t) * (size_t)n);
    aq->fluid_type = (uint8_t *)malloc(sizeof(uint8_t) * (size_t)n);
    aq->cache_valid = (uint8_t *)calloc((size_t)n, 1);
    aq->location = (int64_t *)malloc(sizeof(int64_t) * (size_t)n);
    for (int i = 0; i < n; i++) aq->location[i] = LYSH_POS_UNSET;
}

void lysh_aquifer_begin_chunk(lysh_aquifer *aq, int chunk_x, int chunk_z) {
    /* 与 26.1.2 ctor 逐条一致（ChunkPos.getMinBlockX()/getMaxBlockX() 就是 ±16 方块）。
     * ⚠️ 需要 column_top 回调**已经设好** —— skipSamplingAboveY 要扫一遍列顶。 */
    int min_block_x = chunk_x * 16, max_block_x = min_block_x + 15;
    int min_block_z = chunk_z * 16, max_block_z = min_block_z + 15;

    int min_gx = grid_x(min_block_x - 5);
    int max_gx = grid_x(max_block_x - 5) + 1;
    int min_gz = grid_z(min_block_z - 5);
    int max_gz = grid_z(max_block_z - 5) + 1;

    aq->min_grid_x = min_gx;
    aq->grid_size_x = max_gx - min_gx + 1;
    aq->min_grid_z = min_gz;
    aq->grid_size_z = max_gz - min_gz + 1;

    int min_gy = grid_y(aq->min_y + 1) - 1;
    int max_gy = grid_y(aq->min_y + aq->height + 1) + 1;
    aq->min_grid_y = min_gy;
    aq->grid_size_y = max_gy - min_gy + 1;

    aq_alloc(aq);

    /* skipSamplingAboveY：
     *   psl = maxPreliminarySurfaceLevel(fromGridX(minGridX,0), fromGridZ(minGridZ,0),
     *                                    fromGridX(maxGridX,9), fromGridZ(maxGridZ,9))
     *   a   = adjustSurfaceLevel(psl) = psl + 8
     *   g   = gridY(a + 12) - (-1)        ← 字节码是 `iconst_m1; isub`，即 **+1**
     *   skipSamplingAboveY = fromGridY(g, 11) - 1                                  */
    int x1 = from_grid_x(min_gx, 0), z1 = from_grid_z(min_gz, 0);
    int x2 = from_grid_x(max_gx, 9), z2 = from_grid_z(max_gz, 9);
    int max_psl = INT32_MIN;
    for (int z = z1; z <= z2; z += 4) {
        for (int x = x1; x <= x2; x += 4) {
            int p = aq->column_top(aq->column_top_user, x, z);
            if (p > max_psl) max_psl = p;
        }
    }
    int g = grid_y((max_psl + 8) + 12) + 1;
    aq->skip_sampling_above_y = from_grid_y(g, 11) - 1;
}

void lysh_aquifer_free(lysh_aquifer *aq) {
    free(aq->fluid_level);
    free(aq->fluid_type);
    free(aq->cache_valid);
    free(aq->location);
    aq->fluid_level = NULL;
    aq->fluid_type = NULL;
    aq->cache_valid = NULL;
    aq->location = NULL;
}

/* ---------------- SURFACE_SAMPLING_OFFSETS_IN_CHUNKS（13 项，(0,0) 在**最前**） --- */

static const int SURFACE_SAMPLING_OFFSETS_IN_CHUNKS[13][2] = {
    { 0,  0},
    {-2, -1}, {-1, -1}, {0, -1}, {1, -1},
    {-3,  0}, {-2,  0}, {-1, 0}, {1, 0},
    {-2,  1}, {-1,  1}, {0,  1}, {1, 1}
};

/* ---------------- deep dark ---------------- */

/* OverworldBiomeBuilder.isDeepDarkRegion(erosion, depth, ctx)
 *   erosion.compute(ctx) < (double)(float)-0.225 && depth.compute(ctx) > (double)(float)0.9
 * `depth` = add(y_clamped_gradient(-64,320,1.5,-1.5), minecraft:overworld/offset)。 */
static int is_deep_dark_region(const lysh_aquifer *aq, int x, int y, int z) {
    if (!aq->terrain) return 0;
    lysh_terrain_info ti;
    lysh_terrain_info_at(aq->terrain, x >> 2, z >> 2, &ti);
    if (!(ti.erosion < LYSH_DEEP_DARK_EROSION)) return 0;
    double depth = y_clamped_gradient((double)y, -64.0, 320.0, 1.5, -1.5)
                 + lysh_overworld_offset(aq->terrain, x, z);
    return depth > LYSH_DEEP_DARK_DEPTH;
}

/* ---------------- computeRandomizedFluidSurfaceLevel ---------------- */

static int aq_randomized_fluid_surface_level(const lysh_aquifer *aq,
                                             int x, int y, int z, int min_cell_top_y) {
    int fx = floor_div(x, 16);
    int fy = floor_div(y, 40);
    int fz = floor_div(z, 16);
    int base = fy * 40 + 20;
    double n = lysh_dblnoise_sample(&aq->spread,
                                    (double)fx,
                                    (double)fy * 0.7142857142857143,
                                    (double)fz) * 10.0;
    int q = (int)floor(n / 3.0) * 3;            /* Mth.quantize(n, 3) */
    int level = base + q;
    return min_cell_top_y < level ? min_cell_top_y : level;
}

/* ---------------- computeSurfaceLevel ---------------- */

static int aq_compute_surface_level(lysh_aquifer *aq, int x, int y, int z,
                                    int global_level, int min_cell_top_y, int has_above) {
    if (is_deep_dark_region(aq, x, y, z)) return LYSH_NO_FLUID_Y;

    int d = (min_cell_top_y + 8) - y;
    double f = has_above
             ? mc_clamped_lerp(1.0, 0.0, mc_lerp_progress((double)d, 0.0, 64.0))
             : 0.0;

    double high = mc_lerp_from_progress(f, 1.0, 0.0, -0.3, 0.8);
    double low  = mc_lerp_from_progress(f, 1.0, 0.0, -0.8, 0.4);

    double floodedness = mc_clamp(
            lysh_dblnoise_sample(&aq->floodedness, (double)x, (double)y * 0.67, (double)z),
            -1.0, 1.0);

    /* ① 充水 → 全局水位 */
    if (floodedness - high > 0.0) return global_level;
    /* ② 噪声化的局部水位 */
    if (floodedness - low > 0.0)
        return aq_randomized_fluid_surface_level(aq, x, y, z, min_cell_top_y);
    /* ③ 无水 */
    return LYSH_NO_FLUID_Y;
}

/* ---------------- computeFluidType ---------------- */

static lysh_block_state aq_compute_fluid_type(const lysh_aquifer *aq,
                                              int x, int y, int z,
                                              lysh_block_state global_type, int level) {
    lysh_block_state t = global_type;
    if (level <= -10 && level != LYSH_NO_FLUID_Y && global_type != LYSH_BLOCK_LAVA) {
        int bx = floor_div(x, 64);
        int by = floor_div(y, 40);
        int bz = floor_div(z, 64);
        double n = lysh_dblnoise_sample(&aq->lava, (double)bx, (double)by, (double)bz);
        if (mc_abs(n) > 0.3) t = LYSH_BLOCK_LAVA;
    }
    return t;
}

/* ---------------- computeFluid ---------------- */

static void aq_compute_fluid(lysh_aquifer *aq, int x, int y, int z,
                             int *out_level, lysh_block_state *out_type) {
    int global_level;
    lysh_block_state global_type;
    global_fluid(aq, y, &global_level, &global_type);

    int min_cell_top_y = INT32_MAX;
    int y_plus12 = y + 12;
    int y_minus12 = y - 12;
    int has_above = 0;

    for (int i = 0; i < 13; i++) {
        int bx = x + SURFACE_SAMPLING_OFFSETS_IN_CHUNKS[i][0] * 16;
        int bz = z + SURFACE_SAMPLING_OFFSETS_IN_CHUNKS[i][1] * 16;
        int top = aq->column_top(aq->column_top_user, bx, bz);
        int top_adj = top + 8;                     /* adjustSurfaceLevel */
        int is_center = (SURFACE_SAMPLING_OFFSETS_IN_CHUNKS[i][0] == 0
                      && SURFACE_SAMPLING_OFFSETS_IN_CHUNKS[i][1] == 0);

        if (is_center && y_minus12 > top_adj) {
            *out_level = global_level;
            *out_type = global_type;
            return;
        }
        int above = y_plus12 > top_adj;
        if (above || is_center) {
            int lvl;
            lysh_block_state typ;
            global_fluid(aq, top_adj, &lvl, &typ);
            if (lysh_aquifer_level_block_at(lvl, typ, top_adj) != LYSH_BLOCK_AIR) {
                if (is_center) has_above = 1;
                if (above) {
                    *out_level = lvl;
                    *out_type = typ;
                    return;
                }
            }
        }
        if (top < min_cell_top_y) min_cell_top_y = top;
    }

    int level = aq_compute_surface_level(aq, x, y, z, global_level, min_cell_top_y, has_above);
    *out_level = level;
    *out_type = aq_compute_fluid_type(aq, x, y, z, global_type, level);
}

void lysh_aquifer_compute_fluid(lysh_aquifer *aq, int x, int y, int z,
                                int *out_level, lysh_block_state *out_type) {
    aq_compute_fluid(aq, x, y, z, out_level, out_type);
}

/* ---------------- getAquiferStatus（aquiferCache 记忆化） ---------------- */

static void aq_get_aquifer_status(lysh_aquifer *aq, int idx,
                                  int *out_level, lysh_block_state *out_type) {
    if (aq->cache_valid[idx]) {
        *out_level = aq->fluid_level[idx];
        *out_type = (lysh_block_state)aq->fluid_type[idx];
        return;
    }
    int64_t p = aq->location[idx];
    int level;
    lysh_block_state type;
    aq_compute_fluid(aq, blockpos_x(p), blockpos_y(p), blockpos_z(p), &level, &type);
    aq->fluid_level[idx] = level;
    aq->fluid_type[idx] = (uint8_t)type;
    aq->cache_valid[idx] = 1;
    *out_level = level;
    *out_type = type;
}

/* ---------------- calculatePressure ---------------- */

/* `barrierAcc` 是 e1/e2/e3 **共享**的 MutableDouble(NaN)：只有第一个非 NaN 才真正采样。 */
static double aq_calculate_pressure(lysh_aquifer *aq, int x, int y, int z,
                                    double *barrier_acc,
                                    int a_level, lysh_block_state a_type,
                                    int b_level, lysh_block_state b_type) {
    lysh_block_state sa = lysh_aquifer_level_block_at(a_level, a_type, y);
    lysh_block_state sb = lysh_aquifer_level_block_at(b_level, b_type, y);

    if ((sa == LYSH_BLOCK_LAVA && sb == LYSH_BLOCK_WATER)
        || (sa == LYSH_BLOCK_WATER && sb == LYSH_BLOCK_LAVA)) {
        return 2.0;
    }

    int dy = a_level - b_level;
    if (dy < 0) dy = -dy;
    if (dy == 0) return 0.0;

    double mid = 0.5 * ((double)a_level + (double)b_level);
    double f = ((double)y + 0.5) - mid;
    double half = (double)dy / 2.0;
    double n = half - mc_abs(f);

    double o;
    if (f > 0.0) {
        double p = 0.0 + n;
        o = p > 0.0 ? p / 1.5 : p / 2.5;
    } else {
        double p = 3.0 + n;
        o = p > 0.0 ? p / 3.0 : p / 10.0;
    }

    double inner;
    if (o < -2.0 || o > 2.0) {
        inner = 0.0;
    } else if (barrier_acc[0] != barrier_acc[0]) {     /* NaN 检测 */
        double q = lysh_dblnoise_sample(&aq->barrier, (double)x, (double)y * 0.5, (double)z);
        barrier_acc[0] = q;
        inner = q;
    } else {
        inner = barrier_acc[0];
    }
    return 2.0 * (inner + o);
}

/* ---------------- computeSubstance ---------------- */

lysh_block_state lysh_aquifer_compute_substance(lysh_aquifer *aq, int x, int y, int z,
                                                double density) {
    /* ① 密度 > 0 → 实心 */
    if (density > 0.0) {
        aq->should_schedule_fluid_update = 0;
        return LYSH_BLOCK_NULL;
    }

    int global_level;
    lysh_block_state global_type;
    global_fluid(aq, y, &global_level, &global_type);

    /* ② 高于 skipSamplingAboveY → 直接信全局 picker */
    if (y > aq->skip_sampling_above_y) {
        aq->should_schedule_fluid_update = 0;
        return lysh_aquifer_level_block_at(global_level, global_type, y);
    }
    /* ③ 该点本身是岩浆层 → 岩浆 */
    if (lysh_aquifer_level_block_at(global_level, global_type, y) == LYSH_BLOCK_LAVA) {
        aq->should_schedule_fluid_update = 0;
        return LYSH_BLOCK_LAVA;
    }

    /* ④ 2×3×2 邻域里找最近 **4** 个随机锚点 */
    int gx = floor_div(x - 5, 16);
    int gy = floor_div(y + 1, 12);
    int gz = floor_div(z - 5, 16);

    int d[4] = {INT32_MAX, INT32_MAX, INT32_MAX, INT32_MAX};
    int slot[4] = {0, 0, 0, 0};
    xoroshiro_t factory;
    factory.lo = aq->rand_lo;
    factory.hi = aq->rand_hi;

    for (int i = 0; i <= 1; i++) {
        for (int j = -1; j <= 1; j++) {
            for (int k = 0; k <= 1; k++) {
                int cx = gx + i, cy = gy + j, cz = gz + k;
                int idx = aq_index(aq, cx, cy, cz);
                if (idx < 0) continue;         /* 越界：真实游戏不会出现 */

                int64_t packed = aq->location[idx];
                if (packed == LYSH_POS_UNSET) {
                    xoroshiro_t rnd;
                    lysh_xr_create_random_pos(&rnd, &factory, cx, cy, cz);
                    int px = from_grid_x(cx, lysh_xr_next_int_bound(&rnd, 10));
                    int py = from_grid_y(cy, lysh_xr_next_int_bound(&rnd, 9));
                    int pz = from_grid_z(cz, lysh_xr_next_int_bound(&rnd, 10));
                    packed = blockpos_as_long(px, py, pz);
                    aq->location[idx] = packed;
                }

                int dx = blockpos_x(packed) - x;
                int dy = blockpos_y(packed) - y;
                int dz = blockpos_z(packed) - z;
                int dist = dx * dx + dy * dy + dz * dz;

                /* ⚠️ 26.1.2 的判据是 `dist <= d[k]`（含等号） */
                if (dist <= d[0]) {
                    d[3] = d[2]; d[2] = d[1]; d[1] = d[0]; d[0] = dist;
                    slot[3] = slot[2]; slot[2] = slot[1]; slot[1] = slot[0]; slot[0] = idx;
                } else if (dist <= d[1]) {
                    d[3] = d[2]; d[2] = d[1]; d[1] = dist;
                    slot[3] = slot[2]; slot[2] = slot[1]; slot[1] = idx;
                } else if (dist <= d[2]) {
                    d[3] = d[2]; d[2] = dist;
                    slot[3] = slot[2]; slot[2] = idx;
                } else if (dist <= d[3]) {
                    d[3] = dist; slot[3] = idx;
                }
            }
        }
    }

    int l0, l1, l2, l3;
    lysh_block_state t0, t1, t2, t3;
    aq_get_aquifer_status(aq, slot[0], &l0, &t0);
    double s01 = similarity(d[0], d[1]);
    lysh_block_state result = lysh_aquifer_level_block_at(l0, t0, y);

    /* ⑤ s01 <= 0：直接返回，并按 FLOWING_UPDATE_SIMULARITY 决定 tick */
    if (!(s01 > 0.0)) {
        if (s01 >= LYSH_FLOWING_UPDATE_SIMULARITY) {
            aq_get_aquifer_status(aq, slot[1], &l1, &t1);
            aq->should_schedule_fluid_update = !(l0 == l1 && t0 == t1);
        } else {
            aq->should_schedule_fluid_update = 0;
        }
        return result;
    }

    /* ⑥ 水面 + 正下方是岩浆 → 岩浆更新 */
    {
        int below_level;
        lysh_block_state below_type;
        global_fluid(aq, y - 1, &below_level, &below_type);
        if (result == LYSH_BLOCK_WATER
            && lysh_aquifer_level_block_at(below_level, below_type, y - 1) == LYSH_BLOCK_LAVA) {
            aq->should_schedule_fluid_update = 1;
            return result;
        }
    }

    double barrier_acc[1];
    barrier_acc[0] = NAN;

    aq_get_aquifer_status(aq, slot[1], &l1, &t1);
    double p1 = s01 * aq_calculate_pressure(aq, x, y, z, barrier_acc, l0, t0, l1, t1);
    if (density + p1 > 0.0) {
        aq->should_schedule_fluid_update = 0;
        return LYSH_BLOCK_NULL;
    }

    aq_get_aquifer_status(aq, slot[2], &l2, &t2);
    double s02 = similarity(d[0], d[2]);
    if (s02 > 0.0) {
        double p2 = s01 * s02 * aq_calculate_pressure(aq, x, y, z, barrier_acc, l0, t0, l2, t2);
        if (density + p2 > 0.0) {
            aq->should_schedule_fluid_update = 0;
            return LYSH_BLOCK_NULL;
        }
    }

    double s12 = similarity(d[1], d[2]);
    if (s12 > 0.0) {
        double p3 = s01 * s12 * aq_calculate_pressure(aq, x, y, z, barrier_acc, l1, t1, l2, t2);
        if (density + p3 > 0.0) {
            aq->should_schedule_fluid_update = 0;
            return LYSH_BLOCK_NULL;
        }
    }

    /* ⑦ shouldScheduleFluidUpdate 的四个判据 */
    int b0 = !(l0 == l1 && t0 == t1);
    int b1 = (s12 >= LYSH_FLOWING_UPDATE_SIMULARITY) && !(l1 == l2 && t1 == t2);
    int b2 = (s02 >= LYSH_FLOWING_UPDATE_SIMULARITY) && !(l0 == l2 && t0 == t2);
    if (b0 || b1 || b2) {
        aq->should_schedule_fluid_update = 1;
    } else {
        aq_get_aquifer_status(aq, slot[3], &l3, &t3);
        double s03 = similarity(d[0], d[3]);
        aq->should_schedule_fluid_update =
              (s02 >= LYSH_FLOWING_UPDATE_SIMULARITY)
           && (s03 >= LYSH_FLOWING_UPDATE_SIMULARITY)
           && !(l0 == l3 && t0 == t3);
    }
    return result;
}
