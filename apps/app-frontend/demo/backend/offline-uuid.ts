const SHIFTS = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21]
const CONSTANTS = Array.from({ length: 64 }, (_, i) =>
    Math.floor(Math.abs(Math.sin(i + 1)) * 2 ** 32),
)

/** MD5 (RFC 1321) of a string's UTF-8 bytes. */
function md5(text: string): Uint8Array {
    const bytes = new TextEncoder().encode(text)
    const words = new Uint32Array((((bytes.length + 8) >>> 6) + 1) * 16)
    bytes.forEach((byte, index) => (words[index >> 2] |= byte << ((index % 4) * 8)))
    words[bytes.length >> 2] |= 0x80 << ((bytes.length % 4) * 8)
    words[words.length - 2] = bytes.length * 8
    const state = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476]
    for (let block = 0; block < words.length; block += 16) {
        let [a, b, c, d] = state
        for (let i = 0; i < 64; i++) {
            const round = i >> 4
            const mix = [(b & c) | (~b & d), (d & b) | (~d & c), b ^ c ^ d, c ^ (b | ~d)][round]
            const word = [i, (5 * i + 1) % 16, (3 * i + 5) % 16, (7 * i) % 16][round]
            const shift = SHIFTS[round * 4 + (i % 4)]
            const sum = (a + mix + CONSTANTS[i] + words[block + word]) >>> 0
            ;[a, d, c] = [d, c, b]
            b = (b + ((sum << shift) | (sum >>> (32 - shift)))) >>> 0
        }
        state[0] = (state[0] + a) >>> 0
        state[1] = (state[1] + b) >>> 0
        state[2] = (state[2] + c) >>> 0
        state[3] = (state[3] + d) >>> 0
    }
    const digest = new Uint8Array(16)
    const view = new DataView(digest.buffer)
    state.forEach((value, index) => view.setUint32(index * 4, value, true))
    return digest
}

/**
 * The UUID the launcher gives an offline account (app-lib `Credentials::offline`): MD5 of
 * `OfflinePlayer:<name>` stamped as a version 3 UUID, as 32 hex digits.
 */
export function offlineUuid(name: string) {
    const bytes = md5(`OfflinePlayer:${name}`)
    bytes[6] = (bytes[6] & 0x0f) | 0x30
    bytes[8] = (bytes[8] & 0x3f) | 0x80
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}
