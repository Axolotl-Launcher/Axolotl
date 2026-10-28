/* lysh column_top.h —— 列顶 Y = 26.1.2 的 `preliminarySurfaceLevel`
 * （= 1.18 字节码里的 `method_39900`）。
 *
 * ⛔ 这里**不是** `sampleNoiseColumn` 的密度扫描。旧实现
 *    （`NoiseColumnSampler.method_38383`：沿整列按 vbs=8 扫
 *    `sampleNoiseColumn(...) > 0.390625`）是错的：它的输入
 *    (`t.offset/t.factor`, 全列地形密度) 与真实函数无关。
 *
 * ✅ 版本归属 / 基线有效性（**不要再重新论证**）：
 *    1.18.2 的 `NoiseColumnSampler.method_38383` 与 26.1.2 的
 *    `NoiseChunk.preliminarySurfaceLevel(x,z)` 在 `columntop_dump.txt` 的
 *    **400/400 个点上给出完全相同的整数**（实测：用真 NoiseChunk
 *    （每个点 fresh，见 `_archive/dev_scratch_20260921.zip -> test/PsBaseline261.java` + `dump_261.txt`）
 *    对拍 1.18.2 那一列，identical=400 / different=0）。
 *    所以 `columntop_dump.txt` 是**有效基线**，diff 它没有版本错配问题。
 *    （曾有人断言两者是不同量、dump 必须作废 —— 那是错的。）
 *
 * 权威来源：`_archive/dev_scratch_20260921.zip -> mcdata_jar/26.1.2/noise_settings/overworld.json` 的
 *           `noise_router.preliminary_surface_level`（`minecraft:find_top_surface`,
 *           cell_height=8, lower_bound=-64），以及 `javap -p -c` 的
 *           `DensityFunctions$FindTopSurface`：
 *
 *   double compute(ctx) {
 *       int y = Mth.floor(upperBound.compute(ctx) / cellHeight) * cellHeight;
 *       if (y <= lowerBound) return lowerBound;
 *       int i = y;
 *       while (i >= lowerBound) {
 *           if (density.compute(SinglePointContext(ctx.blockX(), i, ctx.blockZ())) > 0.0)
 *               return i;
 *           i -= cellHeight;
 *       }
 *       return lowerBound;
 *   }
 *
 * 展开后的表达式（见 column_top.c；`blend_alpha`=1.0、`blend_offset`=0.0）：
 *   offset     = spline_offset(c,e,w) - 0.5037500262260437
 *   factor     = spline_factor(c,e,w)          （float）
 *   density(y) = -0.390625 + (0.1171875 + ygrad(-64->-40 : 0->1)(y)
 *                 * (-0.1171875 + (-0.078125 + ygrad(240->256 : 1->0)(y)
 *                   * (0.078125 + clamp(-0.703125 + cheese, -64, 64)))))
 *   cheese     = 4.0 * quarter_negative((ygrad(-64->320 : 1.5->-1.5)(y) + offset) * factor)
 *   upperBound = clamp(128.0 + 128.0*offset - 35.0/factor, -40.0, 320.0)
 *
 *   坐标：喂给样条的是 **biome 坐标 = block >> 2**。26.1.2 的密度函数路径
 *   `ShiftedNoise.compute` 是 `noise(block*0.25 + shift)`，其中 shift 自身也
 *   乘 0.25 —— 令 biome = block>>2 后与 1.18 `NoiseColumnSampler` 的
 *   `arg + shiftNoise(arg)*4` 逐位一致。实测：C 在 `block>>2` 处算出的
 *   continentalness/erosion/ridges 与 oracle 密度函数在 block 坐标处的值
 *   差 ≤ 2.5e-8（纯 double→float 舍入）。
 *
 * ⚠️ `quarter_negative` 的方向：`DensityFunctions.quarterNegative(v)` =
 *    **`v > 0 ? v : v * 0.25`**（把**负**的一半除以 4，名字就是这个意思）。
 *    因为外面还乘 `4.0`，展开即经典 1.18 形式 `v > 0 ? 4v : v`
 *    （与 src/density.c 一致）。写反会把两条 y 带的斜率对调、
 *    列顶整体偏低 24 → bad=400。
 *
 * ⚠️ 26.1.2 的 `preliminary_surface_level` 里**没有** `jaggedness * half_negative(jag)`
 *    那一项（那是 `sloped_cheese` 的写法）；jaggedness 是独立的路由。
 *    权威 JSON 直读如此，见 _archive/verify_tools_20260921.zip -> cmp_spline_json.ps1 的核对。
 */
#ifndef LYSH_COLUMN_TOP_H
#define LYSH_COLUMN_TOP_H

#include "density.h"
#include "terrain.h"

/* `preliminarySurfaceLevel` 的 cell 高度 / 下界（方块坐标） */
#define LYSH_COLUMN_TOP_CELL_HEIGHT 8
#define LYSH_COLUMN_TOP_LOWER_BOUND (-64)

/* 26.1.2: NoiseColumnSampler.method_39900(int blockX, int blockZ)（flat 世界之外）。
 * 返回 [lower_bound, upper_bound] 内的 8 的倍数；无命中返回 lower_bound (-64)。 */
int lysh_preliminary_surface_level(const lysh_terrain *t, int block_x, int block_z);

/* `minecraft:overworld/offset` 密度函数的 **double** 值（含 -0.5037500262260437 常量）。
 * 含水层的 deep-dark 判定要用它组 `depth = y_clamped_gradient(y) + offset`
 * （见 src/aquifer.c 的 is_deep_dark_region）。 */
double lysh_overworld_offset(const lysh_terrain *t, int block_x, int block_z);

#endif /* LYSH_COLUMN_TOP_H */
