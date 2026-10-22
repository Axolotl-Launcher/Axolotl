#ifndef AXOLOTL_CUBIOMES_BRIDGE_H
#define AXOLOTL_CUBIOMES_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

typedef struct AxolotlSeedMapFeature {
    int32_t x;
    int32_t z;
    uint32_t kind;
    uint8_t approximate;
    int8_t end_ship;
} AxolotlSeedMapFeature;

/*
 * Maps an Axolotl version code (major * 10000 + minor * 100 + patch) to the
 * matching cubiomes MCVersion, or 0 (MC_UNDEF) when unsupported.
 */
uint32_t axolotl_seed_map_abi_version(void);
uint64_t axolotl_seed_map_engine_revision(void);

int32_t axolotl_seed_map_java_version(int32_t version);

/*
 * Renders a biome tile into an RGB buffer (3 bytes per pixel).
 *
 * Coordinates are given in scaled units (block / scale). When `terrain` is
 * non-zero, Overworld tiles use approximate surface heights and nearby End
 * tiles use the dimension's density-derived surface height. `contours`
 * additionally darkens height-band boundaries. `highlight_mask` may be NULL,
 * or point to 256 bytes where a non-zero entry keeps that biome id at full
 * color while all other biomes are faded.
 */
int axolotl_seed_map_render(
    uint64_t seed,
    int32_t minecraft_version,
    int32_t generator_flags,
    int32_t dimension,
    int32_t x,
    int32_t z,
    int32_t scale,
    int32_t width,
    int32_t height,
    int32_t elevation,
    int32_t terrain,
    int32_t contours,
    const uint8_t *highlight_mask,
    uint8_t *rgb,
    size_t rgb_len
);

size_t axolotl_seed_map_find_features(
    uint64_t seed,
    int32_t minecraft_version,
    int32_t generator_flags,
    int32_t dimension,
    int32_t min_x,
    int32_t min_z,
    int32_t max_x,
    int32_t max_z,
    uint32_t feature_mask,
    AxolotlSeedMapFeature *out,
    size_t out_len
);

int axolotl_seed_map_get_spawn(
    uint64_t seed,
    int32_t minecraft_version,
    int32_t generator_flags,
    int32_t *x,
    int32_t *z
);

/*
 * Returns the biome id at a block position (sampled at the given Y level),
 * or -1 when the engine cannot resolve it.
 */
int32_t axolotl_seed_map_biome_at(
    uint64_t seed,
    int32_t minecraft_version,
    int32_t generator_flags,
    int32_t dimension,
    int32_t x,
    int32_t y,
    int32_t z
);

size_t axolotl_seed_map_scan_ores(
    uint64_t seed, int32_t mc, int32_t flags, int32_t dimension,
    int32_t cx, int32_t cz, int32_t kind, int32_t *xyz, size_t capacity
);
size_t axolotl_seed_map_scan_ores_cancellable(uint64_t seed, int32_t mc, int32_t flags, int32_t dimension, int32_t cx, int32_t cz, int32_t kind, int32_t *xyz, size_t capacity, uint64_t request_id, int32_t (*canceled)(uint64_t));

/*
 * Fills `out` (length `width * height`) with approximate Overworld surface
 * heights in blocks. `x` and `z` are in 1:4 scale units and the grid has a
 * stride of one unit (4 blocks). Returns 0 on success.
 */
int axolotl_seed_map_surface_heights(
    uint64_t seed,
    int32_t minecraft_version,
    int32_t generator_flags,
    int32_t x,
    int32_t z,
    int32_t width,
    int32_t height,
    float *out
);

int32_t axolotl_seed_map_capability(int32_t mc, int32_t capability, int32_t value);

char *axolotl_seed_map_structure_details(uint64_t seed, int32_t mc, int32_t flags, int32_t dimension, int32_t x, int32_t z, uint32_t feature, int32_t container);
char *axolotl_seed_map_ancient_city_layout(uint64_t seed, int32_t mc, int32_t x, int32_t z, const char *layout_json, size_t layout_len);
char *axolotl_seed_map_generate_loot(const char *table_json, size_t table_len, uint64_t seed, int32_t mc);
void axolotl_seed_map_free_result(char *result);
int32_t axolotl_seed_map_underground_volume(uint64_t seed, int32_t mc, int32_t flags, int32_t dimension, int32_t x, int32_t z, int32_t y, int32_t width, int32_t height, uint8_t *blocks, uint64_t length);
int32_t axolotl_seed_map_underground_volume_surface(uint64_t seed, int32_t mc, int32_t flags, int32_t dimension, int32_t x, int32_t z, int32_t y, int32_t width, int32_t height, uint8_t *blocks, uint64_t length, int32_t *surface_heights);
int32_t axolotl_seed_map_underground_chunk(uint64_t seed, int32_t mc, int32_t flags, int32_t dimension, int32_t x, int32_t z, uint8_t *blocks, uint64_t length, int32_t *surface_heights, uint64_t request_id, int32_t (*canceled)(uint64_t));
int32_t axolotl_seed_map_underground(uint64_t seed, int32_t mc, int32_t flags, int32_t dimension, int32_t x, int32_t z, int32_t scale, int32_t width, int32_t height, int32_t y, uint32_t layers, uint8_t *rgba, uint64_t length);

#endif
