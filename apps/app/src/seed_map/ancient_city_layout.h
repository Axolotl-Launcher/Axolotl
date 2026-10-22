#ifndef AXOLOTL_ANCIENT_CITY_LAYOUT_H
#define AXOLOTL_ANCIENT_CITY_LAYOUT_H

#include <stdint.h>
#include <stddef.h>

char *axolotl_seed_map_ancient_city_layout(
    uint64_t seed,
    int32_t mc,
    int32_t x,
    int32_t z,
    const char *layout_json,
    size_t layout_len
);

#endif
