import { save } from '../../../lib/platform/dialog.ts'
import { writeFile, writeTextFile } from '../../../lib/platform/fs.ts'
import { strToU8, zipSync } from '../../../lib/zip-writer.ts'
import { DATAPACK_ICON_BASE64 } from './datapack-icon.ts'
import { parseIdentifier, rawId } from './identifier.ts'
import type { JavaVersionId } from './types.ts'
import { getJavaVersionMeta } from './versions.ts'

export type PackFile = {
    path: string
    content: string | Uint8Array
}

export type DatapackRecipe = {
    name: string
    json: object
}

export type DatapackTag = {
    namespace: string
    id: string
    values: string[]
}

export type DatapackSaveSource = PackFile[] | Blob

const PACK_DESCRIPTION = 'Axolotl Recipe Generator'

function formatDatapackTimestamp(date: Date): string {
    const pad = (value: number) => String(value).padStart(2, '0')
    return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(
        date.getHours(),
    )}${pad(date.getMinutes())}${pad(date.getSeconds())}`
}

export function createDatapackFileName(version: JavaVersionId, date = new Date()): string {
    return `axolotl-recipes-${version}-${formatDatapackTimestamp(date)}.zip`
}

export function createDatapackDescription(productNames: readonly string[]): string {
    const names = productNames
        .map((name) => name.trim())
        .filter(Boolean)
        .map((name) => `[${name}]`)
        .join(' ')
    return names ? `${PACK_DESCRIPTION}\n${names}` : PACK_DESCRIPTION
}

function base64ToUint8Array(base64: string): Uint8Array {
    const binary = atob(base64)
    const bytes = new Uint8Array(binary.length)
    for (let index = 0; index < binary.length; index += 1) {
        bytes[index] = binary.charCodeAt(index)
    }
    return bytes
}

let packIconBytes: Uint8Array | null = null

function getPackIconBytes(): Uint8Array {
    if (!packIconBytes) packIconBytes = base64ToUint8Array(DATAPACK_ICON_BASE64)
    return packIconBytes
}

export function createPackMcmeta(
    packFormat: number | [number, number],
    description = PACK_DESCRIPTION,
): string {
    const pack = Array.isArray(packFormat)
        ? { min_format: packFormat, max_format: packFormat }
        : { pack_format: packFormat }
    return JSON.stringify({ pack: { description, ...pack } }, null, 2)
}

export function createDatapackFiles(
    version: JavaVersionId,
    recipes: DatapackRecipe[],
    tags: DatapackTag[],
    description = PACK_DESCRIPTION,
): PackFile[] {
    const meta = getJavaVersionMeta(version)
    if (!meta.packFormat || !meta.recipeDir || !meta.tagDir) {
        throw new Error(`Datapack export is not available for ${version}`)
    }

    const files: PackFile[] = [
        { path: 'pack.mcmeta', content: createPackMcmeta(meta.packFormat, description) },
        { path: 'pack.png', content: getPackIconBytes() },
    ]
    for (const recipe of recipes) {
        files.push({
            path: `data/crafting/${meta.recipeDir}/${recipe.name}.json`,
            content: JSON.stringify(recipe.json, null, 2),
        })
    }
    for (const tag of tags) {
        files.push({
            path: `data/${tag.namespace}/${meta.tagDir}/${tag.id}.json`,
            content: JSON.stringify({ replace: false, values: tag.values }, null, 2),
        })
    }
    return files
}

export function createDatapackBlob(files: PackFile[]): Blob {
    const record: Record<string, Uint8Array> = {}
    for (const file of files) {
        record[file.path] = typeof file.content === 'string' ? strToU8(file.content) : file.content
    }
    const zipped = zipSync(record)
    const source = zipped.buffer.slice(
        zipped.byteOffset,
        zipped.byteOffset + zipped.byteLength,
    ) as ArrayBuffer
    const bytes = new Uint8Array(source)
    return new Blob([bytes], { type: 'application/zip' })
}

export function createTagFiles(customTags: { id: string; values: string[] }[]): DatapackTag[] {
    return customTags.flatMap((tag) => {
        const ref = parseIdentifier(tag.id)
        return [
            {
                namespace: ref.namespace,
                id: ref.id,
                values: tag.values,
            },
        ]
    })
}

export function downloadBlob(blob: Blob, fileName: string): void {
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = fileName
    anchor.click()
    setTimeout(() => URL.revokeObjectURL(url), 1_000)
}

export function downloadJson(value: object, fileName: string): void {
    downloadBlob(new Blob([JSON.stringify(value, null, 2)], { type: 'application/json' }), fileName)
}

export async function saveJsonFile(value: object, defaultFileName: string): Promise<string | null> {
    const path = await save({
        defaultPath: defaultFileName,
        filters: [{ name: 'Minecraft recipe JSON', extensions: ['json'] }],
    })
    if (!path) return null
    await writeTextFile(path, JSON.stringify(value, null, 2))
    return path
}

export async function saveDatapackAs(
    source: DatapackSaveSource,
    defaultFileName: string,
): Promise<string | null> {
    const path = await save({
        defaultPath: defaultFileName,
        filters: [{ name: 'Minecraft datapack', extensions: ['zip'] }],
    })
    if (!path) return null
    const blob = source instanceof Blob ? source : createDatapackBlob(source)
    await writeFile(path, new Uint8Array(await blob.arrayBuffer()))
    return path
}

export function customTagRawId(tag: { id: string }): string {
    return rawId(parseIdentifier(tag.id))
}
