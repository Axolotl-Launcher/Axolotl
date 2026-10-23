import type { ContentProjectType } from './content-types'

/**
 * What `add_project_from_path` files a dropped or picked file as: a `.jar` is a
 * mod; a `.zip` is a shader pack when it carries `shaders/`, a data pack when
 * it carries `data/` next to `pack.mcmeta`, and a resource pack when it carries
 * `assets/` or a `pack.mcmeta`. The entry names come from the archive's central
 * directory, so nothing is decompressed. `null` when the file is none of these.
 */
export async function classifyContentFile(file: File): Promise<ContentProjectType | null> {
    const name = file.name.toLowerCase()
    if (name.endsWith('.jar')) return 'mod'
    if (!name.endsWith('.zip')) return null
    const entries = await zipEntryNames(file).catch(() => null)
    if (!entries) return null
    const has = (prefix: string) => entries.some((entry) => entry.startsWith(prefix))
    if (has('shaders/')) return 'shader'
    const meta = entries.includes('pack.mcmeta')
    if (meta && has('data/')) return 'datapack'
    if (meta || has('assets/')) return 'resourcepack'
    return null
}

const END_OF_CENTRAL_DIRECTORY = 0x06054b50
const CENTRAL_DIRECTORY_ENTRY = 0x02014b50

async function zipEntryNames(file: File): Promise<string[]> {
    // The end record sits in the last 22 bytes plus an optional comment (<= 64 KiB).
    const tailSize = Math.min(file.size, 22 + 0xffff)
    const tail = new DataView(await file.slice(file.size - tailSize).arrayBuffer())
    let end = -1
    for (let offset = tail.byteLength - 22; offset >= 0; offset--) {
        if (tail.getUint32(offset, true) === END_OF_CENTRAL_DIRECTORY) {
            end = offset
            break
        }
    }
    if (end < 0) throw new Error('Not a zip archive')
    const count = tail.getUint16(end + 10, true)
    const size = tail.getUint32(end + 12, true)
    const start = tail.getUint32(end + 16, true)
    const directory = new DataView(await file.slice(start, start + size).arrayBuffer())
    const decoder = new TextDecoder()
    const names: string[] = []
    let offset = 0
    for (let index = 0; index < count && offset + 46 <= directory.byteLength; index++) {
        if (directory.getUint32(offset, true) !== CENTRAL_DIRECTORY_ENTRY) break
        const nameLength = directory.getUint16(offset + 28, true)
        const extraLength = directory.getUint16(offset + 30, true)
        const commentLength = directory.getUint16(offset + 32, true)
        const bytes = new Uint8Array(
            directory.buffer,
            directory.byteOffset + offset + 46,
            nameLength,
        )
        names.push(decoder.decode(bytes).replace(/\\/g, '/'))
        offset += 46 + nameLength + extraLength + commentLength
    }
    return names
}
