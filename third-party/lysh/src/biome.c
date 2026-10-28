/* lysh biome.c —— 见 biome.h 的取证说明。
 *
 * 判定 = MC 的 `MultiNoiseBiomeSource.getNoiseBiome`：
 *   ① 在 quart 位置求 6 个气候密度函数 → 量化成 TargetPoint；
 *   ② 在参数表上做最近点搜索（`Climate.ParameterList.findValue` → `Climate.RTree.search`）。
 *
 * ②用的是 cubiomes 序列化好的**真正的 R 树**（src/biome_tree_262.h / _215.h，MIT，
 * 见那些文件顶部的出处说明）。为什么用现成的树而不是自己扫表：
 *   · 语义完全一致 —— `findValue` 走的是 R 树，**不是**线性 argmin。实测两者在
 *     179685 个采样点上有 49 个结果不同，且 49 个全是 fitness 相等的并列（其中 48 个
 *     会翻转 swamp/非 swamp）。照抄树就没有这个歧义。
 *   · 树在原版里是**代码里建的**（datapack 只写 {"preset":"minecraft:overworld"}），
 *     自己重建要复刻排序/bucketize/cost 那一整套，才是真正的大工程；cubiomes 已经把
 *     每版的树预先序列化好了，逐字节可审计。
 *   · 顺带解决数据版本：26.2 的参数表不在任何 JSON 里，而这棵树就是 26.2 的。
 * 下面的 `get_np_dist` / `get_resulting_node` 逐行照抄 cubiomes biomenoise.c（MIT），
 * 这样与 cubiomes 本身逐位可比（tools 之外的对拍见 README §biome）。
 */
#include "biome.h"

#include "column_top.h"
#include "mcmath.h"

/* ---- cubiomes 序列化树的读取层（逐行照抄 biomenoise.c，见文件头）---- */
typedef struct {
    const uint32_t *steps;
    const int32_t  *param;
    const uint64_t *nodes;
    uint32_t order;
    uint32_t len;
} lysh_biome_tree;

static uint64_t tree_np_dist(const uint64_t np[6], const lysh_biome_tree *bt, int idx) {
    uint64_t ds = 0, node = bt->nodes[idx];
    uint64_t a, b, d;
    uint32_t i;

    for (i = 0; i < 6; i++) {
        idx = (node >> 8 * i) & 0xFF;
        a = np[i] - (uint64_t)(int64_t)bt->param[2 * idx + 1];
        b = (uint64_t)(int64_t)bt->param[2 * idx + 0] - np[i];
        d = (int64_t)a > 0 ? a : (int64_t)b > 0 ? b : 0;
        d = d * d;
        ds += d;
    }
    return ds;
}

static int tree_resulting_node(const uint64_t np[6], const lysh_biome_tree *bt, int idx,
                               int alt, uint64_t ds, int depth) {
    if (bt->steps[depth] == 0) return idx;
    uint32_t step;
    do {
        step = bt->steps[depth];
        depth++;
    } while (idx + step >= bt->len);

    uint64_t node = bt->nodes[idx];
    uint16_t inner = (uint16_t)(node >> 48);

    int leaf = alt;
    uint32_t i, n;

    for (i = 0, n = bt->order; i < n; i++) {
        uint64_t ds_inner = tree_np_dist(np, bt, inner);
        if (ds_inner < ds) {
            int leaf2 = tree_resulting_node(np, bt, inner, leaf, ds, depth);
            uint64_t ds_leaf2;
            if (inner == leaf2) ds_leaf2 = ds_inner;
            else                ds_leaf2 = tree_np_dist(np, bt, leaf2);
            if (ds_leaf2 < ds) {
                ds = ds_leaf2;
                leaf = leaf2;
            }
        }
        inner += step;
        if (inner >= bt->len) break;
    }
    return leaf;
}

static int tree_biome(const lysh_biome_tree *bt, const uint64_t np[6]) {
    int idx = tree_resulting_node(np, bt, 0, 0, (uint64_t)-1, 0);
    return (int)((bt->nodes[idx] >> 48) & 0xFF);
}
/* ---- 照抄结束 ---- */

#include "biome_tree_18.h"
#include "biome_tree_215.h"
#include "biome_tree_262.h"

#define LYSH_TREE_DECL(name)                                              \
    { name##_steps, &name##_param[0][0], name##_nodes, name##_order,       \
      (uint32_t)(sizeof(name##_nodes) / sizeof(uint64_t)) }

static const lysh_biome_tree LYSH_TREE_18   = LYSH_TREE_DECL(btree18);
static const lysh_biome_tree LYSH_TREE_215  = LYSH_TREE_DECL(btree215);
static const lysh_biome_tree LYSH_TREE_262  = LYSH_TREE_DECL(btree262);

/* Climate.quantizeCoord(float) = (long)(f * 10000.0f)
 * ⚠️ 乘法必须在 **float** 里做（先转 double 再乘会差 1）。 */
static inline long long quantize(float f) {
    return (long long)(f * 10000.0f);
}

/* DensityFunctions.yClampedGradient(-64 -> 320, 1.5 -> -1.5)，`overworld/depth` 的
 * argument1（argument2 是 `overworld/offset`）。与 column_top.c 的 ygrad 同一口径。 */
static inline double depth_gradient(double y) {
    double t = mc_clamp((y - -64.0) / (320.0 - -64.0), 0.0, 1.0);
    return mc_lerp(t, 1.5, -1.5);
}

void lysh_biome_init(lysh_biome *b, uint64_t world_seed, int large_biomes, int tree_sel) {
    lysh_dblnoise_from_seed_id(&b->temperature, world_seed,
                               large_biomes ? "minecraft:temperature_large"
                                            : "minecraft:temperature");
    lysh_dblnoise_from_seed_id(&b->vegetation, world_seed,
                               large_biomes ? "minecraft:vegetation_large"
                                            : "minecraft:vegetation");
    switch (tree_sel) {
        case LYSH_BIOME_TREE_1_18:   b->tree = &LYSH_TREE_18;  break;
        case LYSH_BIOME_TREE_1_21_5: b->tree = &LYSH_TREE_215; break;
        default:                     b->tree = &LYSH_TREE_262; break;
    }
}

const char *lysh_biome_data_version(void) { return "26.2"; }

int lysh_swamp_hut_biome_ok(const lysh_biome *b, const lysh_terrain *t,
                            int hut_x, int hut_z,
                            lysh_biome_height_fn height_fn, void *height_user,
                            lysh_biome_result *out) {
    if (out) {
        for (int i = 0; i < 6; i++) out->target.t[i] = 0;
        out->target.qx = out->target.qy = out->target.qz = 0;
        out->target.occ_y = 0;
        out->winner = -1;
        out->ok = 0;
    }
    if (!out || !height_fn || !b->tree) return 0;

    /* ---- Structure.onTopOfChunkCenter ---- */
    int chunk_x = hut_x >> 4;
    int chunk_z = hut_z >> 4;
    int mid_x = chunk_x * 16 + 8;          /* ChunkPos.getMiddleBlockX */
    int mid_z = chunk_z * 16 + 8;

    /* getFirstOccupiedHeight(...) = getBaseHeight(WORLD_SURFACE_WG) - 1 */
    int y = height_fn(height_user, mid_x, mid_z) - 1;

    /* ---- QuartPos.fromBlock ---- */
    int qx = mid_x >> 2;
    int qy = y >> 2;
    int qz = mid_z >> 2;

    /* ---- Climate.Sampler.sample：六个密度函数在 quart 位置求值 ---- */
    /* ShiftedNoise 的 xz_scale 是 0.25、y_scale 是 0.0，且这些噪声共用同一对 shift；
     * lysh_terrain_info_at 吃的就是 biome(quart) 坐标，shifted_x/shifted_z 已算好，
     * 与密度函数路径逐位等价（见 column_top.h 的实测）。*/
    lysh_terrain_info ti;
    lysh_terrain_info_at(t, qx, qz, &ti);

    double temperature = lysh_dblnoise_sample_y0(&b->temperature, ti.shifted_x, ti.shifted_z);
    double humidity    = lysh_dblnoise_sample_y0(&b->vegetation,  ti.shifted_x, ti.shifted_z);

    /* depth = y_clamped_gradient(y) + overworld/offset（都是 double 语义）。
     * 方块 y 用 QuartPos.toBlock(qy) = qy*4。
     * ⚠️ LARGE_BIOMES 用的是 `overworld_large_biomes/offset`（与 NORMAL 不同的样条），
     *    这里复用的是 NORMAL 的那一条 ⇒ LARGE_BIOMES 下 depth 这一维是近似值。
     *    默认的 NORMAL 世界不受影响。 */
    double depth = depth_gradient((double)(qy * 4)) + lysh_overworld_offset(t, qx * 4, qz * 4);

    uint64_t np[6];
    np[0] = (uint64_t)quantize((float)temperature);
    np[1] = (uint64_t)quantize((float)humidity);
    np[2] = (uint64_t)quantize((float)ti.continentalness);
    np[3] = (uint64_t)quantize((float)ti.erosion);
    np[4] = (uint64_t)quantize((float)depth);
    np[5] = (uint64_t)quantize((float)ti.weirdness);

    int winner = tree_biome((const lysh_biome_tree *)b->tree, np);

    for (int a = 0; a < 6; a++) out->target.t[a] = (long long)np[a];
    out->target.qx = qx;
    out->target.qy = qy;
    out->target.qz = qz;
    out->target.occ_y = y;
    out->winner = winner;
    out->ok = (winner == LYSH_BIOME_ID_SWAMP) ? 1 : 0;
    return out->ok;
}
