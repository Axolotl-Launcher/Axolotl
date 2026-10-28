/* ⛔ 版本归属（见 README §0）：本文件实现的是 **1.18 的地形语义**，默认无效。
 * 只有显式选择版本 `1.18.2` 时才作为该分支的实现保留。
 * 1.21+ 的权威是客户端 jar 的 worldgen JSON（`overworld/continents.json` /
 * `erosion.json` / `ridges.json` / `ridges_folded.json` / `offset.json` 等）。
 */

/* lysh terrain.h —— (biome x, biome z) → TerrainNoisePoint。
 *
 * 取证：MC 1.18.1 `NoiseColumnSampler.method_39330(int,int,Blender)`
 *       -> _archive/dev_scratch_20260921.zip -> MC_NoiseColumnSampler.txt:1088-1149
 *
 * 还原后的语义（无混合世界）：
 *
 *   double shiftedX = biomeX + sampleShiftNoise(biomeX, 0, biomeZ);
 *   double shiftedZ = biomeZ + sampleShiftNoise(biomeZ, biomeX, 0);
 *   double continentalness = sampleContinentalnessNoise(shiftedX, 0, shiftedZ);
 *   double weirdness       = sampleWeirdnessNoise(shiftedX, 0, shiftedZ);
 *   double erosion         = sampleErosionNoise(shiftedX, 0, shiftedZ);
 *   TerrainNoisePoint t = createTerrainNoisePoint(x, z, (float)cont, (float)weird, (float)ero, blender);
 *
 * 六个必须照抄的细节：
 *  1. 入参是 **biome 坐标**（= block >> 2），不是方块坐标。
 *  2. `sampleShiftNoise(a,b,c)` = `shiftNoise.sample(a,b,c) * 4.0`（乘 4 在采样之后）。
 *  3. 第二个 shift 的实参顺序是 **(biomeZ, biomeX, 0)**，不是 (biomeZ, 0, biomeX)。
 *  4. 气候噪声 **不加任何缩放**，直接 `noise.sample(shiftedX, 0, shiftedZ)`。
 *     （`SearchCoords` 里那个 `climateX / 4.0` 是调用方自己做的，等价。）
 *  5. 三个 float 传给 createNoisePoint 的顺序是 (continentalness, **erosion**, **weirdness**)；
 *     而调用处的局部变量顺序是 (cont, weird, ero) —— 中间有一次换位。写反了只会
 *     在 erosion≈weirdness 的格子上"看起来对"。
 *  6. 噪声名：MC 的 `shiftNoise` 在数据里叫 **minecraft:offset**，
 *     `weirdnessNoise` 叫 **minecraft:ridge**。别按 Java 字段名去查表。
 *
 * ⚠️ noise-sampler 库的同名方法在这一点上是**错的**：它的 `createNoiseInfo`
 * 把 (cont, weird, ero) 直接当 (cont, erosion, weirdness) 传下去，erosion 与 weirdness 互换。
 * 所以本层以 **MC 的字节码**为准，对拍基线也取自 MC 的 `NoiseColumnSampler`。
 */
#ifndef LYSH_TERRAIN_H
#define LYSH_TERRAIN_H

#include <stdint.h>

#include "noise.h"
#include "spline.h"

typedef struct {
    lysh_dblnoise shift;            /* minecraft:offset  —— MC 里的 shiftNoise */
    lysh_dblnoise continentalness;  /* minecraft:continentalness[_large] */
    lysh_dblnoise erosion;          /* minecraft:erosion[_large]         */
    lysh_dblnoise weirdness;        /* minecraft:ridge                   */
    int semantics_261;              /* 1 = 26.1.2 的 ridges_folded（double 求值） */
} lysh_terrain;

typedef struct {
    double shifted_x, shifted_z;
    double continentalness, weirdness, erosion;
    lysh_terrain_point point;       /* offset / factor / peaks（float） */
} lysh_terrain_info;

void lysh_terrain_init(lysh_terrain *t, uint64_t world_seed, int large_biomes);

/* 切到 26.1.2 的 `overworld/ridges_folded` 语义（double 求值，最后才 float）。
 * 1.18.2 分支保持默认（全程 float）——两版在密度上差 ~1e-6，都会各自的对拍基线。 */
void lysh_terrain_set_semantics_261(lysh_terrain *t, int on);

/* biome_x = block_x >> 2（算术右移 = floorDiv） */
void lysh_terrain_info_at(const lysh_terrain *t, int biome_x, int biome_z, lysh_terrain_info *out);

#endif /* LYSH_TERRAIN_H */
