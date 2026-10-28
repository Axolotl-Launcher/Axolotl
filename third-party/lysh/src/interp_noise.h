/* ⛔ 版本归属（见 README §0）：本文件实现的是 **1.18 的地形语义**，默认无效。
 *
 * 只有显式选择版本 `1.18.2` 时才作为该分支的实现保留。
 * 1.21+ 的权威是客户端 jar 的 worldgen JSON，**不是**本文件所依据的 1.18.1 字节码。
 *
 * ⚠️ 实测 1.21.1 的 `base_3d_noise` 参数与本文件的 1.18.1 值不同：
 *      type: minecraft:old_blended_noise；xz_scale 0.25（非 1.0）；y_scale 0.125（非 1.0）；
 *      另有 smear_scale_multiplier = 8.0（1.18 无此字段）。
 *    构造接口 `lysh_interp_noise_init()` 的参数是传入的，可覆盖两版；
 *    但**两版是否数学等价尚未验证**。
 */

/* lysh interp_noise.h —— `base_3d_noise` = MC 的 InterpolatedNoiseSampler。
 *
 * 取证（全部来自 MC 1.18.1 字节码，不是 noise-sampler 库）：
 *   · 构造调用方：`_archive/dev_scratch_20260921.zip -> MC_NoiseColumnSampler.txt:206-243`
 *       terrainNoise = new InterpolatedNoiseSampler(
 *               randomDeriver.createRandom(new Identifier("terrain")),   // ← 字面量就是 "terrain"
 *               shape.sampling(), shape.horizontalBlockSize(), shape.verticalBlockSize());
 *   · 构造：(MC_InterpolatedNoiseSampler.txt:72-88) **同一个 random 顺序消费**
 *       lower = OctavePerlinNoiseSampler.createLegacy(random, rangeClosed(-15, 0))
 *       upper = OctavePerlinNoiseSampler.createLegacy(random, rangeClosed(-15, 0))
 *       interp= OctavePerlinNoiseSampler.createLegacy(random, rangeClosed(-7,  0))
 *   · 缩放：(MC_InterpolatedNoiseSampler.txt:19-70)
 *       xzScale     = 684.412 * sampling.getXZScale()
 *       yScale      = 684.412 * sampling.getYScale()
 *       xzMainScale = xzScale / sampling.getXZFactor()
 *       yMainScale  = yScale  / sampling.getYFactor()
 *     ⚠️ noise-sampler 1.20.0 把 factor 硬编码成 80.0/160.0；
 *        MC 1.18.1 是**读 factor**。vanilla overworld 两者数值相同（xz=1, y=1, f=80/160）
 *        —— 实测 1.18.1 运行时 dump：xz_scale=y_scale=1.0、xz_factor=80、y_factor=160。
 *        （注意这与 1.21.1 的 `noise_settings/overworld.json` 不一样：那份的
 *         `final_density` 里 sampling 是 xz_scale=1.0 / y_scale=0.5。别跨版本抄。）
 *   · 求值：`calculateNoise(int,int,int)`（MC_InterpolatedNoiseSampler.txt:90-305，
 *     规格 §17.7 亦已逐条对齐）。
 *
 * 三个易错点（照抄，不要"化简"）：
 *   1. 第 ① 轮循环用 xzMainScale/yMainScale，第 ② 轮用**原始** xzScale/yScale。
 *   2. `sample` 的第 5 个参数（yMax）是 `j * (yScale*m)` —— 与第 4 个参数同源同乘 m。
 *   3. skipLower/skipUpper 是**跳过**：t>=1 跳过 lower、t<=0 跳过 upper。
 */
#ifndef LYSH_INTERP_NOISE_H
#define LYSH_INTERP_NOISE_H

#include <stdint.h>

#include "noise.h"

typedef struct {
    /* 注意构造顺序：lower / upper 先于 interp —— 三者共用同一个 random 顺序消费 */
    lysh_octave_legacy lower;
    lysh_octave_legacy upper;
    lysh_octave_legacy interp;

    /* --- 1.18.2 的 InterpolatedNoiseSampler：main scale 是**预先算好存下来**的 --- */
    double xz_scale, y_scale, xz_main_scale, y_main_scale;

    /* --- 26.1.2 的 BlendedNoise：只存 multiplier/factor，求值时才做除法；
     *     并且 y 的那两个比例参数要乘 smear_scale_multiplier --- */
    double xz_multiplier, y_multiplier, xz_factor, y_factor, smear_scale_multiplier;

    int cell_width, cell_height;
    int inited;
} lysh_interp_noise;

/* overworld 的 sampling（1.18.1 运行时实测）：
 *   xz_scale = 1.0, y_scale = 1.0, xz_factor = 80.0, y_factor = 160.0
 * cell_width/height 由 GenerationShapeConfig.horizontalBlockSize()/verticalBlockSize() 给出。 */
void lysh_interp_noise_init(lysh_interp_noise *s, uint64_t world_seed,
                            double sampling_xz_scale, double sampling_y_scale,
                            double sampling_xz_factor, double sampling_y_factor,
                            int cell_width, int cell_height);

/* 26.1.2 的 `minecraft:old_blended_noise`（= 类 `synth.BlendedNoise`）：
 * `noise_settings/overworld.json` 的 `overworld/base_3d_noise` 参数是
 *   xz_scale 0.25, y_scale 0.125, xz_factor 80.0, y_factor 160.0, smear_scale_multiplier 8.0
 * 构造顺序/随机消费与 1.18 完全一样，只有**求值代数**不同（见 .c 里的注释）。 */
void lysh_interp_noise_init_261(lysh_interp_noise *s, uint64_t world_seed,
                                double sampling_xz_scale, double sampling_y_scale,
                                double sampling_xz_factor, double sampling_y_factor,
                                double smear_scale_multiplier,
                                int cell_width, int cell_height);

/* MC 26.1.2 `BlendedNoise.compute(ctx)` —— x/y/z 是**方块坐标**（不按 cell 量化）。
 * 与 1.18 的三点差别：
 *   1. 不做 floorDiv(x, cellWidth)（26.1.2 的 cell 量化在显式 `interpolated` 节点上）；
 *   2. `(x * xzMultiplier) / xzFactor` 在**求值时**才除（1.18 是预先算好 main scale，
 *      多一次舍入 —— 在 maintainPrecision 之后会被放大到 ~1e-8，必须按 26.1.2 写）；
 *   3. y 的比例参数乘 smear_scale_multiplier（yMax 那一路不乘）。 */
double lysh_interp_calculate_noise_261(const lysh_interp_noise *s, int x, int y, int z);

#endif /* LYSH_INTERP_NOISE_H */
