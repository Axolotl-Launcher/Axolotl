/* lysh noise.c —— 见 noise.h 顶部的语义来源与派生链说明。
 *
 * 求值部分与 _archive/dev_scratch_20260921.zip -> test/noise.c 相同（那份已对拍 100000/100000 逐位一致），
 * 这里把它整理成库，并补上「从种子派生」的部分。
 */
#include "noise.h"

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "noise_table.h"

/* MC PerlinNoiseSampler.GRADIENTS（已由 Java dump 校验） */
const int LYSH_GRADIENTS[16][3] = {
    { 1,  1,  0}, {-1,  1,  0}, { 1, -1,  0}, {-1, -1,  0},
    { 1,  0,  1}, {-1,  0,  1}, { 1,  0, -1}, {-1,  0, -1},
    { 0,  1,  1}, { 0, -1,  1}, { 0,  1, -1}, { 0, -1, -1},
    { 1,  1,  0}, { 0, -1,  1}, {-1,  1,  0}, { 0, -1, -1}
};

/* MC 的 SKIP_COUNT 魔数。注意：PerlinNoiseSampler 的构造实际只消耗 259 次 RNG
 * （3×nextDouble + 256×nextInt），而跳过用的是 262 —— 这是 MC 的真实行为，照抄。 */

/* ---------------- MC 工具函数 ---------------- */
static inline long lfloor_d(double v) { return (long)floor(v); }

/* MC 原式： v - lfloor(v / 2^25 + 0.5) * 2^25
 * 1) 2^25 是 2 的幂 → 除法可无损换成乘法
 * 2) |v| < 1e7 时 lfloor(...) 恒为 0 → 直接返回 v（逐位等价的快路径） */
static inline double maintain_precision(double v) {
    if (v > -1.0e7 && v < 1.0e7) return v;
    return v - (double)lfloor_d(v * 2.9802322387695312E-08 + 0.5) * 3.3554432E7;
}

static inline double perlin_fade(double d) {
    return d * d * d * (d * (d * 6.0 - 15.0) + 10.0);
}
static inline double lerp(double delta, double start, double end) {
    return start + delta * (end - start);
}
static inline double lerp2(double dx, double dy, double v00, double v10, double v01, double v11) {
    return lerp(dy, lerp(dx, v00, v10), lerp(dx, v01, v11));
}
static inline double lerp3(double dx, double dy, double dz,
                           double v000, double v100, double v010, double v110,
                           double v001, double v101, double v011, double v111) {
    return lerp(dz, lerp2(dx, dy, v000, v100, v010, v110),
                    lerp2(dx, dy, v001, v101, v011, v111));
}

static inline double grad(int hash, double x, double y, double z) {
    const int *g = LYSH_GRADIENTS[hash & 15];
    return (double)g[0] * x + (double)g[1] * y + (double)g[2] * z;
}
static inline int get_gradient(const lysh_perlin *p, int i) {
    return p->perm[i & 255] & 255;
}

/* ---------------- 构造 ---------------- */
void lysh_perlin_create(lysh_perlin *p, xoroshiro_t *rng) {
    p->ox = lysh_xr_next_double(rng) * 256.0;
    p->oy = lysh_xr_next_double(rng) * 256.0;
    p->oz = lysh_xr_next_double(rng) * 256.0;
    for (int i = 0; i < 256; i++) p->perm[i] = (unsigned char)i;
    for (int i = 0; i < 256; i++) {
        int j = lysh_xr_next_int_bound(rng, 256 - i);
        unsigned char b = p->perm[i];
        p->perm[i] = p->perm[i + j];
        p->perm[i + j] = b;
    }
    p->present = 1;
    /* y=0 特化常量 */
    p->jy = (int)floor(p->oy);
    p->hy = p->oy - p->jy;
    p->hy1 = p->hy - 1.0;
    p->fade_y = perlin_fade(p->hy);
}

void lysh_octave_create(lysh_octave *o, xoroshiro_t *rng,
                        int first_octave, const double *amps, int n) {
    if (n > LYSH_MAX_OCT) n = LYSH_MAX_OCT;
    o->n = n;
    memcpy(o->amp, amps, sizeof(double) * (size_t)n);
    o->lacunarity = pow(2.0, (double)first_octave);
    o->persistence = pow(2.0, (double)(n - 1)) / (pow(2.0, (double)n) - 1.0);
    for (int i = 0; i < LYSH_MAX_OCT; i++) o->oct[i].present = 0;

    /* MC 的真实路径（易读错，务必注意）：
     *   OctavePerlinNoiseSampler.create(random, firstOctave, amplitudes)
     *       -> new OctavePerlinNoiseSampler(random, Pair(firstOctave, amps), legacy = TRUE)
     * 这个 boolean 是 true，所以走的是【按名字派生】分支，而不是顺序消费分支：
     *   deriver = random.createRandomDeriver()                  // 消费 random 的 2 次 nextLong
     *   for i in 0..n-1: if amps[i] != 0:
     *       octaveSamplers[i] = new PerlinNoiseSampler(deriver.createRandom("octave_" + (firstOctave + i)))
     * 名字的配方来自常量池的 makeConcatWithConstants：BootstrapMethods 里是 "octave_\u0001"。
     * 于是每个 octave 的名字就是它的下标字符串（如 "-9"、"-6"），互相独立、与顺序无关。
     * 顺序消费分支（带 SKIP_COUNT=262）只用于 nether/legacy，我们不需要。 */
    xoroshiro_t deriver;
    lysh_xr_deriver(&deriver, rng);
    for (int i = 0; i < n; i++) {
        if (amps[i] == 0.0) continue;
        char name[32];
        snprintf(name, sizeof(name), "octave_%d", first_octave + i);
        xoroshiro_t per_octave;
        lysh_xr_create_random(&per_octave, &deriver, name);
        lysh_perlin_create(&o->oct[i], &per_octave);
    }
}

void lysh_dblnoise_create(lysh_dblnoise *dn, xoroshiro_t *rng,
                          int first_octave, const double *amps, int n) {
    lysh_octave_create(&dn->sub[0], rng, first_octave, amps, n);
    lysh_octave_create(&dn->sub[1], rng, first_octave, amps, n);

    /* MC 的 amplitude 公式（已用 5 个 dump 值验证）：
     *   span = maxIdx - minIdx          （非零 amplitude 的下标跨度，注意不是 +1）
     *   createAmplitude(s) = 0.1 * (1.0 + 1.0 / (s + 1.0))
     *   amplitude = 0.16666666666666666 / createAmplitude(span)
     * 例：AQUIFER_BARRIER span=0 -> 0.8333333333333333
     *     EROSION         span=4 -> 1.3888888888888888
     *     CONTINENTALNESS span=8 -> 1.4999999999999998（浮点，不是 1.5） */
    int lo = 0, hi = 0, seen = 0;
    for (int i = 0; i < n; i++) {
        if (amps[i] == 0.0) continue;
        if (!seen) { lo = i; hi = i; seen = 1; }
        else { if (i < lo) lo = i; if (i > hi) hi = i; }
    }
    double span = (double)(hi - lo);
    dn->amplitude = 0.16666666666666666 / (0.1 * (1.0 + 1.0 / (span + 1.0)));
}

void lysh_dblnoise_from_seed(lysh_dblnoise *dn, uint64_t world_seed,
                             const char *identifier,
                             int first_octave, const double *amps, int n) {
    xoroshiro_t r;
    lysh_xr_init(&r, world_seed);
    xoroshiro_t deriver;
    lysh_xr_deriver(&deriver, &r);
    xoroshiro_t per_noise;
    lysh_xr_create_random(&per_noise, &deriver, identifier);
    lysh_dblnoise_create(dn, &per_noise, first_octave, amps, n);
}

void lysh_dblnoise_from_seed_id(lysh_dblnoise *dn, uint64_t world_seed,
                                const char *identifier) {
    for (size_t i = 0; i < LYSH_NOISE_TABLE_N; i++) {
        const lysh_noise_def *d = &LYSH_NOISE_TABLE[i];
        if (strcmp(d->id, identifier) == 0) {
            lysh_dblnoise_from_seed(dn, world_seed, d->id, d->first_octave, d->amps, d->n);
            return;
        }
    }
    fprintf(stderr, "lysh: unknown noise identifier \"%s\"\n", identifier);
    memset(dn, 0, sizeof(*dn));
}

double lysh_maintain_precision(double v) { return maintain_precision(v); }

/* ===========================================================================
 * legacy octave —— 见 noise.h 的详细说明。顺序消费 + SKIP_COUNT=262 那条路径。
 * =========================================================================== */

/* MC 的 SKIP_COUNT（OctavePerlinNoiseSampler.skipCalls） */
#define LYSH_SKIP_COUNT 262

void lysh_octave_legacy_create(lysh_octave_legacy *o, xoroshiro_t *rng,
                               int first_octave, const double *amps, int n) {
    if (n > LYSH_MAX_OCT) n = LYSH_MAX_OCT;
    o->n = n;
    o->first_octave = first_octave;
    memcpy(o->amp, amps, sizeof(double) * (size_t)n);
    for (int i = 0; i < LYSH_MAX_OCT; i++) { o->oct[i].present = 0; o->present[i] = 0; }
    /* 注意：不是 pow(2, firstOctave)。字节码 :383-407 是
     *   persistence = pow(2, n-1) / (pow(2, n) - 1)      ← 位运算顺序照抄
     *   lacunarity  = pow(2, -i0)  其中 i0 = -firstOctave
     * 而且它把**除数**存进 persistence 字段，sample 里每档再 /2。 */
    o->lacunarity = pow(2.0, (double)(-first_octave));
    o->persistence = pow(2.0, (double)(n - 1)) / (pow(2.0, (double)n) - 1.0);

    int i0 = -first_octave;

    /* ① 无条件先建一个（会消耗 RNG） */
    lysh_perlin shared;
    lysh_perlin_create(&shared, rng);

    /* ② 可能复用到 i0 档 */
    if (i0 >= 0 && i0 < n && amps[i0] != 0.0) {
        o->oct[i0] = shared;
        o->present[i0] = 1;
    }

    /* ③ j 从 i0-1 递减到 0 */
    for (int j = i0 - 1; j >= 0; j--) {
        if (j < n && amps[j] != 0.0) {
            lysh_perlin_create(&o->oct[j], rng);
            o->present[j] = 1;
        } else {
            lysh_xr_consume(rng, LYSH_SKIP_COUNT);
        }
    }

    /* ④ 断言：非空档位数 == 非零 amplitude 个数（MC 抛 IllegalStateException；
     * C 端不抛，但把不一致打到 stderr —— 静默失败是这套代码最大的风险） */
    int non_null = 0, nonzero_amp = 0;
    for (int i = 0; i < n; i++) {
        if (o->present[i]) non_null++;
        if (amps[i] != 0.0) nonzero_amp++;
    }
    if (non_null != nonzero_amp) {
        fprintf(stderr, "lysh: octave_legacy nonNull=%d != nonzeroAmp=%d (firstOctave=%d n=%d)\n",
                non_null, nonzero_amp, first_octave, n);
    }
}

const lysh_perlin *lysh_octave_legacy_get(const lysh_octave_legacy *o, int i) {
    int idx = o->n - 1 - i;
    if (idx < 0 || idx >= o->n) return NULL;
    if (!o->present[idx]) return NULL;
    return &o->oct[idx];
}

/* ---------------- 求值 ---------------- */
static double perlin_sample_at(const lysh_perlin *p, int i, int j, int k,
                               double x, double y, double z, double fade_y) {
    int a = get_gradient(p, i);
    int b = get_gradient(p, i + 1);
    int c = get_gradient(p, a + j);
    int d = get_gradient(p, a + j + 1);
    int e = get_gradient(p, b + j);
    int f = get_gradient(p, b + j + 1);

    double v000 = grad(get_gradient(p, c + k),     x,     y,     z);
    double v100 = grad(get_gradient(p, e + k),     x - 1, y,     z);
    double v010 = grad(get_gradient(p, d + k),     x,     y - 1, z);
    double v110 = grad(get_gradient(p, f + k),     x - 1, y - 1, z);
    double v001 = grad(get_gradient(p, c + k + 1), x,     y,     z - 1);
    double v101 = grad(get_gradient(p, e + k + 1), x - 1, y,     z - 1);
    double v011 = grad(get_gradient(p, d + k + 1), x,     y - 1, z - 1);
    double v111 = grad(get_gradient(p, f + k + 1), x - 1, y - 1, z - 1);

    return lerp3(perlin_fade(x), perlin_fade(fade_y), perlin_fade(z),
                 v000, v100, v010, v110, v001, v101, v011, v111);
}

double lysh_perlin_sample(const lysh_perlin *p, double x, double y, double z,
                          double y_scale, double y_max) {
    double d = x + p->ox;
    double e = y + p->oy;
    double f = z + p->oz;
    int i = (int)floor(d), j = (int)floor(e), k = (int)floor(f);
    double g = d - i, h = e - j, l = f - k;
    double m;
    if (y_scale != 0.0) {
        double nn = (y_max >= 0.0 && y_max < h) ? y_max : h;
        m = (double)floor(nn / y_scale + 1.0000000116860974E-7) * y_scale;
    } else {
        m = 0.0;
    }
    return perlin_sample_at(p, i, j, k, g, h - m, l, h);
}

double lysh_octave_sample(const lysh_octave *o, double x, double y, double z) {
    double value = 0.0;
    double lacunarity = o->lacunarity;
    double persistence = o->persistence;
    for (int i = 0; i < o->n; i++) {
        const lysh_perlin *p = &o->oct[i];
        if (p->present) {
            double v = lysh_perlin_sample(p,
                    maintain_precision(x * lacunarity),
                    maintain_precision(y * lacunarity),
                    maintain_precision(z * lacunarity),
                    0.0 * lacunarity, 0.0 * lacunarity);
            value += (o->amp[i] * v) * persistence;
        }
        lacunarity *= 2.0;
        persistence /= 2.0;
    }
    return value;
}

#define LYSH_DBL_SCALE 1.0181268882175227

double lysh_dblnoise_sample(const lysh_dblnoise *dn, double x, double y, double z) {
    return (lysh_octave_sample(&dn->sub[0], x, y, z)
          + lysh_octave_sample(&dn->sub[1], x * LYSH_DBL_SCALE, y * LYSH_DBL_SCALE, z * LYSH_DBL_SCALE))
          * dn->amplitude;
}

/* ---- y=0 特化 ---- */
static inline double perlin_sample_y0(const lysh_perlin *p, double x, double z) {
    double d = x + p->ox;
    double f = z + p->oz;
    int i = (int)floor(d), k = (int)floor(f);
    double g = d - i, l = f - k;
    int j = p->jy;
    double h = p->hy, h1 = p->hy1;

    int a = get_gradient(p, i);
    int b = get_gradient(p, i + 1);
    int c = get_gradient(p, a + j);
    int dd = get_gradient(p, a + j + 1);
    int e2 = get_gradient(p, b + j);
    int ff = get_gradient(p, b + j + 1);

    double v000 = grad(get_gradient(p, c + k),      g,     h,  l);
    double v100 = grad(get_gradient(p, e2 + k),     g - 1, h,  l);
    double v010 = grad(get_gradient(p, dd + k),     g,     h1, l);
    double v110 = grad(get_gradient(p, ff + k),     g - 1, h1, l);
    double v001 = grad(get_gradient(p, c + k + 1),  g,     h,  l - 1);
    double v101 = grad(get_gradient(p, e2 + k + 1), g - 1, h,  l - 1);
    double v011 = grad(get_gradient(p, dd + k + 1), g,     h1, l - 1);
    double v111 = grad(get_gradient(p, ff + k + 1), g - 1, h1, l - 1);

    return lerp3(perlin_fade(g), p->fade_y, perlin_fade(l),
                 v000, v100, v010, v110, v001, v101, v011, v111);
}

static double octave_sample_y0(const lysh_octave *o, double x, double z) {
    double value = 0.0;
    double lacunarity = o->lacunarity;
    double persistence = o->persistence;
    for (int i = 0; i < o->n; i++) {
        const lysh_perlin *p = &o->oct[i];
        if (p->present) {
            double v = perlin_sample_y0(p,
                    maintain_precision(x * lacunarity),
                    maintain_precision(z * lacunarity));
            value += (o->amp[i] * v) * persistence;
        }
        lacunarity *= 2.0;
        persistence /= 2.0;
    }
    return value;
}

double lysh_dblnoise_sample_y0(const lysh_dblnoise *dn, double x, double z) {
    return (octave_sample_y0(&dn->sub[0], x, z)
          + octave_sample_y0(&dn->sub[1], x * LYSH_DBL_SCALE, z * LYSH_DBL_SCALE))
          * dn->amplitude;
}

/* ===========================================================================
 * 分层提前退出（tiered early exit）
 *
 * 详见 noise.h 的说明。这里的关键点：
 *   1) 求值顺序按【贡献】交错：0A,0B,1A,1B,... （A = sub[0]，B = sub[1]）。
 *      两个 sub-sampler 的 amplitudes 数组相同，所以非空槽位一一对应，
 *      可以安全地按 (level, A/B) 交错。
 *   2) 每个 octave 的坐标缩放（lacunarity）与权重（amp[i] * persistence）
 *      全部在 init 时预计算 —— 注意 lacunarity/persistence 是**每个槽位**
 *      都翻倍的，**包括 amplitude 为 0 的空槽位**（erosion 的槽位 2）。
 *      所以这里按 slot 迭代（不是按"非空"迭代），空槽位只更新两个乘数。
 *   3) 累计和乘 dn->amplitude 之后才与阈值比 —— 与最终值同单位。
 *      最后一档阈值 == 原始门的阈值，所以"全部档都不触发" ⟺ 原始判定通过。
 * ========================================================================= */

/* 每档在「算完第 k 个求值」之后检查（k 从 0 起；最后一档 k=7 = 全部算完） */
/* tier 名称表：**只有 n_tier（= 7）项**，与 LYSH_EROSION_TIER_SPEC.n_tier 对齐。
 * 多留的 "(unused)" 第 8 项已删除 —— nth_tier 是 7，index 7 永远不会被读。 */
static const char *const LYSH_EROSION_TIER_NAMES[7] = {
    "after 0A+0B      (< 0.05)",
    "after 0A+0B+1A   (< 0.22)",
    "after ...+1B     (< 0.42)",
    "after ...+2A     (< 0.45)",
    "after ...+2B     (< 0.49)",
    "after ...+3A     (< 0.51)",
    "after all 8      (< 0.55)"
};

static const lysh_tier_spec LYSH_EROSION_TIER_SPEC = {
    7,                                  /* n_tier：7 个真正的提前退出档 */
    { 0.05, 0.22, 0.42, 0.45, 0.49, 0.51, 0.55 },
    { 1, 2, 3, 4, 5, 6, 7 },
    LYSH_EROSION_TIER_NAMES
};

const lysh_tier_spec *lysh_tier_spec_erosion(void) { return &LYSH_EROSION_TIER_SPEC; }

const char *lysh_tier_name(const lysh_tier_spec *spec, int tier) {
    if (!spec || tier < 0 || tier >= spec->n_tier) return "?";
    if (spec->names && spec->names[tier]) return spec->names[tier];
    return "?";
}

int lysh_tier_seq_init(lysh_tier_seq *seq, const lysh_dblnoise *dn) {
    if (!seq || !dn) return 1;
    const lysh_octave *a = &dn->sub[0];
    const lysh_octave *b = &dn->sub[1];

    if (a->n != b->n || a->n > LYSH_MAX_OCT) return 2;

    /* 数一下非空槽位：必须两边一致，否则无法按 (level, A/B) 交错 */
    int used = 0;
    for (int i = 0; i < a->n; i++) {
        if (a->oct[i].present != b->oct[i].present) return 3;
        if (a->oct[i].present) used++;
    }
    if (used < 1 || 2 * used > LYSH_TIER_MAX) return 4;

    seq->n = 0;
    double lac_a = a->lacunarity, per_a = a->persistence;
    double lac_b = b->lacunarity, per_b = b->persistence;
    for (int i = 0; i < a->n; i++) {
        /* ⚠️ 空槽位也必须推进 lacunarity / persistence（与原实现逐字一致） */
        if (a->oct[i].present) {
            double w_a = a->amp[i] * per_a;
            double w_b = b->amp[i] * per_b;
            if (w_a == 0.0 || w_b == 0.0) return 5;

            int k = seq->n;
            seq->sub_scale[k] = 1.0;
            seq->x_scale[k]   = lac_a;
            seq->p[k]         = &a->oct[i];
            seq->weight[k]    = w_a;

            k = seq->n + 1;
            seq->sub_scale[k] = LYSH_DBL_SCALE;
            seq->x_scale[k]   = lac_b;
            seq->p[k]         = &b->oct[i];
            seq->weight[k]    = w_b;

            seq->n += 2;
        }
        lac_a *= 2.0; per_a /= 2.0;
        lac_b *= 2.0; per_b /= 2.0;
    }
    return 0;
}

int lysh_dblnoise_tiered_y0(const lysh_dblnoise *dn, const lysh_tier_seq *seq,
                            const lysh_tier_spec *spec,
                            double x, double z,
                            double *out, int *tier_hit) {
    if (tier_hit) *tier_hit = -1;
    double sum = 0.0;
    const int n_tier = spec->n_tier;
    int next = 0;                       /* 下一个待检查的档 */

    for (int k = 0; k < seq->n; k++) {
        const double sc = seq->sub_scale[k];
        const double v = perlin_sample_y0(seq->p[k],
                maintain_precision(x * sc * seq->x_scale[k]),
                maintain_precision(z * sc * seq->x_scale[k]));
        sum += seq->weight[k] * v;

        if (k == spec->k_of_tier[next]) {
            if (sum * dn->amplitude < spec->threshold[next]) {
                if (tier_hit) *tier_hit = next;
                if (out) *out = sum * dn->amplitude;
                return 1;               /* 提前淘汰：剩下的 Perlin 全部省掉 */
            }
            if (++next >= n_tier) break; /* 最后一档也没触发 -> 直接收尾 */
        }
    }
    if (out) *out = sum * dn->amplitude;
    return 0;
}
