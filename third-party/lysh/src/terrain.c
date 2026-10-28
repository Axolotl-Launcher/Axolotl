/* lysh terrain.c —— 见 terrain.h 顶部的语义与陷阱说明。 */
#include "terrain.h"

void lysh_terrain_init(lysh_terrain *t, uint64_t world_seed, int large_biomes) {
    lysh_dblnoise_from_seed_id(&t->shift, world_seed, "minecraft:offset");
    lysh_dblnoise_from_seed_id(&t->continentalness, world_seed,
                               large_biomes ? "minecraft:continentalness_large"
                                            : "minecraft:continentalness");
    lysh_dblnoise_from_seed_id(&t->erosion, world_seed,
                               large_biomes ? "minecraft:erosion_large" : "minecraft:erosion");
    lysh_dblnoise_from_seed_id(&t->weirdness, world_seed, "minecraft:ridge");
    t->semantics_261 = 0;       /* 默认 1.18.2 语义（全程 float） */
}

void lysh_terrain_set_semantics_261(lysh_terrain *t, int on) { t->semantics_261 = on ? 1 : 0; }

/* NoiseColumnSampler.sampleShiftNoise(int a, int b, int c) = shift.sample(a,b,c) * 4.0 */
static inline double sample_shift(const lysh_terrain *t, int a, int b, int c) {
    return lysh_dblnoise_sample(&t->shift, (double)a, (double)b, (double)c) * 4.0;
}

void lysh_terrain_info_at(const lysh_terrain *t, int biome_x, int biome_z, lysh_terrain_info *out) {
    /* ⚠️ 第二个的实参顺序是 (biomeZ, biomeX, 0)，不是 (biomeZ, 0, biomeX) */
    double shifted_x = (double)biome_x + sample_shift(t, biome_x, 0, biome_z);
    double shifted_z = (double)biome_z + sample_shift(t, biome_z, biome_x, 0);

    /* 气候噪声不加缩放；y 恒为 0 → 走 y=0 特化 */
    double c = lysh_dblnoise_sample_y0(&t->continentalness, shifted_x, shifted_z);
    double w = lysh_dblnoise_sample_y0(&t->weirdness, shifted_x, shifted_z);
    double e = lysh_dblnoise_sample_y0(&t->erosion, shifted_x, shifted_z);

    out->shifted_x = shifted_x;
    out->shifted_z = shifted_z;
    out->continentalness = c;
    out->weirdness = w;
    out->erosion = e;

    /* ⚠️ 顺序是 (continentalness, erosion, weirdness)，不是 (cont, weird, ero) */
    if (t->semantics_261) {
        out->point = lysh_terrain_noise_point_261(c, e, w);
    } else {
        out->point = lysh_terrain_noise_point((float)c, (float)e, (float)w);
    }
}
