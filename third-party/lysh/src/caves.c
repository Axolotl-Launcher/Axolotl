/* lysh caves.c —— 见 caves.h 顶部的取证与陷阱说明。
 *
 * 这一份是 SearchCoords.Entrance / Entrance2 / Cheese 的逐行移植。
 * 移植纪律：**表达式结构照抄**，包括括号位置与乘法结合方向 ——
 * 浮点加法/乘法不满足结合律，任何"化简"都可能改变最后一位。
 */
#include "caves.h"

#include "mcmath.h"

/* NoiseHelper.lerpFromProgress(DoublePerlinNoiseSampler n, double x, double y, double z,
 *                              double start, double end)
 *   d = n.sample(x, y, z);
 *   return MathHelper.lerpFromProgress(d, -1.0, 1.0, start, end);
 */
static inline double noise_lerp_from_progress(const lysh_dblnoise *n,
                                              double x, double y, double z,
                                              double start, double end) {
    double d = lysh_dblnoise_sample(n, x, y, z);
    return mc_lerp_from_progress(d, -1.0, 1.0, start, end);
}

/* NoiseColumnSampler.sample(NoiseSampler n, double x, double y, double z, double s)
 *   return n.sample(x / s, y / s, z / s);
 *
 * ⚠️ 是除法不是乘法。s ∈ {0.75, 1.0, 1.5, 2.0}，x/0.75 与 x*(1/0.75) 的舍入不同。 */
static inline double ncs_sample(const lysh_dblnoise *n,
                                double x, double y, double z, double s) {
    return lysh_dblnoise_sample(n, x / s, y / s, z / s);
}

/* ---------------- CaveScaler ---------------- */

/* 字节码：dload_0 / -0.5 / dcmpg / ifge  ->  依次 0.75, 1.0, 1.5, 2.0
 * NaN 时 dcmpg 压入 1，所有 ifge 都跳走，落到最后的 2.0 —— 与 Java 一致。 */
double lysh_scale_tunnels(double v) {
    if (v < -0.5) return 0.75;
    if (v < 0.0) return 1.0;
    if (v < 0.5) return 1.5;
    return 2.0;
}

/* 字节码：-0.75 -> 0.5, -0.5 -> 0.75, 0.5 -> 1.0, 0.75 -> 2.0, 否则 3.0 */
double lysh_scale_caves(double v) {
    if (v < -0.75) return 0.5;
    if (v < -0.5) return 0.75;
    if (v < 0.5) return 1.0;
    if (v < 0.75) return 2.0;
    return 3.0;
}

/* ---------------- 噪声派生 ---------------- */

void lysh_caves_init(lysh_caves *c, uint64_t world_seed) {
    lysh_dblnoise_from_seed_id(&c->cave_entrance, world_seed, "minecraft:cave_entrance");
    lysh_dblnoise_from_seed_id(&c->spaghetti_3d_rarity, world_seed, "minecraft:spaghetti_3d_rarity");
    lysh_dblnoise_from_seed_id(&c->spaghetti_3d_thickness, world_seed, "minecraft:spaghetti_3d_thickness");
    lysh_dblnoise_from_seed_id(&c->spaghetti_3d_1, world_seed, "minecraft:spaghetti_3d_1");
    lysh_dblnoise_from_seed_id(&c->spaghetti_3d_2, world_seed, "minecraft:spaghetti_3d_2");
    lysh_dblnoise_from_seed_id(&c->spaghetti_roughness_modulator, world_seed,
                               "minecraft:spaghetti_roughness_modulator");
    lysh_dblnoise_from_seed_id(&c->spaghetti_roughness, world_seed, "minecraft:spaghetti_roughness");
    lysh_dblnoise_from_seed_id(&c->cave_layer, world_seed, "minecraft:cave_layer");
    lysh_dblnoise_from_seed_id(&c->cave_cheese, world_seed, "minecraft:cave_cheese");
}

/* ---------------- Entrance / Entrance2 / Cheese ---------------- */

/* 与 Entrance / Entrance2 共用的中段（p 之后到 q）。
 * 两边的 Java 代码是逐字重复的，所以这里也保持一份实现、两处调用。 */
static inline double spaghetti_pair(const lysh_caves *c, int x, int y, int z) {
    double d = lysh_dblnoise_sample(&c->spaghetti_3d_rarity,
                                    (double)(x * 2), (double)y, (double)(z * 2));
    double e = lysh_scale_tunnels(d);

    double h = noise_lerp_from_progress(&c->spaghetti_3d_thickness,
                                        (double)x, (double)y, (double)z, 0.065, 0.088);

    double l = ncs_sample(&c->spaghetti_3d_1, (double)x, (double)y, (double)z, e);
    double m = mc_abs(e * l) - h;
    double n = ncs_sample(&c->spaghetti_3d_2, (double)x, (double)y, (double)z, e);
    double o = mc_abs(e * n) - h;
    double p = mc_clamp(mc_max(m, o), -1.0, 1.0);

    double mod = lysh_dblnoise_sample(&c->spaghetti_roughness_modulator, (double)x, (double)y, (double)z);
    double rgh = lysh_dblnoise_sample(&c->spaghetti_roughness, (double)x, (double)y, (double)z);
    double q = (-0.05 + (-0.05 * mod)) * (-0.4 + mc_abs(rgh));

    return p + q;
}

/* SearchCoords.Entrance:1176-1191
 *
 *   double c = caveEntrance.sample(x * 0.75, y * 0.5, z * 0.75) + 0.37
 *            + MathHelper.clampedLerp(0.3, 0.0, (10 + (double) y) / 40.0);
 *   ...
 *   return Math.min(c, p + q);
 */
double lysh_entrance(const lysh_caves *c, int x, int y, int z) {
    double ent = lysh_dblnoise_sample(&c->cave_entrance,
                                      (double)x * 0.75, (double)y * 0.5, (double)z * 0.75);
    /* 注意 Java 是 ((sample + 0.37) + clampedLerp)，左结合、顺序不能换 */
    double cc = ent + 0.37 + mc_clamped_lerp(0.3, 0.0, (10.0 + (double)y) / 40.0);
    double pq = spaghetti_pair(c, x, y, z);
    return mc_min(cc, pq);
}

/* SearchCoords.Entrance2:1200-1213 —— 与 Entrance 共用后半段，但没有 c 项 */
double lysh_entrance2(const lysh_caves *c, int x, int y, int z) {
    return spaghetti_pair(c, x, y, z);
}

/* SearchCoords.Cheese:1193-1198
 *
 *   double a = 4 * caveLayer.sample(x, y * 8, z) * caveLayer.sample(x, y * 8, z);
 *   double b = MathHelper.clamp((0.27 + caveCheese.sample(x, y * 0.6666666666666666, z)), -1, 1);
 *   return a + b;
 *
 * `y * 8` 是 int 乘法（可能溢出，与 Java 一致地溢出）；`y * 0.6666666666666666` 是 double 乘法。
 */
double lysh_cheese(const lysh_caves *c, int x, int y, int z) {
    double layer = lysh_dblnoise_sample(&c->cave_layer, (double)x, (double)(y * 8), (double)z);
    double a = 4.0 * layer * layer;

    double cheese_raw = lysh_dblnoise_sample(&c->cave_cheese, (double)x,
                                             (double)y * 0.6666666666666666, (double)z);
    double b = mc_clamp(0.27 + cheese_raw, -1.0, 1.0);

    return a + b;
}
