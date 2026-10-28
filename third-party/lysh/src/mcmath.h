/* lysh mcmath.h —— Java 语义的数学工具函数（逐位等价，含 NaN / 负零边界）。
 *
 * 这些函数看着"显然"，但 Java 的实现在边界上与直觉不同，直接写 `a > b ? a : b`
 * 会在 ±0.0 上产生不同结果。既然目标是与 Java 逐位对拍，就照抄它的分支结构。
 *
 * 取证：
 *   - MathHelper.clamp / lerp / clampedLerp / getLerpProgress / lerpFromProgress
 *       -> javap net.minecraft.util.math.MathHelper
 *   - Math.max / Math.min / Math.abs
 *       -> javap java.lang.Math（JDK 9+ 带负零特判的实现，见 _archive/dev_scratch_20260921.zip -> JDK_Math.txt）
 */
#ifndef LYSH_MCMATH_H
#define LYSH_MCMATH_H

#include <math.h>
#include <stdint.h>
#include <string.h>

/* Math.abs(double)：清符号位。用 fabs 而非 `v < 0 ? -v : v`
 * —— 后者对 -0.0 返回 -0.0，Java 返回 +0.0。 */
static inline double mc_abs(double v) { return fabs(v); }

/* MathHelper.clamp(double v, double min, double max)
 *   v < min -> min; v > max -> max; 否则 v（NaN 落到最后一条，返回 v） */
static inline double mc_clamp(double v, double lo, double hi) {
    if (v < lo) return lo;
    if (v > hi) return hi;
    return v;
}

/* MathHelper.lerp(double delta, double start, double end) = start + delta * (end - start) */
static inline double mc_lerp(double delta, double start, double end) {
    return start + delta * (end - start);
}

/* MathHelper.clampedLerp(double start, double end, double delta) */
static inline double mc_clamped_lerp(double start, double end, double delta) {
    if (delta < 0.0) return start;
    if (delta > 1.0) return end;
    return mc_lerp(delta, start, end);
}

/* MathHelper.getLerpProgress(double value, double start, double end) */
static inline double mc_lerp_progress(double value, double start, double end) {
    return (value - start) / (end - start);
}

/* MathHelper.lerpFromProgress(double value, double min, double max, double minOut, double maxOut) */
static inline double mc_lerp_from_progress(double value, double min, double max,
                                           double min_out, double max_out) {
    return mc_lerp(mc_lerp_progress(value, min, max), min_out, max_out);
}

static inline uint64_t mc_raw_bits(double v) {
    uint64_t b;
    memcpy(&b, &v, sizeof(b));
    return b;
}
#define MC_NEG_ZERO_BITS 0x8000000000000000ULL

/*
 * Math.max(double a, double b)
 *   if (a != a) return a;                                   // a 是 NaN
 *   if (a == 0 && b == 0 && bits(a) == -0.0) return b;      // max(-0.0, +0.0) = +0.0
 *   return (a >= b) ? a : b;                                // b 是 NaN 时返回 b
 */
static inline double mc_max(double a, double b) {
    if (a != a) return a;
    if (a == 0.0 && b == 0.0 && mc_raw_bits(a) == MC_NEG_ZERO_BITS) return b;
    return (a >= b) ? a : b;
}

/* Math.min(double a, double b)
 *   if (a != a) return a;
 *   if (a == 0 && b == 0 && bits(b) == -0.0) return b;      // min(+0.0, -0.0) = -0.0
 *   return (a <= b) ? a : b;
 */
static inline double mc_min(double a, double b) {
    if (a != a) return a;
    if (a == 0.0 && b == 0.0 && mc_raw_bits(b) == MC_NEG_ZERO_BITS) return b;
    return (a <= b) ? a : b;
}

#endif /* LYSH_MCMATH_H */
