#include "cubiomes_bridge.h"
#include "finders.h"
#include "features/stronghold.h"
#include "loot/loot_tables.h"
#include "loot/mc_loot.h"
#include "loot/loot_table_parser.h"
#include "loot/cjson/cJSON.h"
#include "carver.h"

#include <math.h>
#include <stdlib.h>
#include <string.h>

static int structure_type(uint32_t feature, int dimension) {
    static const int types[] = {
        Village, Outpost, Shipwreck, Monument, Mansion, Ancient_City,
        Trail_Ruins, Trial_Chambers, Ruined_Portal, Stronghold, -1,
        Desert_Pyramid, Jungle_Temple, Swamp_Hut, Igloo, Ocean_Ruin,
        Treasure, Mineshaft, Desert_Well, Geode, Fortress, Bastion,
        End_City, End_Gateway, Abandoned_Camp, Nether_Fossil,
    };
    for (unsigned int i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
        if (feature == (UINT32_C(1) << i)) {
            return i == 8 && dimension == DIM_NETHER ? Ruined_Portal_N : types[i];
        }
    }
    return -1;
}

static cJSON *position(int x, int y, int z) {
    cJSON *value = cJSON_CreateObject();
    cJSON_AddNumberToObject(value, "x", x);
    cJSON_AddNumberToObject(value, "y", y);
    cJSON_AddNumberToObject(value, "z", z);
    return value;
}

static int supports_pieces(int type, int mc) {
    if (mc < MC_1_13) return type == Stronghold && mc >= MC_1_8;
    switch (type) {
        case Desert_Pyramid: case Igloo: case Jungle_Temple: case Outpost:
        case Shipwreck: case Swamp_Hut: case Fortress: case Bastion:
        case End_City: case Stronghold: case Abandoned_Camp: case Treasure:
        case Ruined_Portal: case Ruined_Portal_N: return 1;
        default: return 0;
    }
}

static void add_loot(cJSON *result, const Piece *piece, int chest, int mc) {
    LootTableContext *context = NULL;
    const char *table = piece->lootTables[chest];
    int supported = table && init_loot_table_name(&context, table, mc);
    cJSON_ReplaceItemInObject(result, "supported", cJSON_CreateBool(supported));
    cJSON *items = cJSON_AddArrayToObject(result, "items");
    cJSON *unresolved = cJSON_AddArrayToObject(result, "unresolvedTables");
    if (!supported || !context) return;
    set_loot_seed(context, piece->lootSeeds[chest]);
    generate_loot(context);
    for (int i = 0; i < context->generated_item_count && i < 27; i++) {
        const ItemStack stack = context->generated_items[i];
        if (stack.count <= 0) continue;
        const char *name = get_item_name(context, stack.item);
        if (!name) continue;
        cJSON *item = cJSON_CreateObject();
        cJSON_AddStringToObject(item, "id", name);
        cJSON_AddNumberToObject(item, "count", stack.count);
        cJSON *enchantments = cJSON_AddArrayToObject(item, "enchantments");
        for (int e = 0; e < stack.enchantment_count && e < 16; e++) {
            cJSON *enchantment = cJSON_CreateObject();
            const char *name = get_enchantment_name(stack.enchantments[e].enchantment);
            cJSON_AddStringToObject(enchantment, "id", name ? name : "unknown");
            cJSON_AddNumberToObject(enchantment, "level", stack.enchantments[e].level);
            cJSON_AddItemToArray(enchantments, enchantment);
        }
        if (stack.mob_effect.duration > 0 && stack.mob_effect.effect >= 0 && stack.mob_effect.effect < EFFECT_NUM) {
            cJSON_AddStringToObject(item, "effect", MOB_EFFECTS[stack.mob_effect.effect].effect_name);
            cJSON_AddNumberToObject(item, "duration", stack.mob_effect.duration);
        }
        cJSON_AddItemToArray(items, item);
    }
    for (int i = 0; i < context->unresolved_subtable_count && i < 8; i++) {
        cJSON_AddItemToArray(unresolved, cJSON_CreateString(context->unresolved_subtable_names[i]));
    }
}

char *axolotl_seed_map_structure_details(
    uint64_t seed, int32_t mc, int32_t flags, int32_t dimension,
    int32_t x, int32_t z, uint32_t feature, int32_t container
) {
    cJSON *result = cJSON_CreateObject();
    cJSON_AddBoolToObject(result, "supported", 0);
    cJSON_AddBoolToObject(result, "truncated", 0);
    cJSON *pieces = cJSON_AddArrayToObject(result, "pieces");
    cJSON *containers = cJSON_AddArrayToObject(result, "containers");
    cJSON *limits = cJSON_AddArrayToObject(result, "limits");
    cJSON *capabilities = cJSON_AddObjectToObject(result, "capabilities");
    cJSON_AddBoolToObject(capabilities, "variant", 0);
    cJSON_AddBoolToObject(capabilities, "containers", 0);
    int type = structure_type(feature, dimension);
    if (type < 0) goto finish;
    Generator g;
    setupGenerator(&g, mc, flags);
    applySeed(&g, dimension, seed);
    int biome = getBiomeAt(&g, 4, x >> 2, dimension == DIM_OVERWORLD ? 80 : 16, z >> 2);
    StructureVariant variant = {0};
    int has_variant = getVariant(&variant, type, mc, seed, x, z, biome);
    if (has_variant && type == Nether_Fossil) {
        BlendedNoise terrain;
        initBlendedNoise(&terrain, seed, DIM_NETHER);
        has_variant = isViableNetherFossilTerrain(x >> 4, z >> 4, &variant, &terrain, mc);
    }
    if (has_variant) {
        cJSON_ReplaceItemInObject(capabilities, "variant", cJSON_CreateBool(1));
        cJSON *value = cJSON_AddObjectToObject(result, "variant");
        cJSON_AddNumberToObject(value, "rotation", variant.rotation * 90);
        cJSON_AddNumberToObject(value, "mirror", variant.mirror);
        cJSON_AddNumberToObject(value, "start", variant.start);
        cJSON_AddNumberToObject(value, "biome", variant.biome);
        cJSON_AddBoolToObject(value, "abandoned", variant.abandoned);
        cJSON_AddBoolToObject(value, "giant", variant.giant);
        cJSON_AddBoolToObject(value, "underground", variant.underground);
        cJSON_AddBoolToObject(value, "airPocket", variant.airpocket);
        cJSON_AddBoolToObject(value, "basement", variant.basement);
        cJSON_AddBoolToObject(value, "cracked", variant.cracked);
        cJSON_AddNumberToObject(value, "size", variant.size);
        cJSON_AddItemToObject(value, "offset", position(variant.x, variant.y, variant.z));
        cJSON_AddItemToObject(value, "size3d", position(variant.sx, variant.sy, variant.sz));
        cJSON_ReplaceItemInObject(result, "supported", cJSON_CreateBool(1));
    }
    if (!supports_pieces(type, mc)) {
        cJSON_AddItemToArray(limits, cJSON_CreateString("layout_unavailable"));
        goto finish;
    }
    StructureSaltConfig salt = {0};
    int has_salt = mc >= MC_1_13 && getStructureSaltConfig(type, mc, has_variant && mc >= MC_1_18 ? variant.biome : biome, &salt);
    cJSON_ReplaceItemInObject(capabilities, "containers", cJSON_CreateBool(has_salt));
    Piece *list = calloc(2048, sizeof(Piece));
    if (!list) goto finish;
    int count;
    if (type == Stronghold && !has_salt) count = getStrongholdPieces(list, 2048, mc, seed, x >> 4, z >> 4);
    else count = getStructurePieces(list, 2048, type, salt, &variant, mc, seed, x, z);
    if (count < 0) count = 0;
    if (count >= 2048) cJSON_ReplaceItemInObject(result, "truncated", cJSON_CreateBool(1));
    if (count > 2048) count = 2048;
    cJSON_ReplaceItemInObject(result, "supported", cJSON_CreateBool(has_variant || count > 0));
    cJSON_AddItemToArray(limits, cJSON_CreateString("terrain_unverified"));
    if (type == Stronghold || type == Fortress || type == End_City) {
        cJSON_AddItemToArray(limits, cJSON_CreateString("relative_height"));
    }
    if (type == Stronghold || type == Outpost || type == Desert_Pyramid || type == Jungle_Temple || type == Swamp_Hut || type == Treasure || type == Ruined_Portal || type == Ruined_Portal_N || type == Igloo || type == Shipwreck) {
        cJSON_AddItemToArray(limits, cJSON_CreateString("height_placeholder"));
    }
    if (type == Ruined_Portal || type == Ruined_Portal_N) cJSON_AddItemToArray(limits, cJSON_CreateString("container_positions_approximate"));
    if ((type == Abandoned_Camp || type == Bastion)) cJSON_AddItemToArray(limits, cJSON_CreateString("piece_positions_unavailable"));
    if (type == Outpost || type == Bastion || type == Ruined_Portal || type == Ruined_Portal_N) {
        cJSON_AddItemToArray(limits, cJSON_CreateString("partial_layout"));
    }
    for (int i = 0; i < count; i++) {
        Piece *piece = &list[i];
        if (type == Shipwreck) {
            piece->bb0.x += piece->pos.x + (piece->bb0.x < 0);
            piece->bb0.y += piece->pos.y;
            piece->bb0.z += piece->pos.z + (piece->bb0.z < 0);
            piece->bb1.x += piece->pos.x - (piece->bb1.x > 0);
            piece->bb1.y += piece->pos.y - 1;
            piece->bb1.z += piece->pos.z - (piece->bb1.z > 0);
        }
        if (container >= 0 && container / 4 == i && container % 4 < piece->chestCount && has_salt) {
            add_loot(result, piece, container % 4, mc);
        }
        if (container >= 0) continue;
        cJSON *value = cJSON_CreateObject();
        cJSON_AddNumberToObject(value, "id", i);
        cJSON_AddStringToObject(value, "name", piece->name ? piece->name : "unknown");
        cJSON_AddBoolToObject(value, "endPortalRoom", type == Stronghold && piece->type == SH_PORTAL_ROOM);
        cJSON_AddNumberToObject(value, "rotation", (type == Abandoned_Camp || type == Bastion || type == Shipwreck || type == Igloo) ? variant.rotation * 90 : piece->rot * 90);
        cJSON_AddBoolToObject(value, "positionKnown", type != Abandoned_Camp && type != Bastion);
        cJSON_AddItemToObject(value, "position", position(piece->pos.x, piece->pos.y, piece->pos.z));
        if (piece->bb1.x > piece->bb0.x || piece->bb1.z > piece->bb0.z) {
            cJSON_AddItemToObject(value, "min", position(piece->bb0.x, piece->bb0.y, piece->bb0.z));
            cJSON_AddItemToObject(value, "max", position(piece->bb1.x, piece->bb1.y, piece->bb1.z));
        }
        if (type == Stronghold && piece->type == SH_PORTAL_ROOM && has_salt) {
            unsigned int eyes = (unsigned int)piece->additionalData & 4095;
            int eye_count = 0;
            while (eyes) { eye_count += eyes & 1; eyes >>= 1; }
            cJSON_AddNumberToObject(value, "portalEyes", eye_count);
        }
        cJSON_AddItemToArray(pieces, value);
        for (int chest = 0; chest < piece->chestCount && chest < 4; chest++) {
            if (!has_salt || !piece->lootTables[chest]) continue;
            cJSON *value = cJSON_CreateObject();
            cJSON_AddNumberToObject(value, "id", i * 4 + chest);
            cJSON_AddNumberToObject(value, "piece", i);
            cJSON_AddNumberToObject(value, "x", piece->chestPoses[chest].x);
            cJSON_AddNumberToObject(value, "z", piece->chestPoses[chest].z);
            cJSON_AddStringToObject(value, "table", piece->lootTables[chest]);
            cJSON_AddItemToArray(containers, value);
        }
    }
    free(list);
finish:
    {
        char *json = cJSON_PrintUnformatted(result);
        cJSON_Delete(result);
        return json;
    }
}

void axolotl_seed_map_free_result(char *result) { cJSON_free(result); }

char *axolotl_seed_map_generate_loot(
    const char *table_json, size_t table_len, uint64_t seed, int32_t mc
) {
    if (!table_json || !table_len || table_len > 1024 * 1024) return NULL;
    LootTableContext context = {0};
    char *copy = malloc(table_len + 1);
    if (!copy) return NULL;
    memcpy(copy, table_json, table_len);
    copy[table_len] = '\0';
    if (init_loot_table(copy, &context, mc) != 0) {
        free(copy);
        return NULL;
    }
    free(copy);
    set_loot_seed(&context, seed);
    generate_loot(&context);

    cJSON *result = cJSON_CreateObject();
    cJSON_AddBoolToObject(result, "supported", 1);
    cJSON_AddBoolToObject(result, "estimated", 1);
    cJSON_AddBoolToObject(result, "truncated", context.generated_item_count > 27);
    cJSON *items = cJSON_AddArrayToObject(result, "items");
    cJSON *unresolved = cJSON_AddArrayToObject(result, "unresolvedTables");
    cJSON_AddItemToArray(
        cJSON_AddArrayToObject(result, "limits"),
        cJSON_CreateString("loot_seed_estimated"));
    for (int i = 0; i < context.generated_item_count && i < 27; i++) {
        const ItemStack stack = context.generated_items[i];
        const char *name = get_item_name(&context, stack.item);
        if (stack.count <= 0 || !name) continue;
        cJSON *item = cJSON_CreateObject();
        cJSON_AddStringToObject(item, "id", name);
        cJSON_AddNumberToObject(item, "count", stack.count);
        cJSON *enchantments = cJSON_AddArrayToObject(item, "enchantments");
        for (int e = 0; e < stack.enchantment_count && e < 16; e++) {
            cJSON *enchantment = cJSON_CreateObject();
            const char *enchantment_name =
                get_enchantment_name(stack.enchantments[e].enchantment);
            cJSON_AddStringToObject(
                enchantment, "id", enchantment_name ? enchantment_name : "unknown");
            cJSON_AddNumberToObject(enchantment, "level", stack.enchantments[e].level);
            cJSON_AddItemToArray(enchantments, enchantment);
        }
        if (stack.mob_effect.duration > 0 && stack.mob_effect.effect >= 0 &&
            stack.mob_effect.effect < EFFECT_NUM) {
            cJSON_AddStringToObject(
                item, "effect", MOB_EFFECTS[stack.mob_effect.effect].effect_name);
            cJSON_AddNumberToObject(item, "duration", stack.mob_effect.duration);
        }
        cJSON_AddItemToArray(items, item);
    }
    for (int i = 0; i < context.unresolved_subtable_count && i < 8; i++) {
        cJSON_AddItemToArray(
            unresolved,
            cJSON_CreateString(context.unresolved_subtable_names[i]));
    }
    free_loot_table(&context);
    char *json = cJSON_PrintUnformatted(result);
    cJSON_Delete(result);
    return json;
}

int32_t axolotl_seed_map_underground(
    uint64_t seed, int32_t mc, int32_t flags, int32_t dimension,
    int32_t x, int32_t z, int32_t scale, int32_t width, int32_t height,
    int32_t y, uint32_t layers, uint8_t *rgba, uint64_t length
) {
    if (mc < MC_1_13 || ((layers & 1) && mc < MC_1_14) || (scale != 1 && scale != 4) || width <= 0 || height <= 0 || width > 64 || height > 64 ||
        width % 16 || height % 16 ||
        width * scale > 256 || height * scale > 256 || length != (uint64_t)width * height * 4 ||
        x % 16 || z % 16) return -1;
    TerrainNoise *terrain = calloc(1, sizeof(TerrainNoise));
    if (!terrain) return -2;
    if (layers & 1) {
        setupTerrainNoise(terrain, mc, flags);
        initTerrainNoise(terrain, seed, dimension);
    } else {
        setupGenerator(&terrain->g, mc, flags);
        applySeed(&terrain->g, dimension, seed);
    }
    const int min_y = dimension == DIM_OVERWORLD && mc >= MC_1_18 ? -64 : 0;
    const int max_y = dimension == DIM_OVERWORLD ? (mc >= MC_1_18 ? 319 : 255) : 127;
    if (y < min_y || y > max_y) { free(terrain); return -3; }
    const int cell_height = dimension == DIM_END ? 4 : 8;
    const int column = (y - min_y) / cell_height;
    const int world_width = width * scale;
    const int world_height = height * scale;
    uint8_t *blocks = calloc((size_t)world_width * world_height, cell_height);
    if (!blocks) { free(terrain); return -2; }
    if (layers & 1) generateRegion(terrain, x / 16, z / 16, world_width / 16, world_height / 16,
        (uint8_t (*)[])blocks, column, column + 1, NULL, 0);
    memset(rgba, 0, length);
    for (int row = 0; row < height; row++) for (int col = 0; col < width; col++) {
        size_t pixel = ((size_t)row * width + col) * 4;
        int solid = blocks[((size_t)col * scale * world_height + row * scale) * cell_height + (y - min_y) % cell_height];
        if (layers & 1) {
            rgba[pixel] = solid ? 104 : 15;
            rgba[pixel + 1] = solid ? 107 : 27;
            rgba[pixel + 2] = solid ? 116 : 43;
            rgba[pixel + 3] = 255;
        }
    }
    free(blocks);
    if (dimension != DIM_END && (layers & 6)) {
        for (int cx = x / 16; cx < (x + world_width) / 16; cx++) for (int cz = z / 16; cz < (z + world_height) / 16; cz++) {
            int biomes[17][17];
            for (int rz = 0; rz < 17; rz++) for (int rx = 0; rx < 17; rx++)
                biomes[rz][rx] = getBiomeAt(&terrain->g, 4, (cx + rx - 8) * 4, 0, (cz + rz - 8) * 4);
            for (int layer = 1; layer <= 2; layer++) {
                if (!(layers & (1U << layer))) continue;
                int types = layer == 1 ? CAVE_CARVER_NUM : CANYON_CARVER_NUM;
                for (int type = 0; type < types; type++) {
                    Pos3List positions;
                    createPos3List(&positions, 1024);
                    if (layer == 1) {
                        CaveCarverConfig config;
                        int biome = getBiomeAt(&terrain->g, 4, cx * 4, 0, cz * 4);
                        if (getCaveCarverConfig(type, mc, biome, &config) && config.dim == dimension)
                            carveCave(seed, mc, cx, cz, config, type, biomes, &positions);
                    } else {
                        CanyonCarverConfig config;
                        if (getCanyonCarverConfig(type, mc, &config) && config.dim == dimension)
                            carveCanyon(seed, mc, cx, cz, config, type, biomes, &positions);
                    }
                    for (int i = 0; i < positions.size; i++) {
                        const Pos3 p = positions.pos3s[i];
                        if (p.y != y || p.x < x || p.z < z || p.x >= x + world_width || p.z >= z + world_height) continue;
                        size_t pixel = ((size_t)((p.z - z) / scale) * width + (p.x - x) / scale) * 4;
                        rgba[pixel] = layer == 1 ? 68 : 232;
                        rgba[pixel + 1] = layer == 1 ? 170 : 151;
                        rgba[pixel + 2] = layer == 1 ? 214 : 66;
                        rgba[pixel + 3] = 255;
                    }
                    freePos3List(&positions);
                }
            }
        }
    }
    free(terrain);
    return 0;
}
