/* lysh carver.h —— MC 26.1.2 的洞穴 / 峡谷雕刻器（`WorldCarver` 系）。
 *
 * 为什么需要它：女巫小屋的 Y 来自 `ScatteredFeaturePiece.updateAverageGroundHeight`，
 * 它在 FEATURES 阶段读 `getHeightmapPos(MOTION_BLOCKING_NO_LEAVES, pos)`，而 FEATURES 的
 * 第一件事就是 `Heightmap.primeHeightmaps(...)` —— **从活方块重量一遍**。雕刻器跑在更早的
 * CARVERS 阶段，所以小屋 Y 是**雕刻之后**的高度图。本模块就是补上这一步。
 *
 * 复现 `NoiseBasedChunkGenerator.applyCarvers` 的完整遍历（逐条来自 `_scratch/dump/`：
 * 大小与顺序都不是推测）：
 *   · 对**目标** chunk 建 **一个** CarvingMask（height 384 / minY -64），整个遍历共用；
 *   · `for (dx = -8; dx <= 8; dx++) for (dz = -8; dz <= 8; dz++)`（**dx 在外**，
 *     半径是**硬编码的 8**，不是 `getRange()`）；
 *   · 每个 origin 按群系声明顺序取 3 个雕刻器，`setLargeFeatureSeed(seed + i, ox, oz)`
 *     （26.1.2 没有 `setCarverSeed`，也没有 `| 1L`），再 `isStartChunk`；
 *   · **`carve` 的 ChunkPos 参数是 origin（邻居）**：洞穴/峡谷的中心就取在邻居 chunk 里
 *     （`CaveWorldCarver.carve` 的 `aload 7`）；而 `ChunkAccess` 参数恒为**目标** chunk，
 *     `WorldCarver.carveEllipsoid` 里所有写入都按**目标 chunk** 的 pos 夹到局部 0..15。
 *     ⇒ 邻居里的雕刻会啃到目标 chunk 的边缘，这正是本 bug 的成因。
 *   · mask 只按**局部**坐标索引（`(x&15)|((z&15)<<4)|((y-minY)<<8)`），所以 289 个 origin
 *     的 mask 位互相别名 —— 顺序因此是结果的一部分，必须逐字复现。
 *
 * 群系 → 雕刻器表：从 jar 里数过，**54 个主世界群系全部声明同一个列表**
 *   ["minecraft:cave", "minecraft:cave_extra_underground", "minecraft:canyon"]
 * （data/minecraft/worldgen/biome 下所有 json），所以这里直接写死三项 + 概率
 * 0.15 / 0.07 / 0.01（configured_carver 各 json 的 `probability`）。
 * 主世界路径不可能出现别的列表（下界/末地群系不会进 NoiseBasedChunkGenerator）。
 *
 * ⚠️ 版本口径（三层证据，2026-xx 复核）：
 *   · **播种与版本无关**：1.18.2 / 1.19.2 / 1.20 / 1.21 / 26.1.2 的
 *     `WorldgenRandom.setLargeFeatureSeed(JII)`（旧版混淆名 `cuv.c` / `dbo.c` /
 *     `dij.c` / `dzx.c`）是**逐条相同的字节码**：setSeed(seed); l=nextLong();
 *     m=nextLong(); setSeed(chunkX*l ^ chunkZ*m ^ seed)，**都没有 `| 1L`**；
 *     调用点也都是 `seed + carverIndex` + origin 的 ChunkPos，落在硬编码 -8..8 里。
 *   · **配置**：1.20 / 1.21 / 1.21.4 的官方 client jar 与 26.1.2 的 server jar 里的
 *     configured_carver json **逐字段相同**（0.15/0.07/0.01；y = above_bottom(8)..180 /
 *     ..47 / absolute 10..67；yScale 0.1..0.9；canyon 形状参数一致）。
 *   · ⚠️ **1.18.2 / 1.19.2 的 configured_carver 本地无法核对**（那两个版本的 client
 *     jar 不带 worldgen 数据，只有 server jar 里有，本机没有、也不能联网下载）。
 *     本实现因此对这两个版本沿用 26.1.2 的配置；cubiomes 的独立实现
 *     （`c_cave_118` / `c_cave_extra_underground_118` / `c_canyon_carver_118`）
 *     恰好是同一组参数，即"1.18 起未变"，但**这条不是官方 jar 证据**。
 *
 * 方块状态的近似（唯一一处不是逐字照抄的地方，见 carver.c 的 `can_replace`）：
 * 本模型只知道 4 种状态（实心 / 空气 / 水 / 岩浆），没有 `SurfaceSystem`。地表规则
 * 只在**类型**上改写（草/土/沙/砾…），不改变"实心↔非实心"的几何，而
 * `#minecraft:overworld_carver_replaceables` 恰好包含全部这些类型 ⇒ 几何与可替换性一致。
 * 例外只有冰 / 粘土 / 基岩（不在标签里，只出现在海面附近与最底层），远离小屋的 footprint 列。
 * `carveBlock` 里的 `topMaterial`（草/菌丝 + 下方泥土 → 地表规则）同样依赖 `SurfaceSystem`，
 * 本实现不建模；它只会把泥土换成另一种挡运动的地表方块，不改变 MOTION_BLOCKING 高度。
 */
#ifndef LYSH_CARVER_H
#define LYSH_CARVER_H

#include <stdint.h>

#include "aquifer.h"

/* 目标 chunk 的雕刻结果单元。0 = 未被雕刻（= 密度 + 含水层的原始状态）。 */
enum {
    LYSH_CARVE_UNCHANGED = 0,
    LYSH_CARVE_AIR       = 1,
    LYSH_CARVE_WATER     = 2,
    LYSH_CARVE_LAVA      = 3
};

#define LYSH_CARVE_MIN_Y   (-64)
#define LYSH_CARVE_HEIGHT  (384)
#define LYSH_CARVE_CELLS   (LYSH_CARVE_HEIGHT * 16 * 16)          /* 98304 */
#define LYSH_CARVE_MASK_WORDS ((LYSH_CARVE_CELLS + 63) / 64)      /* 1536 = 12 KiB */

/* cells / mask 的下标 —— 与 `CarvingMask.getIndex` 逐位相同（x 最低、z 次之、y 最高）。 */
#define LYSH_CARVE_INDEX(lx, y, lz) \
    ((((lz) & 15) << 4) | ((lx) & 15) | (((y) - LYSH_CARVE_MIN_Y) << 8))

/* 未雕刻时的方块状态（= `lysh_phase2_block_at`：密度 + 含水层）。
 * 返回 LYSH_BLOCK_NULL 表示实心（游戏里的石头/深板岩）。 */
typedef lysh_block_state (*lysh_carver_base_fn)(void *user, int x, int y, int z);

/* 复现 `applyCarvers` 对一个**目标 chunk** 的雕刻。
 *   · target_chunk_x/z 是目标 chunk 坐标；
 *   · base_fn/base_user 给出未雕刻状态；
 *   · aq 必须**已经** `lysh_aquifer_begin_chunk` 到该目标 chunk（本函数不重建网格）；
 *   · 结果写进 cells（长度 LYSH_CARVE_CELLS，值见 LYSH_CARVE_*）。
 * 掩码是本函数的局部变量（目标 chunk 一份，整个 17x17 遍历共用）。 */
void lysh_carver_apply(uint64_t world_seed,
                       int target_chunk_x, int target_chunk_z,
                       lysh_carver_base_fn base_fn, void *base_user,
                       lysh_aquifer *aq, uint8_t *cells);

/* `Mth.sin` / `Mth.cos`（65536 项 float 表，见 sin_table.h）。 */
float lysh_mc_sin(double a);
float lysh_mc_cos(double a);

#endif /* LYSH_CARVER_H */
