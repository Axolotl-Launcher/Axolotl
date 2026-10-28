/* lysh jni_bridge.c —— 把 C 内核暴露给 Java GUI 壳。
 *
 * 设计原则：JNI 层只做「拆参数 / 拷结果」，**不含任何判定逻辑** ——
 * 这样它永远不可能与被对拍过的 C 核心产生分歧。所有数学都在
 * src/phase1.c / src/phase2.c / src/eval.c。
 *
 * ── 两个 Java 类 ────────────────────────────────────────────────────
 *
 * ① `project.NativePhase1`
 *      String coreVersion()      // 只用来确认 lysh.dll 已加载、以及加载的是哪一版
 *
 * ② `project.NativePhase2`（完整流水线：阶段 1 + 阶段 2）
 *      long[] gradeScanNative(long seed, int mc_1_18_2, int single_biome, int v26_2,
 *                             int large_biomes, int maxHeight, int salt, int threads,
 *                             int rx0, int rx1, int rz0, int rz1, int maxY,
 *                             int[] outHits)
 *      long   scanOpen(long seed, int mc_1_18_2, int single_biome, int v26_2, int large_biomes,
 *                      int maxHeight, int salt, int threads)
 *      long[] scanBand(long handle, int rx0, int rx1, int rz0, int rz1, int maxY, int[] outHits)
 *      void   scanClose(long handle)
 *      int    gradeIntsPerHit()  -> LYSH_HUT_GRADE_INTS（= 12）
 *      String coreVersion()
 *
 *      `gradeScanNative` 返回 long[20] = 头 + 明细（明细放在 outHits int[]）：
 *        [0] scanned（区域格数）          [1] phase1Accepted（阶段 1 幸存）
 *        [2] evaluated（= 阶段 1 幸存数）  [3] accepted（阶段 2 通过数）
 *        [4] rejectedY   [5] rejectedFlood
 *        [6] hitsWritten（写进 outHits 的条数）
 *        [7] capacityNeeded = accepted * 12（outHits 太小时为 -1 且 hitsWritten = 0）
 *        [8] rx0 [9] rx1 [10] rz0 [11] rz1
 *        [12] maxY  [13] maxHeight  [14] salt  [15] threads
 *        [16] ms（这一遍的墙钟，毫秒）   [17] ms_phase2（阶段 2 的 CPU 累计毫秒）
 *        [18] hitsIntsPerHit（= 12，冗余，防止 Java 侧写死）
 *        [19] floodedHitCount（明细里 flooded == 1 的条数；正常搜索应为 0）
 *
 *      `scanBand` 返回**同一个** long[20] 布局（两个入口共用 fill 代码，见
 *      grade_result_to_java）：[8..12] 是**本带**的值，[13..15] 是 open 时的
 *      会话级参数回显。
 *
 *      outHits 布局：每候选 12 个 int（见 eval.h 的 grade_fill_hit）：
 *        [0] hutX  [1] hutZ  [2] rx  [3] rz
 *        [4] dir（0=N 1=E 2=S 3=W）  [5] avg_y（footprint 平均高度）
 *        [6] flooded  [7] ok  [8] wet_columns  [9] size_x  [10] size_z  [11] reject
 *
 *      ⚠️ outHits 只装**通过阶段 2 的**候选（ok == 1）。被拒的候选不计入明细，
 *         只计入 [4]/[5] 的计数。
 *
 * ── scanOpen / scanBand / scanClose（多种子性能修复）─────────────────
 *   Java 多种子模式把一个种子切成 256 个 Z 带，老写法每带一次 gradeScanNative ⇒
 *   每带重建 worker ctx + 明细回放再建一份，65k 格实测 95.4 ms/种子，而纯计算只要
 *   ~7.5 ms（成本只跟带数走）。现在：一个种子 scanOpen 一次（建一次噪声栈），
 *   每带 scanBand 复用它，最后 scanClose。
 *
 *   Java 侧怎么用（SearchCoords.runNativeScan 就是这么写的）：
 *     long s = NativePhase2.scanOpen(seed, 1, 0, 0, 0, -50, 0, 1);
 *     try {
 *         long[] st = NativePhase2.scanBand(s, rx0, rx1, z, zEnd, -40, buf);
 *         ...
 *     } finally { NativePhase2.scanClose(s); }
 *
 *   ⚠️ scanClose(0) 是安全的空操作；同一个非 0 handle 只许关一次（Java 侧关完把
 *      字段清零 —— 这就是"可重复调用"的实现方式）。
 *
 * ── Java 侧怎么用（GUI 全流程）────────────────────────────────────────
 *   int[] buf = new int[4096];
 *   long[] st = NativePhase2.gradeScanNative(seed, 1, 0, 0, 0, -50, 8, SALT_INTERNAL,
 *                                            rx0, rx1, rz0, rz1, -40, buf);
 *   int written = (int) st[6];
 *   for (int i = 0; i < written; i++) {
 *       int b = i * 12;
 *       emit(String.format("/tp %d %.0f %d", buf[b], (double) buf[b+5], buf[b+1]));
 *   }
 *
 *   salt 传 0 表示用内核默认的 swamp-hut salt（不要传 Java 侧的默认值）。
 *   threads 传 0 表示用 CPU 核数。
 *
 * ── 线程安全 ─────────────────────────────────────────────────────────
 *   `gradeScanNative` 与 `scanBand` 都是**自包含**的（各自的 handle 里带自己的
 *   worker ctx），所以可以并发调用；不同 handle 之间不共享任何可写状态。
 *   同一个 handle 不许多线程同时用（与 C 侧的 lysh_scan_session 一样）。
 *
 * ── 已删除的入口（不要加回来）────────────────────────────────────────
 *   · `NativePhase1.scanNative`（阶段 1-only 命中列表）：产品只走 gradeScanNative，
 *     Java 侧 `NativePhase1.scan` 也没有任何调用方，两边一起删了。
 *   · `NativePhase2.gradeHutNative` / `hitsCapacity`（单候选评估、容量换算）：
 *     同样没有任何调用方。
 *   · `LYSH_JNI_TRACE=1` 的 stderr trace：版本位现在由 `describe()` / `[engine]`
 *     行报告，不需要在热路径入口里留一条 printf。
 */
#include <jni.h>
#include <stdint.h>     /* intptr_t：句柄在 jlong 与指针之间转换 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "eval.h"
#include "search.h"
#include "structure.h"

/* ------------------------------------------------------------------ */
/* project.NativePhase1                                                */
/* ------------------------------------------------------------------ */
JNIEXPORT jstring JNICALL
Java_project_NativePhase1_coreVersion(JNIEnv *env, jclass cls) {
    (void)cls;
    return (*env)->NewStringUTF(env, "lysh 2.0.2 (phase1: climate + cave ladder + continentalness + floodedness; phase2: exact footprint height + aquifer)");
}

/* ------------------------------------------------------------------ */
/* project.NativePhase2 —— 完整流水线（阶段 1 + 阶段 2）                */
/* ------------------------------------------------------------------ */

/* 头部 long[] 的长度（两个入口共用；Java 侧不要写死 20）。 */
enum { LYSH_JNI_HDR = 20 };

/* 头部的"参数回显"部分：[8..15] + [18]。成绩单相关的那几个下标由
 * grade_result_to_java 填 —— 两个入口共用同一份，布局永远不会分叉。 */
static void grade_header_init(jlong out[LYSH_JNI_HDR],
                              int rx0, int rx1, int rz0, int rz1,
                              int max_y, int max_height, int salt, int threads) {
    memset(out, 0, LYSH_JNI_HDR * sizeof(jlong));
    out[8] = rx0; out[9] = rx1; out[10] = rz0; out[11] = rz1;
    out[12] = max_y; out[13] = max_height;
    out[14] = salt; out[15] = threads;
    out[18] = LYSH_HUT_GRADE_INTS;
}

/* 成绩单 → Java long[20]（并把明细拷进 out_hits）。
 * `hdr` 是 grade_header_init 填好的参数回显；cap 是 out_hits 能装几条命中。 */
static jlongArray grade_result_to_java(JNIEnv *env, const jlong hdr[LYSH_JNI_HDR],
                                       const lysh_hut_grade *stats,
                                       const int *buf, jint cap, jintArray out_hits) {
    jlong out[LYSH_JNI_HDR];
    memcpy(out, hdr, sizeof(out));
    out[0] = stats->scanned;
    out[1] = stats->evaluated;      /* 阶段 1 幸存数 */
    out[2] = stats->evaluated;
    out[3] = stats->accepted;
    out[4] = stats->rejected_y;
    out[5] = stats->rejected_flood;
    out[7] = stats->accepted * LYSH_HUT_GRADE_INTS;
    out[16] = (jlong)stats->ms;     /* 这一遍（阶段 1 + 阶段 2 回放）的墙钟 */
    out[17] = (jlong)stats->p2_ms;  /* 其中阶段 2 的 CPU 累计毫秒 */

    if (out_hits != NULL) {
        if (stats->accepted > 0 && cap < stats->accepted) {
            out[7] = -1;            /* 数组太小：告诉 Java 侧要多大，明细不写 */
        } else if (buf && stats->hits_written > 0) {
            (*env)->SetIntArrayRegion(env, out_hits, 0,
                                      (jsize)(stats->hits_written * LYSH_HUT_GRADE_INTS),
                                      (const jint *)buf);
            out[6] = stats->hits_written;

            /* 明细里 flooded 的条数（正常搜索结果应为 0 —— 灌满的一律被拒） */
            long long nf = 0;
            for (int i = 0; i < stats->hits_written; i++) {
                if (buf[i * LYSH_HUT_GRADE_INTS + 6]) nf++;
            }
            out[19] = nf;
        }
    }

    jlongArray a = (*env)->NewLongArray(env, LYSH_JNI_HDR);
    if (a) (*env)->SetLongArrayRegion(env, a, 0, LYSH_JNI_HDR, out);
    return a;
}

/* 一块调用方给的明细缓冲（int 个数 = cap * 12）。分配失败时返回 NULL。 */
static int *grade_buf_alloc(jintArray out_hits, jint cap) {
    if (out_hits == NULL || cap <= 0) return NULL;
    return (int *)malloc((size_t)cap * LYSH_HUT_GRADE_INTS * sizeof(int));
}

/* 把 JNI 的 4 个版本/预设 int 翻成 C 的 lysh_phase1_opts（两个入口共用）。 */
static void grade_opts_from_java(lysh_phase1_opts *opts,
                                 jint mc_1_18_2, jint single_biome, jint v26_2, jint large_biomes) {
    memset(opts, 0, sizeof(*opts));
    opts->mc_1_18_2 = mc_1_18_2 ? 1 : 0;
    opts->single_biome = single_biome ? 1 : 0;
    /* ⚠️ 反向映射：Java 的 v262 = 1 表示 26.2 ⇒ C 的 pre_26_2 = 0。
     * 全零初始化即 26.2（见 phase1.h），所以"没传"也拿到 26.2 语义。 */
    opts->pre_26_2 = v26_2 ? 0 : 1;
    opts->large_biomes = large_biomes ? 1 : 0;
}

JNIEXPORT jlongArray JNICALL
Java_project_NativePhase2_gradeScanNative(JNIEnv *env, jclass cls,
                                          jlong seed,
                                          jint mc_1_18_2, jint single_biome, jint v26_2, jint large_biomes,
                                          jint max_height, jint salt, jint threads,
                                          jint rx0, jint rx1, jint rz0, jint rz1,
                                          jint max_y,
                                          jintArray out_hits) {
    (void)cls;
    jlong hdr[LYSH_JNI_HDR];
    grade_header_init(hdr, rx0, rx1, rz0, rz1, max_y, max_height, salt, threads);

    lysh_phase1_opts opts;
    grade_opts_from_java(&opts, mc_1_18_2, single_biome, v26_2, large_biomes);

    int use_salt = salt ? salt : LYSH_SWAMP_HUT_SALT;

    /* ---------------------------------------------------------------- *
     * ⚡ 单遍扫描（2026-xx 性能修复）
     *
     * 这里原来跑**两遍阶段 1**：
     *   ① lysh_scan_rect()          —— 只为拿 scanned / 幸存数 / 阶段 1 墙钟
     *   ② lysh_grade_scan(...)      —— 同样的阶段 1，外加阶段 2
     * 全图 1.373e10 格、阶段 1 约 29 ns/格 ⇒ 多扫一遍就是多花 ~400 s。
     * 这正是 Java 产品 948 s vs C CLI ~400 s 的主因。
     *
     * 现在只跑 ②，统计全部由 lysh_hut_grade 返回：
     *   · scanned / evaluated / accepted / rejectedY / rejectedFlood / rejectedBiome
     * 明细直接写进调用方的 out_hits：容量够就一次到位；不够时返回
     * capacityNeeded = -1 并让调用方放大重试（只有那一带会重扫）。
     *
     * ⚠️ 这只是"一个带"的入口；一个种子要扫很多带时请用 scanOpen/scanBand，
     *    否则每带都要重建一次 worker ctx（见文件头的实测数字）。
     * ---------------------------------------------------------------- */
    jsize cap = 0;
    if (out_hits != NULL) cap = (*env)->GetArrayLength(env, out_hits) / LYSH_HUT_GRADE_INTS;

    lysh_hut_grade stats;
    memset(&stats, 0, sizeof(stats));
    int *buf = grade_buf_alloc(out_hits, cap);
    if (buf) { stats.hits = buf; stats.hits_cap = (int)cap; }

    lysh_grade_scan((uint64_t)seed, &opts, max_height, rx0, rx1, rz0, rz1,
                    use_salt, threads, max_y, &stats);

    jlongArray a = grade_result_to_java(env, hdr, &stats, buf, cap, out_hits);
    free(buf);
    return a;
}

/* ------------------------------------------------------------------ */
/* 扫描会话：一个种子只初始化一次噪声栈（多种子模式的性能修复）        */
/* ------------------------------------------------------------------ */

/* scanOpen 返回给 Java 的句柄：C 会话 + 头部要回显的"会话级"参数。
 * （lysh_scan_session 是不透明类型，scanBand 只拿到句柄，所以在这里留一份副本。） */
typedef struct {
    lysh_scan_session *session;
    int max_height;
    int salt;
    int threads;
} jni_scan_handle;

JNIEXPORT jlong JNICALL
Java_project_NativePhase2_scanOpen(JNIEnv *env, jclass cls,
                                   jlong seed,
                                   jint mc_1_18_2, jint single_biome, jint v26_2, jint large_biomes,
                                   jint max_height, jint salt, jint threads) {
    (void)env; (void)cls;
    lysh_phase1_opts opts;
    grade_opts_from_java(&opts, mc_1_18_2, single_biome, v26_2, large_biomes);

    jni_scan_handle *h = (jni_scan_handle *)calloc(1, sizeof(*h));
    if (!h) return 0;
    /* ⚠️ 这里（也只有这里）建整个种子的 worker ctx / 噪声栈。 */
    h->session = lysh_scan_session_open((uint64_t)seed, &opts, max_height, salt, threads);
    if (!h->session) { free(h); return 0; }
    h->max_height = max_height;
    h->salt = salt;
    h->threads = threads;
    return (jlong)(intptr_t)h;
}

JNIEXPORT jlongArray JNICALL
Java_project_NativePhase2_scanBand(JNIEnv *env, jclass cls, jlong handle,
                                   jint rx0, jint rx1, jint rz0, jint rz1,
                                   jint max_y, jintArray out_hits) {
    (void)cls;
    jni_scan_handle *h = (jni_scan_handle *)(intptr_t)handle;
    if (!h) return NULL;

    jlong hdr[LYSH_JNI_HDR];
    grade_header_init(hdr, rx0, rx1, rz0, rz1, max_y, h->max_height, h->salt, h->threads);

    jsize cap = 0;
    if (out_hits != NULL) cap = (*env)->GetArrayLength(env, out_hits) / LYSH_HUT_GRADE_INTS;

    lysh_hut_grade stats;
    memset(&stats, 0, sizeof(stats));
    int *buf = grade_buf_alloc(out_hits, cap);
    if (buf) { stats.hits = buf; stats.hits_cap = (int)cap; }

    /* 复用会话里已建好的 ctx：本带不再初始化任何噪声栈（回放也不用）。 */
    lysh_scan_session_band(h->session, rx0, rx1, rz0, rz1, max_y, &stats);

    jlongArray a = grade_result_to_java(env, hdr, &stats, buf, cap, out_hits);
    free(buf);
    return a;
}

JNIEXPORT void JNICALL
Java_project_NativePhase2_scanClose(JNIEnv *env, jclass cls, jlong handle) {
    (void)env; (void)cls;
    jni_scan_handle *h = (jni_scan_handle *)(intptr_t)handle;
    if (!h) return;                     /* 0 是合法句柄（open 失败）→ 空操作 */
    lysh_scan_session_free(h->session);
    free(h);
}

JNIEXPORT jint JNICALL
Java_project_NativePhase2_gradeIntsPerHit(JNIEnv *env, jclass cls) {
    (void)env; (void)cls;
    return (jint)LYSH_HUT_GRADE_INTS;
}

JNIEXPORT jstring JNICALL
Java_project_NativePhase2_coreVersion(JNIEnv *env, jclass cls) {
    (void)cls;
    return (*env)->NewStringUTF(env, "lysh 2.0.2 (phase1+phase2 end-to-end: exact footprint avg height + aquifer flood judgment)");
}
