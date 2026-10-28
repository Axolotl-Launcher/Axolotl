/* lysh phase1.c —— 见 phase1.h 顶部的判定顺序与近似说明。 */
#include "phase1.h"

#include <string.h>

/* 与 SearchCoords 的常量保持一致 */
#define LYSH_PHASE1_MIN_CHECK_HEIGHT (-50)

/* ---------------- 初始化 ---------------- */

void lysh_phase1_init(lysh_phase1 *p, uint64_t world_seed, const lysh_phase1_opts *opts) {
    int large = opts ? opts->large_biomes : 0;
    lysh_climate_init(&p->climate, world_seed, large);
    lysh_caves_init(&p->caves, world_seed);
    lysh_dblnoise_from_seed_id(&p->aquifer_floodedness, world_seed,
                               "minecraft:aquifer_fluid_level_floodedness");
}

/* ---------------- 原始值采样（与 Java 的表达式一一对应） ---------------- */

/* cache.erosion.sample(climateX / 4.0, 0, climateZ / 4.0)，climateX = hutX + 8
 *
 * 阶段 1 的气候噪声【恒在 y = 0 采样】，所以必须走 y=0 特化路径
 * （lysh_dblnoise_sample_y0，预计算了 floor(oy)/fade(oy)，省掉每次调用的
 *  floor、fade 与 y 相关的 maintainPrecision）。两条路径逐位等价，
 * 由 phase1_diff 的逐位对拍兜底 —— 换实现时一定要重跑它。 */
static inline double sample_climate(const lysh_dblnoise *n, int hut_x, int hut_z) {
    return lysh_dblnoise_sample_y0(n, (hut_x + 8) / 4.0, (hut_z + 8) / 4.0);
}

/* cache.aquiferFloodedness.sample(heightX, y * 0.67, heightZ)，heightX = hutX + 3 */
static inline double sample_floodedness(const lysh_phase1 *p, int hut_x, int hut_z, int y) {
    return lysh_dblnoise_sample(&p->aquifer_floodedness,
                                (double)(hut_x + 3), (double)y * 0.67, (double)(hut_z + 3));
}

/* ---------------- 探针（固定顺序，无短路） ---------------- */

/* 顺序定义（两边必须完全一致）：
 *   0..3    erosion / temperature / ridge / continentalness
 *   4,5     Entrance(50), Entrance(60)
 *   6,7     Entrance2(maxHeight), Cheese(maxHeight)
 *   8..17   Entrance2/Cheese @ y = 0, -10, -20, -30, -40
 *   18..25  Entrance/Cheese  @ y = 10, 20, 30, 40
 *   26..37  floodedness      @ y = -50, -40, ..., 60
 */
void lysh_phase1_probe(const lysh_phase1 *p, int hut_x, int hut_z, int max_height,
                       double *out) {
    int hx = hut_x + 3;
    int hz = hut_z + 3;
    int k = 0;

    out[k++] = sample_climate(&p->climate.erosion, hut_x, hut_z);
    out[k++] = sample_climate(&p->climate.temperature, hut_x, hut_z);
    out[k++] = sample_climate(&p->climate.ridge, hut_x, hut_z);
    out[k++] = sample_climate(&p->climate.continentalness, hut_x, hut_z);

    out[k++] = lysh_entrance(&p->caves, hx, 50, hz);
    out[k++] = lysh_entrance(&p->caves, hx, 60, hz);

    out[k++] = lysh_entrance2(&p->caves, hx, max_height, hz);
    out[k++] = lysh_cheese(&p->caves, hx, max_height, hz);

    for (int y = 0; y >= -40; y -= 10) {
        out[k++] = lysh_entrance2(&p->caves, hx, y, hz);
        out[k++] = lysh_cheese(&p->caves, hx, y, hz);
    }
    for (int y = 10; y <= 40; y += 10) {
        out[k++] = lysh_entrance(&p->caves, hx, y, hz);
        out[k++] = lysh_cheese(&p->caves, hx, y, hz);
    }
    for (int y = -50; y <= 60; y += 10) {
        out[k++] = sample_floodedness(p, hut_x, hut_z, y);
    }

    /* 顺序表写错时立刻暴露，而不是悄悄越界 */
    if (k != LYSH_P1_PROBE_N) {
        for (; k < LYSH_P1_PROBE_N; k++) out[k] = 0.0;
    }
}

/* ---------------- 判定 ---------------- */

/* 第 1、7 步的群系近似门（与 climate.c 的门保持同一套判据）
 *
 * erosion 门只有**一条**路：分层提前退出（tiered early exit）。实测 95% 的格子死在
 * 这一门，而原路径要为此付满 8 次 Perlin 求值。最后一档阈值 == 0.55，所以「没有提前
 * 退出」与原始 `e < 0.55` 判定等价；提前退出则是**近似**（假阴性率见 README §7）。
 *
 * ⚠️⚠️ `else` 分支**不是**死代码，更不是"另一条可选路径"，不许删：
 *    `p->climate.erosion_tier_ready == 0` 时（`lysh_tier_seq_init` 失败，
 *    climate.c 会往 stderr 报 "erosion tier seq unavailable … falling back to
 *    the exact path"），整个扫描**只能**走精确表达式。删掉它 =
 *    tier 预计算一旦失败，erosion 门就静默全通过 —— 全图几乎每个格子都会漏过这一门。 */
static int climate_path(const lysh_phase1 *p, const lysh_phase1_opts *o,
                        int hut_x, int hut_z,
                        long long *tier_hist) {
    if (o->single_biome) return LYSH_P1_ACCEPT;

    double e;
    if (p->climate.erosion_tier_ready) {
        int hit = -1;
        if (lysh_dblnoise_tiered_y0(&p->climate.erosion, &p->climate.erosion_tier_seq,
                                    p->climate.erosion_tier_spec,
                                    (hut_x + 8) / 4.0, (hut_z + 8) / 4.0, &e, &hit)) {
            if (tier_hist && hit >= 0 && hit < LYSH_TIER_MAX) tier_hist[hit]++;
            return LYSH_P1_REJECT_EROSION;
        }
    } else {
        /* 兜底：精确路径（= 原 `--exact-erosion` 的那个表达式）。**必须保留**。 */
        e = sample_climate(&p->climate.erosion, hut_x, hut_z);
    }
    if (e < 0.55) return LYSH_P1_REJECT_EROSION;

    double t = sample_climate(&p->climate.temperature, hut_x, hut_z);
    if (o->mc_1_18_2) {
        if (t < -0.45) return LYSH_P1_REJECT_TEMPERATURE;
    } else {
        if (t > 0.2 || t < -0.45) return LYSH_P1_REJECT_TEMPERATURE;
    }

    double r = sample_climate(&p->climate.ridge, hut_x, hut_z);
    if ((r > 0.42 && r < 0.91) || (r < -0.42 && r > -0.91)) return LYSH_P1_REJECT_RIDGE;
    /* 26.2（默认，见 phase1.h 的 pre_26_2 说明）：ridge <= -0.91 也淘汰 */
    if (!o->pre_26_2 && r <= -0.91) return LYSH_P1_REJECT_RIDGE;

    return LYSH_P1_ACCEPT;
}

int lysh_phase1_check_ex(const lysh_phase1 *p, const lysh_phase1_opts *o,
                         int hut_x, int hut_z, int max_height,
                         lysh_phase1_result *out,
                         long long *tier_hist) {
    int gate = climate_path(p, o, hut_x, hut_z, tier_hist);
    if (gate != LYSH_P1_ACCEPT) {
        if (out) out->reject = gate;
        return 0;
    }

    int hx = hut_x + 3;
    int hz = hut_z + 3;

    /* ⚠️ 顺序与旧 Java 实现**有意不同**（旧版见 _archive 里的 SearchCoords.java）：
     *   旧：Entrance@50 单独淘汰 → 会把「Entrance@50 略大于 0、但 Cheese@50 为负」
     *       的候选误杀（实测确有这种小屋：E50=+0.0089 / C50=-0.0941）。
     *   新：y=60 提到最前（实验表明它的排除率高，前置比放在组后更快——每提前淘汰
     *       一格就省下整组的 5 次 Entrance + 最多 5 次 Cheese）；
     *       y=50 并入下面 10..50 那一组，与 10..40 同判据：两者都 >= 0 才淘汰。
     *   ⇒ 新拒绝集是旧拒绝集的**子集**，只会救回、不会新增淘汰。 */
    if (lysh_entrance(&p->caves, hx, 60, hz) >= 0) {
        if (out) out->reject = LYSH_P1_REJECT_ENTRANCE_60;
        return 0;
    }
    if (lysh_entrance2(&p->caves, hx, max_height, hz) >= 0 &&
        lysh_cheese(&p->caves, hx, max_height, hz) >= 0) {
        if (out) out->reject = LYSH_P1_REJECT_CAVE_AT_MAXH;
        return 0;
    }

    /* 0 以下用 Entrance2；ladderFloor = max(-50, min(-40, maxHeight))
     * 例：maxHeight = -50 → -50；-54 → -50；100 → -40；-40 → -40 */
    int ladder_floor = (-40 < max_height) ? -40 : max_height;      /* min(-40, maxHeight) */
    if (ladder_floor < LYSH_PHASE1_MIN_CHECK_HEIGHT) {
        ladder_floor = LYSH_PHASE1_MIN_CHECK_HEIGHT;               /* max(-50, ...)      */
    }

    for (int y = 0; y >= ladder_floor; y -= 10) {
        if (max_height < y) {
            if (lysh_entrance2(&p->caves, hx, y, hz) >= 0 &&
                lysh_cheese(&p->caves, hx, y, hz) >= 0) {
                if (out) out->reject = LYSH_P1_REJECT_CAVE_LADDER_NEG;
                return 0;
            }
        }
    }

    /* 10-50 用 Entrance（y=50 已并入本组，见上面的说明） */
    for (int y = 10; y <= 50; y += 10) {
        if (lysh_entrance(&p->caves, hx, y, hz) >= 0 &&
            lysh_cheese(&p->caves, hx, y, hz) >= 0) {
            if (out) out->reject = LYSH_P1_REJECT_CAVE_LADDER_POS;
            return 0;
        }
    }

    if (!o->single_biome) {
        double c = sample_climate(&p->climate.continentalness, hut_x, hut_z);
        if (c < -0.11) {
            if (out) out->reject = LYSH_P1_REJECT_CONTINENTALNESS;
            return 0;
        }
    }

    for (int y = max_height; y <= 60; y += 10) {
        if (sample_floodedness(p, hut_x, hut_z, y) > 0.41) {
            if (out) out->reject = LYSH_P1_REJECT_FLOODEDNESS;
            return 0;
        }
    }

    if (out) out->reject = LYSH_P1_ACCEPT;
    return 1;
}
