const SHIFTS = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21]
const CONSTANTS = Array.from(
    { length: 64 },
    (_, index) => Math.floor(Math.abs(Math.sin(index + 1)) * 2 ** 32) >>> 0,
)

/** MD5 (RFC 1321) of a byte string. Web Crypto has no MD5, so the preview carries this port. */
export function md5(input: Uint8Array): Uint8Array {
    const length = input.length
    const padded = new Uint8Array(((length + 72) >>> 6) << 6)
    padded.set(input)
    padded[length] = 0x80
    const view = new DataView(padded.buffer)
    view.setUint32(padded.length - 8, (length * 8) >>> 0, true)
    view.setUint32(padded.length - 4, Math.floor(length / 0x20000000), true)

    let a0 = 0x67452301
    let b0 = 0xefcdab89
    let c0 = 0x98badcfe
    let d0 = 0x10325476
    for (let offset = 0; offset < padded.length; offset += 64) {
        let a = a0
        let b = b0
        let c = c0
        let d = d0
        for (let index = 0; index < 64; index++) {
            let mixed: number
            let word: number
            if (index < 16) {
                mixed = (b & c) | (~b & d)
                word = index
            } else if (index < 32) {
                mixed = (d & b) | (~d & c)
                word = (5 * index + 1) % 16
            } else if (index < 48) {
                mixed = b ^ c ^ d
                word = (3 * index + 5) % 16
            } else {
                mixed = c ^ (b | ~d)
                word = (7 * index) % 16
            }
            const shift = SHIFTS[(index >>> 4) * 4 + (index % 4)]
            const sum = (a + mixed + CONSTANTS[index] + view.getUint32(offset + word * 4, true)) | 0
            a = d
            d = c
            c = b
            b = (b + ((sum << shift) | (sum >>> (32 - shift)))) | 0
        }
        a0 = (a0 + a) | 0
        b0 = (b0 + b) | 0
        c0 = (c0 + c) | 0
        d0 = (d0 + d) | 0
    }

    const digest = new Uint8Array(16)
    const digestView = new DataView(digest.buffer)
    for (const [index, word] of [a0, b0, c0, d0].entries()) {
        digestView.setUint32(index * 4, word, true)
    }
    return digest
}

/**
 * The UUID the launcher gives an offline account: the name-based (version 3)
 * UUID of `OfflinePlayer:<name>`, as Java's `UUID.nameUUIDFromBytes` makes it
 * (packages/app-lib/src/state/minecraft_auth.rs `Credentials::offline`).
 */
export function offlineUuid(name: string) {
    const bytes = md5(new TextEncoder().encode(`OfflinePlayer:${name}`))
    bytes[6] = (bytes[6] & 0x0f) | 0x30
    bytes[8] = (bytes[8] & 0x3f) | 0x80
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

export function hyphenateUuid(id: string) {
    return `${id.slice(0, 8)}-${id.slice(8, 12)}-${id.slice(12, 16)}-${id.slice(16, 20)}-${id.slice(20)}`
}
