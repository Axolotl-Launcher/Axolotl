#include "cubiomes_bridge.h"

#include "biomenoise.h"
#include "finders.h"
#include "features/end_city.h"
#include "features/ore.h"
#include "generator.h"
#include "noise.h"
#include "rng.h"
#include "util.h"

#include <math.h>
#include <stdlib.h>
#include <string.h>

static float axolotl_clamp(float value, float low, float high);

/* Sample coarse Overworld pixels at the centre of their block cell while
 * retaining the same 3D Voronoi surface lookup used by scale=1. Without
 * this, scale=16+ reads the raw noise point directly and can select a
 * different vertical biome layer at tile boundaries. */
static void axolotl_gen_surface_biomes(
    int *out,
    const Generator *generator,
    int32_t x,
    int32_t z,
    int32_t scale,
    int32_t width,
    int32_t height,
    int32_t elevation
) {
    const int32_t y = elevation;
    for (int row = 0; row < height; row++) {
        for (int column = 0; column < width; column++) {
            const int block_x = x * scale + column * scale + scale / 2;
            const int block_z = z * scale + row * scale + scale / 2;
            int x4, y4, z4;
            voronoiAccess3D(generator->sha, block_x, y, block_z, &x4, &y4, &z4);
            out[(size_t)row * width + column] =
                sampleBiomeNoise(&generator->bn, NULL, x4, y4, z4, NULL, 0);
        }
    }
}

static int axolotl_map_approx_height(
    float *heights,
    const Generator *generator,
    const SurfaceNoise *surface_noise,
    int x,
    int z,
    int width,
    int height,
    int sparse
) {
    if (generator->dim != DIM_OVERWORLD || generator->mc < MC_1_18 ||
        (generator->bn.nptype != -1 && generator->bn.nptype != NP_DEPTH)) {
        return mapApproxHeight(
            heights, NULL, generator, surface_noise, x, z, width, height);
    }

    const uint32_t flags = SAMPLE_NO_BIOME | (sparse ? SAMPLE_NO_SHIFT : 0);
    for (int row = 0; row < height; row++) {
        for (int column = 0; column < width; column++) {
            int64_t noise_parameters[6];
            sampleBiomeNoise(
                &generator->bn, noise_parameters, x + column, 0, z + row, NULL, flags);
            heights[(size_t)row * width + column] =
                (float)noise_parameters[NP_DEPTH] / 76.0f;
        }
    }
    return 0;
}

enum {
    AXOLOTL_FEATURE_VILLAGE = 1 << 0,
    AXOLOTL_FEATURE_OUTPOST = 1 << 1,
    AXOLOTL_FEATURE_SHIPWRECK = 1 << 2,
    AXOLOTL_FEATURE_MONUMENT = 1 << 3,
    AXOLOTL_FEATURE_MANSION = 1 << 4,
    AXOLOTL_FEATURE_ANCIENT_CITY = 1 << 5,
    AXOLOTL_FEATURE_TRAIL_RUINS = 1 << 6,
    AXOLOTL_FEATURE_TRIAL_CHAMBERS = 1 << 7,
    AXOLOTL_FEATURE_RUINED_PORTAL = 1 << 8,
    AXOLOTL_FEATURE_STRONGHOLD = 1 << 9,
    AXOLOTL_FEATURE_SLIME_CHUNK = 1 << 10,
    AXOLOTL_FEATURE_DESERT_PYRAMID = 1 << 11,
    AXOLOTL_FEATURE_JUNGLE_TEMPLE = 1 << 12,
    AXOLOTL_FEATURE_SWAMP_HUT = 1 << 13,
    AXOLOTL_FEATURE_IGLOO = 1 << 14,
    AXOLOTL_FEATURE_OCEAN_RUIN = 1 << 15,
    AXOLOTL_FEATURE_BURIED_TREASURE = 1 << 16,
    AXOLOTL_FEATURE_MINESHAFT = 1 << 17,
    AXOLOTL_FEATURE_DESERT_WELL = 1 << 18,
    AXOLOTL_FEATURE_GEODE = 1 << 19,
    AXOLOTL_FEATURE_FORTRESS = 1 << 20,
    AXOLOTL_FEATURE_BASTION = 1 << 21,
    AXOLOTL_FEATURE_END_CITY = 1 << 22,
    AXOLOTL_FEATURE_END_GATEWAY = 1 << 23,
    AXOLOTL_FEATURE_ABANDONED_CAMP = 1 << 24,
    AXOLOTL_FEATURE_NETHER_FOSSIL = 1 << 25,
};

typedef struct AxolotlStructureDefinition {
    int type;
    uint32_t flag;
} AxolotlStructureDefinition;

static const AxolotlStructureDefinition AXOLOTL_STRUCTURES[] = {
    {Village, AXOLOTL_FEATURE_VILLAGE},
    {Outpost, AXOLOTL_FEATURE_OUTPOST},
    {Shipwreck, AXOLOTL_FEATURE_SHIPWRECK},
    {Monument, AXOLOTL_FEATURE_MONUMENT},
    {Mansion, AXOLOTL_FEATURE_MANSION},
    {Ancient_City, AXOLOTL_FEATURE_ANCIENT_CITY},
    {Trail_Ruins, AXOLOTL_FEATURE_TRAIL_RUINS},
    {Trial_Chambers, AXOLOTL_FEATURE_TRIAL_CHAMBERS},
    {Ruined_Portal, AXOLOTL_FEATURE_RUINED_PORTAL},
    {Ruined_Portal_N, AXOLOTL_FEATURE_RUINED_PORTAL},
    {Desert_Pyramid, AXOLOTL_FEATURE_DESERT_PYRAMID},
    {Jungle_Temple, AXOLOTL_FEATURE_JUNGLE_TEMPLE},
    {Swamp_Hut, AXOLOTL_FEATURE_SWAMP_HUT},
    {Igloo, AXOLOTL_FEATURE_IGLOO},
    {Ocean_Ruin, AXOLOTL_FEATURE_OCEAN_RUIN},
    {Treasure, AXOLOTL_FEATURE_BURIED_TREASURE},
    {Mineshaft, AXOLOTL_FEATURE_MINESHAFT},
    {Desert_Well, AXOLOTL_FEATURE_DESERT_WELL},
    {Geode, AXOLOTL_FEATURE_GEODE},
    {Fortress, AXOLOTL_FEATURE_FORTRESS},
    {Bastion, AXOLOTL_FEATURE_BASTION},
    {End_City, AXOLOTL_FEATURE_END_CITY},
    {End_Gateway, AXOLOTL_FEATURE_END_GATEWAY},
    {Abandoned_Camp, AXOLOTL_FEATURE_ABANDONED_CAMP},
    {Nether_Fossil, AXOLOTL_FEATURE_NETHER_FOSSIL},
};

uint32_t axolotl_seed_map_abi_version(void) { return 2; }
uint64_t axolotl_seed_map_engine_revision(void) { return UINT64_C(0xa49d5bc5b709e6e2); }

int32_t axolotl_seed_map_java_version(int32_t version) {
    switch (version) {
        case -10700: return MC_B1_7;
        case -10800: return MC_B1_8;
        case 10000: return MC_1_0_0;
        case 10100: return MC_1_1_0;
        case 10205: return MC_1_2_5;
        case 10302: return MC_1_3_2;
        case 10407: return MC_1_4_7;
        case 10502: return MC_1_5_2;
        case 10604: return MC_1_6_4;
        case 10710: return MC_1_7_10;
        case 10809: return MC_1_8_9;
        case 10904: return MC_1_9_4;
        case 11002: return MC_1_10_2;
        case 11102: return MC_1_11_2;
        case 11202: return MC_1_12_2;
        case 11302: return MC_1_13_2;
        case 11404: return MC_1_14_4;
        case 11502: return MC_1_15_2;
        case 11601: return MC_1_16_1;
        case 11605: return MC_1_16_5;
        case 11701: return MC_1_17_1;
        case 11802: return MC_1_18_2;
        case 11902: return MC_1_19_2;
        case 11904: return MC_1_19_4;
        case 12006: return MC_1_20_6;
        case 12101: return MC_1_21_1;
        case 12103: return MC_1_21_3;
        case 12104: return MC_1_21_4;
        case 12105: return MC_1_21_5;
        case 12106: return MC_1_21_6;
        case 12109: return MC_1_21_9;
        case 12111: return MC_1_21_11;
        case 260100: return MC_26_1;
        case 260200: return MC_26_2;
        case 260300: return MC_26_3;
        case 260102: return MC_26_1;
        case 12000: return MC_1_20_6;
        case 11800: return MC_1_18_2;
        case 11700: return MC_1_17_1;
        case 11600: return MC_1_16_5;
        case 11500: return MC_1_15_2;
        case 11400: return MC_1_14_4;
        case 11300: return MC_1_13_2;
        case 11200: return MC_1_12_2;
        case 11100: return MC_1_11_2;
        case 11000: return MC_1_10_2;
        case 10900: return MC_1_9_4;
        case 10800: return MC_1_8_9;
        case 10700: return MC_1_7_10;
        case 10600: return MC_1_6_4;
        case 10500: return MC_1_5_2;
        case 10400: return MC_1_4_7;
        case 10300: return MC_1_3_2;
        case 10200: return MC_1_2_5;
        default: return MC_UNDEF;
    }
}

static const unsigned char AXOLOTL_END_VOID[3] = {0x4B, 0x4B, 0xAB};

static int axolotl_smooth_height_grid(
    float *grid,
    int width,
    int height,
    int passes
) {
    float *scratch = malloc((size_t)width * (size_t)height * sizeof(float));
    if (!scratch) return -1;
    for (int pass = 0; pass < passes; pass++) {
        for (int row = 0; row < height; row++) {
            const int offset = row * width;
            for (int column = 0; column < width; column++) {
                const float west = grid[offset + (column > 0 ? column - 1 : 0)];
                const float east = grid[offset + (column < width - 1 ? column + 1 : width - 1)];
                scratch[offset + column] =
                    (west + 2.0f * grid[offset + column] + east) * 0.25f;
            }
        }
        for (int row = 0; row < height; row++) {
            const int north = (row > 0 ? row - 1 : 0) * width;
            const int offset = row * width;
            const int south = (row < height - 1 ? row + 1 : height - 1) * width;
            for (int column = 0; column < width; column++) {
                grid[offset + column] =
                    (scratch[north + column] + 2.0f * scratch[offset + column] +
                        scratch[south + column]) *
                    0.25f;
            }
        }
    }
    free(scratch);
    return 0;
}

/*
 * The End uses the game's density noise to distinguish real island surface
 * from void. Like the reference map, the height grid is smoothed before slope
 * lighting so surface noise does not make the broad, flat islands look hilly.
 */
static void axolotl_render_end(
    uint8_t *rgb,
    const Generator *generator,
    const SurfaceNoise *surface_noise,
    int32_t x,
    int32_t z,
    int32_t scale,
    int32_t width,
    int32_t height,
    int32_t contours
) {
    if (scale > 4) return;
    const int passes = scale == 1 ? 3 : 1;
    const int padding = passes + 1;
    const int grid_width = width + 2 * padding;
    const int grid_height = height + 2 * padding;
    float *heights = malloc((size_t)grid_width * (size_t)grid_height * sizeof(float));
    if (!heights) return;
    uint8_t *land = malloc((size_t)width * (size_t)height);
    if (!land) {
        free(heights);
        return;
    }
    if (mapEndSurfaceHeight(
            heights, &generator->en, surface_noise,
            x - padding, z - padding, grid_width, grid_height, scale, 0) != 0) {
        free(land);
        free(heights);
        return;
    }
    for (int row = 0; row < height; row++) {
        for (int column = 0; column < width; column++) {
            land[(size_t)row * width + column] =
                heights[(row + padding) * grid_width + column + padding] > 4.0f;
        }
    }
    if (axolotl_smooth_height_grid(heights, grid_width, grid_height, passes) != 0) {
        free(land);
        free(heights);
        return;
    }
    for (int row = 0; row < height; row++) {
        for (int column = 0; column < width; column++) {
            const size_t index = (size_t)row * (size_t)width + (size_t)column;
            uint8_t *pixel = rgb + index * 3;
            if (!land[index]) {
                memcpy(pixel, AXOLOTL_END_VOID, 3);
                continue;
            }
            const int grid_row = row + padding;
            const int grid_column = column + padding;
            const int surface = (int)floorf(heights[grid_row * grid_width + grid_column]);
            const int west = (int)floorf(heights[grid_row * grid_width + grid_column - 1]);
            const int north = (int)floorf(heights[(grid_row - 1) * grid_width + grid_column]);
            const int northwest =
                (int)floorf(heights[(grid_row - 1) * grid_width + grid_column - 1]);
            float factor = 1.0f;
            if (contours && (surface / 16 != west / 16 || surface / 16 != north / 16)) {
                factor = 0.15f;
            } else {
                const int slope = 3 * surface - west - north - northwest;
                if (slope != 0) {
                    const float delta = axolotl_clamp((float)slope / 16.0f, -0.3f, 0.3f);
                    factor = 1.0f + delta + (delta > 0.0f ? 0.1f : -0.1f);
                    if (factor < 0.72f) factor = 0.72f;
                }
            }
            for (int channel = 0; channel < 3; channel++) {
                pixel[channel] =
                    (uint8_t)axolotl_clamp((float)pixel[channel] * factor, 0.0f, 255.0f);
            }
        }
    }
    free(land);
    free(heights);
}

static int axolotl_block_height(
    const float *grid,
    int grid_width,
    int grid_height,
    int column,
    int row
) {
    if (column < 0) column = 0;
    if (row < 0) row = 0;
    if (column > grid_width - 1) column = grid_width - 1;
    if (row > grid_height - 1) row = grid_height - 1;
    return (int)floorf(grid[row * grid_width + column]);
}

static float axolotl_clamp(float value, float low, float high) {
    if (value < low) return low;
    if (value > high) return high;
    return value;
}

/*
 * Minecraft maps compare each cell with its northwestern neighbours and use
 * discrete brightness levels for the slope. The reference map applies the
 * same rule to its cubiomes height grid, including 16-block contour bands.
 */
static void axolotl_shade_height_map(
    uint8_t *rgb,
    int32_t width,
    int32_t height,
    int32_t contours,
    const float *grid,
    int grid_width,
    int grid_height,
    int grid_origin_column,
    int grid_origin_row,
    int numerator,
    int denominator
) {
    for (int row = 0; row < height; row++) {
        const int cell_row =
            grid_origin_row + (int)(((int64_t)row * numerator) / denominator);
        for (int column = 0; column < width; column++) {
            const int cell_column =
                grid_origin_column + (int)(((int64_t)column * numerator) / denominator);
            const size_t index = (size_t)row * (size_t)width + (size_t)column;
            const int here =
                axolotl_block_height(grid, grid_width, grid_height, cell_column, cell_row);
            const int west =
                axolotl_block_height(grid, grid_width, grid_height, cell_column - 1, cell_row);
            const int north =
                axolotl_block_height(grid, grid_width, grid_height, cell_column, cell_row - 1);
            const int northwest =
                axolotl_block_height(grid, grid_width, grid_height, cell_column - 1, cell_row - 1);
            float factor = 1.0f;
            if (contours) {
                const int band = here / 16;
                if (band != west / 16 || band != north / 16) factor = 0.15f;
            }
            if (factor == 1.0f) {
                const int slope = 3 * here - west - north - northwest;
                if (slope != 0) {
                    const float delta = axolotl_clamp((float)slope / 16.0f, -0.3f, 0.3f);
                    factor = 1.0f + delta + (delta > 0.0f ? 0.1f : -0.1f);
                    if (factor < 0.72f) factor = 0.72f;
                }
            }
            uint8_t *pixel = rgb + index * 3;
            const int strongest = pixel[0] > pixel[1]
                ? (pixel[0] > pixel[2] ? pixel[0] : pixel[2])
                : (pixel[1] > pixel[2] ? pixel[1] : pixel[2]);
            if (factor < 1.0f && strongest > 25 && strongest - strongest * factor < 25)
                factor = (strongest - 25.0f) / strongest;
            for (int channel = 0; channel < 3; channel++) {
                const float shaded = (float)pixel[channel] * factor;
                pixel[channel] = (uint8_t)axolotl_clamp(shaded, 0.0f, 255.0f);
            }
        }
    }
}

/*
 * Far-zoom tiles cover too many blocks for the contiguous 1:4 height map, so
 * relief comes from a sparse grid sampled point by point at each cell
 * center. The grid stays coarse enough to keep far tiles interactive.
 */
static int axolotl_fill_sparse_height_grid(
    const Generator *generator,
    const SurfaceNoise *surface_noise,
    int64_t min_block_x,
    int64_t min_block_z,
    int64_t span_blocks,
    int grid,
    float *out
) {
    for (int row = 0; row < grid; row++) {
        const int64_t block_z =
            min_block_z + span_blocks * (2 * row + 1) / (2 * grid);
        for (int column = 0; column < grid; column++) {
            const int64_t block_x =
                min_block_x + span_blocks * (2 * column + 1) / (2 * grid);
            if (axolotl_map_approx_height(
                    out + (size_t)row * grid + column, generator, surface_noise,
                    (int)(block_x >> 2), (int)(block_z >> 2), 1, 1, 1) != 0) {
                return -1;
            }
        }
    }
    return 0;
}

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
) {
    if (!rgb || width <= 0 || height <= 0 || scale <= 0) return -1;
    if (rgb_len < (size_t)width * (size_t)height * 3) return -2;
    const int32_t render_elevation = elevation;

    Generator generator;
    setupGenerator(&generator, minecraft_version, generator_flags);
    applySeed(&generator, dimension, seed);

    Range range = {
        .scale = scale,
        .x = x,
        .z = z,
        .sx = width,
        .sz = height,
        .y = render_elevation / 4,
        .sy = 1,
    };
    int *biomes = allocCache(&generator, range);
    if (!biomes) return -3;
    int result;
    if (dimension == DIM_OVERWORLD && minecraft_version >= MC_1_18) {
        /* Use the same surface/Voronoi path at every coarse scale. */
        axolotl_gen_surface_biomes(
            biomes, &generator, x, z, scale, width, height, render_elevation);
        result = 0;
    } else {
        result = genBiomes(&generator, biomes, range);
    }
    if (result != 0) {
        free(biomes);
        return result;
    }

    unsigned char colors[256][3];
    initBiomeColors(colors);
    biomesToImage(rgb, colors, biomes, width, height, 1, 2);

    if (highlight_mask) {
        for (int row = 0; row < height; row++) {
            for (int column = 0; column < width; column++) {
                size_t index = (size_t)row * (size_t)width + (size_t)column;
                const int biome = biomes[index];
                if (biome >= 0 && biome < 256 && highlight_mask[biome]) continue;
                size_t pixel = index * 3;
                if (dimension != DIM_OVERWORLD) {
                    rgb[pixel] = (uint8_t)(255 - (((255 - rgb[pixel]) * 51) >> 8));
                    rgb[pixel + 1] = (uint8_t)(255 - (((255 - rgb[pixel + 1]) * 51) >> 8));
                    rgb[pixel + 2] = (uint8_t)(255 - (((255 - rgb[pixel + 2]) * 51) >> 8));
                } else {
                    rgb[pixel] = (uint8_t)((77 * rgb[pixel]) >> 8);
                    rgb[pixel + 1] = (uint8_t)((77 * rgb[pixel + 1]) >> 8);
                    rgb[pixel + 2] = (uint8_t)((77 * rgb[pixel + 2]) >> 8);
                }
            }
        }
    }

    const int64_t min_block_x = (int64_t)x * scale;
    const int64_t min_block_z = (int64_t)z * scale;
    if (terrain && dimension == DIM_END) {
        SurfaceNoise surface_noise;
        initSurfaceNoise(&surface_noise, DIM_END, seed);
        axolotl_render_end(
            rgb, &generator, &surface_noise,
            x, z, scale, width, height, contours);
    }

    if (terrain && dimension == DIM_OVERWORLD) {
        SurfaceNoise surface_noise;
        initSurfaceNoise(&surface_noise, DIM_OVERWORLD, seed);
        if (scale <= 4) {
            const int grid_x =
                (int)(min_block_x >= 0 ? min_block_x / 4 : (min_block_x - 3) / 4) - 1;
            const int grid_z =
                (int)(min_block_z >= 0 ? min_block_z / 4 : (min_block_z - 3) / 4) - 1;
            const int grid_width = (width * scale) / 4 + 3;
            const int grid_height = (height * scale) / 4 + 3;
            float *heights =
                malloc((size_t)grid_width * (size_t)grid_height * sizeof(float));
            if (heights) {
                if (axolotl_map_approx_height(
                        heights, &generator, &surface_noise,
                        grid_x, grid_z, grid_width, grid_height, 0) == 0) {
                    axolotl_shade_height_map(
                        rgb, width, height, contours,
                        heights, grid_width, grid_height, 1, 1, scale, 4);
                }
                free(heights);
            }
        } else {
            const int grid = scale == 16 ? 96 : 64;
            float *heights = malloc((size_t)grid * (size_t)grid * sizeof(float));
            if (heights) {
                if (axolotl_fill_sparse_height_grid(
                        &generator, &surface_noise, min_block_x, min_block_z,
                        (int64_t)width * scale, grid, heights) == 0) {
                    axolotl_shade_height_map(
                        rgb, width, height, contours,
                        heights, grid, grid, 0, 0, grid, width);
                }
                free(heights);
            }
        }
    }
    free(biomes);

    return 0;
}

static int add_feature(
    AxolotlSeedMapFeature *out,
    size_t out_len,
    size_t *count,
    int x,
    int z,
    uint32_t kind,
    uint8_t approximate,
    int8_t end_ship
) {
    if (*count >= out_len) return 0;
    out[*count].x = x;
    out[*count].z = z;
    out[*count].kind = kind;
    out[*count].approximate = approximate;
    out[*count].end_ship = end_ship;
    *count += 1;
    return 1;
}

static int8_t axolotl_end_city_has_ship(uint64_t seed, int block_x, int block_z) {
    Piece pieces[END_CITY_PIECES_MAX];
    const int count = getEndCityPieces(pieces, seed, block_x >> 4, block_z >> 4);
    for (int index = 0; index < count; index++) {
        if (pieces[index].type == END_SHIP) return 1;
    }
    return 0;
}

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
) {
    if (!out || out_len == 0) return 0;
    Generator generator;
    setupGenerator(&generator, minecraft_version, generator_flags);
    applySeed(&generator, dimension, seed);
    SurfaceNoise end_surface_noise;
    if (dimension == DIM_END) initSurfaceNoise(&end_surface_noise, DIM_END, seed);
    BlendedNoise nether_terrain;
    if (dimension == DIM_NETHER && (feature_mask & AXOLOTL_FEATURE_NETHER_FOSSIL))
        initBlendedNoise(&nether_terrain, seed, DIM_NETHER);

    size_t count = 0;
    for (size_t index = 0; index < sizeof(AXOLOTL_STRUCTURES) / sizeof(AXOLOTL_STRUCTURES[0]); index++) {
        const AxolotlStructureDefinition definition = AXOLOTL_STRUCTURES[index];
        if (!(feature_mask & definition.flag)) continue;

        StructureConfig config;
        if (!getStructureConfig(definition.type, minecraft_version, &config)) continue;
        if (config.dim != dimension || config.regionSize <= 0) continue;

        const int region_span = config.regionSize * 16;
        const int min_region_x = min_x >= 0 ? min_x / region_span : (min_x - region_span + 1) / region_span;
        const int min_region_z = min_z >= 0 ? min_z / region_span : (min_z - region_span + 1) / region_span;
        const int max_region_x = max_x >= 0 ? max_x / region_span : (max_x - region_span + 1) / region_span;
        const int max_region_z = max_z >= 0 ? max_z / region_span : (max_z - region_span + 1) / region_span;

        for (int region_x = min_region_x; region_x <= max_region_x; region_x++) {
            for (int region_z = min_region_z; region_z <= max_region_z; region_z++) {
                Pos position;
                if (!getStructurePos(definition.type, minecraft_version, seed, region_x, region_z, &position)) continue;
                if (definition.type == Nether_Fossil) {
                    StructureVariant variant;
                    if (!getVariant(&variant, Nether_Fossil, minecraft_version, seed, position.x, position.z, -1) ||
                        !isViableNetherFossilTerrain(position.x >> 4, position.z >> 4, &variant, &nether_terrain, minecraft_version))
                        continue;
                    position.x += variant.x;
                    position.z += variant.z;
                    if (!isViableFeatureBiome(minecraft_version, Nether_Fossil,
                            getBiomeAt(&generator, 4, position.x >> 2, variant.y >> 2, position.z >> 2)))
                        continue;
                }
                if (position.x < min_x || position.x > max_x || position.z < min_z || position.z > max_z) continue;
                if (definition.type != Nether_Fossil &&
                    !isViableStructurePos(definition.type, &generator, position.x, position.z, 0)) continue;
                if (definition.type == End_City &&
                    !isViableEndCityTerrain(&generator, &end_surface_noise, position.x, position.z))
                    continue;
                if (dimension == DIM_OVERWORLD && minecraft_version >= MC_1_18 &&
                    !isViableStructureTerrain(definition.type, &generator, position.x, position.z))
                    continue;
                const int8_t end_ship = definition.type == End_City
                    ? axolotl_end_city_has_ship(seed, position.x, position.z)
                    : -1;
                if (!add_feature(
                        out, out_len, &count, position.x, position.z,
                        definition.flag, 0, end_ship)) return count;
            }
        }
    }

    if (dimension == DIM_OVERWORLD && feature_mask & AXOLOTL_FEATURE_STRONGHOLD) {
        StrongholdIter stronghold;
        initFirstStronghold(&stronghold, minecraft_version, seed);
        for (int index = 0; index < 32; index++) {
            if (nextStronghold(&stronghold, &generator) <= 0) break;
            if (stronghold.pos.x >= min_x && stronghold.pos.x <= max_x && stronghold.pos.z >= min_z && stronghold.pos.z <= max_z) {
                if (!add_feature(
                        out, out_len, &count, stronghold.pos.x, stronghold.pos.z,
                        AXOLOTL_FEATURE_STRONGHOLD, 0, -1)) return count;
            }
        }
    }

    if (dimension == DIM_OVERWORLD && feature_mask & AXOLOTL_FEATURE_SLIME_CHUNK) {
        const int min_chunk_x = min_x >= 0 ? min_x / 16 : (min_x - 15) / 16;
        const int min_chunk_z = min_z >= 0 ? min_z / 16 : (min_z - 15) / 16;
        const int max_chunk_x = max_x >= 0 ? max_x / 16 : (max_x - 15) / 16;
        const int max_chunk_z = max_z >= 0 ? max_z / 16 : (max_z - 15) / 16;
        for (int chunk_x = min_chunk_x; chunk_x <= max_chunk_x; chunk_x++) {
            for (int chunk_z = min_chunk_z; chunk_z <= max_chunk_z; chunk_z++) {
                if (!isSlimeChunk(seed, chunk_x, chunk_z)) continue;
                if (!add_feature(
                        out, out_len, &count, chunk_x * 16 + 8, chunk_z * 16 + 8,
                        AXOLOTL_FEATURE_SLIME_CHUNK, 0, -1)) return count;
            }
        }
    }

    return count;
}

int axolotl_seed_map_get_spawn(
    uint64_t seed,
    int32_t minecraft_version,
    int32_t generator_flags,
    int32_t *x,
    int32_t *z
) {
    if (!x || !z) return -1;
    Generator generator;
    setupGenerator(&generator, minecraft_version, generator_flags);
    applySeed(&generator, DIM_OVERWORLD, seed);
    Pos position = estimateSpawn(&generator, NULL);
    *x = position.x;
    *z = position.z;
    return 0;
}

int32_t axolotl_seed_map_biome_at(
    uint64_t seed,
    int32_t minecraft_version,
    int32_t generator_flags,
    int32_t dimension,
    int32_t x,
    int32_t y,
    int32_t z
) {
    Generator generator;
    setupGenerator(&generator, minecraft_version, generator_flags);
    applySeed(&generator, dimension, seed);
    return getBiomeAt(&generator, 1, x, y, z);
}

static const int ore_groups[][7] = {
        {DiamondOre, BuriedDiamondOre, LargeDiamondOre, MediumDiamondOre, -1},
        {IronOre, UpperIronOre, MiddleIronOre, SmallIronOre, -1},
        {-1}, {CopperOre, LargeCopperOre, -1}, {-1},
        {GoldOre, ExtraGoldOre, LowerGoldOre, -1},
        {RedstoneOre, LowerRedstoneOre, -1}, {LapisOre, BuriedLapisOre, -1},
        {CoalOre, UpperCoalOre, LowerCoalOre, -1}, {LargeDebrisOre, SmallDebrisOre, -1},
        {EmeraldOre, -1}, {NetherQuartzOre, DeltasQuartzOre, -1}, {NetherGoldOre, DeltasGoldOre, -1},
        {AndesiteOre, LowerAndesiteOre, UpperAndesiteOre, -1},
        {DioriteOre, LowerDioriteOre, UpperDioriteOre, -1},
        {GraniteOre, LowerGraniteOre, UpperGraniteOre, -1},
        {DirtOre, -1}, {GravelOre, NetherGravelOre, -1}, {InfestedOre, -1},
        {DeepslateOre, -1}, {TuffOre, -1}, {ClayOre, -1},
        {BlackstoneOre, -1}, {MagmaOre, -1}, {SoulSandOre, -1},
    };

static size_t scan_ores_cancellable(
    uint64_t seed, int32_t mc, int32_t flags, int32_t dimension,
    int32_t cx, int32_t cz, int32_t kind, int32_t *xyz, size_t capacity,
    uint64_t request_id, int32_t (*canceled)(uint64_t)
) {
    if (!xyz || capacity == 0 || mc < MC_1_13) return 0;
    Generator g;
    setupGenerator(&g, mc, flags);
    applySeed(&g, dimension, seed);
    size_t count = 0;
    if (kind == 2 || kind == 4) {
        if (dimension != DIM_OVERWORLD || mc < MC_1_18) return 0;
        OreVeinParameters noise;
        if (!initOreVeinNoise(&noise, seed, mc)) return 0;
        const int copper = kind == 4;
        for (int y = copper ? 0 : -60; y <= (copper ? 50 : -8); y++) {
            if (canceled && canceled(request_id)) return count;
            for (int x = cx * 16; x < cx * 16 + 16; x++) {
                for (int z = cz * 16; z < cz * 16 + 16; z++) {
                    const int block = getOreVeinBlockAt(x, y, z, &noise);
                    if (block != (copper ? COPPER_ORE : IRON_ORE) && block != (copper ? RAW_COPPER_BLOCK : RAW_IRON_BLOCK)) continue;
                    if (count == capacity) return count;
                    xyz[count * 3] = x; xyz[count * 3 + 1] = y; xyz[count * 3 + 2] = z;
                    count++;
                }
            }
        }
        return count;
    }
    if (kind < 0 || kind >= (int)(sizeof(ore_groups) / sizeof(ore_groups[0]))) return 0;
    SurfaceNoise sn;
    initSurfaceNoise(&sn, dimension, seed);
    int biome = getBiomeForOreGen(&g, cx, cz, 64);
    for (int index = 0; index < 7 && ore_groups[kind][index] >= 0; index++) {
        if (canceled && canceled(request_id)) return count;
        OreConfig config;
        if (!getOreConfig(ore_groups[kind][index], mc, biome, &config) || config.dim != dimension) continue;
        Pos3List positions;
        if (!canceled) positions = generateOres(&g, &sn, config, cx, cz);
        else {
            uint64_t population_seed = getPopulationSeed(mc, seed, cx * 16, cz * 16);
            RandomSource random = {.type = mc <= MC_1_17 ? JAVA_RANDOM : XOROSHIRO_J};
            absSetSeed(&random, population_seed + config.index + 10000 * config.step);
            int repeats = config.repeatCount;
            if (config.oreType == LargeDiamondOre || config.oreType == UpperAndesiteOre ||
                config.oreType == UpperDioriteOre || config.oreType == UpperGraniteOre)
                repeats = absNextFloat(&random) < 1.0F / config.repeatCount;
            createPos3List(&positions, 1024);
            for (int repeat = 0; repeat < repeats; repeat++) {
                if (canceled(request_id)) { freePos3List(&positions); return count; }
                Pos3 base = generateBaseOrePosition(mc, config, cx, cz, &random);
                int ore_biome = getBiomeAt(&g, 1, base.x, base.y, base.z);
                if (isViableOreBiome(mc, config.oreType, ore_biome))
                    generateOrePositions(&g, &sn, config, base, &random, &positions);
            }
        }
        for (int i = 0; i < positions.size && count < capacity; i++) {
            xyz[count * 3] = positions.pos3s[i].x;
            xyz[count * 3 + 1] = positions.pos3s[i].y;
            xyz[count * 3 + 2] = positions.pos3s[i].z;
            count++;
        }
        freePos3List(&positions);
        if (count == capacity) break;
    }
    return count;
}

size_t axolotl_seed_map_scan_ores(uint64_t seed, int32_t mc, int32_t flags, int32_t dimension,
    int32_t cx, int32_t cz, int32_t kind, int32_t *xyz, size_t capacity) {
    return scan_ores_cancellable(seed, mc, flags, dimension, cx, cz, kind, xyz, capacity, 0, NULL);
}

size_t axolotl_seed_map_scan_ores_cancellable(uint64_t seed, int32_t mc, int32_t flags, int32_t dimension,
    int32_t cx, int32_t cz, int32_t kind, int32_t *xyz, size_t capacity,
    uint64_t request_id, int32_t (*canceled)(uint64_t)) {
    return scan_ores_cancellable(seed, mc, flags, dimension, cx, cz, kind, xyz, capacity, request_id, canceled);
}

int axolotl_seed_map_surface_heights(
    uint64_t seed,
    int32_t minecraft_version,
    int32_t generator_flags,
    int32_t x,
    int32_t z,
    int32_t width,
    int32_t height,
    float *out
) {
    if (!out || width <= 0 || height <= 0) return -1;
    Generator generator;
    setupGenerator(&generator, minecraft_version, generator_flags);
    applySeed(&generator, DIM_OVERWORLD, seed);
    SurfaceNoise surface_noise;
    initSurfaceNoise(&surface_noise, DIM_OVERWORLD, seed);
    return mapApproxHeight(out, NULL, &generator, &surface_noise, x, z, width, height);
}

int32_t axolotl_seed_map_capability(int32_t mc, int32_t capability, int32_t value) {
    if (capability == 0) return biomeExists(mc, value);
    if (capability == 1) {
        uint32_t mask = value == DIM_OVERWORLD ? AXOLOTL_FEATURE_SLIME_CHUNK : 0;
        if (value == DIM_OVERWORLD && mc >= MC_B1_8) mask |= AXOLOTL_FEATURE_STRONGHOLD;
        for (size_t i = 0; i < sizeof(AXOLOTL_STRUCTURES) / sizeof(AXOLOTL_STRUCTURES[0]); i++) {
            StructureConfig config;
            if (getStructureConfig(AXOLOTL_STRUCTURES[i].type, mc, &config) && config.dim == value) mask |= AXOLOTL_STRUCTURES[i].flag;
        }
        return (int32_t)mask;
    }
    if (capability == 3) return (mc >= MC_1_14 ? 1 : 0) | (mc >= MC_1_13 && value != DIM_END ? (value == DIM_NETHER ? 2 : 6) : 0);
    if (capability == 2 && value >= 0 && value < 25) {
        if (value == 2 || value == 4) return mc >= MC_1_18;
        for (int i = 0; i < 7 && ore_groups[value][i] >= 0; i++) {
            OreConfig config;
            if (getOreConfig(ore_groups[value][i], mc, plains, &config)) return 1;
        }
    }
    return 0;
}
