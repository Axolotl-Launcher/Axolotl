#include "ancient_city_layout.h"

#include "finders.h"
#include "loot/cjson/cJSON.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define ANCIENT_CITY_MAX_PIECES 512
#define ANCIENT_CITY_MAX_DEPTH 7

typedef struct {
    cJSON *template;
    int x, y, z;
    int rotation;
    int depth;
    int minx, miny, minz, maxx, maxy, maxz;
} AncientPiece;

typedef struct {
    cJSON *layout;
    cJSON *templates;
    cJSON *pools;
    AncientPiece pieces[ANCIENT_CITY_MAX_PIECES];
    int count;
    int unsupported;
    uint64_t random;
} AncientContext;

static int next_int(AncientContext *ctx, int bound) {
    return bound <= 1 ? 0 : nextInt(&ctx->random, bound);
}

static cJSON *object(cJSON *parent, const char *name) {
    return cJSON_GetObjectItemCaseSensitive(parent, name);
}

static int number(cJSON *parent, const char *name, int fallback) {
    cJSON *value = object(parent, name);
    return cJSON_IsNumber(value) ? value->valueint : fallback;
}

static const char *string(cJSON *parent, const char *name) {
    cJSON *value = object(parent, name);
    return cJSON_IsString(value) ? value->valuestring : NULL;
}

static cJSON *template_by_id(AncientContext *ctx, const char *id);
static cJSON *matching_jigsaw(cJSON *template, const char *name, int *index);

static void add_number(cJSON *array, int value) {
    cJSON_AddItemToArray(array, cJSON_CreateNumber(value));
}

static int direction(const char *orientation) {
    if (!orientation) return 0;
    if (!strncmp(orientation, "north", 5)) return 0;
    if (!strncmp(orientation, "east", 4)) return 1;
    if (!strncmp(orientation, "south", 5)) return 2;
    if (!strncmp(orientation, "west", 4)) return 3;
    return 0;
}

static int opposite(int value) { return (value + 2) & 3; }

static void rotate(int x, int z, int rotation, int *out_x, int *out_z) {
    switch ((rotation / 90) & 3) {
    case 0: *out_x = x; *out_z = z; break;
    case 1: *out_x = -z; *out_z = x; break;
    case 2: *out_x = -x; *out_z = -z; break;
    default: *out_x = z; *out_z = -x; break;
    }
}

static int rotated_direction(int value, int rotation) {
    return (value + ((rotation / 90) & 3)) & 3;
}

static cJSON *template_by_id(AncientContext *ctx, const char *id) {
    return id ? object(ctx->templates, id) : NULL;
}

static void bounds(AncientPiece *piece) {
    cJSON *size = object(piece->template, "size");
    int sx = size && cJSON_IsArray(size) ? cJSON_GetArrayItem(size, 0)->valueint : 0;
    int sz = size && cJSON_IsArray(size) ? cJSON_GetArrayItem(size, 2)->valueint : 0;
    int sy = size && cJSON_IsArray(size) ? cJSON_GetArrayItem(size, 1)->valueint : 0;
    int corners[4][2] = {{0, 0}, {sx - 1, 0}, {0, sz - 1}, {sx - 1, sz - 1}};
    piece->minx = piece->minz = INT32_MAX;
    piece->maxx = piece->maxz = INT32_MIN;
    for (int i = 0; i < 4; i++) {
        int rx, rz;
        rotate(corners[i][0], corners[i][1], piece->rotation, &rx, &rz);
        if (piece->x + rx < piece->minx) piece->minx = piece->x + rx;
        if (piece->x + rx > piece->maxx) piece->maxx = piece->x + rx;
        if (piece->z + rz < piece->minz) piece->minz = piece->z + rz;
        if (piece->z + rz > piece->maxz) piece->maxz = piece->z + rz;
    }
    piece->miny = piece->y;
    piece->maxy = piece->y + sy - 1;
}

static int intersects(const AncientPiece *a, const AncientPiece *b) {
    return a->minx < b->maxx && a->maxx > b->minx &&
        a->miny < b->maxy && a->maxy > b->miny &&
        a->minz < b->maxz && a->maxz > b->minz;
}

static int element_matches_target(AncientContext *ctx, cJSON *element, const char *target) {
    if (!element || !target) return 0;
    const char *type = string(element, "element_type");
    if (strcmp(type ? type : "", "minecraft:single_pool_element")) return 0;
    cJSON *template = template_by_id(ctx, string(element, "location"));
    return matching_jigsaw(template, target, NULL) != NULL;
}

static void add_pool_candidate(
    cJSON **candidates,
    int *weights,
    int *count,
    int *total,
    cJSON *element,
    int weight
) {
    if (*count >= 128) return;
    const int normalized_weight = weight > 0 ? weight : 1;
    candidates[*count] = element;
    weights[*count] = normalized_weight;
    (*count)++;
    *total += normalized_weight;
}

static cJSON *pool_element(AncientContext *ctx, cJSON *pool, const char *target) {
    cJSON *elements = object(pool, "elements");
    if (!cJSON_IsArray(elements)) return NULL;
    cJSON *candidates[128];
    int weights[128];
    int count = 0;
    int total = 0;
    cJSON *entry;
    cJSON_ArrayForEach(entry, elements) {
        cJSON *element = object(entry, "element");
        if (!element) continue;
        const char *type = string(element, "element_type");
        int weight = number(entry, "weight", 1);
        if (!strcmp(type ? type : "", "minecraft:empty_pool_element")) {
            add_pool_candidate(candidates, weights, &count, &total, element, weight);
        } else if (!strcmp(type ? type : "", "minecraft:list_pool_element")) {
            cJSON *nested = object(element, "elements");
            if (!cJSON_IsArray(nested)) { ctx->unsupported = 1; continue; }
            cJSON *nested_element;
            cJSON_ArrayForEach(nested_element, nested) {
                if (element_matches_target(ctx, nested_element, target) && count < 128) {
                    add_pool_candidate(
                        candidates, weights, &count, &total, nested_element, weight);
                }
            }
        } else if (!strcmp(type ? type : "", "minecraft:single_pool_element")) {
            if (element_matches_target(ctx, element, target) && count < 128) {
                add_pool_candidate(candidates, weights, &count, &total, element, weight);
            }
        } else if (!strcmp(type ? type : "", "minecraft:feature_pool_element")) {
            ctx->unsupported = 1;
        }
    }
    if (!count || total <= 0) return NULL;
    int selected = next_int(ctx, total);
    for (int i = 0; i < count; i++) {
        if ((selected -= weights[i]) < 0) return candidates[i];
    }
    return candidates[count - 1];
}

static cJSON *matching_jigsaw(cJSON *template, const char *name, int *index) {
    cJSON *jigsaws = object(template, "jigsaws");
    if (!cJSON_IsArray(jigsaws)) return NULL;
    cJSON *jigsaw;
    int i = 0;
    cJSON_ArrayForEach(jigsaw, jigsaws) {
        if (name && string(jigsaw, "name") && !strcmp(name, string(jigsaw, "name"))) {
            if (index) *index = i;
            return jigsaw;
        }
        i++;
    }
    return NULL;
}

static void expand_from(AncientContext *ctx, int parent_index) {
    AncientPiece parent = ctx->pieces[parent_index];
    cJSON *jigsaws = object(parent.template, "jigsaws");
    if (!cJSON_IsArray(jigsaws) || parent.depth >= ANCIENT_CITY_MAX_DEPTH) return;
    cJSON *jigsaw;
    cJSON_ArrayForEach(jigsaw, jigsaws) {
        const char *pool_id = string(jigsaw, "pool");
        const char *target = string(jigsaw, "target");
        if (!pool_id || !strcmp(pool_id, "minecraft:empty") || !target) continue;
        cJSON *pool = object(ctx->pools, pool_id);
        cJSON *element = pool_element(ctx, pool, target);
        if (element && !strcmp(string(element, "element_type") ? string(element, "element_type") : "", "minecraft:empty_pool_element")) continue;
        const char *location = element ? string(element, "location") : NULL;
        cJSON *child_template = template_by_id(ctx, location);
        if (!child_template) { ctx->unsupported = 1; continue; }
        cJSON *child_jigsaw = matching_jigsaw(child_template, target, NULL);
        if (!child_jigsaw) { ctx->unsupported = 1; continue; }
        cJSON *parent_pos = object(jigsaw, "position");
        cJSON *child_pos = object(child_jigsaw, "position");
        if (!cJSON_IsArray(parent_pos) || !cJSON_IsArray(child_pos)) continue;
        int px = cJSON_GetArrayItem(parent_pos, 0)->valueint;
        int py = cJSON_GetArrayItem(parent_pos, 1)->valueint;
        int pz = cJSON_GetArrayItem(parent_pos, 2)->valueint;
        int cx = cJSON_GetArrayItem(child_pos, 0)->valueint;
        int cy = cJSON_GetArrayItem(child_pos, 1)->valueint;
        int cz = cJSON_GetArrayItem(child_pos, 2)->valueint;
        int parent_rx, parent_rz;
        rotate(px, pz, parent.rotation, &parent_rx, &parent_rz);
        int parent_direction = rotated_direction(direction(string(jigsaw, "orientation")), parent.rotation);
        int child_rotation = parent.rotation;
        int child_direction = direction(string(child_jigsaw, "orientation"));
        for (int rotation = 0; rotation < 360; rotation += 90) {
            if (rotated_direction(child_direction, rotation) == opposite(parent_direction)) {
                child_rotation = rotation;
                break;
            }
        }
        int child_rx, child_rz;
        rotate(cx, cz, child_rotation, &child_rx, &child_rz);
        if (ctx->count >= ANCIENT_CITY_MAX_PIECES) { ctx->unsupported = 1; return; }
        AncientPiece child = {
            .template = child_template,
            .x = parent.x + parent_rx - child_rx,
            .y = parent.y + py - cy,
            .z = parent.z + parent_rz - child_rz,
            .rotation = child_rotation,
            .depth = parent.depth + 1,
        };
        bounds(&child);
        int collision = 0;
        for (int i = 0; i < ctx->count; i++) {
            if (i != parent_index && intersects(&child, &ctx->pieces[i])) { collision = 1; break; }
        }
        if (collision) continue;
        ctx->pieces[ctx->count] = child;
        int child_index = ctx->count++;
        expand_from(ctx, child_index);
    }
}

char *axolotl_seed_map_ancient_city_layout(
    uint64_t seed, int32_t mc, int32_t x, int32_t z,
    const char *layout_json, size_t layout_len
) {
    if (!layout_json || !layout_len || mc < MC_1_19_2) return NULL;
    char *copy = malloc(layout_len + 1);
    if (!copy) return NULL;
    memcpy(copy, layout_json, layout_len);
    copy[layout_len] = 0;
    cJSON *layout = cJSON_ParseWithLength(copy, layout_len);
    free(copy);
    if (!layout) return NULL;
    AncientContext ctx = {.layout = layout, .templates = object(layout, "templates"), .pools = object(layout, "pools"), .random = chunkGenerateRnd(seed, x >> 4, z >> 4)};
    if (!ctx.templates || !ctx.pools) { cJSON_Delete(layout); return NULL; }
    StructureVariant variant;
    if (!getVariant(&variant, Ancient_City, mc, seed, x, z, -1)) { cJSON_Delete(layout); return NULL; }
    next_int(&ctx, 4);
    next_int(&ctx, 3);
    char start_id[128];
    snprintf(start_id, sizeof(start_id), "minecraft:ancient_city/city_center/city_center_%d", variant.start);
    cJSON *start = template_by_id(&ctx, start_id);
    if (!start) { cJSON_Delete(layout); return NULL; }
    AncientPiece root = {
        .template = start,
        .x = (x & ~15) + variant.x,
        .y = variant.y,
        .z = (z & ~15) + variant.z,
        .rotation = variant.rotation * 90,
        .depth = 0,
    };
    bounds(&root);
    ctx.pieces[ctx.count++] = root;
    expand_from(&ctx, 0);
    cJSON *result = cJSON_CreateObject();
    cJSON_AddBoolToObject(result, "supported", 1);
    cJSON_AddBoolToObject(result, "complete", !ctx.unsupported);
    cJSON_AddNumberToObject(result, "rotation", variant.rotation * 90);
    cJSON_AddNumberToObject(result, "start", variant.start);
    cJSON_AddItemToObject(result, "origin", cJSON_CreateArray());
    add_number(object(result, "origin"), root.x);
    add_number(object(result, "origin"), variant.y);
    add_number(object(result, "origin"), root.z);
    cJSON *pieces = cJSON_AddArrayToObject(result, "pieces");
    for (int i = 0; i < ctx.count; i++) {
        AncientPiece *piece = &ctx.pieces[i];
        cJSON *value = cJSON_CreateObject();
        cJSON_AddNumberToObject(value, "id", i);
        cJSON_AddStringToObject(value, "templateId", string(piece->template, "id") ? string(piece->template, "id") : "");
        cJSON_AddStringToObject(value, "name", string(piece->template, "id") ? string(piece->template, "id") : "unknown");
        cJSON_AddNumberToObject(value, "rotation", piece->rotation);
        cJSON *position = cJSON_AddArrayToObject(value, "position");
        add_number(position, piece->x); add_number(position, piece->y); add_number(position, piece->z);
        cJSON *min = cJSON_AddArrayToObject(value, "min");
        add_number(min, piece->minx); add_number(min, piece->miny); add_number(min, piece->minz);
        cJSON *max = cJSON_AddArrayToObject(value, "max");
        add_number(max, piece->maxx); add_number(max, piece->maxy); add_number(max, piece->maxz);
        cJSON_AddItemToArray(pieces, value);
    }
    if (ctx.unsupported) cJSON_AddItemToObject(result, "limits", cJSON_CreateStringArray((const char *[]){"jigsaw_pool_or_template_unsupported"}, 1));
    else cJSON_AddItemToObject(result, "limits", cJSON_CreateArray());
    char *json = cJSON_PrintUnformatted(result);
    cJSON_Delete(result); cJSON_Delete(layout);
    return json;
}
