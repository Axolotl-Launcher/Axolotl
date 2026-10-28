/* lysh main.c —— 命令行入口。
 *
 * 阶段 1（候选筛选）+ 阶段 2（精确 footprint 平均高度 + 灌水判定）已经全接上：
 * `lysh scan` 的行为就是完整流水线，走的代码路径与 JNI（Java GUI）**完全同一条**：
 *     lysh_scan_rect(hook=...) → 每个幸存者 → lysh_eval_hut()
 *
 * ⚠️ **没有 phase-1-only 模式**：`lysh scan` 永远跑阶段 1 + 阶段 2。历史上有个
 *    `--no-phase2`（以及 `--list` 的四元组旧格式）已删除；`--list` 现在是唯一的
 *    六列格式，见下。
 *
 * 用法：
 *   lysh scan --seed <long> [--regions N | --rx0 A --rx1 B --rz0 C --rz1 D]
 *             [--threads T] [--max-y M] [--phase2-max-y Y] [--version V] [--preset P]
 *             [--list] [--quiet] [--repeat N]
 *   lysh hut  --seed <long> --x <hutX> --z <hutZ> [--max-y M] [--phase2-max-y Y]
 *             [--version V] [--preset P]
 *
 *   --version  26.2~26.3（默认）| 1.19.x~26.1 | 1.18.x
 *              （亦接受旧别名：26.2 / 26.3 / 1.21 / 1.20.1 / 1.19.2 / 1.18.2 等）
 *   --preset   NORMAL（默认）| LARGE_BIOMES | SINGLE_BIOME
 *   --max-y    阶段 1 的 maxHeight，默认 -40；给 -50/-54 时内部一律按 -50
 *              （与生产代码的 phase1CheckHeight 一致）
 *   --phase2-max-y  阶段 2 的最终 Y 门槛（判据 avg_y <= 该值）。默认 = 阶段 1 的 maxHeight
 *   --list     输出每个候选的结果
 *   --repeat N 同一批扫 N 遍（基准 + 自洽自检，见下面 cmd_scan 里的说明）
 *
 * `--list` 的**唯一**输出格式（列 = rx rz x y z flooded）：
 *     hit -585656  -568941  -18740992 -54 -18206128 0
 *   y 是 footprint 平均高度（Java 产品 `/tp %d %.0f %d` 的 Y）。
 *   第七列 flooded：1 = 整片 footprint 在 y=62 被水灌满（这类候选一律 reject）。
 *   每行一条、以 "hit " 开头，按位置取前 5 列即可。（旧的
 *   `hit rx rz hutX hutZ` 四元组已删除 —— 需要它的脚本请改用六列，前 5 列含义相同。）
 *
 * erosion 门（95% 的格子死在这一门）只有**一条**路径：分层提前退出
 * （近似；用户规格的七档阈值，见 README §7）。报告里的 `tier hit distribution`
 * 给出每档淘汰了多少格。预计算失败时自动回退到精确表达式（climate.c 会报一行）。
 *
 *   lysh selftest <seed>
 */
#include <inttypes.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "eval.h"
#include "noise.h"
#include "search.h"
#include "structure.h"

static void usage(void) {
    printf("lysh 2.0.2 -- LowYSwampHut C core (phase 1 + phase 2)\n\n");
    printf("usage:\n");
    printf("  lysh scan --seed <long> [--regions N | --rx0 A --rx1 B --rz0 C --rz1 D]\n");
    printf("            [--threads T] [--max-y M] [--phase2-max-y Y] [--version V] [--preset P]\n");
    printf("            [--list] [--quiet] [--repeat N]\n");
    printf("  lysh hut  --seed <long> --x <hutX> --z <hutZ> [--max-y M] [--phase2-max-y Y]\n");
    printf("            [--version V] [--preset P]\n");
    printf("  lysh selftest <seed>\n\n");
    printf("  --version  26.2~26.3 (default) | 1.19.x~26.1 | 1.18.x\n");
    printf("  --preset   NORMAL (default) | LARGE_BIOMES | SINGLE_BIOME\n");
    printf("  --max-y    phase-1 maxHeight, default -40; -50/-54 are clamped to -50\n");
    printf("  --phase2-max-y  final Y gate (avg_y <= this); default = --max-y\n");
    printf("  --repeat N       run the same scan N times, report best/worst (benchmark + self-check)\n");
}

/* ------------------------------------------------------------------ */
/* selftest（里程碑 1 的自检）                                          */
/* ------------------------------------------------------------------ */
static int selftest(uint64_t seed) {
    lysh_dblnoise erosion;
    lysh_dblnoise_from_seed_id(&erosion, seed, "minecraft:erosion");

    int nonnull0 = 0, nonnull1 = 0;
    for (int i = 0; i < erosion.sub[0].n; i++) {
        if (erosion.sub[0].oct[i].present) nonnull0++;
        if (erosion.sub[1].oct[i].present) nonnull1++;
    }

    printf("seed            = %" PRIu64 "\n", seed);
    printf("erosion slots   = %d  (non-null: %d + %d)\n", erosion.sub[0].n, nonnull0, nonnull1);
    printf("amplitude       = %.17g\n", erosion.amplitude);
    printf("lacunarity      = %.17g\n", erosion.sub[0].lacunarity);
    printf("persistence     = %.17g\n", erosion.sub[0].persistence);
    printf("\nsample y=0  x=1000 z=2000  = %.17g\n", lysh_dblnoise_sample_y0(&erosion, 1000.0, 2000.0));
    printf("sample y=0  x=4*12345+2 z=4*(-6789)+2 = %.17g\n",
           lysh_dblnoise_sample_y0(&erosion, 4.0 * 12345 + 2, 4.0 * -6789 + 2));
    return 0;
}

/* ------------------------------------------------------------------ */
/* 参数解析                                                            */
/* ------------------------------------------------------------------ */
static const char *const REJECT_NAMES[LYSH_P1_REJECT_KIND_N] = {
    "erosion < 0.55", "temperature", "ridge", "Entrance@50", "Entrance@60",
    "cave@maxHeight", "cave ladder (y<=0)", "cave ladder (y>0)",
    "continentalness < -0.11", "floodedness > 0.41", "ACCEPT"
};

/* 版本 → 阶段 1 的门。
 * ⚠️ **默认是 26.2~26.3**（用户明确要求）：所有 opts 全零初始化即 26.2 语义，
 *    所以这里只有在**不是** 26.2 路径时才去动 pre_26_2。
 * 底层三条逻辑：1.18.x / 1.19.x~26.1 / 26.2~26.3。 */
static int parse_version(const char *s, lysh_phase1_opts *o) {
    if (!strcmp(s, "1.18.2") || !strcmp(s, "1.18") || !strcmp(s, "1.18.x") || !strcmp(s, "1.18.1")) {
        o->mc_1_18_2 = 1; o->pre_26_2 = 1; return 0;
    }
    if (!strcmp(s, "1.19.2") || !strcmp(s, "1.19") || !strcmp(s, "1.19.x")
        || !strcmp(s, "1.20.1") || !strcmp(s, "1.20") || !strcmp(s, "1.20.x")
        || !strcmp(s, "1.21") || !strcmp(s, "1.21.1") || !strcmp(s, "1.21.x~26.1")
        || !strcmp(s, "1.21.5~26.1") || !strcmp(s, "1.19.x~26.1") || !strcmp(s, "26.1")) {
        o->mc_1_18_2 = 0; o->pre_26_2 = 1; return 0;
    }
    if (!strcmp(s, "26.2") || !strcmp(s, "26.3") || !strcmp(s, "26.2~26.3")) {
        o->mc_1_18_2 = 0; o->pre_26_2 = 0; return 0;
    }
    return 1;
}

static int parse_preset(const char *s, lysh_phase1_opts *o) {
    if (!strcmp(s, "NORMAL")) return 0;
    if (!strcmp(s, "LARGE_BIOMES")) { o->large_biomes = 1; return 0; }
    if (!strcmp(s, "SINGLE_BIOME")) { o->single_biome = 1; return 0; }
    return 1;
}

/* 生产代码的 phase1CheckHeight：选 -50/-54 时阶段 1 一律按 -50 */
static int phase1_check_height(int max_height) {
    return max_height < -50 ? -50 : max_height;
}

static int cmd_scan(int argc, char **argv) {
    lysh_scan_opts o;
    memset(&o, 0, sizeof(o));
    o.seed = 0;
    o.max_height = -40;
    o.threads = 0;
    o.salt = LYSH_SWAMP_HUT_SALT;
    /* phase2_hook 留 NULL：第一遍只收 funnel 数字。阶段 2 一点不省 —— 它统一由
     * lysh_grade_scan 承担（唯一一处真正接线的地方，CLI 与 JNI 共用）。 */

    int have_seed = 0, have_regions = 0;
    long long regions = 0;
    /* 哨兵必须用 INT_MIN：区域坐标的合法值包括负数（全图是 ±58594），
     * 用 -1 当"未设置"会把 rx0=-58594 夹成 0，静默只扫一个象限。 */
    int rx0 = INT_MIN, rx1 = INT_MIN, rz0 = INT_MIN, rz1 = INT_MIN;
    int list = 0, quiet = 0;
    /* --repeat N：同一批扫 N 遍，报告 best/worst。这不只是基准开关 —— 循环里还会
     * **逐遍比对** scanned/accepted/erosion 是否与第 0 遍一致。ctest 那套回归网删掉
     * 之后，这是产品里**唯一**剩下的"同一份代码跑两遍必须给出同一个答案"的自检，
     * 而且 README §7.1 的实测数字靠它取最好值。**不要删。** */
    int repeat = 1;
    int p2_max_y = INT_MIN;     /* INT_MIN = 未指定 → 用阶段 1 的 maxHeight */

    for (int i = 0; i < argc; i++) {
        const char *a = argv[i];
        const char *val = (i + 1 < argc) ? argv[i + 1] : NULL;
#define NEED() do { if (!val) { fprintf(stderr, "lysh: %s needs a value\n", a); return 2; } i++; } while (0)
        if (!strcmp(a, "--seed")) { NEED(); o.seed = strtoull(val, NULL, 10); have_seed = 1; }
        else if (!strcmp(a, "--regions")) { NEED(); regions = strtoll(val, NULL, 10); have_regions = 1; }
        else if (!strcmp(a, "--rx0")) { NEED(); rx0 = atoi(val); }
        else if (!strcmp(a, "--rx1")) { NEED(); rx1 = atoi(val); }
        else if (!strcmp(a, "--rz0")) { NEED(); rz0 = atoi(val); }
        else if (!strcmp(a, "--rz1")) { NEED(); rz1 = atoi(val); }
        else if (!strcmp(a, "--threads")) { NEED(); o.threads = atoi(val); }
        else if (!strcmp(a, "--max-y")) { NEED(); o.max_height = atoi(val); }
        else if (!strcmp(a, "--phase2-max-y")) { NEED(); p2_max_y = atoi(val); }
        else if (!strcmp(a, "--version")) { NEED(); if (parse_version(val, &o.opts)) { fprintf(stderr, "lysh: bad --version %s\n", val); return 2; } }
        else if (!strcmp(a, "--preset")) { NEED(); if (parse_preset(val, &o.opts)) { fprintf(stderr, "lysh: bad --preset %s\n", val); return 2; } }
        else if (!strcmp(a, "--list")) { list = 1; }
        else if (!strcmp(a, "--quiet")) { quiet = 1; }
        else if (!strcmp(a, "--repeat")) { NEED(); repeat = atoi(val); }
        else { fprintf(stderr, "lysh: unknown option %s\n", a); return 2; }
#undef NEED
    }

    if (!have_seed) { fprintf(stderr, "lysh: --seed is required\n"); return 2; }

    int rect = (rx0 != INT_MIN || rx1 != INT_MIN || rz0 != INT_MIN || rz1 != INT_MIN);
    if (!rect && !have_regions) { have_regions = 1; regions = 6000LL * 6000LL; }
    if (rect) {
        if (rx0 == INT_MIN || rz0 == INT_MIN || rx1 == INT_MIN || rz1 == INT_MIN) {
            fprintf(stderr, "lysh: a rect scan needs all of --rx0 --rx1 --rz0 --rz1\n");
            return 2;
        }
    }

    o.max_height = phase1_check_height(o.max_height);
    if (p2_max_y == INT_MIN) p2_max_y = o.max_height;
    if (repeat < 1) repeat = 1;

    /* 阶段 2 的统计容器。命中明细**不在这里收集** —— 钩子只累加计数，
     * 明细在扫描结束后（单线程）由 lysh_grade_scan 精确回放一遍，这样
     * ① 没有并发写共享缓冲的数据竞争，② 列表不会因为容量不足被截断。 */
    lysh_hut_grade p2grade;
    memset(&p2grade, 0, sizeof(p2grade));

    lysh_scan_result res;
    int rc = 0;
    double best = 1e30, worst = 0.0;
    for (int rep = 0; rep < repeat; rep++) {
        lysh_scan_result r1;
        if (rect) rc = lysh_scan_rect(&o, rx0, rx1, rz0, rz1, &r1);
        else      rc = lysh_scan_linear(&o, regions, &r1);
        if (rc) { fprintf(stderr, "lysh: scan failed (%d)\n", rc); return 1; }

        if (r1.seconds < best) best = r1.seconds;
        if (r1.seconds > worst) worst = r1.seconds;
        if (rep == 0) {
            res = r1;                       /* 保留第一遍的统计 */
        } else {
            /* 后面的遍只用来计时；统计必须与第一遍一致（否则代码有状态泄漏）。
             * 这是产品里剩下的最后一道自检 —— 见上面 --repeat 的说明。 */
            if (r1.scanned != res.scanned || r1.accepted != res.accepted
                || r1.reject_hist[LYSH_P1_REJECT_EROSION] != res.reject_hist[LYSH_P1_REJECT_EROSION]) {
                fprintf(stderr, "lysh: !! repeat run %d disagrees with run 0 "
                                "(scanned %lld/%lld, accepted %lld/%lld, erosion %lld/%lld)\n",
                        rep, r1.scanned, res.scanned, r1.accepted, res.accepted,
                        r1.reject_hist[LYSH_P1_REJECT_EROSION],
                        res.reject_hist[LYSH_P1_REJECT_EROSION]);
            }
        }
    }

    /* ------------------------------------------------------------------ *
     * 阶段 2：交给 lysh_grade_scan（= 阶段 1 过滤 + 每个幸存者 lysh_eval_hut）。
     * 刻意**不**用自己的 phase2_hook 做统计：那个钩子的 user 是全局的，
     * 多线程会并发写同一份计数器（数据竞争）。lysh_grade_scan 内部虽然也用
     * 同一个钩子机制，但它的 grade 是**每线程私有的局部变量**，结束后再合并。
     * 代价：阶段 1 要再跑一遍（本机 100M 格 ≈ 2.6 s），换来的是"统计与明细
     * 必然自洽 + 无锁无竞争"。
     * ------------------------------------------------------------------ */
    int *p2hits = NULL;
    {
        int r0, r1_, z0, z1;
        if (rect) {
            r0 = rx0; r1_ = rx1; z0 = rz0; z1 = rz1;
        } else {
            /* linear：区域格枚举顺序不影响"哪些候选被评估过"这个集合，
             * 用覆盖前 n 个线性格的矩形包络即可。 */
            long long rows = (regions + 5999) / 6000;
            if (rows < 1) rows = 1;
            if (rows > 6000) rows = 6000;
            r0 = 0; r1_ = 6000; z0 = 0; z1 = (int)rows;
        }

        long long cap_ll = res.accepted;             /* 通过数不可能超过幸存数 */
        if (cap_ll > 1000000) cap_ll = 1000000;      /* 明细缓冲上限，防止意外吃内存 */
        int cap = (int)cap_ll;
        if (list && cap > 0) {
            p2hits = (int *)malloc((size_t)cap * LYSH_HUT_GRADE_INTS * sizeof(int));
            if (!p2hits) { cap = 0; fprintf(stderr, "lysh: --list 明细缓冲分配失败，改为只统计\n"); }
        }
        p2grade.hits = p2hits;
        p2grade.hits_cap = cap;
        lysh_grade_scan(o.seed, &o.opts, o.max_height, r0, r1_, z0, z1,
                        o.salt ? o.salt : LYSH_SWAMP_HUT_SALT, o.threads,
                        p2_max_y, &p2grade);

        /* 自洽检查：两遍阶段 1 的幸存数必须一致（否则枚举范围错了） */
        if (p2grade.evaluated != res.accepted) {
            fprintf(stderr, "lysh: !! phase-2 pass saw %lld survivors but the phase-1 pass "
                            "saw %lld -- 枚举范围不一致，请报告这个 bug\n",
                    p2grade.evaluated, res.accepted);
        }
    }

    if (!quiet) {
        printf("lysh scan  (phase 1 + phase 2)\n");
        printf("  seed        : %" PRId64 "\n", (int64_t)o.seed);
        printf("  version     : %s%s\n", o.opts.mc_1_18_2 ? "1.18.x" : (o.opts.pre_26_2 ? "1.19.x~26.1" : "26.2~26.3"),
               o.opts.single_biome ? "  preset=SINGLE_BIOME" : (o.opts.large_biomes ? "  preset=LARGE_BIOMES" : ""));
        printf("  erosion     : TIERED early exit (approximate; exact fallback if unavailable)\n");
        printf("  maxHeight   : %d   (phase 1)\n", o.max_height);
        printf("  phase2 max-y: %d   (final gate: avg_y <= max-y, and not flooded)\n", p2_max_y);
        printf("  threads     : %d\n", o.threads > 0 ? o.threads : lysh_cpu_count());
        if (rect) {
            long long expect = (long long)(rx1 - rx0) * (long long)(rz1 - rz0);
            printf("  rect        : rx [%d, %d)  rz [%d, %d)  = %lld cells\n",
                   rx0, rx1, rz0, rz1, expect);
            if (expect != res.scanned) {
                printf("  !! scanned %lld != expected %lld -- 枚举范围与请求不符\n",
                       res.scanned, expect);
            }
        }
        printf("  scanned     : %lld\n", res.scanned);
        if (repeat > 1) {
            printf("  elapsed     : best %.3f s / worst %.3f s  (%d runs)\n", best, worst, repeat);
            printf("  ns/cell     : best %.1f   worst %.1f   (from best)\n",
                   res.scanned ? best * 1e9 / (double)res.scanned : 0.0,
                   res.scanned ? worst * 1e9 / (double)res.scanned : 0.0);
            printf("  throughput  : %.2f M cells/s (best)\n",
                   best > 0 ? (double)res.scanned / best / 1e6 : 0.0);
        } else {
            printf("  elapsed     : %.3f s   (%.1f ns/cell)\n", res.seconds,
                   res.scanned ? res.seconds * 1e9 / (double)res.scanned : 0.0);
            printf("  throughput  : %.2f M cells/s\n",
                   res.seconds > 0 ? (double)res.scanned / res.seconds / 1e6 : 0.0);
        }
        printf("\n  funnel:\n");
        for (int i = 0; i < LYSH_P1_REJECT_KIND_N; i++) {
            if (!res.reject_hist[i]) continue;
            printf("    %-24s %12lld  (%7.4f%%)\n", REJECT_NAMES[i], res.reject_hist[i],
                   res.scanned ? 100.0 * (double)res.reject_hist[i] / (double)res.scanned : 0.0);
        }
        printf("    reached cave ladder    %12lld\n", res.reached_ladder);
        printf("\n  phase-1 survivors: %lld\n", res.accepted);

        {
            int nthreads = o.threads > 0 ? o.threads : lysh_cpu_count();
            printf("\n  phase 2 (exact footprint average + aquifer judgment)\n");
            printf("    evaluated    : %lld\n", p2grade.evaluated);
            printf("    accepted     : %lld\n", p2grade.accepted);
            printf("    reject biome : %lld   (chunk-centre surface biome is not minecraft:swamp)\n",
                   p2grade.rejected_biome);
            printf("    reject avg_y : %lld\n", p2grade.rejected_y);
            printf("    reject flood : %lld\n", p2grade.rejected_flood);
            printf("    phase-2 cpu  : %.3f s total across %d thread(s) = %.2f ms/candidate\n",
                   p2grade.p2_ms / 1000.0, nthreads,
                   p2grade.evaluated ? p2grade.p2_ms / (double)p2grade.evaluated : 0.0);
            printf("    full pass    : %.3f s (phase 1 + phase 2, re-run of the scan above)\n",
                   p2grade.ms / 1000.0);
        }

        {
            const lysh_tier_spec *spec = lysh_tier_spec_erosion();
            long long tier_total = 0;
            for (int i = 0; i < spec->n_tier; i++) tier_total += res.tier_hist[i];
            printf("\n  erosion tiered early exit  (thresholds verbatim from the spec)\n");
            for (int i = 0; i < spec->n_tier; i++) {
                printf("    tier %d  %-24s %12lld  (%7.4f%% of scanned, %6.2f%% of the gate)\n",
                       i, lysh_tier_name(spec, i), res.tier_hist[i],
                       res.scanned ? 100.0 * (double)res.tier_hist[i] / (double)res.scanned : 0.0,
                       tier_total ? 100.0 * (double)res.tier_hist[i] / (double)tier_total : 0.0);
            }
            printf("    erosion gate rejections  %12lld\n", res.reject_hist[LYSH_P1_REJECT_EROSION]);
            printf("    tier hits total          %12lld  (%.4f%% of the erosion gate)\n",
                   tier_total,
                   res.reject_hist[LYSH_P1_REJECT_EROSION]
                       ? 100.0 * (double)tier_total / (double)res.reject_hist[LYSH_P1_REJECT_EROSION]
                       : 0.0);
        }
    }

    if (list) {
        /* 明细已经在上面那次 lysh_grade_scan 里算好了（唯一的阶段 2 / 明细出口）。
         * 列： rx rz x y z flooded —— y 就是 Java 产品 `/tp` 的 Y。
         * ⚠️ 这是**唯一**的 `--list` 格式；旧的四元组随 --no-phase2 一起删除。 */
        for (int i = 0; i < p2grade.hits_written; i++) {
            const int *g = p2hits + (size_t)i * LYSH_HUT_GRADE_INTS;
            printf("hit %d %d %d %d %d %d\n", g[2], g[3], g[0], g[5], g[1], g[6]);
        }
    }

    free(p2hits);
    return 0;
}

/* ------------------------------------------------------------------ */
/* lysh hut —— 单个候选的产品输出（与 Java 的 `/tp` 对齐）              */
/* ------------------------------------------------------------------ */
static int cmd_hut(int argc, char **argv) {
    lysh_phase1_opts opts;
    memset(&opts, 0, sizeof(opts));
    /* 默认 26.2（用户明确要求）；opts 全零就已经是 26.2 语义（pre_26_2 = 0）。 */

    uint64_t seed = 0;
    int hut_x = 0, hut_z = 0, max_height = -40, p2_max_y = INT_MIN;
    int have_seed = 0, have_x = 0, have_z = 0;

    for (int i = 0; i < argc; i++) {
        const char *a = argv[i];
        const char *val = (i + 1 < argc) ? argv[i + 1] : NULL;
#define NEED() do { if (!val) { fprintf(stderr, "lysh: %s needs a value\n", a); return 2; } i++; } while (0)
        if (!strcmp(a, "--seed")) { NEED(); seed = strtoull(val, NULL, 10); have_seed = 1; }
        else if (!strcmp(a, "--x")) { NEED(); hut_x = atoi(val); have_x = 1; }
        else if (!strcmp(a, "--z")) { NEED(); hut_z = atoi(val); have_z = 1; }
        else if (!strcmp(a, "--max-y")) { NEED(); max_height = atoi(val); }
        else if (!strcmp(a, "--phase2-max-y")) { NEED(); p2_max_y = atoi(val); }
        else if (!strcmp(a, "--version")) { NEED(); if (parse_version(val, &opts)) { fprintf(stderr, "lysh: bad --version %s\n", val); return 2; } }
        else if (!strcmp(a, "--preset")) { NEED(); if (parse_preset(val, &opts)) { fprintf(stderr, "lysh: bad --preset %s\n", val); return 2; } }
        else { fprintf(stderr, "lysh: unknown option %s\n", a); return 2; }
#undef NEED
    }
    if (!have_seed || !have_x || !have_z) {
        fprintf(stderr, "lysh hut: --seed --x --z are required\n");
        return 2;
    }
    max_height = phase1_check_height(max_height);
    if (p2_max_y == INT_MIN) p2_max_y = max_height;

    lysh_search_ctx ctx;
    lysh_search_ctx_init(&ctx, seed, &opts, max_height);
    lysh_hut_result r = lysh_eval_hut(&ctx, hut_x, hut_z, p2_max_y);

    /* 阶段 1 的气候探针（与门同源）：erosion / temperature / ridge / continentalness。
     * 顺序即 lysh_phase1_probe 的前四项（见 phase1.c 的顺序定义）。用来解释"这个候选
     * 是被哪一门放行/淘汰的"，也是版本门（26.2 vs 更早）差异的直接证据。 */
    double probe[LYSH_P1_PROBE_N];
    lysh_phase1_probe(&ctx.p1, hut_x, hut_z, max_height, probe);
    lysh_search_ctx_free(&ctx);

    {
        const lysh_phase1_opts *o = &opts;
        int t_ok = o->mc_1_18_2 ? (probe[1] >= -0.45)
                                : (probe[1] >= -0.45 && probe[1] <= 0.2);
        int g_ok = !((probe[2] > 0.42 && probe[2] < 0.91)
                  || (probe[2] < -0.42 && probe[2] > -0.91));
        if (!o->pre_26_2 && probe[2] <= -0.91) g_ok = 0;
        printf("  climate     : erosion=%.6f  temperature=%.6f  ridge=%.6f  continentalness=%.6f\n",
               probe[0], probe[1], probe[2], probe[3]);
        printf("  version gate: erosion>=0.55 %s | temperature %s | ridge(non-band%s) %s | cont>=-0.11 %s\n",
               probe[0] >= 0.55 ? "PASS" : "FAIL", t_ok ? "PASS" : "FAIL",
               o->pre_26_2 ? "" : " + >-0.91", g_ok ? "PASS" : "FAIL",
               probe[3] >= -0.11 ? "PASS" : "FAIL");
    }

    printf("seed %" PRId64 "  hut (%d, %d)\n", (int64_t)seed, hut_x, hut_z);
    printf("  orientation : dir=%d  footprint %d(x) x %d(z)\n", r.dir, r.size_x, r.size_z);
    printf("  footprint   : sum(h)=%lld / 63  ->  avg_y = %d\n", r.sum_h, r.avg_y);
    printf("  aquifer     : %d/63 columns flooded at y=62  ->  %s\n",
           r.wet_columns, r.flooded ? "FLOODED" : (r.all_dry ? "all dry" : "partly wet"));
    /* 真实群系门（MC 的 Structure.isValidBiome）：chunk 中心列、WORLD_SURFACE_WG 高度。
     * 中心方块 = (chunkX*16+8, chunkZ*16+8) = (qx*4, qz*4)（中心列是 4 的倍数）。 */
    printf("  biome       : chunk-centre block (%d,%d)  occupied_y=%d  quart=(%d,%d,%d)\n",
           r.biome_qx * 4, r.biome_qz * 4, r.biome_occ_y,
           r.biome_qx, r.biome_qy, r.biome_qz);
    printf("  biome params: t=%lld h=%lld c=%lld e=%lld d=%lld w=%lld   (quantized)\n",
           r.biome_t[0], r.biome_t[1], r.biome_t[2], r.biome_t[3], r.biome_t[4], r.biome_t[5]);
    printf("  biome argmin: nearest-biome search returned id=%d (6 = swamp) -> %s\n",
           r.biome_winner, r.biome_ok ? "swamp OK" : "NOT a swamp biome");
    printf("  biome tree  : %s parameter data, version-selected"
           " (btree18 / btree215 / btree262; see biome.h)\n", lysh_biome_data_version());
    {
        const char *reason = r.ok ? "ok"
            : (r.reject == LYSH_HUT_REJECT_BIOME
                   ? "biome: chunk-centre surface biome is not minecraft:swamp"
               : (r.reject == LYSH_HUT_REJECT_Y ? "avg_y > max-y"
               : (r.reject == LYSH_HUT_REJECT_FLOOD ? "footprint flooded at y=62" : "?")));
        printf("  verdict     : %s  reason=%s\n", r.ok ? "ACCEPT" : "REJECT", reason);
        printf("  gates       : biome %s | avg_y %d %s %d | footprint %s\n",
               r.biome_ok ? "PASS" : "FAIL", r.avg_y, r.avg_y <= p2_max_y ? "<=" : ">", p2_max_y,
               r.flooded ? "FLOODED (reject)" : "not flooded");
    }
    /* 与 Java 产品同一口径的一行：/tp %d %.0f %d（Y = footprint 平均高度） */
    printf("  %s\n", lysh_hut_tp_line(&r));
    return r.ok ? 0 : 1;
}

int main(int argc, char **argv) {
    if (argc >= 2 && !strcmp(argv[1], "selftest")) {
        if (argc < 3) { usage(); return 2; }
        return selftest(strtoull(argv[2], NULL, 10));
    }
    if (argc >= 2 && !strcmp(argv[1], "scan")) {
        return cmd_scan(argc - 2, argv + 2);
    }
    if (argc >= 2 && !strcmp(argv[1], "hut")) {
        return cmd_hut(argc - 2, argv + 2);
    }
    usage();
    return argc == 1 ? 0 : 2;
}
