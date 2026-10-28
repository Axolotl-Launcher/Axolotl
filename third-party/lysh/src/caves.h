/* lysh caves.h —— 阶段 1 洞穴梯子：Entrance / Entrance2 / Cheese。
 *
 * 目标不是「复现 vanilla 的噪声洞穴」，而是【逐位复现 SearchCoords.check() 里那三个
 * 启发式函数】—— 阶段 1 的判定权威就是现有 Java 程序，C 端只要和它一个 bit 都不差，
 * 召回率就不会变。
 *
 * 语义来源（全部从字节码取，未靠记忆）：
 *   - NoiseColumnSampler$CaveScaler.scaleTunnels(D)D      -> _archive/dev_scratch_20260921.zip -> MC_CaveScaler.txt
 *   - NoiseHelper.lerpFromProgress(DoublePerlinNoiseSampler,DDDDD)D
 *                                                          -> _archive/dev_scratch_20260921.zip -> MC_NoiseHelper.txt
 *   - MathHelper.clamp / clampedLerp / getLerpProgress      -> javap MathHelper
 *
 * 三条容易写错的：
 *   1. scaleTunnels 是【阶梯函数】，不是插值：< -0.5 -> 0.75，< 0 -> 1.0，< 0.5 -> 1.5，否则 2.0。
 *      NaN 走最后一条（字节码 dcmpg + ifge，NaN 被视为「不小于」）。
 *   2. lerpFromProgress(sampler,x,y,z,a,b) 先 sample(x,y,z)，再把它从 [-1,1] 线性映到 [a,b]：
 *        a + ((v + 1.0) / 2.0) * (b - a)
 *      注意是 (v - (-1.0)) / (1.0 - (-1.0))，与 v+1.0 / 2.0 逐位等价。
 *   3. NoiseColumnSampler.sample(n,x,y,z,s) == n.sample(x/s, y/s, z/s)。
 *      当 s 由 scaleTunnels 给出时它是 0.75/1.0/1.5/2.0，全是 2 的幂或精确值，
 *      但【不要】手工改写成乘法 —— 除法和乘法的舍入不同。
 *
 * ⚠️ 整数与浮点的边界：Java 里 `y * 8`、`x * 2` 是【int 乘法】后才拓宽成 double，
 *    `x * 0.75` 才是 double 乘法。C 端一律写成显式 (double)(int_expr) / (double)i * k，
 *    不给编译器留下改写的空间。
 */
#ifndef LYSH_CAVES_H
#define LYSH_CAVES_H

#include "noise.h"

typedef struct {
    lysh_dblnoise cave_entrance;                  /* minecraft:cave_entrance                 */
    lysh_dblnoise spaghetti_3d_rarity;            /* minecraft:spaghetti_3d_rarity           */
    lysh_dblnoise spaghetti_3d_thickness;         /* minecraft:spaghetti_3d_thickness        */
    lysh_dblnoise spaghetti_3d_1;                 /* minecraft:spaghetti_3d_1                */
    lysh_dblnoise spaghetti_3d_2;                 /* minecraft:spaghetti_3d_2                */
    lysh_dblnoise spaghetti_roughness_modulator;  /* minecraft:spaghetti_roughness_modulator */
    lysh_dblnoise spaghetti_roughness;            /* minecraft:spaghetti_roughness           */
    lysh_dblnoise cave_layer;                     /* minecraft:cave_layer                    */
    lysh_dblnoise cave_cheese;                    /* minecraft:cave_cheese                   */
} lysh_caves;

/* 与创建顺序无关：每个噪声各自从 worldSeed 派生（见 noise.h） */
void lysh_caves_init(lysh_caves *c, uint64_t world_seed);

/* MC 的阶梯函数（导出以便单测） */
double lysh_scale_tunnels(double v);
double lysh_scale_caves(double v);

/* 与 SearchCoords.Entrance / Entrance2 / Cheese 逐位一致 */
double lysh_entrance(const lysh_caves *c, int x, int y, int z);
double lysh_entrance2(const lysh_caves *c, int x, int y, int z);
double lysh_cheese(const lysh_caves *c, int x, int y, int z);

#endif /* LYSH_CAVES_H */
