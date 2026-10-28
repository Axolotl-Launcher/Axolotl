/* lysh — LowYSwampHut 的 C 内核
 *
 * rng.h —— 与 Minecraft Java 版逐位一致的随机数（Xoroshiro128++ 系）。
 *
 * 全部语义来自 noise-sampler 1.20.0 的字节码（该类已由 L1 测试证明与 MC 逐位一致）：
 *   Xoroshiro128PlusPlusRandom(long)      -> seed = upgradeSeedTo128bit(s)
 *   Xoroshiro128PlusPlusRandom(long,long) -> 两参数直接作为 lo/hi；(lo|hi)==0 时有哨兵
 *   nextLong()                            -> Xoroshiro128++ 标准式
 *   nextBits(n)                           -> nextLong() >>> (64 - n)   【取高位】
 *   nextDouble()                          -> (double)nextBits(53) * 2^-53
 *   nextFloat()                           -> (float) nextBits(24) * 2^-24
 *   nextInt()                             -> (int) nextLong()
 *   nextInt(bound)                        -> Java 17 的 Lemire 拒绝采样（见 rng.c 注释）
 *   consumeCount(n)                       -> n 次 nextInt()
 *   createRandomDeriver()                 -> new Xoroshiro(nextLong(), nextLong())
 *   createRandom(String)                  -> MD5(name) 前 16 字节按大端拆成两个 long，
 *                                            分别与当前 lo/hi 异或，再走两参数构造
 */
#ifndef LYSH_RNG_H
#define LYSH_RNG_H

#include <stdint.h>

typedef struct { uint64_t lo, hi; } xoroshiro_t;

/* splitmix64 finalizer（MC 的 mixStafford13） */
uint64_t lysh_mix_stafford13(uint64_t z);

/* 由单个 long 种子初始化（MC 的 upgradeSeedTo128bit） */
void lysh_xr_init(xoroshiro_t *s, uint64_t seed);

/* 两参数初始化（含 (lo|hi)==0 的哨兵分支） */
void lysh_xr_init2(xoroshiro_t *s, uint64_t lo, uint64_t hi);

uint64_t lysh_xr_next_long(xoroshiro_t *s);
int      lysh_xr_next_int(xoroshiro_t *s);
int      lysh_xr_next_int_bound(xoroshiro_t *s, int bound);
double   lysh_xr_next_double(xoroshiro_t *s);
void     lysh_xr_consume(xoroshiro_t *s, int count);

/* createRandomDeriver() */
void lysh_xr_deriver(xoroshiro_t *out, xoroshiro_t *s);

/* createRandom(String name) */
void lysh_xr_create_random(xoroshiro_t *out, const xoroshiro_t *s, const char *name);

/* MC MathHelper.hashCode(int,int,int)（位置派生用的 64 位哈希） */
int64_t lysh_mc_hash_code(int x, int y, int z);

/* createRandom(int x, int y, int z)：RandomDeriver 的位置派生
 *   hash = MathHelper.hashCode(x,y,z)
 *   new Xoroshiro(deriver.lo ^ hash, deriver.hi)
 * 含水层的 per-cell 随机锚点用的就是这条。 */
void lysh_xr_create_random_pos(xoroshiro_t *out, const xoroshiro_t *s, int x, int y, int z);

/* 便于测试：直接暴露 MD5 */
void lysh_md5(const uint8_t *data, size_t len, uint8_t out[16]);

/* ---------------------------------------------------------------------------
 * 遗留 java.util.Random（48 位 LCG）—— 结构放置用的是这一套，不是 Xoroshiro。
 *
 * mc_feature 的 ChunkRand 继承 mcseed.rand.JRand，而 JRand.setSeed(s) 就是
 *   seed = (s ^ 0x5DEECE66D) & ((1<<48)-1)
 * 这也是为什么 ChunkRand.setRegionSeed / setCarverSeed 返回 `& MASK_48`。
 * ------------------------------------------------------------------------- */
typedef struct { uint64_t seed; } lysh_lcg;

void lysh_lcg_set_seed(lysh_lcg *r, uint64_t s);
int  lysh_lcg_next_bits(lysh_lcg *r, int bits);
int  lysh_lcg_next_int_bound(lysh_lcg *r, int bound);
uint64_t lysh_lcg_next_long(lysh_lcg *r);

#endif /* LYSH_RNG_H */
