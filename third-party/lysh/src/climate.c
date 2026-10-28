/* lysh climate.c —— 见 climate.h 顶部的约束说明（保持近似，不动）。 */
#include "climate.h"

#include <stdio.h>

#include "noise_table.h"

void lysh_climate_init(lysh_climate *c, uint64_t world_seed, int large_biomes) {
    lysh_dblnoise_from_seed_id(&c->erosion, world_seed,
                               large_biomes ? "minecraft:erosion_large" : "minecraft:erosion");
    lysh_dblnoise_from_seed_id(&c->temperature, world_seed,
                               large_biomes ? "minecraft:temperature_large" : "minecraft:temperature");
    lysh_dblnoise_from_seed_id(&c->continentalness, world_seed,
                               large_biomes ? "minecraft:continentalness_large" : "minecraft:continentalness");
    lysh_dblnoise_from_seed_id(&c->ridge, world_seed, "minecraft:ridge");

    /* erosion 门的提前退出：把「按贡献交错」的求值序列预计算出来。 */
    c->erosion_tier_spec = lysh_tier_spec_erosion();
    c->erosion_tier_ready = 0;
    int rc = lysh_tier_seq_init(&c->erosion_tier_seq, &c->erosion);
    if (rc == 0) {
        c->erosion_tier_ready = 1;
    } else {
        /* 静默失败是这套代码最大的风险 —— 表结构不满足假设就明确报出来 */
        fprintf(stderr, "lysh: erosion tier seq unavailable (rc=%d); "
                        "falling back to the exact path\n", rc);
    }
}
