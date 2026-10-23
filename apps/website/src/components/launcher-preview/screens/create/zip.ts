/**
 * A small ZIP reader for files the visitor picks: it lists the central directory
 * and extracts single entries (stored or deflated, via `DecompressionStream`), which
 * is all the drop classifier and the modpack readers need. Nothing leaves the page.
 */

export interface ZipEntry {
    name: string
    directory: boolean
    method: number
    encrypted: boolean
    compressedSize: number
    uncompressedSize: number
    localHeaderOffset: number
}

export interface ZipArchive {
    source: Blob
    entries: ZipEntry[]
}

const EOCD_SIGNATURE = 0x06054b50
const ZIP64_LOCATOR_SIGNATURE = 0x07064b50
const ZIP64_EOCD_SIGNATURE = 0x06064b50
const CENTRAL_SIGNATURE = 0x02014b50
const LOCAL_SIGNATURE = 0x04034b50

async function readRange(blob: Blob, start: number, end: number): Promise<DataView> {
    const buffer = await blob.slice(start, end).arrayBuffer()
    return new DataView(buffer)
}

function readUint64(view: DataView, offset: number): number {
    const low = view.getUint32(offset, true)
    const high = view.getUint32(offset + 4, true)
    return high * 0x1_0000_0000 + low
}

const utf8 = new TextDecoder('utf-8')

export class ZipFormatError extends Error {}

/** Lists the archive's entries; throws `ZipFormatError` for files that are not ZIPs. */
export async function openZip(source: Blob): Promise<ZipArchive> {
    const tailLength = Math.min(source.size, 22 + 0xffff)
    if (tailLength < 22) throw new ZipFormatError('File is not a valid ZIP archive')
    const tailStart = source.size - tailLength
    const tail = await readRange(source, tailStart, source.size)

    let eocd = -1
    for (let offset = tail.byteLength - 22; offset >= 0; offset--) {
        if (tail.getUint32(offset, true) === EOCD_SIGNATURE) {
            eocd = offset
            break
        }
    }
    if (eocd < 0) throw new ZipFormatError('File is not a valid ZIP archive')

    let entryCount = tail.getUint16(eocd + 10, true)
    let directorySize = tail.getUint32(eocd + 12, true)
    let directoryOffset = tail.getUint32(eocd + 16, true)

    if (
        (entryCount === 0xffff || directorySize === 0xffffffff || directoryOffset === 0xffffffff) &&
        eocd >= 20 &&
        tail.getUint32(eocd - 20, true) === ZIP64_LOCATOR_SIGNATURE
    ) {
        const zip64Offset = readUint64(tail, eocd - 20 + 8)
        const zip64 = await readRange(source, zip64Offset, zip64Offset + 56)
        if (zip64.getUint32(0, true) === ZIP64_EOCD_SIGNATURE) {
            entryCount = readUint64(zip64, 32)
            directorySize = readUint64(zip64, 40)
            directoryOffset = readUint64(zip64, 48)
        }
    }

    if (directoryOffset + directorySize > source.size) {
        throw new ZipFormatError('File is not a valid ZIP archive')
    }
    const directory = await readRange(source, directoryOffset, directoryOffset + directorySize)
    const entries: ZipEntry[] = []
    let cursor = 0
    for (let index = 0; index < entryCount && cursor + 46 <= directory.byteLength; index++) {
        if (directory.getUint32(cursor, true) !== CENTRAL_SIGNATURE) break
        const flags = directory.getUint16(cursor + 8, true)
        const method = directory.getUint16(cursor + 10, true)
        let compressedSize = directory.getUint32(cursor + 20, true)
        let uncompressedSize = directory.getUint32(cursor + 24, true)
        const nameLength = directory.getUint16(cursor + 28, true)
        const extraLength = directory.getUint16(cursor + 30, true)
        const commentLength = directory.getUint16(cursor + 32, true)
        let localHeaderOffset = directory.getUint32(cursor + 42, true)
        const nameBytes = new Uint8Array(
            directory.buffer,
            directory.byteOffset + cursor + 46,
            nameLength,
        )
        const name = utf8.decode(nameBytes).replace(/\\/g, '/')

        // ZIP64 extended information extra field.
        let extra = cursor + 46 + nameLength
        const extraEnd = extra + extraLength
        while (extra + 4 <= extraEnd) {
            const headerId = directory.getUint16(extra, true)
            const size = directory.getUint16(extra + 2, true)
            if (headerId === 0x0001) {
                let field = extra + 4
                if (uncompressedSize === 0xffffffff) {
                    uncompressedSize = readUint64(directory, field)
                    field += 8
                }
                if (compressedSize === 0xffffffff) {
                    compressedSize = readUint64(directory, field)
                    field += 8
                }
                if (localHeaderOffset === 0xffffffff) {
                    localHeaderOffset = readUint64(directory, field)
                }
            }
            extra += 4 + size
        }

        entries.push({
            name,
            directory: name.endsWith('/'),
            method,
            encrypted: (flags & 0x1) === 0x1,
            compressedSize,
            uncompressedSize,
            localHeaderOffset,
        })
        cursor += 46 + nameLength + extraLength + commentLength
    }
    return { source, entries }
}

/** Extracts one entry's bytes. */
export async function readZipEntry(archive: ZipArchive, entry: ZipEntry): Promise<Uint8Array> {
    if (entry.encrypted) throw new ZipFormatError('Archive entry is encrypted')
    const header = await readRange(
        archive.source,
        entry.localHeaderOffset,
        entry.localHeaderOffset + 30,
    )
    if (header.getUint32(0, true) !== LOCAL_SIGNATURE) {
        throw new ZipFormatError('File is not a valid ZIP archive')
    }
    const nameLength = header.getUint16(26, true)
    const extraLength = header.getUint16(28, true)
    const start = entry.localHeaderOffset + 30 + nameLength + extraLength
    const data = archive.source.slice(start, start + entry.compressedSize)
    if (entry.method === 0) return new Uint8Array(await data.arrayBuffer())
    if (entry.method === 8) {
        const stream = data.stream().pipeThrough(new DecompressionStream('deflate-raw'))
        return new Uint8Array(await new Response(stream).arrayBuffer())
    }
    throw new ZipFormatError(`Unsupported compression method ${entry.method}`)
}

export async function readZipText(archive: ZipArchive, name: string): Promise<string | null> {
    const entry = archive.entries.find((item) => item.name === name)
    if (!entry) return null
    return utf8.decode(await readZipEntry(archive, entry))
}

export async function readZipJson<T = unknown>(
    archive: ZipArchive,
    name: string,
): Promise<T | null> {
    const text = await readZipText(archive, name)
    if (text === null) return null
    return JSON.parse(text.replace(/^\uFEFF/, '')) as T
}
