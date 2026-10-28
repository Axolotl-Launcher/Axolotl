/* lysh climate.h —— 阶段 1 的气候门（erosion / temperature / ridge / continentalness）。
 *
 * ⚠️ 重要约束（用户明确要求）：这部分**保持原样，不要动**。
 *
 * 现有实现严格来说只是【近似】于 vanilla 的群系判定：
 *   - vanilla 要算完整 NoiseValuePoint（含 depth/weirdness，需要 offset/factor/jagged
 *     + 地形参数），再走 MultiNoiseUtil.SearchTree 遍历 60 个群系条目；
 *   - 这里只在 (x/4, 0, z/4) 单点直接采样 4 个气候噪声，并在 erosion 处提前退出
 *     （95% 的格子在第一次采样后就返回）。
 *
 * 换成精确版本会慢约十倍，而阶段 1 要跑 1.37e10 格 —— 所以**保持近似**。
 * C 端的目标是【逐位复现这个近似】，funnel 数字必须一个不差。
 */
#ifndef LYSH_CLIMATE_H
#define LYSH_CLIMATE_H

#include <stdint.h>

#include "noise.h"

typedef struct {
    /* NORMAL 世界类型用这四个；LARGE_BIOMES 换 _LARGE 变体 */
    lysh_dblnoise erosion;
    lysh_dblnoise temperature;
    lysh_dblnoise continentalness;
    lysh_dblnoise ridge;

    /* ---- erosion 门的「分层提前退出」预计算（见 noise.h）----
     * tier_seq 与 rust 无关，只由 erosion 的 perm 表指针 + 每档权重构成，
     * 在这里算一次、每格复用。tier_spec 指向内建的静态表。 */
    lysh_tier_seq erosion_tier_seq;
    const lysh_tier_spec *erosion_tier_spec;
    int erosion_tier_ready;      /* tier_seq 构造成功才置 1 */
} lysh_climate;

/* erosion 门只有一条路：**分层提前退出**（近似，假阴性率见 README §7）。
 * 预计算失败（`erosion_tier_ready == 0`）时**回退到精确路径** ——
 * 回退不是静默的，`lysh_climate_init` 会往 stderr 报一行。 */

/* 从种子派生（与创建顺序无关，见 noise.h） */
void lysh_climate_init(lysh_climate *c, uint64_t world_seed, int large_biomes);

/* 各门的返回码，便于 funnel 统计 */
enum {
    LYSH_GATE_EROSION = 0,     /* erosion < 0.55 淘汰 */
    LYSH_GATE_TEMPERATURE = 1, /* 温度越界淘汰 */
    LYSH_GATE_RIDGE = 2,       /* ridge 落在带状区淘汰 */
    LYSH_GATE_PASS = 3         /* 前三门都通过（洞穴与大陆性还没做） */
};

#endif /* LYSH_CLIMATE_H */
