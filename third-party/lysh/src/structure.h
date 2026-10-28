/* lysh structure.h —— 女巫小屋的潜在生成位置（不考虑生物群系）。
 *
 * 用户确认：这就是几个简单的随机数计算，直接实现即可。
 *
 * 参数（来自 mc_feature 的字节码）：
 *   OldStructure.Config.SPACING    = 32
 *   OldStructure.Config.SEPARATION = 8
 *   SwampHut.CONFIGS: v1_8 -> salt 14357617, v1_13+ -> salt 14357620
 *   → 1.18.2 及以后用 14357620
 *
 * 算法（ChunkRand 是遗留 java.util.Random，不是 Xoroshiro）：
 *   s = worldSeed + regionX*341873128712 + regionZ*132897987541 + salt
 *   setSeed(s)                       // LCG
 *   j = nextInt(spacing - separation)  // = nextInt(24)
 *   k = nextInt(spacing - separation)
 *   chunk = (regionX*spacing + j, regionZ*spacing + k)
 */
#ifndef LYSH_STRUCTURE_H
#define LYSH_STRUCTURE_H

#include <stdint.h>

#include "rng.h"

#define LYSH_SWAMP_HUT_SPACING    32
#define LYSH_SWAMP_HUT_SEPARATION  8
#define LYSH_SWAMP_HUT_SALT       14357620   /* v1_13+ */

typedef struct { int chunkX, chunkZ; } lysh_chunkpos;

/* setRegionSeed 的返回值（即 & MASK_48 之后的 LCG 种子），便于对拍 */
uint64_t lysh_set_region_seed(lysh_lcg *r, uint64_t worldSeed,
                              int regionX, int regionZ, int salt);

/* 不考虑生物群系时，区域 (regionX, regionZ) 内女巫小屋所在的 chunk */
lysh_chunkpos lysh_swamp_hut_in_region(uint64_t worldSeed, int regionX, int regionZ, int salt);

/* setCarverSeed(seed, chunkX, chunkZ) —— 小屋朝向判定与高度图都要用 */
uint64_t lysh_set_carver_seed(lysh_lcg *r, uint64_t worldSeed, int chunkX, int chunkZ);

#endif /* LYSH_STRUCTURE_H */
