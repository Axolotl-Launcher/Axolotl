/* lysh biome.h —— 女巫小屋的**真实群系门**（MC 的 Structure.isValidBiome）。
 *
 * 为什么需要它：阶段 1 的气候门（src/phase1.c）复现的是**旧 Java 程序的近似**
 * （erosion/temperature/ridge/continentalness 四个阈值 + 洞穴梯子），它**不等价**于
 * MC 真正的群系判定。少了这一门，全图扫描会多报 4 个候选（实测：那 4 个点在真实群系
 * 判定下是 minecraft:dripstone_caves，不是沼泽）。
 *
 * ── 判定点（javap 自 26.1.2 的 server jar，逐条照抄）────────────────────
 *   Structure.onTopOfChunkCenter(context, Heightmap.Types.WORLD_SURFACE_WG, ...):
 *       ChunkPos c = context.chunkPos();
 *       int x = c.getMiddleBlockX();                    // chunkX*16 + 8
 *       int z = c.getMiddleBlockZ();
 *       int y = context.chunkGenerator().getFirstOccupiedHeight(
 *                   x, z, WORLD_SURFACE_WG, heightAccessor, randomState);
 *                   // = getBaseHeight(...) - 1  ← 注意 -1
 *   Structure.isValidBiome(stub, context):
 *       context.validBiome.test(biomeSource.getNoiseBiome(
 *           QuartPos.fromBlock(x), QuartPos.fromBlock(y), QuartPos.fromBlock(z),
 *           context.randomState.sampler()));
 *   QuartPos.fromBlock(i) = i >> 2（算术右移）。
 *
 * ── 群系是怎么从那 6 个参数算出来的 ────────────────────────────────────
 *   Climate.Sampler.sample(qx,qy,qz):
 *       SinglePointContext(QuartPos.toBlock(qx), QuartPos.toBlock(qy), QuartPos.toBlock(qz))
 *       六个密度函数各求一次 double，**转 float**，再 Climate.target() 量化：
 *       quantizeCoord(f) = (long)(f * 10000.0f)
 *       顺序：temperature, humidity(vegetation), continentalness, erosion, depth, weirdness
 *   Climate.ParameterList.findValue(target):
 *       argmin over entries of ParameterPoint.fitness(target)
 *       fitness = Σ_i square(Parameter.distance_i(target_i)) + square(offset)
 *       Parameter.distance(v) = let a = v - max, b = min - v in a > 0 ? a : max(b, 0)
 *
 * ── 本实现为什么用 cubiomes 的序列化树，而不是自己扫参数表 ───────────────
 *   MC 的 `findValue` 走 `Climate.RTree.search`，**不是**线性 argmin：实测两者在
 *   179685 个采样点上有 49 个结果不同，且 49 个全是 fitness 相等的并列（其中 48 个
 *   会翻转 swamp/非 swamp）。参数表在原版里是**代码常量**（datapack 只写
 *   {"preset":"minecraft:overworld"}），自己重建那棵树要复刻排序 / bucketize /
 *   cost 一整套 —— 那是真正的大工程。cubiomes 已经把每版的树预先序列化好
 *   （`tables/btreeNNN.h`，MIT），所以这里直接读它：语义与原版一致，而且
 *   **26.2 的参数表也只能从这里拿到**（本机没有 26.2 的参数 JSON）。
 *
 * ⚠️ 数据版本：**26.2**（默认路径，biome_tree_262.h）；pre_26_2 = 1 时用
 *    biome_tree_215.h（cubiomes: 1.21.5 ~ 26.1）。出处与版本映射见那两个文件顶部。
 *
 * ⚠️ LARGE_BIOMES：`overworld_large_biomes/offset` 与 NORMAL 不是同一条样条，
 *    这里复用的是 NORMAL 的那一条 ⇒ LARGE_BIOMES 下 depth 维度是近似值。
 *    默认的 NORMAL 世界不受影响。
 */
#ifndef LYSH_BIOME_H
#define LYSH_BIOME_H

#include <stdint.h>

#include "noise.h"
#include "terrain.h"

/* 六个气候噪声里，terrain 已经持有 shift/continentalness/erosion/ridge(weirdness)；
 * 这里补上温度与湿度（植被），外加选定的参数树。都是 POD，无堆分配。 */
typedef struct {
    lysh_dblnoise temperature;   /* minecraft:temperature  (shifted_noise xz 0.25, y 0) */
    lysh_dblnoise vegetation;    /* minecraft:vegetation   (shifted_noise xz 0.25, y 0) */
    const void *tree;            /* 选定的 cubiomes 参数树（按版本，见 biome.c） */
} lysh_biome;

/* 参数树的版本选择。cubiomes 的映射（biomenoise.c climateToBiome）：
 *   1.18.x                     -> LYSH_BIOME_TREE_1_18     (btree18)
 *   1.21.5 .. 26.1（含 1.19/1.20/1.21 的近似）-> LYSH_BIOME_TREE_1_21_5 (btree215)
 *   26.2（默认）                -> LYSH_BIOME_TREE_26_2     (btree262)
 * ⚠️ 必须按版本选表，不能"一张表走天下"：1.18.2 没有 mangrove_swamp，用新表会把
 *    高温沼泽点判成 mangrove_swamp 从而**误杀 1.18.2 合法的小屋**。 */
enum {
    LYSH_BIOME_TREE_1_18   = 0,
    LYSH_BIOME_TREE_1_21_5 = 1,
    LYSH_BIOME_TREE_26_2   = 2      /* 默认 */
};

void lysh_biome_init(lysh_biome *b, uint64_t world_seed, int large_biomes, int tree_sel);

/* 参数表的数据版本 —— 给 CLI / 诊断打印用，免得调用方去 include 那些大表。 */
const char *lysh_biome_data_version(void);

/* 树里存的是 cubiomes `biomes.h` 的枚举序号（不是注册表 id）。swamp = 6；
 * 4 个假阳性在那里是 174 = dripstone_caves。这个常量由 4/11 的端到端验收兜底：
 * 序号一旦对不上，4 个假阳性就不会被拒。 */
#define LYSH_BIOME_ID_SWAMP 6

/* 量化后的目标点（诊断 / 对拍用）；顺序与 Climate.TargetPoint 一致。 */
typedef struct {
    long long t[6];              /* temperature humidity continentalness erosion depth weirdness */
    int       qx, qy, qz;        /* 判定用的 quart 坐标 */
    int       occ_y;             /* getFirstOccupiedHeight(WORLD_SURFACE_WG) = baseHeight - 1 */
} lysh_biome_target;

/* 单候选的群系判定结果。 */
typedef struct {
    int ok;                      /* 1 = 判定出的群系就是 swamp（允许放小屋） */
    int winner;                  /* 参数树给出的群系序号（cubiomes biomes.h 序数） */
    lysh_biome_target target;
} lysh_biome_result;

/* 列顶回调：返回方块 (x, z) 处的 **WORLD_SURFACE_WG** 高度（= game 的 getBaseHeight）。
 * 生产路径传 lysh_phase2_height（它的 NOT_AIR 谓词与返回值 y+1 就是同一口径）。
 * 用回调而不是直接吃 lysh_phase2，是为了让本层不依赖阶段 2 —— 与
 * lysh_phase2_set_column_top 的既有风格一致。 */
typedef int (*lysh_biome_height_fn)(void *user, int x, int z);

/* 女巫小屋在 (hut_x, hut_z) 处是否位于允许的群系里。
 * hut_x/hut_z 是方块坐标（= chunkX*16）。返回 0 表示**不是**沼泽 ⇒ 阶段 2 之前就该拒。
 * height_fn 在 **chunk 中心列** 上被调用一次（x = chunkX*16+8，z = chunkZ*16+8）。 */
int lysh_swamp_hut_biome_ok(const lysh_biome *b, const lysh_terrain *t,
                            int hut_x, int hut_z,
                            lysh_biome_height_fn height_fn, void *height_user,
                            lysh_biome_result *out);

#endif /* LYSH_BIOME_H */
