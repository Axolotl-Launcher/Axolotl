/* lysh phase2.c —— 见 phase2.h 顶部的取证与语义说明。
 *
 * ⭐ 26.1.2 的 `interpolated` 节点（三线性插值）已实现。
 *
 * JSON（`_archive/dev_scratch_20260921.zip -> mcdata/26.1.2/noise_settings/overworld.json` 的 `final_density`）：
 *
 *   min( squeeze( mul(0.64, interpolated( blend_density( add(0.1171875, ... ) ) ) ) ),
 *        "minecraft:overworld/caves/noodle" )
 *
 * 也就是说：**`interpolated` 包住了除 squeeze / 0.64 / min-with-noodle 之外的全部**。
 * 所以 C 的最终密度是
 *
 *   final = min( lysh_squeeze(interp(X)), noodle )
 *
 * 其中 X = 逐点求值的 `lysh_sample_noise_column`（= 1.18 的 `sampleNoiseColumn`），
 * `interp` 是 4(x) × 8(y) × 4(z) cell 上的三线性插值。
 * `lysh_squeeze(v)` 恰好就是 MC 的 `squeeze(0.64 * v)`：JSON 里的 0.64 与
 * `DensityFunctions$Mapped` 的 SQUEEZE 分支（`clamp(v,-1,1)/2 - v^3/24`）
 * 合成一步（该分支里**没有**第二个 0.64 —— javap 确认）。
 *
 * 插值的几何（全部来自 26.1.2 字节码，不是猜的）：
 *   · `NoiseChunk` 的 cell = 4(x) × 8(y) × 4(z)；角点列取在
 *     (cellX*4, -64 + i*8, cellZ*4)，i = 0..48 —— 4 个 (x,z) 角点各一列。
 *   · `NoiseBasedChunkGenerator.iterateNoiseColumn` 每格做
 *     `selectCellYZ(cellY, 0)` + `updateForY/X/Z`；而 `selectCellYZ` 会把
 *     `fillingCell` 置真，于是 `NoiseInterpolator.compute` 走
 *     `Mth.lerp3(dx,dy,dz,n000,n100,n010,n110,n001,n101,n011,n111)` 分支。
 *   · `Mth.lerp3` = lerp(dz, lerp(dy, lerp(dx,..), lerp(dx,..)), ...)：
 *     **x 最内、y 次之、z 最外** —— 与 cubiomes 的 lerp3 一致。
 *   · `noodle` 在 `interpolated` **之外**（逐点 min），但它自己有 3 个
 *     `interpolated` 子节点（thickness / ridge_a / ridge_b），
 *     必须**各自插值后再组合**。
 *
 * 含水层这一侧是 **26.1.2 语义**（`Aquifer$NoiseBasedAquifer`，见 src/aquifer.c）：
 * `computeSubstance(ctx, density)` —— 只有一个密度输入，没有 1.18 的 `d1 <= -64` 分支。
 */
#include "phase2.h"

#include <math.h>
#include <stdlib.h>
#include <string.h>

#include "carver.h"
#include "mcmath.h"

/* Mth.lerp(delta, start, end) = start + delta * (end - start) */
static inline double lerp3_d(double delta, double start, double end) {
    return start + delta * (end - start);
}

/* Mth.lerp3(dx, dy, dz, n000, n100, n010, n110, n001, n101, n011, n111)
 *   = lerp(dz, lerp(dy, lerp(dx,n000,n100), lerp(dx,n010,n110)),
 *               lerp(dy, lerp(dx,n001,n101), lerp(dx,n011,n111)))
 * 参数顺序与 MC / cubiomes 完全一致（x 最内、z 最外）。 */
static inline double mc_lerp3(double dx, double dy, double dz,
                              double n000, double n100, double n010, double n110,
                              double n001, double n101, double n011, double n111) {
    double xz00 = lerp3_d(dy, n000, n010);
    double xz10 = lerp3_d(dy, n100, n110);
    double xz01 = lerp3_d(dy, n001, n011);
    double xz11 = lerp3_d(dy, n101, n111);
    double z0 = lerp3_d(dx, xz00, xz10);
    double z1 = lerp3_d(dx, xz01, xz11);
    return lerp3_d(dz, z0, z1);
}

/* ---------------- `interpolated` 网格 ---------------- */

/* 一个角点列的 49 个值：cellX/cellZ 是 **cell 坐标**（不是方块坐标）。
 * 方块坐标 = cellX * horizontal_block_size；y = minimum_y + i * vertical_block_size。 */
static void sample_cell_column(lysh_phase2 *p2, int cell_x, int cell_z, double out[49]) {
    int bx = cell_x * p2->horizontal_block_size;
    int bz = cell_z * p2->horizontal_block_size;
    int vbs = p2->vertical_block_size;

    /* biome 坐标就是 cell 坐标（TerrainNoisePoint 按 block >> 2 取） */
    lysh_terrain_info ti;
    lysh_terrain_info_at(&p2->terrain, cell_x, cell_z, &ti);

    for (int i = 0; i <= p2->vertical_block_count; i++) {
        int y = p2->minimum_y + i * vbs;
        double base = lysh_interp_calculate_noise_261(&p2->interp, bx, y, bz);
        out[i] = lysh_sample_noise_column(&p2->density, bx, y, bz, &ti.point, base,
                                          1 /*useJagged*/, 0 /*noNoiseCaves*/);
    }
}

/* noodle 的某一个 `interpolated` 子节点在一个角点列上的 49 个值。
 * which: 0 = thickness, 1 = ridge_a, 2 = ridge_b（noodle 噪声本身不插值）。 */
static void sample_noodle_column(lysh_phase2 *p2, int which, int cell_x, int cell_z,
                                 double out[49]) {
    int bx = cell_x * p2->horizontal_block_size;
    int bz = cell_z * p2->horizontal_block_size;
    int vbs = p2->vertical_block_size;
    int child = (which == 0) ? LYSH_NOODLE_THICKNESS
              : (which == 1) ? LYSH_NOODLE_RIDGE_A
                             : LYSH_NOODLE_RIDGE_B;

    for (int i = 0; i <= p2->vertical_block_count; i++) {
        int y = p2->minimum_y + i * vbs;
        out[i] = lysh_noodle_child(&p2->density, child, bx, y, bz);
    }
}

/* 保证 (cell_x, cell_z) 这个 cell 的 4 个角点列已在**当前 chunk 的角点列网格**里。
 * 网格 = 当前 chunk 的 5×5 个角点列（相邻 cell 共用），下标 gx*5 + gz（gx/gz = 0..4）。
 * `base = cell & ~3` 对负数也是 floor 到 4 的倍数（补码 AND）。 */
static void ensure_cell_cache(lysh_phase2 *p2, int cell_x, int cell_z) {
    int base_x = cell_x & ~3, base_z = cell_z & ~3;
    if (p2->cell_col_x != base_x || p2->cell_col_z != base_z) {
        p2->cell_col_x = base_x;
        p2->cell_col_z = base_z;
        p2->cell_col_valid = 0;
    }
    int gx0 = cell_x - base_x, gz0 = cell_z - base_z;      /* 0..3 */
    for (int dx = 0; dx < 2; dx++) {
        for (int dz = 0; dz < 2; dz++) {
            int idx = (gx0 + dx) * 5 + (gz0 + dz);
            if (p2->cell_col_valid & (1u << idx)) continue;
            int cx = base_x + gx0 + dx, cz = base_z + gz0 + dz;
            sample_cell_column(p2, cx, cz, p2->cell_col_main[idx]);
            sample_noodle_column(p2, 0, cx, cz, p2->cell_col_noodle[0][idx]);
            sample_noodle_column(p2, 1, cx, cz, p2->cell_col_noodle[1][idx]);
            sample_noodle_column(p2, 2, cx, cz, p2->cell_col_noodle[2][idx]);
            p2->cell_col_valid |= 1u << idx;
        }
    }
}

/* 对一个 cell 的 4 个角点列做三线性插值（Mth.lerp3）。cell_y ∈ [0, 47]。
 * 角点取自当前 chunk 的 5×5 网格，取值与旧的 4 角点数组**逐位相同**（只是不再重复算角点）。 */
static double interp_corners_chunk(const double grid[25][49],
                                   int cell_x, int cell_z,
                                   int rel_x, int rel_y, int rel_z, int cell_y) {
    int gx = cell_x & 3, gz = cell_z & 3;
    const double *c00 = grid[gx * 5 + gz];           /* (x0, z0) */
    const double *c10 = grid[(gx + 1) * 5 + gz];     /* (x1, z0) */
    const double *c01 = grid[gx * 5 + gz + 1];       /* (x0, z1) */
    const double *c11 = grid[(gx + 1) * 5 + gz + 1]; /* (x1, z1) */
    double dx = (double)rel_x / 4.0;
    double dy = (double)rel_y / 8.0;
    double dz = (double)rel_z / 4.0;
    return mc_lerp3(dx, dy, dz,
                    c00[cell_y],     c10[cell_y],
                    c00[cell_y + 1], c10[cell_y + 1],
                    c01[cell_y],     c11[cell_y],
                    c01[cell_y + 1], c11[cell_y + 1]);
}

/* `interpolated` 节点的**内容**（逐点求值，不做插值）——
 * 只为对拍/诊断保留，不参与产品路径。 */
double lysh_phase2_density_raw_at(lysh_phase2 *p2, int x, int y, int z) {
    lysh_terrain_info ti;
    lysh_terrain_info_at(&p2->terrain, x >> 2, z >> 2, &ti);
    double base = lysh_interp_calculate_noise_261(&p2->interp, x, y, z);
    return lysh_sample_noise_column(&p2->density, x, y, z, &ti.point,
                                    base, 1 /*useJagged*/, 0 /*noNoiseCaves*/);
}

/* 26.1.2 的 `final_density`（= `NoiseChunk.getInterpolatedDensity()`，含水层拿到的那个）。 */
double lysh_phase2_density_at(lysh_phase2 *p2, int x, int y, int z) {
    int vbs = p2->vertical_block_size;
    int y_off = y - p2->minimum_y;
    if (y_off < 0 || y_off >= p2->height) {
        /* 网格覆盖不到：退回逐点（产品路径不会走到这里）。 */
        return lysh_phase2_density_raw_at(p2, x, y, z);
    }

    int cell_x = x >> 2;            /* floorDiv(x, 4) */
    int cell_z = z >> 2;
    int rel_x = x & 3;              /* floorMod(x, 4) */
    int rel_z = z & 3;
    int cell_y = y_off / vbs;       /* 0..47 */
    int rel_y = y_off - cell_y * vbs;

    ensure_cell_cache(p2, cell_x, cell_z);

    double v = interp_corners_chunk(p2->cell_col_main, cell_x, cell_z,
                                    rel_x, rel_y, rel_z, cell_y);
    double squeezy = lysh_squeeze(v);

    /* noodle：thickness / ridge_a / ridge_b 三个 `interpolated` 子节点各自插值，
     * 再按 range_choice 组合；noodle 噪声本身逐点求值。 */
    double noodle_n = lysh_noodle_child(&p2->density, LYSH_NOODLE_N, x, y, z);
    double thick = interp_corners_chunk(p2->cell_col_noodle[0], cell_x, cell_z,
                                        rel_x, rel_y, rel_z, cell_y);
    double ridge_a = interp_corners_chunk(p2->cell_col_noodle[1], cell_x, cell_z,
                                          rel_x, rel_y, rel_z, cell_y);
    double ridge_b = interp_corners_chunk(p2->cell_col_noodle[2], cell_x, cell_z,
                                          rel_x, rel_y, rel_z, cell_y);
    double noodle = lysh_noodle_combine(noodle_n, thick, ridge_a, ridge_b);

    return mc_min(squeezy, noodle);
}

/* ---------------- CARVERS 阶段（carver.c）---------------- */

#define LYSH_CARVE_SLOTS 4

typedef struct {
    int valid;
    int chunk_x, chunk_z;
    unsigned long long lru;
    uint8_t cells[LYSH_CARVE_CELLS];      /* LYSH_CARVE_UNCHANGED / _AIR / _WATER / _LAVA */
} lysh_carve_slot;

struct lysh_carve_cache {
    lysh_carve_slot slot[LYSH_CARVE_SLOTS];
    unsigned long long tick;
};

/* 雕刻器的 base_fn：**雕刻前**的方块状态（= 密度 + 含水层）。
 * ⚠️ 绝不能换成雕刻后的口径 —— carver.c 自己维护"被改过的格子"，
 *    这里必须是 NOISE 阶段的原始结果。 */
static lysh_block_state phase2_carve_base_cb(void *user, int x, int y, int z) {
    return lysh_phase2_block_at((lysh_phase2 *)user, x, y, z);
}

/* 取目标 chunk 的雕刻结果；没算过就算一遍（LRU 4 槽）。返回 NULL = 关闭/分配失败。 */
static const uint8_t *carve_cells_for(lysh_phase2 *p2, int cx, int cz) {
    if (!p2->carve_enabled || !p2->carve) return NULL;
    lysh_carve_cache *cc = p2->carve;

    for (int i = 0; i < LYSH_CARVE_SLOTS; i++) {
        if (cc->slot[i].valid && cc->slot[i].chunk_x == cx && cc->slot[i].chunk_z == cz) {
            cc->slot[i].lru = ++cc->tick;
            return cc->slot[i].cells;
        }
    }

    int victim = 0;
    for (int i = 0; i < LYSH_CARVE_SLOTS; i++) {
        if (!cc->slot[i].valid) { victim = i; break; }
        if (cc->slot[i].lru < cc->slot[victim].lru) victim = i;
    }
    lysh_carve_slot *s = &cc->slot[victim];

    /* 26.1.2 的 `applyCarvers` 用 `chunk.getOrCreateNoiseChunk(...).aquifer()`，
     * 也就是**目标 chunk** 的含水层网格；carveBlock 里的 computeSubstance 全在这个网格上。 */
    lysh_phase2_ensure_chunk(p2, cx * 16, cz * 16);
    lysh_carver_apply(p2->world_seed, cx, cz, phase2_carve_base_cb, p2,
                      &p2->aquifer, s->cells);
    s->valid = 1;
    s->chunk_x = cx;
    s->chunk_z = cz;
    s->lru = ++cc->tick;
    return s->cells;
}

void lysh_phase2_init(lysh_phase2 *p2, uint64_t world_seed, int large_biomes) {
    memset(p2, 0, sizeof(*p2));
    p2->horizontal_block_size = 4;
    p2->vertical_block_size = 8;
    p2->minimum_block_y = -8;
    p2->vertical_block_count = 48;
    p2->minimum_y = -64;
    p2->height = 384;
    p2->sea_level = 63;
    p2->aq_chunk_x = INT32_MIN;
    p2->aq_chunk_z = INT32_MIN;
    p2->world_seed = world_seed;
    p2->carve_enabled = 1;

    lysh_terrain_init(&p2->terrain, world_seed, large_biomes);
    /* 26.1.2 的 `overworld/ridges_folded` 是 double 求值（见 spline.h） */
    lysh_terrain_set_semantics_261(&p2->terrain, 1);
    lysh_density_init(&p2->density, world_seed);
    lysh_aquifer_init(&p2->aquifer, world_seed, &p2->terrain,
                      p2->minimum_y, p2->height);
    /* L4：base_3d_noise = 26.1.2 的 `minecraft:old_blended_noise`（类 `synth.BlendedNoise`）。
     * 参数取自 `_archive/dev_scratch_20260921.zip -> mcdata/26.1.2/density_function/overworld/base_3d_noise.json`：
     *   xz_scale 0.25 / y_scale 0.125 / xz_factor 80.0 / y_factor 160.0
     *   smear_scale_multiplier 8.0
     * （1.18.1 是 xz_scale = y_scale = 1.0 且没有 smear —— 两版**不能混用**。） */
    lysh_interp_noise_init_261(&p2->interp, world_seed,
                               0.25, 0.125, 80.0, 160.0, 8.0,
                               p2->horizontal_block_size, p2->vertical_block_size);

    /* CARVERS 阶段的结果缓存。4 个槽：footprint 最多跨 2x2 = 4 个 chunk，
     * 一个槽 98304 字节（384*16*16）。分配失败就静默退回"雕刻前"高度。 */
    p2->carve = (lysh_carve_cache *)calloc(1, sizeof(lysh_carve_cache));
    p2->inited = 1;
}

void lysh_phase2_free(lysh_phase2 *p2) {
    lysh_aquifer_free(&p2->aquifer);
    free(p2->carve);
    p2->carve = NULL;
}

void lysh_phase2_set_carvers(lysh_phase2 *p2, int enable) {
    p2->carve_enabled = enable ? 1 : 0;
}

void lysh_phase2_set_column_top(lysh_phase2 *p2, lysh_aquifer_column_top_fn fn, void *user) {
    p2->aquifer.column_top = fn;
    p2->aquifer.column_top_user = user;
    p2->aq_chunk_x = INT32_MIN;     /* 网格依赖列顶 -> 强制重建 */
    p2->aq_chunk_z = INT32_MIN;
}

/* 保证含水层网格对应 (x,z) 所在的 chunk（26.1.2 的 ctor 按 ChunkPos 建网格）。 */
void lysh_phase2_ensure_chunk(lysh_phase2 *p2, int x, int z) {
    int cx = x >> 4, cz = z >> 4;
    if (p2->aq_chunk_x == cx && p2->aq_chunk_z == cz && p2->aquifer.grid_len > 0) return;
    lysh_aquifer_begin_chunk(&p2->aquifer, cx, cz);
    p2->aq_chunk_x = cx;
    p2->aq_chunk_z = cz;
}

lysh_block_state lysh_phase2_block_at(lysh_phase2 *p2, int x, int y, int z) {
    lysh_phase2_ensure_chunk(p2, x, z);
    /* 26.1.2：含水层拿到的是 **插值后** 的 final_density（= getInterpolatedDensity） */
    double density = lysh_phase2_density_at(p2, x, y, z);
    return lysh_aquifer_compute_substance(&p2->aquifer, x, y, z, density);
}

int lysh_phase2_height_pre_carver(lysh_phase2 *p2, int x, int z) {
    int vbs = p2->vertical_block_size;
    int min_cell = p2->minimum_block_y;
    int count = p2->vertical_block_count;

    lysh_phase2_ensure_chunk(p2, x, z);

    /* 谓词 = NOT_AIR：任何非空气方块（含石头、水、岩浆）都算表面 */
    for (int i = count - 1; i >= 0; i--) {
        for (int j = vbs - 1; j >= 0; j--) {
            int y = (min_cell + i) * vbs + j;
            lysh_block_state st = lysh_phase2_block_at(p2, x, y, z);
            if (st != LYSH_BLOCK_AIR) return y + 1;    /* ★ 返回值是 y+1 */
        }
    }
    return p2->minimum_y;                              /* orElse(bottomY) */
}

/* 雕刻后的方块状态；UNCHANGED 表示"没被改过"，回退密度 + 含水层。 */
static inline lysh_block_state carved_state_at(lysh_phase2 *p2, const uint8_t *cells,
                                               int x, int y, int z) {
    uint8_t v = cells ? cells[LYSH_CARVE_INDEX(x & 15, y, z & 15)]
                      : (uint8_t)LYSH_CARVE_UNCHANGED;
    if (v == LYSH_CARVE_AIR)   return LYSH_BLOCK_AIR;
    if (v == LYSH_CARVE_WATER) return LYSH_BLOCK_WATER;
    if (v == LYSH_CARVE_LAVA)  return LYSH_BLOCK_LAVA;
    return lysh_phase2_block_at(p2, x, y, z);
}

int lysh_phase2_height(lysh_phase2 *p2, int x, int z) {
    int vbs = p2->vertical_block_size;
    int min_cell = p2->minimum_block_y;
    int count = p2->vertical_block_count;

    lysh_phase2_ensure_chunk(p2, x, z);
    /* 小屋读的是 FEATURES 开头 `primeHeightmaps` 从**活方块**重量的高度图，
     * 也就是 CARVERS 之后的状态（见 carver.h / carver_invoke_spec §3.3）。 */
    const uint8_t *cells = carve_cells_for(p2, x >> 4, z >> 4);

    for (int i = count - 1; i >= 0; i--) {
        for (int j = vbs - 1; j >= 0; j--) {
            int y = (min_cell + i) * vbs + j;
            lysh_block_state st = carved_state_at(p2, cells, x, y, z);
            if (st != LYSH_BLOCK_AIR) return y + 1;
        }
    }
    return p2->minimum_y;
}
