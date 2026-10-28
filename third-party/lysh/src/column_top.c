/* lysh column_top.c —— 见 column_top.h 的取证与公式说明。 */
#include "column_top.h"

#include "mcmath.h"
#include "spline.h"
#include "terrain_table.h"   /* LYSH_TERRAIN_SURFACE_OFFSET / _FACTOR */

/* DensityFunctions.yClampedGradient(fromY, toY, fromValue, toValue)：
 *   double t = Mth.clamp((y - fromY) / (toY - fromY), 0.0, 1.0);
 *   return Mth.lerp(t, fromValue, toValue);
 * overworld 的三处 (fromY,toY) 都非零，除法安全。 */
static inline double ygrad(double y, double from_y, double to_y,
                           double from_value, double to_value) {
    double t = mc_clamp((y - from_y) / (to_y - from_y), 0.0, 1.0);
    return mc_lerp(t, from_value, to_value);
}

/* DensityFunctions.quarterNegative：**把负的一半除以 4**，不是正的。
 *   quarterNegative(v) = v > 0.0 ? v : v * 0.25
 * 名字就是这个意思。因为节点外面还乘了 4.0，展开后正是经典 1.18 形式
 *   dens > 0 ? dens * 4.0 : dens
 * —— 与 src/density.c 里那一行完全一致。
 * ⚠️ 写反（v > 0 ? v*0.25 : v）会让 v>0 的分支塌成 v、v<0 的分支变成 4v，
 *    正好把两条 y 带的斜率对调 → 列顶系统性偏低 24。 */
static inline double quarter_negative(double v) { return v > 0.0 ? v : v * 0.25; }

/* `preliminary_surface_level.density` 节点在方块 y 处的值。
 * offset / factor 是 2D 缓存值（cache_2d），与 y 无关。
 * 结构照抄 JSON，不做任何"化简"（浮点不满足结合律）。 */
static double ps_density(double y, double offset, double factor) {
    double depth = ygrad(y, -64.0, 320.0, 1.5, -1.5) + offset;
    double cheese = 4.0 * quarter_negative(depth * factor);
    double c = mc_clamp(-0.703125 + cheese, -64.0, 64.0);
    double a1 = ygrad(y, 240.0, 256.0, 1.0, 0.0);
    double a2 = ygrad(y, -64.0, -40.0, 0.0, 1.0);
    return -0.390625 + (0.1171875 + a2 * (-0.1171875 + (-0.078125 + a1 * (0.078125 + c))));
}

/* find_top_surface 的 upperBound，方块坐标，[128 + 128*offset - 35/factor] clamp 到 [-40, 320]。
 * 35.0 是精确值：128.0 * 0.2734375 == 35.0。 */
static double ps_upper_bound(double offset, double factor) {
    double inv = 1.0 / factor;                       /* minecraft:invert */
    double inner = inv * 0.2734375 + offset * -1.0;  /* mul + mul 的加和顺序照抄 JSON */
    return mc_clamp(128.0 + -128.0 * inner, -40.0, 320.0);
}

/* `preliminary_surface_level` 里的 offset / factor 是**密度函数节点**，按 double 求值：
 *   offset = blend_offset*(1 - blend_alpha) + (-0.5037500262260437 + spline_offset) * blend_alpha
 *   factor = 10.0 + blend_alpha * (-10.0 + spline_factor)
 * 其中 blend_alpha = 1.0、blend_offset = 0.0（javap：`BlendAlpha.compute` 返回 dconst_1，
 * `BlendOffset.compute` 返回 dconst_0）。表达式顺序照抄 JSON，不做化简。
 *
 * ⚠️ **不要用 `ti.point.offset`**：那是 `spline + (-0.50375f)` 的 **float** 结果
 *    （1.18 `TerrainNoisePoint` 的语义），和这里的 double 版不是同一个数。
 *    早期版本写成 `point.offset - 0.5037500262260437`（叠加了第二次），
 *    offset 偏负 0.50375 → upperBound 低 ~64.5 → bad=400。 */
static void ps_offset_factor(const lysh_terrain_info *ti,
                             double *out_offset, double *out_factor) {
    lysh_noise_point p;
    float c = (float)ti->continentalness;
    float e = (float)ti->erosion;
    float w = (float)ti->weirdness;
    p.continentalness = c;
    p.erosion = e;
    p.normalized_weirdness = lysh_normalized_weirdness(w);
    p.weirdness = w;

    double raw_offset = (double)lysh_spline_apply(LYSH_TERRAIN_SURFACE_OFFSET, &p);
    double raw_factor = (double)lysh_spline_apply(LYSH_TERRAIN_SURFACE_FACTOR, &p);

    const double blend_alpha = 1.0;
    const double blend_offset = 0.0;

    *out_offset = blend_offset * (1.0 + -1.0 * blend_alpha)
                + (-0.5037500262260437 + raw_offset) * blend_alpha;
    *out_factor = 10.0 + blend_alpha * (-10.0 + raw_factor);
}

/* 见 column_top.h 顶部的取证说明：`lysh_terrain_info_at` 吃的是 **biome 坐标**。 */
static inline void ps_terrain_info(const lysh_terrain *t, int block_x, int block_z,
                                   lysh_terrain_info *out) {
    lysh_terrain_info_at(t, block_x >> 2, block_z >> 2, out);
}

int lysh_preliminary_surface_level(const lysh_terrain *t, int block_x, int block_z) {
    /* ⚠️ 这里传的是 **biome 坐标 (block >> 2)**。
     *
     * `lysh_terrain_info_at` 实现的是 1.18 `NoiseColumnSampler` 的语义：
     *   shifted = arg + offsetNoise.sample(arg) * 4     （arg 为 biome 坐标）
     *   cont/ero/ridge = noise.sample(shifted, 0, shifted)
     * 而 `lysh_dblnoise_sample*` 内部已经乘过噪声自己的 xz_scale=0.25。展开：
     *   shifted = biome + offsetNoise.sample(biome)
     *   climate = climateNoise.sample(shifted)
     * 令 biome = block >> 2 ⇔ block = 4*biome，则
     *   biome + offsetNoise.sample(biome) == (block*0.25) + offsetNoise.sample(block*0.25)
     * 正是 26.1.2 密度函数路径 `ShiftedNoise.compute` 的形式：
     *   x = blockX * xz_scale(0.25) + shiftX.compute(ctx)
     *   shiftX = offsetNoise.getValue(blockX * 0.25, 0, blockZ * 0.25) * 4   —— 也乘 0.25，
     *   所以偏移量同样按 biome 尺度取样。
     *
     * 实测（_archive/verify_tools_20260921.zip -> ct_probe_cmp.c vs oracle `PsCoordProbe3`）：传 block>>2 时，
     * C 的 cont/ero/weird 与 oracle 密度函数在 block 坐标处的值差 ≤ 2.5e-8
     * （纯 double→float 舍入）；传 block 时两者完全对不上。
     * `columntop_diff.c` 里 TerrainNoisePoint 那一路也用 >>2，与此一致。
     *
     * ⚠️ y 恒为 0：`ShiftedNoise.compute` 里 `y = blockY * yScale(0.0) + shiftY`，
     *    而 shiftY 是常量 0.0，所以密度函数路径永远在 y=0 的噪声平面上取值。 */
    lysh_terrain_info ti;
    ps_terrain_info(t, block_x, block_z, &ti);

    /* offset / factor 按密度函数的 double 语义求（见 ps_offset_factor） */
    double offset, factor;
    ps_offset_factor(&ti, &offset, &factor);

    int y = (int)(ps_upper_bound(offset, factor) / 8.0) * 8;   /* Mth.floor，值域内 */
    if (y <= LYSH_COLUMN_TOP_LOWER_BOUND) return LYSH_COLUMN_TOP_LOWER_BOUND;

    for (int i = y; i >= LYSH_COLUMN_TOP_LOWER_BOUND; i -= 8) {
        if (ps_density((double)i, offset, factor) > 0.0) return i;
    }
    return LYSH_COLUMN_TOP_LOWER_BOUND;
}

double lysh_overworld_offset(const lysh_terrain *t, int block_x, int block_z) {
    lysh_terrain_info ti;
    ps_terrain_info(t, block_x, block_z, &ti);
    double offset, factor;
    ps_offset_factor(&ti, &offset, &factor);
    return offset;
}
