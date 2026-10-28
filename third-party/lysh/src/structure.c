/* lysh structure.c —— 见 structure.h 顶部的算法与来源说明。 */
#include "structure.h"

uint64_t lysh_set_region_seed(lysh_lcg *r, uint64_t worldSeed,
                              int regionX, int regionZ, int salt) {
    uint64_t s = (uint64_t)((int64_t)regionX * 341873128712LL)
               + (uint64_t)((int64_t)regionZ * 132897987541LL)
               + worldSeed
               + (uint64_t)(int64_t)salt;
    lysh_lcg_set_seed(r, s);
    return s & UINT64_C(0xFFFFFFFFFFFF);   /* Mth.MASK_48 */
}

lysh_chunkpos lysh_swamp_hut_in_region(uint64_t worldSeed, int regionX, int regionZ, int salt) {
    lysh_lcg r;
    lysh_set_region_seed(&r, worldSeed, regionX, regionZ, salt);
    int m = LYSH_SWAMP_HUT_SPACING - LYSH_SWAMP_HUT_SEPARATION;   /* 24 */
    int j = lysh_lcg_next_int_bound(&r, m);
    int k = lysh_lcg_next_int_bound(&r, m);
    lysh_chunkpos out;
    out.chunkX = regionX * LYSH_SWAMP_HUT_SPACING + j;
    out.chunkZ = regionZ * LYSH_SWAMP_HUT_SPACING + k;
    return out;
}

uint64_t lysh_set_carver_seed(lysh_lcg *r, uint64_t worldSeed, int chunkX, int chunkZ) {
    /* mc_feature:
     *   setSeed(worldSeed);
     *   long a = nextLong();
     *   long b = nextLong();
     *   long s = chunkX * a ^ chunkZ * b ^ worldSeed;    // '*' 优先于 '^'
     *   setSeed(s);
     *   return s & MASK_48; */
    lysh_lcg_set_seed(r, worldSeed);
    uint64_t a = lysh_lcg_next_long(r);
    uint64_t b = lysh_lcg_next_long(r);
    uint64_t s = (uint64_t)((int64_t)chunkX * (int64_t)a)
               ^ (uint64_t)((int64_t)chunkZ * (int64_t)b)
               ^ worldSeed;
    lysh_lcg_set_seed(r, s);
    return s & UINT64_C(0xFFFFFFFFFFFF);
}
