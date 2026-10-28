/* ⛔ 版本归属（见 README §0）：本文件实现的是 **1.18 的地形语义**，默认无效。
 * 只有显式选择版本 `1.18.2` 时才作为该分支的实现保留。
 * 1.21+ 的权威是客户端 jar 的 worldgen JSON：`overworld/depth.json`、
 * `overworld/factor.json`、`overworld/sloped_cheese.json`、
 * overworld/caves 目录下的若干 json，以及 `noise_settings/overworld.json` 的 `final_density` 树。
 */

/* lysh density.h —— sampleNoiseColumn：完整的 final_density（不含含水层）。
 *
 * 取证：`_archive/dev_scratch_20260921.zip -> MC_NoiseColumnSampler.txt:620-834`，伪码见规格 §2.1/§2.2。
 *
 * overworld 参数（与 vanilla JSON 一致）：
 *   verticalBlockSize  = 8
 *   minimumBlockY      = -8   ← **cell 坐标**，不是方块坐标 -64
 *   verticalBlockCount = 48
 *   topSlide    = (target -0.078125, size 2, offset 8)
 *   bottomSlide = (target  0.1171875, size 3, offset 0)
 *
 * 三个最容易错的点：
 *  1. `depthAt(y) = 1.0 - y / 128.0 + t.offset()`（y 是方块坐标，除以 128.0 是 double）。
 *  2. `noiseGradientDensity`：`f = d * factor; f > 0 ? f*4 : f` —— 正负分支不同乘数。
 *  3. `applySlides(v, y / verticalBlockSize)` 里的 `/` 是 **int 除法（向零截断）**，
 *     不是 floorDiv。y=-63 时两者差 1，会改变 bottomSlide 的输入。
 */
#ifndef LYSH_DENSITY_H
#define LYSH_DENSITY_H

#include <stdint.h>

#include "noise.h"
#include "terrain.h"

typedef struct {
    int vertical_block_size;    /* 8  */
    int minimum_block_y;        /* -8（cell） */
    int vertical_block_count;   /* 48 */

    double top_target;      int top_size;    int top_offset;      /* -0.078125, 2, 8 */
    double bottom_target;   int bottom_size; int bottom_offset;   /*  0.1171875, 3, 0 */

    lysh_dblnoise jagged;
    lysh_dblnoise cave_entrance;
    lysh_dblnoise spaghetti_roughness;
    lysh_dblnoise spaghetti_roughness_modulator;
    lysh_dblnoise spaghetti_3d_1;
    lysh_dblnoise spaghetti_3d_2;
    lysh_dblnoise spaghetti_3d_rarity;
    lysh_dblnoise spaghetti_3d_thickness;
    lysh_dblnoise spaghetti_2d;
    lysh_dblnoise spaghetti_2d_elevation;
    lysh_dblnoise spaghetti_2d_modulator;
    lysh_dblnoise spaghetti_2d_thickness;
    lysh_dblnoise pillar;
    lysh_dblnoise pillar_rareness;
    lysh_dblnoise pillar_thickness;
    lysh_dblnoise cave_layer;
    lysh_dblnoise cave_cheese;
    /* 26.1.2 `overworld/caves/noodle`（在 `interpolated` **之外**，逐点求值，
     * 但它自己有 3 个 `interpolated` 子节点 —— 见 lysh_noodle_child） */
    lysh_dblnoise noodle;
    lysh_dblnoise noodle_thickness;
    lysh_dblnoise noodle_ridge_a;
    lysh_dblnoise noodle_ridge_b;
} lysh_density;

void lysh_density_init(lysh_density *d, uint64_t world_seed);

/* MC: NoiseColumnSampler.sampleNoiseColumn(int x, int y, int z, TerrainNoisePoint t,
 *        double baseNoise, boolean useJagged, boolean noNoiseCaves, Blender blender)
 * x/z 是方块坐标；baseNoise 由调用方给（method_38383 里恒为 -0.703125）。 */
double lysh_sample_noise_column(const lysh_density *d,
                                int x, int y, int z,
                                const lysh_terrain_point *t,
                                double base_noise,
                                int use_jagged, int no_noise_caves);

/* 单项，供分层对拍时定位 */
double lysh_jagged(const lysh_density *d, double peaks, int x, int z);
double lysh_cave_entrance_noise(const lysh_density *d, int x, int y, int z);
double lysh_spaghetti_roughness_noise(const lysh_density *d, int x, int y, int z);
double lysh_spaghetti_3d_noise(const lysh_density *d, int x, int y, int z);
double lysh_spaghetti_2d_noise(const lysh_density *d, int x, int y, int z);
double lysh_pillar_noise(const lysh_density *d, int x, int y, int z);
double lysh_cave_layer_noise(const lysh_density *d, int x, int y, int z);

/* ---- 供 L6 的 `method_38386` 组装用（规格 §1.2 的 d2 修正）----
 * 原来这里还导出 `lysh_spaghetti_2d_sample` / `lysh_spaghetti_2d_modulator_sample`
 * 供 `method_38386` 在外面单独采样 —— 那两个函数**没有任何调用方**（在产品路径和归档
 * 工具里都没有），已删除；`sampleNoiseColumn` 内部用的是 density.c 的私有 helper。 */
/* Squeeze（`method_38386` 的 d2 前半）：
 *   v = clamp(v * 0.64, -1.0, 1.0);  return v/2 - v*v*v/24      */
double lysh_squeeze(double v);

/* ---- 26.1.2 `overworld/caves/noodle` ----------------------------------
 * JSON（`_archive/dev_scratch_20260921.zip -> mcdata/26.1.2/density_function/overworld/caves/noodle.json`）：
 *   range_choice(input = interpolated(range_choice(y, -60<=y<=320, noodle_noise, -1)),
 *                -1000000, 0, when_in_range = 64.0,
 *                when_out_of_range = interpolated(range_choice(y, ..., -0.07500000000000001
 *                                                      - 0.025*noodle_thickness, 0))
 *                                  + 1.5 * max(|interpolated(.. ridge_a ..)|,
 *                                              |interpolated(.. ridge_b ..)|))
 *
 * 三个 `interpolated` 子节点由调用方做三线性插值；这里只提供**角点上的逐点值**
 * 与最后的组合（range_choice + add/max）。
 *   which = LYSH_NOODLE_N / _THICKNESS / _RIDGE_A / _RIDGE_B */
enum {
    LYSH_NOODLE_N = 0,
    LYSH_NOODLE_THICKNESS = 1,
    LYSH_NOODLE_RIDGE_A = 2,
    LYSH_NOODLE_RIDGE_B = 3
};

/* 角点（方块坐标）上的逐点值；y 越界时按 JSON 的 when_out_of_range 取值。 */
double lysh_noodle_child(const lysh_density *d, int which, int x, int y, int z);

/* 26.1.2 noodle 节点的最终求值（三个子节点已插值完）。 */
double lysh_noodle_combine(double n, double thickness, double ridge_a, double ridge_b);

#endif /* LYSH_DENSITY_H */
