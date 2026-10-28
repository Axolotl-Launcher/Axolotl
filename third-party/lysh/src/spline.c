/* lysh spline.c —— 见 spline.h 顶部的取证说明。 */
#include "spline.h"

#include <math.h>

#include "terrain_table.h"

/* MathHelper.binarySearch(0, n, i -> v < locations[i])
 * —— 返回第一个满足谓词的下标，都不满足则返回 n。NaN 时谓词恒 false，与 Java 一致。 */
static int lower_bound(const float *loc, int n, float v) {
    int start = 0, len = n;
    while (len > 0) {
        int half = len / 2;
        int mid = start + half;
        if (v < loc[mid]) {
            len = half;
        } else {
            start = mid + 1;
            len -= half + 1;
        }
    }
    return start;
}

/* MathHelper.lerp(float delta, float start, float end) */
static inline float lerpf(float delta, float a, float b) { return a + delta * (b - a); }

float lysh_spline_apply(int root, const lysh_noise_point *p) {
    const lysh_spline_node *n = &LYSH_TERRAIN_NODES[root];

    if (n->kind == 1) return n->constant;      /* FixedFloatFunction */

    float f = ((const float *)p)[n->coord];
    const float *loc = &LYSH_TERRAIN_FLOATS[n->loc];
    const float *der = &LYSH_TERRAIN_FLOATS[n->der];
    const int *ch = &LYSH_TERRAIN_CHILD[n->child];

    int i = lower_bound(loc, n->n, f) - 1;
    int j = n->n - 1;

    if (i < 0) {
        return lysh_spline_apply(ch[0], p) + der[0] * (f - loc[0]);
    }
    if (i == j) {
        return lysh_spline_apply(ch[j], p) + der[j] * (f - loc[j]);
    }

    float g = loc[i];
    float h = loc[i + 1];
    float k = (f - g) / (h - g);

    const int ci = ch[i], cj = ch[i + 1];
    float l = lysh_spline_apply(ci, p);
    float m = lysh_spline_apply(cj, p);
    float d1 = der[i], d2 = der[i + 1];

    /* 三次 Hermite：注意两个 p/q 的符号与顺序，照抄字节码 */
    float pp = d1 * (h - g) - (m - l);
    float qq = -d2 * (h - g) + (m - l);

    return lerpf(k, l, m) + k * (1.0f - k) * lerpf(k, pp, qq);
}

float lysh_normalized_weirdness(float w) {
    /* -(|(|w| - 0.6666667f)| - 0.33333334f) * 3.0f */
    float a = w < 0.0f ? -w : w;
    float b = a - 0.6666667f;
    float c = b < 0.0f ? -b : b;
    float d = c - 0.33333334f;
    return -d * 3.0f;
}

/* 26.1.2 `overworld/ridges_folded`（JSON 的常数是 double；全程 double，最后转 float）。
 * 求值顺序严格按 JSON：abs → add → abs → add → mul，不化简。 */
float lysh_normalized_weirdness_261(double w) {
    double a = fabs(w);
    double b = -0.6666666666666666 + a;
    double c = fabs(b);
    double d = -0.3333333333333333 + c;
    return (float)(-3.0 * d);
}

static inline void make_point(lysh_noise_point *p, float c, float e, float w) {
    p->continentalness = c;
    p->erosion = e;
    p->normalized_weirdness = lysh_normalized_weirdness(w);
    p->weirdness = w;
}

float lysh_terrain_offset(float c, float e, float w) {
    lysh_noise_point p;
    make_point(&p, c, e, w);
    return lysh_spline_apply(LYSH_TERRAIN_SURFACE_OFFSET, &p) + (-0.50375f);
}

float lysh_terrain_factor(float c, float e, float w) {
    lysh_noise_point p;
    make_point(&p, c, e, w);
    return lysh_spline_apply(LYSH_TERRAIN_SURFACE_FACTOR, &p);
}

float lysh_terrain_peak(float c, float e, float w) {
    lysh_noise_point p;
    make_point(&p, c, e, w);
    return lysh_spline_apply(LYSH_TERRAIN_SURFACE_PEAK, &p);
}

lysh_terrain_point lysh_terrain_noise_point(float continentalness, float erosion, float weirdness) {
    lysh_terrain_point t;
    t.offset = lysh_terrain_offset(continentalness, erosion, weirdness);
    t.factor = lysh_terrain_factor(continentalness, erosion, weirdness);
    t.peaks = lysh_terrain_peak(continentalness, erosion, weirdness);
    return t;
}

/* 26.1.2：坐标取气候噪声的原始 double，只有 normalized_weirdness 按 double 算，
 * 其余在样条入口转 float（样条本身仍是 float，= MC 的 CubicSpline.apply）。 */
lysh_terrain_point lysh_terrain_noise_point_261(double continentalness, double erosion,
                                                double weirdness) {
    lysh_noise_point p;
    p.continentalness = (float)continentalness;
    p.erosion = (float)erosion;
    p.normalized_weirdness = lysh_normalized_weirdness_261(weirdness);
    p.weirdness = (float)weirdness;

    lysh_terrain_point t;
    t.offset = lysh_spline_apply(LYSH_TERRAIN_SURFACE_OFFSET, &p) + (-0.50375f);
    t.factor = lysh_spline_apply(LYSH_TERRAIN_SURFACE_FACTOR, &p);
    t.peaks = lysh_spline_apply(LYSH_TERRAIN_SURFACE_PEAK, &p);
    return t;
}
