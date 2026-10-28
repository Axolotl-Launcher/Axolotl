/* lysh carver.c —— 见 carver.h 顶部的取证与语义说明。
 *
 * 本文件是 `WorldCarver` / `CaveWorldCarver` / `CanyonWorldCarver` /
 * `NoiseBasedChunkGenerator.applyCarvers` 的逐条移植（26.1.2）。
 * 所有"看起来可以化简"的地方都保留原样，并在注释里点名为什么不能改：
 *   · 嵌套的 `nextInt(nextInt(nextInt(15)+1)+1)`（不是 nextInt(bound)+1）；
 *   · 房间的 `MutableBoolean` 是**每列**一个、跨 y 不重置；
 *   · 掩码位在 `carveBlock` **之前**、且与是否真写入无关地置位；
 *   · `canReach` 的 `dy = totalSteps - i`、`r = thickness + 2.0f + 16.0f`（float 相加再拓宽）；
 *   · `carveEllipsoid` 的 y 上界 `minGenY + genDepth - 1 - 7`（`isUpgrading()` 为假）；
 *   · RNG 消耗顺序：所有 sample 都按字节码顺序，且**同一表达式里有多个 nextX 时必须
 *     先取到局部变量**（C 的函数实参求值顺序未定义）。
 */
#include "carver.h"

#include <math.h>
#include <string.h>

#include "mcmath.h"
#include "rng.h"
#include "sin_table.h"
#include "structure.h"       /* lysh_set_carver_seed（= setLargeFeatureSeed） */

/* ---------------- 常量（全部来自 jar / javap） ---------------- */

#define CARVE_MIN_Y        LYSH_CARVE_MIN_Y          /* CarvingContext.getMinGenY() */
#define CARVE_GEN_DEPTH    LYSH_CARVE_HEIGHT         /* CarvingContext.getGenDepth() */
#define CARVE_LAVA_Y       (-56)                     /* AboveBottom(8).resolveY = minGenY + 8 */
#define CARVE_I9           (112)                     /* SectionPos.sectionToBlockCoord(getRange()*2-1) */
#define CARVE_CAVE_BOUND   (15)                      /* CaveWorldCarver.getCaveBound() */

/* configured_carver 各 JSON 的 probability 与 y 范围（resolveY 后） */
#define CARVE_CAVE_PROB    (0.15f)
#define CARVE_EXTRA_PROB   (0.07f)
#define CARVE_CANYON_PROB  (0.01f)
#define CARVE_CAVE_Y_LO    (-56)                     /* AboveBottom(8) */
#define CARVE_CAVE_Y_HI    (180)                     /* Absolute(180) */
#define CARVE_EXTRA_Y_HI   (47)                      /* Absolute(47) */
#define CARVE_CANYON_Y_LO  (10)                      /* Absolute(10) */
#define CARVE_CANYON_Y_HI  (67)                      /* Absolute(67) */
#define CARVE_CANYON_Y_SCALE   (3.0f)                /* ConstantFloat(3.0f)：不消耗 RNG */
#define CARVE_CANYON_SMOOTH    (3)                   /* shape.width_smoothness */
#define CARVE_CANYON_VR_DEFAULT (0.0f)               /* shape.vertical_radius_default_factor */
#define CARVE_CANYON_VR_CENTER  (1.0f)               /* shape.vertical_radius_center_factor */

/* `Mth.sin` / `Mth.cos` 的缩放常量。Java 侧是 10430.378350470453d；
 * 这里用等值的 hex float 写死，并用 _Static_assert 钉住十进制写法确实是同一个 double。 */
#define MC_SIN_SCALE 0x1.45f306dc9c883p13
_Static_assert(MC_SIN_SCALE == 10430.378350470453,
               "MC_SIN_SCALE must be the exact double 10430.378350470453d");

/* 表在 sin_table.h（自动生成，逐位来自 JVM 的 Mth.SIN；JVM/平台相关，不能重算）。 */
float lysh_mc_sin(double a) {
    return LYSH_MC_SIN[(int)((int64_t)(a * MC_SIN_SCALE) & 65535LL)];
}

float lysh_mc_cos(double a) {
    return LYSH_MC_SIN[(int)((int64_t)(a * MC_SIN_SCALE + 16384.0) & 65535LL)];
}

/* ---------------- 小工具 ---------------- */

static inline int imax(int a, int b) { return a > b ? a : b; }
static inline int imin(int a, int b) { return a < b ? a : b; }

/* Mth.floor(double) = (int)Math.floor(d) */
static inline int jfloor(double d) { return (int)floor(d); }

/* `java.util.Random.nextFloat()`（= BitRandomSource.nextFloat 的 LCG 版） */
static inline float lcg_next_float(lysh_lcg *r) {
    return (float)lysh_lcg_next_bits(r, 24) * 5.9604645e-8f;      /* 2^-24 */
}

/* `Mth.randomBetween(r, min, max) = r.nextFloat() * (max - min) + min`（**全 float**；
 * max-min 必须在 float 里算，见 carver_algo_spec §10.1 的可移植性警告）。 */
static inline float uniform_float(lysh_lcg *r, float lo, float hi) {
    return lcg_next_float(r) * (hi - lo) + lo;
}

/* `TrapezoidFloat.sample`（min + r1*f + r2*e，f = d - e，e = (d - plateau)/2） */
static inline float trapezoid_float(lysh_lcg *r, float lo, float hi, float plateau) {
    float d = hi - lo;
    float e = (d - plateau) / 2.0f;
    float f = d - e;
    float a = lcg_next_float(r) * f + lo;
    return a + lcg_next_float(r) * e;
}

/* `UniformHeight.sample` = Mth.randomBetweenInclusive(r, lo, hi) = nextInt(hi-lo+1) + lo */
static inline int uniform_height(lysh_lcg *r, int lo, int hi) {
    return lysh_lcg_next_int_bound(r, hi - lo + 1) + lo;
}

/* `Mth.abs(float)` */
static inline float jfabs(float v) { return fabsf(v); }

/* ---------------- 上下文 ---------------- */

typedef struct {
    int kind;                 /* 0 = cave（floorLevel 平面），1 = canyon（宽度因子表） */
    double floor_level;
    const float *width;       /* canyon：widthFactors[] */
} carve_skip;

typedef struct {
    int tgt_min_x, tgt_min_z;     /* 目标 chunk 的 minBlockX / minBlockZ */
    double tgt_mid_x, tgt_mid_z;  /* ChunkPos.getMiddleBlockX/Z = minBlock + 8 */
    lysh_carver_base_fn base_fn;
    void *base_user;
    lysh_aquifer *aq;
    uint8_t *cells;
    uint64_t *mask;
} carve_ctx;

/* ---- 掩码（`CarvingMask`，目标 chunk 一份，17x17 遍历共用）---- */

static inline void mask_set(uint64_t *m, int lx, int y, int lz) {
    unsigned i = (unsigned)LYSH_CARVE_INDEX(lx, y, lz);
    m[i >> 6] |= (uint64_t)1 << (i & 63);
}

static inline int mask_get(const uint64_t *m, int lx, int y, int lz) {
    unsigned i = (unsigned)LYSH_CARVE_INDEX(lx, y, lz);
    return (int)((m[i >> 6] >> (i & 63)) & 1u);
}

/* ---- 方块状态 ---- */

/* `#minecraft:overworld_carver_replaceables`（从 26.1.2 jar 的标签递归展开，52 个方块：
 * base_stone_overworld / substrate_overworld / sand / terracotta / iron_ores / copper_ores /
 * snow + water, gravel, suspicious_gravel, sandstone, red_sandstone, calcite, packed_ice,
 * raw_iron_block, raw_copper_block）里，属于本模型 4 种状态的只有两种：
 *   实心（= 石头/深板岩/土/沙/砾/雪…）与水 **可以**；
 *   空气与岩浆 **不可以**（岩浆、冰、粘土、基岩都不在标签里）。
 * ⚠️ 这是唯一一处"类型"近似：本模型不跑 SurfaceSystem，所以地表规则的草/土/沙都算实心。
 *    它们全在标签里，所以可替换性判断与真实一致。 */
static inline int can_replace(lysh_block_state s) {
    return s == LYSH_BLOCK_NULL || s == LYSH_BLOCK_WATER;
}

/* 当前（可能已被本次雕刻改过）的方块状态。 */
static lysh_block_state cur_state(const carve_ctx *c, int lx, int y, int lz) {
    uint8_t v = c->cells[LYSH_CARVE_INDEX(lx, y, lz)];
    if (v == LYSH_CARVE_AIR)   return LYSH_BLOCK_AIR;
    if (v == LYSH_CARVE_WATER) return LYSH_BLOCK_WATER;
    if (v == LYSH_CARVE_LAVA)  return LYSH_BLOCK_LAVA;
    return c->base_fn(c->base_user, c->tgt_min_x + lx, y, c->tgt_min_z + lz);
}

/* `WorldCarver.carveBlock`（主世界路径）。返回 1 = 真的写了一个方块。 */
static int carve_block(carve_ctx *c, const carve_skip *sk, int lx, int y, int lz, int *grass) {
    (void)sk; (void)grass;
    int bx = c->tgt_min_x + lx, bz = c->tgt_min_z + lz;
    lysh_block_state st = cur_state(c, lx, y, lz);

    /* GRASS_BLOCK / MYCELIUM 的标志位：本模型没有地表规则 ⇒ 永远为假。
     * 它只影响下方的 `topMaterial` 分支（也要 SurfaceSystem），不影响方块/掩码。 */
    if (!can_replace(st)) return 0;

    lysh_block_state cs;
    if (y <= CARVE_LAVA_Y) {
        cs = LYSH_BLOCK_LAVA;                       /* Fluids.LAVA.createLegacyBlock() */
    } else {
        /* `Aquifer.computeSubstance(SinglePointContext, 0.0)` —— 密度参数**就是 0.0**；
         * 返回 null（= LYSH_BLOCK_NULL）表示"此处不写"。 */
        cs = lysh_aquifer_compute_substance(c->aq, bx, y, bz, 0.0);
        if (cs == LYSH_BLOCK_NULL) return 0;
    }
    c->cells[LYSH_CARVE_INDEX(lx, y, lz)] =
          (cs == LYSH_BLOCK_AIR)   ? LYSH_CARVE_AIR
        : (cs == LYSH_BLOCK_WATER) ? LYSH_CARVE_WATER
                                   : LYSH_CARVE_LAVA;
    /* `markPosForPostprocessing` / `shouldScheduleFluidUpdate`：只关系到流体刻，与高度图无关。 */
    return 1;
}

/* ---------------- `WorldCarver$CarveSkipChecker` ---------------- */

static int skip_should_skip(const carve_skip *sk,
                            double nx, double ny, double nz, int block_y) {
    if (sk->kind == 0) {
        /* CaveWorldCarver.shouldSkip(x,y,z,floorLevel)：
         *   y <= floorLevel -> true；否则 (x²+y²+z²) >= 1 -> true */
        if (ny <= sk->floor_level) return 1;
        return (nx * nx + ny * ny + nz * nz) >= 1.0;
    }
    /* CanyonWorldCarver.shouldSkip：
     *   (x²+z²)*wf[blockY-minGenY-1] + y²/6.0 >= 1.0
     * ⚠️ 26.1.2 **没有** `blockY <= getMinGenY()+1` 那一支（javap 只有一个比较 + ireturn）。 */
    {
        int idx = block_y - CARVE_MIN_Y - 1;
        return (nx * nx + nz * nz) * (double)sk->width[idx] + ny * ny / 6.0 >= 1.0;
    }
}

/* ---------------- `WorldCarver.carveEllipsoid` ---------------- */

static int carve_ellipsoid(carve_ctx *c, const carve_skip *sk,
                           double x, double y, double z,
                           double hr, double vr) {
    /* AABB 早退用的是**目标** chunk 的中心（`aload_3.getPos()`）。 */
    double reach = 16.0 + hr * 2.0;
    if (fabs(x - c->tgt_mid_x) > reach || fabs(z - c->tgt_mid_z) > reach) return 0;

    int min_bx = c->tgt_min_x, min_bz = c->tgt_min_z;

    int i = imax(jfloor(x - hr) - min_bx - 1, 0);
    int j = imin(jfloor(x + hr) - min_bx, 15);
    int k = imax(jfloor(y - vr) - 1, CARVE_MIN_Y + 1);
    int upg_pad = 7;                                   /* chunk.isUpgrading() == false */
    int m = imin(jfloor(y + vr) + 1, CARVE_MIN_Y + CARVE_GEN_DEPTH - 1 - upg_pad);
    int n = imax(jfloor(z - hr) - min_bz - 1, 0);
    int o = imin(jfloor(z + hr) - min_bz, 15);

    int carved_any = 0;
    for (int p = i; p <= j; p++) {
        int bx = min_bx + p;
        double nx = ((double)bx + 0.5 - x) / hr;
        for (int s = n; s <= o; s++) {
            int bz = min_bz + s;
            double nz = ((double)bz + 0.5 - z) / hr;
            if (nx * nx + nz * nz >= 1.0) continue;     /* 水平单位圆盘外 */
            /* ⚠️ 这个是**每列**一个（不是每个体素），跨 y 迭代**不重置**。 */
            int grass = 0;
            for (int v = m; v > k; v--) {
                /* ⚠️ **y 用 dsub**（与 x/z 的 dadd 不对称）：字节码 offset 348-360 是
                 *     iload v; i2d; ldc 0.5d; dsub; dload y; dsub; dload vr; ddiv
                 *   ⇒ ny = (v - 0.5 - y)/vr。写成 +0.5 会让每个椭球少啃最上面一格、
                 *   多啃最下面一格（setCarverSeed 修好之后实测差 1 格）。 */
                double ny = ((double)v - 0.5 - y) / vr;
                if (skip_should_skip(sk, nx, ny, nz, v)) continue;
                /* isDebugEnabled(cfg) 恒为假（SharedConstants.DEBUG_CARVERS = false，
                 * 三个配置的 debug_mode 都缺省）⇒ 掩码位已置就跳过。 */
                if (mask_get(c->mask, p, v, s)) continue;
                mask_set(c->mask, p, v, s);             /* 先置位，与是否真写无关 */
                carved_any |= carve_block(c, sk, p, v, s, &grass);
            }
        }
    }
    return carved_any;
}

/* ---------------- `CaveWorldCarver` ---------------- */

static float cave_get_thickness(lysh_lcg *r) {
    /* float f = nextFloat() * 2.0f + nextFloat();
     * if (nextInt(10) == 0) f = f * (nextFloat() * nextFloat() * 3.0f + 1.0f); */
    float f = lcg_next_float(r) * 2.0f + lcg_next_float(r);
    if (lysh_lcg_next_int_bound(r, 10) == 0) {
        float a = lcg_next_float(r);
        float b = lcg_next_float(r);
        f = f * (a * b * 3.0f + 1.0f);
    }
    return f;
}

static void cave_create_room(carve_ctx *c, const carve_skip *sk,
                             double x, double y, double z, float f1, double y_scale_cfg) {
    /* d17 = 1.5 + (double)(Mth.sin(1.5707963705062866d) * f1)
     * 该角度处的表项恰是 1.0f（已在 Java 侧对拍过）。 */
    float sf = lysh_mc_sin(1.5707963705062866);
    double d17 = 1.5 + (double)(sf * f1);
    double d19 = d17 * y_scale_cfg;
    carve_ellipsoid(c, sk, x + 1.0, y, z, d17, d19);
}

static void cave_create_tunnel(carve_ctx *c, const carve_skip *sk, uint64_t seed,
                               double x, double y, double z, double d4, double d5,
                               float thickness, float yaw, float pitch,
                               int first_step, int total_steps, double y_scale) {
    lysh_lcg r;                                     /* RandomSource.createThreadLocalInstance(seed) */
    lysh_lcg_set_seed(&r, seed);

    int branch_step = lysh_lcg_next_int_bound(&r, total_steps / 2) + total_steps / 4;
    int wide = (lysh_lcg_next_int_bound(&r, 6) == 0);

    float f30 = 0.0f;                               /* yaw 漂移 */
    float f31 = 0.0f;                               /* pitch 漂移 */

    for (int i = first_step; i < total_steps; i++) {
        /* radius = 1.5 + (double)(Mth.sin((double)((float)i * 3.1415927f / (float)totalSteps)) * thickness)
         * —— 角度在 **float** 里算，再拓宽成 double 查表。 */
        float ang = (float)i * 3.1415927f / (float)total_steps;
        double radius = 1.5 + (double)(lysh_mc_sin((double)ang) * thickness);
        double vert_base = radius * y_scale;

        float cos_pitch = lysh_mc_cos((double)pitch);
        x += (double)(lysh_mc_cos((double)yaw) * cos_pitch);
        y += (double)lysh_mc_sin((double)pitch);
        z += (double)(lysh_mc_sin((double)yaw) * cos_pitch);

        pitch *= wide ? 0.92f : 0.7f;
        pitch += f31 * 0.1f;
        yaw   += f30 * 0.1f;
        f31   *= 0.9f;
        f30   *= 0.75f;
        {
            /* ⚠️ 顺序必须 a,b,c：先取到局部再算，C 的实参求值顺序未定义。 */
            float a1 = lcg_next_float(&r), b1 = lcg_next_float(&r), c1 = lcg_next_float(&r);
            f31 += (a1 - b1) * c1 * 2.0f;
            float a2 = lcg_next_float(&r), b2 = lcg_next_float(&r), c2 = lcg_next_float(&r);
            f30 += (a2 - b2) * c2 * 4.0f;
        }

        if (i == branch_step && thickness > 1.0f) {
            /* 两次递归（thickness <= 1.0 ⇒ 递归体里不会再分叉），然后把整条隧道 return 掉：
             * 本步的 nextInt(4)/canReach/carveEllipsoid **不执行**。 */
            uint64_t s1 = lysh_lcg_next_long(&r);
            float t1 = lcg_next_float(&r) * 0.5f + 0.5f;
            cave_create_tunnel(c, sk, s1, x, y, z, d4, d5, t1,
                               yaw - 1.5707964f, pitch / 3.0f, i, total_steps, 1.0);
            uint64_t s2 = lysh_lcg_next_long(&r);
            float t2 = lcg_next_float(&r) * 0.5f + 0.5f;
            cave_create_tunnel(c, sk, s2, x, y, z, d4, d5, t2,
                               yaw + 1.5707964f, pitch / 3.0f, i, total_steps, 1.0);
            return;
        }

        if (lysh_lcg_next_int_bound(&r, 4) == 0) continue;
        /* `WorldCarver.canReach(chunk.getPos(), x, z, i, totalSteps, thickness)`
         * —— ChunkPos 恒为目标 chunk（`aload_3`），不是 origin。 */
        {
            double dx = x - c->tgt_mid_x, dz = z - c->tgt_mid_z;
            double dy = (double)(total_steps - i);
            double rr = (double)(thickness + 2.0f + 16.0f);   /* float 相加后拓宽 */
            if (!(dx * dx + dz * dz - dy * dy <= rr * rr)) return;
        }
        carve_ellipsoid(c, sk, x, y, z, radius * d4, vert_base * d5);
    }
}

/* `CaveWorldCarver.carve`。origin 是**邻居** chunk（中心就取在邻居里）。 */
static void cave_carve(carve_ctx *c, lysh_lcg *r, int origin_cx, int origin_cz, int y_hi) {
    int i10 = lysh_lcg_next_int_bound(r,
                  lysh_lcg_next_int_bound(r,
                      lysh_lcg_next_int_bound(r, CARVE_CAVE_BOUND) + 1) + 1);

    for (int k = 0; k < i10; k++) {
        /* ⚠️ 这里的抽样顺序 = 字节码顺序：x, y, z, hrm, vrm, floorLevel。 */
        int lx = lysh_lcg_next_int_bound(r, 16);
        double x = (double)(origin_cx * 16 + lx);
        int y = uniform_height(r, CARVE_CAVE_Y_LO, y_hi);
        int lz = lysh_lcg_next_int_bound(r, 16);
        double z = (double)(origin_cz * 16 + lz);
        double hrm = (double)uniform_float(r, 0.7f, 1.4f);
        double vrm = (double)uniform_float(r, 0.8f, 1.3f);
        double floor_level = (double)uniform_float(r, -1.0f, -0.4f);

        carve_skip sk;
        sk.kind = 0;
        sk.floor_level = floor_level;
        sk.width = NULL;

        int tunnels = 1;
        if (lysh_lcg_next_int_bound(r, 4) == 0) {
            double room_y_scale = (double)uniform_float(r, 0.1f, 0.9f);
            float f28 = 1.0f + lcg_next_float(r) * 6.0f;
            cave_create_room(c, &sk, x, (double)y, z, f28, room_y_scale);
            tunnels += lysh_lcg_next_int_bound(r, 4);
        }

        for (int t = 0; t < tunnels; t++) {
            float yaw = lcg_next_float(r) * 6.2831855f;
            float pitch = (lcg_next_float(r) - 0.5f) / 4.0f;
            float thick = cave_get_thickness(r);
            int steps = CARVE_I9 - lysh_lcg_next_int_bound(r, CARVE_I9 / 4);
            uint64_t seed = lysh_lcg_next_long(r);
            cave_create_tunnel(c, &sk, seed, x, (double)y, z, hrm, vrm, thick, yaw, pitch,
                               0, steps, 1.0 /* getYScale() */);
        }
    }
}

/* ---------------- `CanyonWorldCarver` ---------------- */

static void canyon_init_width_factors(lysh_lcg *r, float *out) {
    float f = 1.0f;
    for (int i = 0; i < CARVE_GEN_DEPTH; i++) {
        /* `i == 0` 短路 ⇒ 第 0 项**不消耗** nextInt。 */
        if (i == 0 || lysh_lcg_next_int_bound(r, CARVE_CANYON_SMOOTH) == 0) {
            float a = lcg_next_float(r);
            float b = lcg_next_float(r);
            f = 1.0f + a * b;
        }
        out[i] = f * f;
    }
}

static double canyon_update_vertical_radius(lysh_lcg *r, double d1, float f1, float f2) {
    /* f7 = 1.0f - Mth.abs(0.5f - f2/f1) * 2.0f
     * f8 = defaultFactor + centerFactor * f7
     * return (double)f8 * d1 * (double)Mth.randomBetween(r, 0.75f, 1.0f) */
    float f7 = 1.0f - jfabs(0.5f - f2 / f1) * 2.0f;
    float f8 = CARVE_CANYON_VR_DEFAULT + CARVE_CANYON_VR_CENTER * f7;
    double base = (double)f8 * d1;
    return base * (double)uniform_float(r, 0.75f, 1.0f);
}

static void canyon_do_carve(carve_ctx *c, uint64_t seed,
                            double x, double y, double z, float thickness,
                            float yaw, float pitch,
                            int first_step, int total_steps, double y_scale) {
    lysh_lcg r;
    lysh_lcg_set_seed(&r, seed);

    float width[CARVE_GEN_DEPTH];
    canyon_init_width_factors(&r, width);

    float yaw_drift = 0.0f;
    float pitch_drift = 0.0f;

    for (int i = first_step; i < total_steps; i++) {
        float ang = (float)i * 3.1415927f / (float)total_steps;
        double radius = 1.5 + (double)(lysh_mc_sin((double)ang) * thickness);
        double vert_radius = radius * y_scale;

        radius *= (double)uniform_float(&r, 0.75f, 1.0f);
        vert_radius = canyon_update_vertical_radius(&r, vert_radius,
                                                    (float)total_steps, (float)i);

        float cos_pitch = lysh_mc_cos((double)pitch);
        float sin_pitch = lysh_mc_sin((double)pitch);
        x += (double)(lysh_mc_cos((double)yaw) * cos_pitch);
        y += (double)sin_pitch;
        z += (double)(lysh_mc_sin((double)yaw) * cos_pitch);

        /* ⚠️ 顺序与洞穴相反：先 pitch 后 yaw；衰减系数也不同（0.8 / 0.5 vs 0.9 / 0.75）。 */
        pitch *= 0.7f;
        pitch += pitch_drift * 0.05f;
        yaw   += yaw_drift * 0.05f;
        pitch_drift *= 0.8f;
        yaw_drift   *= 0.5f;
        {
            float a1 = lcg_next_float(&r), b1 = lcg_next_float(&r), c1 = lcg_next_float(&r);
            pitch_drift += (a1 - b1) * c1 * 2.0f;
            float a2 = lcg_next_float(&r), b2 = lcg_next_float(&r), c2 = lcg_next_float(&r);
            yaw_drift += (a2 - b2) * c2 * 4.0f;
        }

        if (lysh_lcg_next_int_bound(&r, 4) == 0) continue;
        {
            double dx = x - c->tgt_mid_x, dz = z - c->tgt_mid_z;
            double dy = (double)(total_steps - i);
            double rr = (double)(thickness + 2.0f + 16.0f);
            if (!(dx * dx + dz * dz - dy * dy <= rr * rr)) return;
        }
        carve_skip sk;
        sk.kind = 1;
        sk.floor_level = 0.0;
        sk.width = width;
        carve_ellipsoid(c, &sk, x, y, z, radius, vert_radius);
    }
}

static void canyon_carve(carve_ctx *c, lysh_lcg *r, int origin_cx, int origin_cz) {
    int i9 = CARVE_I9;
    int lx = lysh_lcg_next_int_bound(r, 16);
    double x = (double)(origin_cx * 16 + lx);
    int y = uniform_height(r, CARVE_CANYON_Y_LO, CARVE_CANYON_Y_HI);
    int lz = lysh_lcg_next_int_bound(r, 16);
    double z = (double)(origin_cz * 16 + lz);
    float yaw = lcg_next_float(r) * 6.2831855f;
    float pitch = uniform_float(r, -0.125f, 0.125f);
    double y_scale = (double)CARVE_CANYON_Y_SCALE;      /* ConstantFloat：不消耗 RNG */
    float thick = trapezoid_float(r, 0.0f, 6.0f, 2.0f);
    float dist = uniform_float(r, 0.75f, 1.0f);
    int steps = (int)((float)i9 * dist);                 /* f2i：向零截断 */
    uint64_t seed = lysh_lcg_next_long(r);
    canyon_do_carve(c, seed, x, (double)y, z, thick, yaw, pitch, 0, steps, y_scale);
}

/* ---------------- `applyCarvers` ---------------- */

void lysh_carver_apply(uint64_t world_seed,
                       int target_chunk_x, int target_chunk_z,
                       lysh_carver_base_fn base_fn, void *base_user,
                       lysh_aquifer *aq, uint8_t *cells) {
    /* 掩码：目标 chunk 一份，17x17 遍历 + 3 个雕刻器**全部共用**（12 KiB 栈上）。 */
    uint64_t mask[LYSH_CARVE_MASK_WORDS];

    carve_ctx c;
    c.tgt_min_x = target_chunk_x * 16;
    c.tgt_min_z = target_chunk_z * 16;
    c.tgt_mid_x = (double)(c.tgt_min_x + 8);
    c.tgt_mid_z = (double)(c.tgt_min_z + 8);
    c.base_fn = base_fn;
    c.base_user = base_user;
    c.aq = aq;
    c.cells = cells;
    c.mask = mask;

    memset(cells, LYSH_CARVE_UNCHANGED, (size_t)LYSH_CARVE_CELLS);
    memset(mask, 0, sizeof(mask));

    for (int dx = -8; dx <= 8; dx++) {                  /* ⚠️ dx 在外层，半径硬编码 8 */
        for (int dz = -8; dz <= 8; dz++) {
            int ox = target_chunk_x + dx;
            int oz = target_chunk_z + dz;
            /* 主世界 54 个群系全部声明同一张雕刻器表（见 carver.h），所以
             * carverIndex / 概率 / y 范围都可以直接写死，不需要查群系。 */
            for (int ci = 0; ci < 3; ci++) {
                float prob = (ci == 0) ? CARVE_CAVE_PROB
                           : (ci == 1) ? CARVE_EXTRA_PROB : CARVE_CANYON_PROB;
                lysh_lcg r;
                /* `WorldgenRandom.setLargeFeatureSeed(worldSeed + carverIndex, ox, oz)`
                 * —— 26.1.2 没有 setCarverSeed，也没有 `| 1L`；逐条对过 1.18.2 的
                 * `cuv.c(JII)`、1.19.2 的 `dbo.c(JII)`、1.20 的 `dij.c(JII)`、
                 * 1.21 的 `dzx.c(JII)`，**字节码逐条相同**（见 carver.h 的版本口径）。 */
                lysh_set_carver_seed(&r, world_seed + (uint64_t)ci, ox, oz);
                float f = lcg_next_float(&r);
                if (!(f <= prob)) continue;             /* `isStartChunk`：fcmpg; ifgt -> false */
                if (ci == 1) {
                    /* cave_extra_underground 与 cave 只差 probability 与 y 上界。 */
                    cave_carve(&c, &r, ox, oz, CARVE_EXTRA_Y_HI);
                } else if (ci == 0) {
                    cave_carve(&c, &r, ox, oz, CARVE_CAVE_Y_HI);
                } else {
                    canyon_carve(&c, &r, ox, oz);
                }
            }
        }
    }
}
