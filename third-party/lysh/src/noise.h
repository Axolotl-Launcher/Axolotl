/* ⛔ 版本归属（见 README §0）：本文件是 **1.18 地形栈的基础层**，默认无效。
 * 只有显式选择版本 `1.18.2` 时才作为该分支的实现保留。
 *
 * 注意：**噪声参数表（`noise_table.h`）本身是版本无关的**
 * （1.21.1 与 26.1.2 的 noise 目录 60 个文件逐字节相同），
 * 无效的是**怎么用这些噪声去算地形**（派生顺序、octave 语义、采样缩放）。
 */

/* lysh noise.h —— 与 MC 1.18 逐位一致的 Perlin 噪声栈。
 *
 * 语义来源：noise-sampler 1.20.0 的字节码（该类已由 L1 测试证明与 MC 逐位一致）。
 *
 * 派生链（关键结论，已由字节码确认）：
 *   r       = Xoroshiro128PlusPlusRandom(worldSeed)
 *   deriver = r.createRandomDeriver()                       // 消费 r 的 2 次 nextLong
 *   perNoise = deriver.createRandom("<identifier>")         // 只读不消费，与顺序无关
 *   first   = OctavePerlin(perNoise, firstOctave, amps)     // 顺序消费 perNoise
 *   second  = OctavePerlin(perNoise, firstOctave, amps)     // 接着消费
 *
 * 非 legacy 的 octave 构造（MC 的真实怪癖，必须照抄）：
 *   1. 先无条件 new 一个 PerlinNoiseSampler（消耗 RNG，可能被丢弃不用）
 *   2. i0 = -firstOctave
 *      若 0 <= i0 < n 且 amps[i0] != 0 → octaveSamplers[i0] = 刚建的那个
 *   3. j 从 i0-1 递减到 0：
 *        若 j < n 且 amps[j] != 0 → octaveSamplers[j] = new Perlin(random)
 *        否则                    → random.consumeCount(262)     ← 262 是 MC 的魔数
 *      若 j >= n                  → random.consumeCount(262)
 */
#ifndef LYSH_NOISE_H
#define LYSH_NOISE_H

#include "rng.h"

#define LYSH_MAX_OCT 16

typedef struct {
    double ox, oy, oz;
    unsigned char perm[256];
    int present;
    /* y=0 特化用的预计算常量（阶段 1 的气候噪声恒在 y=0 采样） */
    int jy;
    double hy, hy1, fade_y;
} lysh_perlin;

typedef struct {
    double lacunarity, persistence;
    int n;                       /* == amplitudes 的长度 */
    lysh_perlin oct[LYSH_MAX_OCT];
    double amp[LYSH_MAX_OCT];
} lysh_octave;

typedef struct {
    double amplitude;
    lysh_octave sub[2];
} lysh_dblnoise;

/* ---- 构造 ---- */
void lysh_perlin_create(lysh_perlin *p, xoroshiro_t *rng);
void lysh_octave_create(lysh_octave *o, xoroshiro_t *rng,
                        int first_octave, const double *amps, int n);
void lysh_dblnoise_create(lysh_dblnoise *dn, xoroshiro_t *rng,
                          int first_octave, const double *amps, int n);

/* 一步到位：由 worldSeed 推出某个 identifier 的噪声（与创建顺序无关） */
void lysh_dblnoise_from_seed(lysh_dblnoise *dn, uint64_t world_seed,
                             const char *identifier,
                             int first_octave, const double *amps, int n);

/* 同上，但参数从内建表 noise_table.h 按完整 identifier 查（如 "minecraft:erosion"）。
 * 表里没有该 identifier 时清零并在 stderr 报警 —— 静默失败是这套代码最大的风险。 */
void lysh_dblnoise_from_seed_id(lysh_dblnoise *dn, uint64_t world_seed,
                                const char *identifier);

/* ---- 求值 ---- */
double lysh_perlin_sample(const lysh_perlin *p, double x, double y, double z,
                          double y_scale, double y_max);
double lysh_octave_sample(const lysh_octave *o, double x, double y, double z);
double lysh_dblnoise_sample(const lysh_dblnoise *dn, double x, double y, double z);

/* y=0 特化（阶段 1 用），结果与上面完全一致 */
double lysh_dblnoise_sample_y0(const lysh_dblnoise *dn, double x, double z);

/* ===========================================================================
 * 分层提前退出（tiered early exit）—— 只用于「值 >= 阈值才继续」这类门。
 *
 * 背景：阶段 1 的第一个气候门是 `erosion < 0.55`，实测淘汰 95% 的格子，
 * 而原来要为此付满 8 次 Perlin 求值（2 个 sub-sampler × 4 个非空 octave）。
 *
 * 思路：把 8 次求值按**贡献从大到小交错**（0A,0B,1A,1B,2A,2B,3A,3B），
 * 每算完一次就把「当前累计归一化值」与那一档的阈值比一次；一旦低于阈值
 * 立刻返回，剩下的 Perlin 求值全部省掉。最后一档阈值就是原始门的阈值，
 * 所以「所有档都没触发」与原始判定【完全等价】。
 *
 * ⚠️ 这是**近似**：中间档用的是部分和，某些格子会在部分和还很低、但最终值
 *    其实 >= 阈值时被提前淘汰（假阴性）。假阴性率由 _archive/verify_tools_20260921.zip -> erosion_tier_test.c
 *    实测（阈值由用户指定，见 README §7）。
 *
 * 「归一化」= 累计和已经乘过 dn->amplitude —— 必须与最终值单位一致，
 * 否则最后一档的 0.55 无法复现原始门。
 * ========================================================================= */
#define LYSH_TIER_MAX 16

typedef struct {
    int n_tier;                         /* 档数（erosion = 8） */
    double threshold[LYSH_TIER_MAX];    /* 每档下界：累计和 < threshold[i] 即淘汰 */
    int    k_of_tier[LYSH_TIER_MAX];    /* 第 i 档是「算完第 k 个求值」之后（k 从 0 起） */
    const char *const *names;           /* 档名，仅用于报告 */
} lysh_tier_spec;

/* 一次「按贡献排序」的交错求值序列：坐标缩放与权重都从这里取 */
typedef struct {
    int n;                                    /* 求值次数（erosion = 8） */
    double sub_scale[LYSH_TIER_MAX];          /* 1.0（sub[0]）或 LYSH_DBL_SCALE（sub[1]） */
    double x_scale[LYSH_TIER_MAX];            /* 坐标乘数：maintain_precision((x*sub_scale) * 这个值) */
    const lysh_perlin *p[LYSH_TIER_MAX];      /* 该次求值用的 perm 表 */
    double weight[LYSH_TIER_MAX];             /* amp[slot] * persistence */
} lysh_tier_seq;

/* 从 dn 推出交错求值序列（A/B 两个 sub-sampler 的非空槽位必须一一对应）。
 * 阶数超出 LYSH_TIER_MAX 或表结构不符合假设时返回非 0（不静默降级）。 */
int lysh_tier_seq_init(lysh_tier_seq *seq, const lysh_dblnoise *dn);

/* ---- y = 0 特化版（阶段 1 的气候噪声恒在 y=0）----
 * 求值入口。out 可以是 NULL（只关心会不会被提前淘汰）。
 *   返回 0 : 没有任何档触发 —— *out = 交错顺序累加出来的最终值（>= 最后一档阈值）
 *   返回 1 : 某一档触发   —— *out 无意义，*tier_hit = 触发的档号（0..n_tier-1）
 *
 * ⚠️ 返回 0 时的值是从【交错顺序】累加的；原路径是 `(0A+1A+2A+3A) + (0B+...)`。
 *    两者只差**结合顺序**（浮点加法可交换，结合律不成立），差 <= 1 ulp，
 *    远小于用户接受的 1e-4 误差预算。这里**不**重复累加第二遍。
 *
 * 语义等价于 `val = lysh_dblnoise_sample_y0(dn, x, z); return val < final_threshold;`
 * 但可能在算满之前就返回 1。
 *
 * ⚠️ 一旦最后一档也判过「>= 0.55」，函数会在那里直接收尾（不再算剩下的求值）——
 *    因为原始门的判据就是 `最终值 < 0.55`，而那一步已经得到了同样的结论。 */
int lysh_dblnoise_tiered_y0(const lysh_dblnoise *dn, const lysh_tier_seq *seq,
                            const lysh_tier_spec *spec,
                            double x, double z,
                            double *out, int *tier_hit);

/* 内建的 erosion 档设定（阈值来自用户规格，逐字实现，禁止改数） */
const lysh_tier_spec *lysh_tier_spec_erosion(void);

/* 供工具使用：给自定义 spec 取档名（NULL 则返回 "tierN" 之类） */
const char *lysh_tier_name(const lysh_tier_spec *spec, int tier);

/* ---------------------------------------------------------------------------
 * legacy octave（InterpolatedNoiseSampler 用的那条路径）
 *
 * ⚠️ 与上面的 lysh_octave（"按名字派生"）是**两条完全不同的路径**，不能混用：
 *   - lysh_octave 走 DoublePerlinNoiseSampler 链（OctavePerlinNoiseSampler.create，
 *     legacy=TRUE）→ 每个 octave 用 deriver.createRandom("octave_<idx>") 独立派生。
 *   - lysh_octave_legacy 走 OctavePerlinNoiseSampler.createLegacy →
 *     **同一个 random 顺序消费**，且零 amplitude 的档位要 consumeCount(262)。
 *
 * 字节码：_archive/dev_scratch_20260921.zip -> NS_OctavePerlin.txt:115-295（构造）与 :304-391（sample）
 *         _archive/dev_scratch_20260921.zip -> MC_InterpolatedNoiseSampler.txt（MC 1.18.1 的调用方）
 *
 * 构造语义（照抄字节码，顺序不能改）：
 *   n = amplitudes.size();  i0 = -firstOctave
 *   ① 无条件先 new 一个 PerlinNoiseSampler（消耗 RNG，可能被丢弃不用）
 *   ② 若 0 <= i0 < n 且 amps[i0] != 0 → octaveSamplers[i0] = ①
 *   ③ j 从 i0-1 递减到 0：
 *        若 j < n 且 amps[j] != 0 → octaveSamplers[j] = new Perlin(random)
 *        否则                    → random.consumeCount(262)
 *  ④ persistence = pow(2, n-1) / (pow(2, n) - 1)      ← 注意它存的是**除数**，
 *     sample 里每档再 /2（实现见 src/interp_noise.c 的逐 octave 展开）
 *  ⑤ lacunarity   = pow(2, -i0)
 *  ⑥ 断言 nonNull 个数 == 非零 amplitude 个数
 *
 * sample 语义：
 *   value = 0; lacunarity = 2^-i0; persistence = pow(2,n-1)/(pow(2,n)-1);
 *   for i in 0..n-1:
 *       p = getOctave(i)                      // = samplers[n-1-i]
 *       if p != NULL:
 *           value += amps[i] * p.sample(maintainPrecision(x*lac), maintainPrecision(y*lac),
 *                                       maintainPrecision(z*lac), y_scale*lac, y_max*lac)
 *                    * persistence;
 *       lacunarity *= 2;  persistence /= 2;
 * ------------------------------------------------------------------------- */
typedef struct {
    double lacunarity, persistence;
    int n;
    int first_octave;
    lysh_perlin oct[LYSH_MAX_OCT];
    double amp[LYSH_MAX_OCT];
    int present[LYSH_MAX_OCT];  /* 与 oct[i].present 冗余，便于断言 */
} lysh_octave_legacy;

/* ---- 构造 ---- */
void lysh_octave_legacy_create(lysh_octave_legacy *o, xoroshiro_t *rng,
                               int first_octave, const double *amps, int n);

/* ---- 求值 ----
 * 上面那段 `sample 语义` 的**唯一实现**在 `src/interp_noise.c`
 * （它逐 octave 调 lysh_octave_legacy_get + lysh_perlin_sample 内联展开，
 * 见 lysh_interp_calculate_noise_261 与相关 helper）——这里不再提供打包版求值函数。
 * y_scale / y_max 就是 MC 的 PerlinNoiseSampler.sample 的第 4/5 个参数
 * （0,0 表示"不做 y 方向折叠"）。 */

/* getOctave(i) = oct[n-1-i]（MC 的真实下标方向）；不存在时返回 NULL */
const lysh_perlin *lysh_octave_legacy_get(const lysh_octave_legacy *o, int i);

/* MC OctavePerlinNoiseSampler.maintainPrecision —— 只此一份实现，
 * InterpolatedNoiseSampler 也必须用它（不要在别处重写，避免两条路径漂移）。 */
double lysh_maintain_precision(double v);

/* 供测试：MC 的 16 项梯度表 */
extern const int LYSH_GRADIENTS[16][3];

#endif /* LYSH_NOISE_H */
