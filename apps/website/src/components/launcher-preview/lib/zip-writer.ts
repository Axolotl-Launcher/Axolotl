/**
 * The two `fflate` functions the app's datapack export uses, for the preview:
 * `strToU8` and `zipSync`. Entries are stored uncompressed, which every zip
 * reader (and Minecraft's datapack loader) accepts.
 */
const encoder = new TextEncoder()

export function strToU8(text: string): Uint8Array {
    return encoder.encode(text)
}

const CRC_TABLE = (() => {
    const table = new Uint32Array(256)
    for (let index = 0; index < 256; index++) {
        let value = index
        for (let bit = 0; bit < 8; bit++)
            value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1
        table[index] = value >>> 0
    }
    return table
})()

function crc32(bytes: Uint8Array): number {
    let crc = 0xffffffff
    for (const byte of bytes) crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8)
    return (crc ^ 0xffffffff) >>> 0
}

function dosDateTime(date: Date) {
    const time =
        (date.getHours() << 11) | (date.getMinutes() << 5) | Math.floor(date.getSeconds() / 2)
    const day = ((date.getFullYear() - 1980) << 9) | ((date.getMonth() + 1) << 5) | date.getDate()
    return { time, day }
}

export function zipSync(files: Record<string, Uint8Array>): Uint8Array {
    const { time, day } = dosDateTime(new Date())
    const locals: Uint8Array[] = []
    const centrals: Uint8Array[] = []
    let offset = 0
    for (const [path, data] of Object.entries(files)) {
        const name = encoder.encode(path)
        const crc = crc32(data)
        const local = new Uint8Array(30 + name.length)
        const localView = new DataView(local.buffer)
        localView.setUint32(0, 0x04034b50, true)
        localView.setUint16(4, 20, true)
        localView.setUint16(6, 0x0800, true)
        localView.setUint16(8, 0, true)
        localView.setUint16(10, time, true)
        localView.setUint16(12, day, true)
        localView.setUint32(14, crc, true)
        localView.setUint32(18, data.length, true)
        localView.setUint32(22, data.length, true)
        localView.setUint16(26, name.length, true)
        local.set(name, 30)
        const central = new Uint8Array(46 + name.length)
        const centralView = new DataView(central.buffer)
        centralView.setUint32(0, 0x02014b50, true)
        centralView.setUint16(4, 20, true)
        centralView.setUint16(6, 20, true)
        centralView.setUint16(8, 0x0800, true)
        centralView.setUint16(10, 0, true)
        centralView.setUint16(12, time, true)
        centralView.setUint16(14, day, true)
        centralView.setUint32(16, crc, true)
        centralView.setUint32(20, data.length, true)
        centralView.setUint32(24, data.length, true)
        centralView.setUint16(28, name.length, true)
        centralView.setUint32(42, offset, true)
        central.set(name, 46)
        locals.push(local, data)
        centrals.push(central)
        offset += local.length + data.length
    }
    const centralSize = centrals.reduce((sum, entry) => sum + entry.length, 0)
    const end = new Uint8Array(22)
    const endView = new DataView(end.buffer)
    endView.setUint32(0, 0x06054b50, true)
    endView.setUint16(8, centrals.length, true)
    endView.setUint16(10, centrals.length, true)
    endView.setUint32(12, centralSize, true)
    endView.setUint32(16, offset, true)
    const output = new Uint8Array(offset + centralSize + end.length)
    let position = 0
    for (const part of [...locals, ...centrals, end]) {
        output.set(part, position)
        position += part.length
    }
    return output
}
