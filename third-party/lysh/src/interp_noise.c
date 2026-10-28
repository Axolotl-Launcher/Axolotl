/* lysh interp_noise.c —— 见 interp_noise.h 顶部的取证与陷阱说明。
 *
 * 表达式结构逐条照抄字节码，不做任何浮点"化简"（不满足结合律）。
 */
#include "interp_noise.h"

#include <math.h>

#include "mcmath.h"

/* MC 的 base_3d_noise 标识符字面量。
 * 取证：MC_NoiseColumnSampler.txt:210-223 —— `ldc_w #296 // String terrain`
 * 拼成 `new Identifier("terrain")`，其 toString() 是 "minecraft:terrain"。
 * 这里按 **toString()** 喂给 createRandom —— 与 L1 的 "minecraft:erosion" 等一致。 */
#define LYSH_TERRAIN_ID "minecraft:terrain"

/* 1.18.1：xzScale = yScale = 684.412 * sampling.*Scale()，factor 从 sampling 读。 */
#define LYSH_INTERP_BASE 684.412

void lysh_interp_noise_init(lysh_interp_noise *s, uint64_t world_seed,
                            double sampling_xz_scale, double sampling_y_scale,
                            double sampling_xz_factor, double sampling_y_factor,
                            int cell_width, int cell_height) {
    /* 构造顺序就是字节码顺序：先 new 出 random，再 lower → upper → interp，
     * 三者**共用同一个 random 顺序消费**。
     * 注意这里**不能**用 lysh_dblnoise_from_seed（那条是按名字派生，两回事）。 */
    xoroshiro_t r;
    lysh_xr_init(&r, world_seed);
    xoroshiro_t deriver;
    lysh_xr_deriver(&deriver, &r);
    xoroshiro_t terrain;
    lysh_xr_create_random(&terrain, &deriver, LYSH_TERRAIN_ID);

    /* rangeClosed(-15, 0) → 16 档，amp 恒为 1.0（createLegacy 的 calculateAmplitudes） */
    double amps16[16], amps8[8];
    for (int i = 0; i < 16; i++) amps16[i] = 1.0;
    for (int i = 0; i < 8; i++) amps8[i] = 1.0;

    lysh_octave_legacy_create(&s->lower, &terrain, -15, amps16, 16);
    lysh_octave_legacy_create(&s->upper, &terrain, -15, amps16, 16);
    lysh_octave_legacy_create(&s->interp, &terrain, -7, amps8, 8);

    s->xz_scale = LYSH_INTERP_BASE * sampling_xz_scale;
    s->y_scale = LYSH_INTERP_BASE * sampling_y_scale;
    s->xz_main_scale = s->xz_scale / sampling_xz_factor;
    s->y_main_scale = s->y_scale / sampling_y_factor;

    /* 26.1.2 那一套（求值时才除 factor；smear 默认 1 —— 这条路径不用它） */
    s->xz_multiplier = LYSH_INTERP_BASE * sampling_xz_scale;
    s->y_multiplier = LYSH_INTERP_BASE * sampling_y_scale;
    s->xz_factor = sampling_xz_factor;
    s->y_factor = sampling_y_factor;
    s->smear_scale_multiplier = 1.0;

    s->cell_width = cell_width;
    s->cell_height = cell_height;
    s->inited = 1;
}

void lysh_interp_noise_init_261(lysh_interp_noise *s, uint64_t world_seed,
                                double sampling_xz_scale, double sampling_y_scale,
                                double sampling_xz_factor, double sampling_y_factor,
                                double smear_scale_multiplier,
                                int cell_width, int cell_height) {
    /* 三个 octave 栈的构造与 1.18 完全一致（同一个 random 顺序消费） */
    lysh_interp_noise_init(s, world_seed, sampling_xz_scale, sampling_y_scale,
                           sampling_xz_factor, sampling_y_factor, cell_width, cell_height);
    s->smear_scale_multiplier = smear_scale_multiplier;
}

/* ---- 26.1.2 `synth.BlendedNoise.compute(ctx)`（javap 逐条对齐） ----
 *
 *   d = blockX * xzMultiplier                 e = blockY * yMultiplier
 *   f = blockZ * xzMultiplier                 g = d / xzFactor
 *   h = e / yFactor                           i = f / xzFactor
 *   j = yMultiplier * smearScaleMultiplier    k = j / yFactor
 *
 *   ① 8 档（mainNoise, firstOctave -7）：
 *        n += oct.noise(wrap(g*q), wrap(h*q), wrap(i*q), k*q, h*q) / q ,  q starts 1, halved
 *      t = (n / 10 + 1) / 2
 *      skipMin = t >= 1 ;  skipMax = t <= 0
 *   ② 16 档（minLimitNoise / maxLimitNoise, firstOctave -15）：
 *        xx = wrap(d*q)  yy = wrap(e*q)  zz = wrap(f*q)  ys = j*q  ymax = e*q
 *        if !skipMin: l += min.noise(xx,yy,zz,ys,ymax) / q
 *        if !skipMax: m += max.noise(xx,yy,zz,ys,ymax) / q
 *   返回 clampedLerp(t, l/512, m/512) / 128
 *
 * 与 1.18 的三点差别都在这段里：不做 floorDiv（直接用方块坐标）、
 * `(blockX * xzMultiplier) / xzFactor` 在求值时才除、y 的比例乘 smear。 */
double lysh_interp_calculate_noise_261(const lysh_interp_noise *s, int x, int y, int z) {
    double d = (double)x * s->xz_multiplier;
    double e = (double)y * s->y_multiplier;
    double f = (double)z * s->xz_multiplier;
    double g = d / s->xz_factor;
    double h = e / s->y_factor;
    double i = f / s->xz_factor;
    double j = s->y_multiplier * s->smear_scale_multiplier;
    double k = j / s->y_factor;

    double n = 0.0;
    double q = 1.0;
    for (int oct = 0; oct < 8; oct++) {
        const lysh_perlin *p = lysh_octave_legacy_get(&s->interp, oct);
        if (p != NULL) {
            n += lysh_perlin_sample(p,
                     lysh_maintain_precision(g * q),
                     lysh_maintain_precision(h * q),
                     lysh_maintain_precision(i * q),
                     k * q,
                     h * q) / q;
        }
        q /= 2.0;
    }
    double t = (n / 10.0 + 1.0) / 2.0;

    int skip_min = (t >= 1.0);
    int skip_max = (t <= 0.0);

    double l = 0.0, m = 0.0;
    q = 1.0;
    for (int oct = 0; oct < 16; oct++) {
        double xx = lysh_maintain_precision(d * q);
        double yy = lysh_maintain_precision(e * q);
        double zz = lysh_maintain_precision(f * q);
        double ys = j * q;
        double ymax = e * q;
        if (!skip_min) {
            const lysh_perlin *p = lysh_octave_legacy_get(&s->lower, oct);
            if (p != NULL) l += lysh_perlin_sample(p, xx, yy, zz, ys, ymax) / q;
        }
        if (!skip_max) {
            const lysh_perlin *p = lysh_octave_legacy_get(&s->upper, oct);
            if (p != NULL) m += lysh_perlin_sample(p, xx, yy, zz, ys, ymax) / q;
        }
        q /= 2.0;
    }

    return mc_clamped_lerp(l / 512.0, m / 512.0, t) / 128.0;
}
