/* ⛔ 版本归属（见 README §0）：本文件服务的是 **1.18 的样条树**，默认无效。
 * 只有显式选择版本 `1.18.2` 时才作为该分支的实现保留。
 *
 * 但**求值器本身是版本无关的**：1.21 的 density_function 目录也用
 * `minecraft:spline` 节点，且实测 1.21.1 与 26.1.2 的样条数据逐字节相同。
 * 变的是**数据来源**（1.18 是 `VanillaTerrainParameters` 字节码构造的嵌套样条，
 * 1.21 是 `overworld/factor.json` 等 JSON）。
 */

/* lysh spline.h —— MC 的嵌套样条求值 + VanillaTerrainParameters。
 *
 * 语义来源（全部来自 noise-sampler 的字节码，未靠记忆）：
 *   Spline$SplineImpl.apply            -> _archive/dev_scratch_20260921.zip -> NS_Spline$SplineImpl.txt
 *   MathHelper.binarySearch            -> javap nl...util.MathHelper
 *   VanillaTerrainParameters.getOffset/getFactor/getPeak/getNormalizedWeirdness
 *                                      -> _archive/dev_scratch_20260921.zip -> NS_VanillaTerrainParameters.txt
 *
 * ⚠️ 全程是 **float（单精度）**，不是 double。MC 的 TerrainShaper 就是 float，
 *    换成 double 会在最后一位上偏离，进而改变 density 的符号。
 *
 * 样条树本身不在这里，而是由 _archive/verify_tools_20260921.zip -> TerrainTable.java 从 Java 运行时 dump 成
 * src/terrain_table.h（115 个节点 / 3 个根）。手抄 950 行样条构造字节码必错。
 */
#ifndef LYSH_SPLINE_H
#define LYSH_SPLINE_H

/* 字段顺序必须与 Java 的 NoisePoint 记录一致：
 *   NoisePoint(float continentalnessNoise, float erosionNoise,
 *              float normalizedWeirdness, float weirdnessNoise)
 * 定位方式是运行期探测（_archive/verify_tools_20260921.zip -> TerrainTable.java 的 buildLocationMap），不是猜名字。 */
typedef struct {
    float continentalness;        /* 0 */
    float erosion;                /* 1 */
    float normalized_weirdness;   /* 2  ← LocationFunction.RIDGES */
    float weirdness;              /* 3  ← LocationFunction.WEIRDNESS */
} lysh_noise_point;

typedef struct {
    float offset;
    float factor;
    float peaks;
} lysh_terrain_point;

/* 求值：root 是 terrain_table.h 里的节点下标 */
float lysh_spline_apply(int root, const lysh_noise_point *p);

/* VanillaTerrainParameters.getNormalizedWeirdness(float) */
float lysh_normalized_weirdness(float w);

/* 26.1.2 的 `overworld/ridges_folded`（JSON）：
 *   mul(-3.0, add(-0.3333333333333333, abs(add(-0.6666666666666666, abs(ridges)))))
 * 三个常数都是 **double**，整个表达式按 double 求值，最后才 (float) 进样条。
 * （1.18.1 的 getNormalizedWeirdness 是**全程 float**，常数是 0.6666667f/0.33333334f
 *   —— 两版的差别可达 1e-8，经 factor*4 放大后是密度上的 ~1e-6。） */
float lysh_normalized_weirdness_261(double weirdness);

/* createNoisePoint + getOffset/getFactor/getPeak */
lysh_terrain_point lysh_terrain_noise_point(float continentalness, float erosion, float weirdness);

/* 26.1.2 版：三个坐标都是 double（气候噪声的原始值），
 * normalized_weirdness 按 double 算，只在样条入口转 float。 */
lysh_terrain_point lysh_terrain_noise_point_261(double continentalness, double erosion,
                                                double weirdness);

/* 供 _archive/verify_tools_20260921.zip -> terrain_test.c 用：直接给出三个原始 float（不做 TerrainPoint 组装） */
float lysh_terrain_offset(float c, float e, float w);
float lysh_terrain_factor(float c, float e, float w);
float lysh_terrain_peak(float c, float e, float w);

#endif /* LYSH_SPLINE_H */
