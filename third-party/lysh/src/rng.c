/* lysh rng.c —— 见 rng.h 顶部的语义来源说明。 */
#include "rng.h"

#include <string.h>

/* ---------------- MD5（RFC 1321，公开算法） ---------------- */
#define MD5_F(x, y, z) (((x) & (y)) | (~(x) & (z)))
#define MD5_G(x, y, z) (((x) & (z)) | ((y) & ~(z)))
#define MD5_H(x, y, z) ((x) ^ (y) ^ (z))
#define MD5_I(x, y, z) ((y) ^ ((x) | ~(z)))
#define MD5_ROTL(x, n) (((x) << (n)) | ((x) >> (32 - (n))))

static void md5_block(uint32_t h[4], const uint8_t *p) {
    static const uint32_t K[64] = {
        0xd76aa478u,0xe8c7b756u,0x242070dbu,0xc1bdceeeu,0xf57c0fafu,0x4787c62au,0xa8304613u,0xfd469501u,
        0x698098d8u,0x8b44f7afu,0xffff5bb1u,0x895cd7beu,0x6b901122u,0xfd987193u,0xa679438eu,0x49b40821u,
        0xf61e2562u,0xc040b340u,0x265e5a51u,0xe9b6c7aau,0xd62f105du,0x02441453u,0xd8a1e681u,0xe7d3fbc8u,
        0x21e1cde6u,0xc33707d6u,0xf4d50d87u,0x455a14edu,0xa9e3e905u,0xfcefa3f8u,0x676f02d9u,0x8d2a4c8au,
        0xfffa3942u,0x8771f681u,0x6d9d6122u,0xfde5380cu,0xa4beea44u,0x4bdecfa9u,0xf6bb4b60u,0xbebfbc70u,
        0x289b7ec6u,0xeaa127fau,0xd4ef3085u,0x04881d05u,0xd9d4d039u,0xe6db99e5u,0x1fa27cf8u,0xc4ac5665u,
        0xf4292244u,0x432aff97u,0xab9423a7u,0xfc93a039u,0x655b59c3u,0x8f0ccc92u,0xffeff47du,0x85845dd1u,
        0x6fa87e4fu,0xfe2ce6e0u,0xa3014314u,0x4e0811a1u,0xf7537e82u,0xbd3af235u,0x2ad7d2bbu,0xeb86d391u
    };
    static const uint8_t S[64] = {
        7,12,17,22, 7,12,17,22, 7,12,17,22, 7,12,17,22,
        5, 9,14,20, 5, 9,14,20, 5, 9,14,20, 5, 9,14,20,
        4,11,16,23, 4,11,16,23, 4,11,16,23, 4,11,16,23,
        6,10,15,21, 6,10,15,21, 6,10,15,21, 6,10,15,21
    };
    uint32_t m[16];
    for (int i = 0; i < 16; i++) {
        m[i] = (uint32_t)p[i * 4] | ((uint32_t)p[i * 4 + 1] << 8) |
               ((uint32_t)p[i * 4 + 2] << 16) | ((uint32_t)p[i * 4 + 3] << 24);
    }
    uint32_t a = h[0], b = h[1], c = h[2], d = h[3];
    for (int i = 0; i < 64; i++) {
        uint32_t f, g;
        if (i < 16)      { f = MD5_F(b, c, d); g = (uint32_t)i; }
        else if (i < 32) { f = MD5_G(b, c, d); g = (uint32_t)(5 * i + 1) & 15u; }
        else if (i < 48) { f = MD5_H(b, c, d); g = (uint32_t)(3 * i + 5) & 15u; }
        else             { f = MD5_I(b, c, d); g = (uint32_t)(7 * i) & 15u; }
        uint32_t tmp = d;
        d = c;
        c = b;
        b = b + MD5_ROTL(a + f + K[i] + m[g], S[i]);
        a = tmp;
    }
    h[0] += a; h[1] += b; h[2] += c; h[3] += d;
}

void lysh_md5(const uint8_t *data, size_t len, uint8_t out[16]) {
    uint32_t h[4] = { 0x67452301u, 0xefcdab89u, 0x98badcfeu, 0x10325476u };
    size_t full = len / 64;
    for (size_t i = 0; i < full; i++) md5_block(h, data + i * 64);

    uint8_t tail[128];
    size_t rem = len - full * 64;
    memset(tail, 0, sizeof(tail));
    memcpy(tail, data + full * 64, rem);
    tail[rem] = 0x80;
    size_t tailLen = (rem + 1 + 8 <= 64) ? 64 : 128;
    uint64_t bits = (uint64_t)len * 8u;
    for (int i = 0; i < 8; i++) tail[tailLen - 8 + i] = (uint8_t)(bits >> (8 * i));
    for (size_t i = 0; i < tailLen; i += 64) md5_block(h, tail + i);

    for (int i = 0; i < 4; i++) {
        out[i * 4]     = (uint8_t)(h[i]);
        out[i * 4 + 1] = (uint8_t)(h[i] >> 8);
        out[i * 4 + 2] = (uint8_t)(h[i] >> 16);
        out[i * 4 + 3] = (uint8_t)(h[i] >> 24);
    }
}

/* ---------------- Xoroshiro128++ ---------------- */
uint64_t lysh_mix_stafford13(uint64_t z) {
    z = (z ^ (z >> 30)) * UINT64_C(0xBF58476D1CE4E5B9);   /* -4658895280553007687 */
    z = (z ^ (z >> 27)) * UINT64_C(0x94D049BB133111EB);   /* -7723592293110705685 */
    return z ^ (z >> 31);
}

void lysh_xr_init(xoroshiro_t *s, uint64_t seed) {
    uint64_t l = seed ^ UINT64_C(0x6A09E667F3BCC909);              /* 7640891576956012809 */
    uint64_t m = l - UINT64_C(0x61C8864680B583EB);                 /* 7046029254386353131 */
    s->lo = lysh_mix_stafford13(l);
    s->hi = lysh_mix_stafford13(m);
}

void lysh_xr_init2(xoroshiro_t *s, uint64_t lo, uint64_t hi) {
    s->lo = lo;
    s->hi = hi;
    if ((lo | hi) == 0) {
        s->lo = UINT64_C(0x9E3779B97F4A7C15);   /* -7046029254386353131 */
        s->hi = UINT64_C(0x6A09E667F3BCC909);   /*  7640891576956012809 */
    }
}

static inline uint64_t rotl64(uint64_t x, int k) { return (x << k) | (x >> (64 - k)); }

uint64_t lysh_xr_next_long(xoroshiro_t *s) {
    uint64_t lo = s->lo, hi = s->hi;
    uint64_t result = rotl64(lo + hi, 17) + lo;
    hi ^= lo;
    s->lo = rotl64(lo, 49) ^ hi ^ (hi << 21);
    s->hi = rotl64(hi, 28);
    return result;
}

int lysh_xr_next_int(xoroshiro_t *s) { return (int)lysh_xr_next_long(s); }

/* Java 17 的 Random.nextInt(bound)：Lemire 拒绝采样。
 *   r = nextLong() & 0xFFFFFFFF            (无符号 32 位)
 *   r *= bound; low = r & 0xFFFFFFFF
 *   if (low < bound) { m = remainderUnsigned(-bound, bound); while (low < m) { 重抽 } }
 *   return (int)(r >>> 32) */
int lysh_xr_next_int_bound(xoroshiro_t *s, int bound) {
    uint64_t r = lysh_xr_next_long(s) & UINT64_C(0xFFFFFFFF);
    r = r * (uint64_t)(int64_t)bound;
    uint64_t low = r & UINT64_C(0xFFFFFFFF);
    if (low < (uint64_t)(int64_t)bound) {
        uint32_t m = (uint32_t)(-(uint32_t)bound) % (uint32_t)bound;   /* remainderUnsigned */
        while (low < (uint64_t)m) {
            r = lysh_xr_next_long(s) & UINT64_C(0xFFFFFFFF);
            r = r * (uint64_t)(int64_t)bound;
            low = r & UINT64_C(0xFFFFFFFF);
        }
    }
    return (int)(r >> 32);
}

/* nextBits(n) = nextLong() >>> (64 - n)：取【高】n 位 */
static inline uint64_t next_bits(xoroshiro_t *s, int n) {
    return lysh_xr_next_long(s) >> (64 - n);
}

double lysh_xr_next_double(xoroshiro_t *s) {
    return (double)next_bits(s, 53) * 1.1102230246251565E-16;   /* 2^-53 */
}

void lysh_xr_consume(xoroshiro_t *s, int count) {
    for (int i = 0; i < count; i++) (void)lysh_xr_next_int(s);
}

void lysh_xr_deriver(xoroshiro_t *out, xoroshiro_t *s) {
    uint64_t a = lysh_xr_next_long(s);
    uint64_t b = lysh_xr_next_long(s);
    lysh_xr_init2(out, a, b);
}

/* fromBytes(byte[], offset)：大端读 8 字节 */
static uint64_t from_bytes_be(const uint8_t *p) {
    uint64_t v = 0;
    for (int i = 0; i < 8; i++) v = (v << 8) | (uint64_t)p[i];
    return v;
}

void lysh_xr_create_random(xoroshiro_t *out, const xoroshiro_t *s, const char *name) {
    uint8_t h[16];
    lysh_md5((const uint8_t *)name, strlen(name), h);
    uint64_t a = from_bytes_be(h);
    uint64_t b = from_bytes_be(h + 8);
    lysh_xr_init2(out, a ^ s->lo, b ^ s->hi);
}

/* MC MathHelper.hashCode(int,int,int) —— 取证：javap MathHelper
 *   i = x * 3129871 ^ (long) z * 116129781L ^ (long) y
 *   i = i * i * 42317861L + i * 11L
 *   return i >> 16
 * 注意 `x * 3129871` 是 **int 乘法**（会溢出），先溢出再拓宽成 long；
 * `z * 116129781L` 才是 long 乘法。 */
int64_t lysh_mc_hash_code(int x, int y, int z) {
    int64_t xm = (int64_t)(int32_t)((uint32_t)x * UINT32_C(3129871));
    int64_t i = xm ^ ((int64_t)z * INT64_C(116129781)) ^ (int64_t)y;
    i = i * i * INT64_C(42317861) + i * INT64_C(11);
    return i >> 16;
}

void lysh_xr_create_random_pos(xoroshiro_t *out, const xoroshiro_t *s, int x, int y, int z) {
    int64_t h = lysh_mc_hash_code(x, y, z);
    lysh_xr_init2(out, (uint64_t)h ^ s->lo, s->hi);
}

/* ---------------- 遗留 java.util.Random（48 位 LCG） ---------------- */
#define LYSH_LCG_MULT   UINT64_C(0x5DEECE66D)
#define LYSH_LCG_ADD    UINT64_C(0xB)
#define LYSH_LCG_MASK   UINT64_C(0xFFFFFFFFFFFF)   /* MASK_48 */

void lysh_lcg_set_seed(lysh_lcg *r, uint64_t s) {
    /* JRand.setSeed(long) -> setSeed(s ^ LCG.JAVA.multiplier) -> Rand.setSeed 存 & MASK_48 */
    r->seed = (s ^ LYSH_LCG_MULT) & LYSH_LCG_MASK;
}

int lysh_lcg_next_bits(lysh_lcg *r, int bits) {
    r->seed = (r->seed * LYSH_LCG_MULT + LYSH_LCG_ADD) & LYSH_LCG_MASK;
    return (int)(r->seed >> (48 - bits));
}

uint64_t lysh_lcg_next_long(lysh_lcg *r) {
    /* ⚠️ java.util.Random.nextLong()（以及 BitRandomSource.nextLong）是
     *     return ((long)next(32) << 32) + next(32);
     * 第二个 next(32) 是 **i2l**（符号扩展）再加，不是零扩展！
     * 所以当第二个 next(32) 的最高位为 1 时，结果会比"拼接"小 2^32。
     * 26.1.2 的字节码逐条（BitRandomSource.nextLong）：
     *     next(32) -> istore_1 ; next(32) -> istore_2 ;
     *     iload_1; i2l; bipush 32; lshl -> lstore_3 ; lload_3; iload_2; i2l; ladd
     * 这里错一位就会让 `setCarverSeed` 派生出的 LCG 状态整体跑偏，
     * 从而让**小屋朝向 dir** 与**雕刻器播种**都错（实测：本种子下 ci=2 的 b 差 2^32）。 */
    uint64_t hi = (uint64_t)(uint32_t)lysh_lcg_next_bits(r, 32);
    int64_t lo = (int64_t)lysh_lcg_next_bits(r, 32);      /* 符号扩展 */
    return (hi << 32) + (uint64_t)lo;
}

/* JDK 的 Random.nextInt(bound) */
int lysh_lcg_next_int_bound(lysh_lcg *r, int bound) {
    if (bound <= 0) return 0;
    if ((bound & (bound - 1)) == 0) {
        /* 2 的幂： (int)((bound * (long)next(31)) >> 31) */
        return (int)(((int64_t)bound * (int64_t)lysh_lcg_next_bits(r, 31)) >> 31);
    }
    int u = lysh_lcg_next_bits(r, 31);
    int v;
    for (;;) {
        v = u % bound;
        if (u - v + (bound - 1) >= 0) break;
        u = lysh_lcg_next_bits(r, 31);
    }
    return v;
}
