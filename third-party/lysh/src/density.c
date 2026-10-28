/* lysh density.c —— 见 density.h 顶部的取证与陷阱说明。
 *
 * 全部表达式结构照抄规格 §2.1/§2.2；浮点不满足结合律，不做任何"化简"。
 */
#include "density.h"

#include <math.h>

#include "caves.h"      /* lysh_scale_tunnels / lysh_scale_caves */
#include "mcmath.h"

/* NoiseHelper.lerpFromProgress(noise, x, y, z, a, b) */
static inline double lerp_from_progress(const lysh_dblnoise *n,
                                        double x, double y, double z,
                                        double a, double b) {
    return mc_lerp_from_progress(lysh_dblnoise_sample(n, x, y, z), -1.0, 1.0, a, b);
}

/* NoiseColumnSampler.sample(n, x, y, z, s) = n.sample(x/s, y/s, z/s) */
static inline double ncs_sample(const lysh_dblnoise *n, double x, double y, double z, double s) {
    return lysh_dblnoise_sample(n, x / s, y / s, z / s);
}

/* ---------------- 初始化 ---------------- */

void lysh_density_init(lysh_density *d, uint64_t world_seed) {
    d->vertical_block_size = 8;
    d->minimum_block_y = -8;
    d->vertical_block_count = 48;
    d->top_target = -0.078125;
    d->top_size = 2;
    d->top_offset = 8;
    d->bottom_target = 0.1171875;
    d->bottom_size = 3;
    d->bottom_offset = 0;

#define D(field, id) lysh_dblnoise_from_seed_id(&d->field, world_seed, id)
    D(jagged, "minecraft:jagged");
    D(cave_entrance, "minecraft:cave_entrance");
    D(spaghetti_roughness, "minecraft:spaghetti_roughness");
    D(spaghetti_roughness_modulator, "minecraft:spaghetti_roughness_modulator");
    D(spaghetti_3d_1, "minecraft:spaghetti_3d_1");
    D(spaghetti_3d_2, "minecraft:spaghetti_3d_2");
    D(spaghetti_3d_rarity, "minecraft:spaghetti_3d_rarity");
    D(spaghetti_3d_thickness, "minecraft:spaghetti_3d_thickness");
    D(spaghetti_2d, "minecraft:spaghetti_2d");
    D(spaghetti_2d_elevation, "minecraft:spaghetti_2d_elevation");
    D(spaghetti_2d_modulator, "minecraft:spaghetti_2d_modulator");
    D(spaghetti_2d_thickness, "minecraft:spaghetti_2d_thickness");
    D(pillar, "minecraft:pillar");
    D(pillar_rareness, "minecraft:pillar_rareness");
    D(pillar_thickness, "minecraft:pillar_thickness");
    D(cave_layer, "minecraft:cave_layer");
    D(cave_cheese, "minecraft:cave_cheese");
    D(noodle, "minecraft:noodle");
    D(noodle_thickness, "minecraft:noodle_thickness");
    D(noodle_ridge_a, "minecraft:noodle_ridge_a");
    D(noodle_ridge_b, "minecraft:noodle_ridge_b");
#undef D
}

/* ---------------- 单项 ---------------- */

/* method_38409 —— jagged（只有 peaks != 0 才生效） */
double lysh_jagged(const lysh_density *d, double peaks, int x, int z) {
    if (peaks == 0.0) return 0.0;
    double n = lysh_dblnoise_sample(&d->jagged, (double)x * 1500.0, 0.0, (double)z * 1500.0);
    return n > 0.0 ? peaks * n : (peaks / 2.0) * n;
}

/* sampleCaveEntranceNoise: caveEntrance.sample(x*0.75, y*0.5, z*0.75) + 0.37
 *                          + clampedLerp(0.3, 0.0, (10 + y) / 40.0) */
double lysh_cave_entrance_noise(const lysh_density *d, int x, int y, int z) {
    double s = lysh_dblnoise_sample(&d->cave_entrance,
                                    (double)x * 0.75, (double)y * 0.5, (double)z * 0.75);
    return s + 0.37 + mc_clamped_lerp(0.3, 0.0, (10.0 + (double)y) / 40.0);
}

/* sampleSpaghettiRoughnessNoise:
 *   d = lerpFromProgress(modulator, x, y, z, 0.0, 0.1)
 *   return (0.4 - |spaghettiRoughness.sample(x,y,z)|) * d
 */
double lysh_spaghetti_roughness_noise(const lysh_density *d, int x, int y, int z) {
    double m = lerp_from_progress(&d->spaghetti_roughness_modulator,
                                  (double)x, (double)y, (double)z, 0.0, 0.1);
    double r = mc_abs(lysh_dblnoise_sample(&d->spaghetti_roughness,
                                           (double)x, (double)y, (double)z));
    return (0.4 - r) * m;
}

/* sampleSpaghetti3dNoise */
double lysh_spaghetti_3d_noise(const lysh_density *d, int x, int y, int z) {
    double rarity = lysh_dblnoise_sample(&d->spaghetti_3d_rarity,
                                         (double)(x * 2), (double)y, (double)(z * 2));
    double e = lysh_scale_tunnels(rarity);
    double h = lerp_from_progress(&d->spaghetti_3d_thickness,
                                  (double)x, (double)y, (double)z, 0.065, 0.088);

    double l = ncs_sample(&d->spaghetti_3d_1, (double)x, (double)y, (double)z, e);
    double m = mc_abs(e * l) - h;
    double n = ncs_sample(&d->spaghetti_3d_2, (double)x, (double)y, (double)z, e);
    double o = mc_abs(e * n) - h;

    return mc_clamp(mc_max(m, o), -1.0, 1.0);
}

/* sampleSpaghetti2dNoise
 *   d = spaghetti2dModulator.sample(x*2, y, z*2);  e = scaleCaves(d)
 *   h = lerpFromProgress(thickness, x*2, y, z*2, 0.6, 1.3)
 *   i = sample(spaghetti2d, x, y, z, e)  -> j = |e*i| - 0.083*h
 *   k = minimumBlockY()  (= -8, cell 坐标)
 *   l = lerpFromProgress(elevation, x, 0, z, k, 8.0)
 *   m = (|l - y/8.0| - h)^3
 *   return clamp(max(m, j), -1, 1)
 */
double lysh_spaghetti_2d_noise(const lysh_density *d, int x, int y, int z) {
    double mod = lysh_dblnoise_sample(&d->spaghetti_2d_modulator,
                                      (double)(x * 2), (double)y, (double)(z * 2));
    double s = lysh_scale_caves(mod);

    double h = lerp_from_progress(&d->spaghetti_2d_thickness,
                                  (double)(x * 2), (double)y, (double)(z * 2), 0.6, 1.3);

    double v = ncs_sample(&d->spaghetti_2d, (double)x, (double)y, (double)z, s);
    double j = mc_abs(s * v) - 0.083 * h;

    double l = lerp_from_progress(&d->spaghetti_2d_elevation,
                                  (double)x, 0.0, (double)z,
                                  (double)d->minimum_block_y, 8.0);
    double m = mc_abs(l - (double)y / 8.0) - h;
    m = m * m * m;

    return mc_clamp(mc_max(m, j), -1.0, 1.0);
}

/* samplePillarNoise
 *   d = lerpFromProgress(pillarRareness, x, y, z, 0.0, 2.0)
 *   e = lerpFromProgress(pillarThickness, x, y, z, 0.0, 1.1); e = e^3
 *   f = pillar.sample(x*25.0, y*0.3, z*25.0)
 *   g = e * (f*2.0 - d)
 *   return g > 0.03 ? g : -Infinity
 */
double lysh_pillar_noise(const lysh_density *d, int x, int y, int z) {
    double rareness = lerp_from_progress(&d->pillar_rareness,
                                         (double)x, (double)y, (double)z, 0.0, 2.0);
    double thick = lerp_from_progress(&d->pillar_thickness,
                                      (double)x, (double)y, (double)z, 0.0, 1.1);
    thick = pow(thick, 3.0);
    double f = lysh_dblnoise_sample(&d->pillar,
                                    (double)x * 25.0, (double)y * 0.3, (double)z * 25.0);
    double g = thick * (f * 2.0 - rareness);
    return g > 0.03 ? g : -INFINITY;
}

/* sampleCaveLayerNoise: square(caveLayer.sample(x, y*8, z)) * 4.0（y*8 是 int 乘法） */
double lysh_cave_layer_noise(const lysh_density *d, int x, int y, int z) {
    double v = lysh_dblnoise_sample(&d->cave_layer, (double)x, (double)(y * 8), (double)z);
    return v * v * 4.0;
}

/* ---------------- 供 L6 组装用的单项（见 density.h） ---------------- */

double lysh_squeeze(double v) {
    double a = mc_clamp(v * 0.64, -1.0, 1.0);
    return a / 2.0 - a * a * a / 24.0;
}

/* ---------------- 26.1.2 noodle（caves/noodle.json） ---------------- */

/* JSON 的 range_choice：min_inclusive = -60, max_exclusive = 321  ->  -60 <= y <= 320 */
static inline int noodle_y_in_range(int y) { return y >= -60 && y <= 320; }

/* ⚠️ JSON 里的常数是 **-0.07500000000000001**（比 -0.075 大 1 ulp；它是 0.05 + 0.025
 * 的浮点结果），不要"化简"成 -0.075。 */
#define LYSH_NOODLE_THICK_BIAS (-0.07500000000000001)
#define LYSH_NOODLE_THICK_MUL  (-0.025)
/* JSON: xz_scale = y_scale = 2.6666666666666665（ridge a/b） */
#define LYSH_NOODLE_RIDGE_SCALE 2.6666666666666665

double lysh_noodle_child(const lysh_density *d, int which, int x, int y, int z) {
    switch (which) {
        case LYSH_NOODLE_N:
            if (!noodle_y_in_range(y)) return -1.0;
            return lysh_dblnoise_sample(&d->noodle, (double)x, (double)y, (double)z);
        case LYSH_NOODLE_THICKNESS:
            if (!noodle_y_in_range(y)) return 0.0;
            return LYSH_NOODLE_THICK_BIAS
                 + LYSH_NOODLE_THICK_MUL
                   * lysh_dblnoise_sample(&d->noodle_thickness, (double)x, (double)y, (double)z);
        case LYSH_NOODLE_RIDGE_A:
        case LYSH_NOODLE_RIDGE_B:
            if (!noodle_y_in_range(y)) return 0.0;
            return lysh_dblnoise_sample(which == LYSH_NOODLE_RIDGE_A ? &d->noodle_ridge_a
                                                                    : &d->noodle_ridge_b,
                                        (double)x * LYSH_NOODLE_RIDGE_SCALE,
                                        (double)y * LYSH_NOODLE_RIDGE_SCALE,
                                        (double)z * LYSH_NOODLE_RIDGE_SCALE);
        default:
            return 0.0;
    }
}

double lysh_noodle_combine(double n, double thickness, double ridge_a, double ridge_b) {
    if (n >= -1000000.0 && n < 0.0) return 64.0;
    return thickness + 1.5 * mc_max(mc_abs(ridge_a), mc_abs(ridge_b));
}

/* ---------------- slides ---------------- */

/* SlideConfig.method_38414: if (size <= 0) return density; d = (y-offset)/size; clampedLerp(target, density, d) */
static double slide_apply(double target, int size, int offset, double density, int y) {
    if (size <= 0) return density;
    double dd = (double)(y - offset) / (double)size;
    return mc_clamped_lerp(target, density, dd);
}

/* ⚠️ 这里两次调用的 y 参数不同，且 cellY 是 **int 除法**（向零截断）得到的 */
static double apply_slides(const lysh_density *d, double density, int cell_y) {
    int i = cell_y - d->minimum_block_y;
    density = slide_apply(d->top_target, d->top_size, d->top_offset,
                          density, d->vertical_block_count - i);
    density = slide_apply(d->bottom_target, d->bottom_size, d->bottom_offset, density, i);
    return density;
}

/* ---------------- 主函数 ---------------- */

double lysh_sample_noise_column(const lysh_density *d,
                                int x, int y, int z,
                                const lysh_terrain_point *t,
                                double base_noise,
                                int use_jagged, int no_noise_caves) {
    /* islandNoise != null 只发生在末地，overworld 不走这条路 */
    double jagged = use_jagged ? lysh_jagged(d, (double)t->peaks, x, z) : 0.0;
    /* method_39331: depth = (1.0 - y/128.0) + offset */
    double depth = (1.0 - (double)y / 128.0) + (double)t->offset;
    double dens = (depth + jagged) * (double)t->factor;
    dens = dens > 0.0 ? dens * 4.0 : dens;

    double g = dens + base_noise;

    double cheese, spaghetti, pillar;
    if (no_noise_caves || g < -64.0) {
        cheese = g;
        spaghetti = 64.0;
        pillar = -64.0;
    } else {
        double i = g - 1.5625;
        int shallow = i < 0.0;

        double cave_entrance = lysh_cave_entrance_noise(d, x, y, z);
        double rough = lysh_spaghetti_roughness_noise(d, x, y, z);
        double sp3d = lysh_spaghetti_3d_noise(d, x, y, z);
        double m = mc_min(cave_entrance, sp3d + rough);

        if (shallow) {
            cheese = g;
            spaghetti = m * 5.0;
            pillar = -64.0;
        } else {
            double layer = lysh_cave_layer_noise(d, x, y, z);
            if (layer > 64.0) {
                cheese = 64.0;
            } else {
                double raw = lysh_dblnoise_sample(&d->cave_cheese,
                                                  (double)x, (double)y / 1.5, (double)z);
                double p = mc_clamp(raw + 0.27, -1.0, 1.0);
                double q = i * 1.28;
                cheese = p + mc_clamped_lerp(0.5, 0.0, q) + layer;
            }
            double sp2d = lysh_spaghetti_2d_noise(d, x, y, z);
            spaghetti = mc_min(m, sp2d + rough);
            pillar = lysh_pillar_noise(d, x, y, z);
        }
    }

    double v = mc_max(mc_min(cheese, spaghetti), pillar);
    v = apply_slides(d, v, y / d->vertical_block_size);   /* int 除法 */
    /* blender.method_39338 在无混合世界是恒等 */
    return mc_clamp(v, -64.0, 64.0);
}
