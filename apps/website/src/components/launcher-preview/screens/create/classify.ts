/**
 * Port of the launcher's drop classifier (`packages/app-lib/src/api/drop_classifier.rs`)
 * for the two sources the preview can read: folders of the simulated Mac
 * (`mac-fs.ts`) and files the visitor picks in the browser (read with `zip.ts`).
 * The checks run in the Rust order and return the same `ClassificationResult`
 * shape the frontend receives (`use-global-drop.ts`), reasons included.
 */
import { baseName, childNamed, type FsNode, hasFile } from './mac-fs'
import {
    openZip,
    readZipEntry,
    readZipJson,
    readZipText,
    type ZipArchive,
    ZipFormatError,
} from './zip'

export interface ClassificationChoice {
    itemType: string
    filePath?: string
    innerBase?: string
    candidates?: string[]
}

export interface ClassificationResult {
    item_type:
        | 'launcher'
        | 'hmcl_launcher'
        | 'mod'
        | 'modpack'
        | 'litematic'
        | 'resource_pack'
        | 'shader_pack'
        | 'world_save'
        | 'shortcut_resolved'
        | 'multiple'
        | 'unknown'
    file_path?: string
    candidates?: string[]
    choices?: ClassificationChoice[]
    launcher_type?: string
    base_path?: string
    innerBase?: string
    launcher_dir?: string
    data_dir?: string
    reason?: string
}

const MAX_NESTING_DEPTH = 5
const NOISE_ENTRY_NAMES = ['__macosx', '.git', 'thumbs.db', '.ds_store']

function isNoise(name: string) {
    return NOISE_ENTRY_NAMES.includes(name.toLowerCase())
}

function isZipName(name: string) {
    return /\.(zip|mrpack)$/i.test(name)
}

// ── Folders of the simulated disk ───────────────────────────────────────────

function hasVersionJson(node: FsNode): boolean {
    const versions = childNamed(node, 'versions')
    return (
        !!versions &&
        versions.kind === 'dir' &&
        (versions.children ?? []).some(
            (child) => child.kind === 'dir' && hasFile(child, `${child.name}.json`),
        )
    )
}

function hasDirectVersionJson(node: FsNode): boolean {
    const matching = (node.children ?? []).filter(
        (child) => child.kind === 'dir' && hasFile(child, `${child.name}.json`),
    )
    if (matching.length === 0) return false
    if (node.name.toLowerCase() === 'versions') return true
    if (matching.length >= 2) return true
    return matching.some((child) => hasFile(child, `${child.name}.jar`))
}

function hasModsJar(node: FsNode): boolean {
    const mods = childNamed(node, 'mods')
    return (
        !!mods &&
        (mods.children ?? []).some((child) => child.kind === 'file' && /\.jar$/i.test(child.name))
    )
}

function hasRootJarAndJson(node: FsNode): boolean {
    const files = (node.children ?? []).filter((child) => child.kind === 'file')
    return (
        files.some((child) => /\.jar$/i.test(child.name)) &&
        files.some((child) => /\.json$/i.test(child.name))
    )
}

function isLauncherInstanceFolder(node: FsNode): boolean {
    return (
        hasVersionJson(node) ||
        hasDirectVersionJson(node) ||
        hasRootJarAndJson(node) ||
        hasModsJar(node)
    )
}

function isAxolotlFolder(node: FsNode): boolean {
    return (
        hasFile(node, 'axolotl_config.json') ||
        (node.children ?? []).some(
            (child) => child.kind === 'dir' && hasFile(child, 'axolotl_config.json'),
        )
    )
}

/** `classify_folder_content_inner`. */
export function classifyFolderNode(node: FsNode, path: string, depth = 0): ClassificationResult {
    const launcher = (launcherType: string): ClassificationResult => ({
        item_type: 'launcher',
        launcher_type: launcherType,
        base_path: path,
    })
    if (isAxolotlFolder(node)) return launcher('Axolotl')
    if (hasFile(node, 'multimc.cfg')) return launcher('MultiMC')
    if (hasFile(node, 'prismlauncher.cfg')) return launcher('PrismLauncher')
    const instances = childNamed(node, 'instances')
    if (instances?.children?.some((child) => hasFile(child, 'instance.cfg'))) {
        return launcher('MultiMC')
    }
    const hmclConfig = childNamed(
        childNamed(childNamed(node, '.hmcl'), 'config'),
        'launcher-settings.json',
    )
    if (hmclConfig) {
        return { item_type: 'hmcl_launcher', launcher_dir: path, data_dir: `${path}/.minecraft` }
    }
    if (node.name.toLowerCase() === '.minecraft') return launcher('Generic')

    if (hasFile(node, 'level.dat')) return { item_type: 'world_save', file_path: path }
    if (hasFile(node, 'pack.mcmeta')) {
        const candidates: string[] = []
        if (childNamed(node, 'data')?.kind === 'dir') candidates.push('data_pack')
        if (childNamed(node, 'assets')?.kind === 'dir') candidates.push('resource_pack')
        return { item_type: 'resource_pack', file_path: path, candidates }
    }
    const shaders = childNamed(node, 'shaders')
    if (shaders?.kind === 'dir' && (shaders.children?.length ?? 0) > 0) {
        return { item_type: 'shader_pack', file_path: path }
    }
    if (isLauncherInstanceFolder(node)) return launcher('Generic')

    if (depth >= MAX_NESTING_DEPTH) {
        return {
            item_type: 'unknown',
            reason: `Folder nesting is too deep to analyze (limit ${MAX_NESTING_DEPTH} levels)`,
        }
    }
    const children = (node.children ?? [])
        .filter((child) => !isNoise(child.name) && (child.kind === 'dir' || isZipName(child.name)))
        .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))
    for (const child of children) {
        const childPath = `${path}/${child.name}`
        const result =
            child.kind === 'dir'
                ? classifyFolderNode(child, childPath, depth + 1)
                : classifyArchiveNode(child, childPath)
        if (result.item_type !== 'unknown') return result
    }
    return { item_type: 'unknown', reason: `Unrecognised content: ${baseName(path)}` }
}

/** A pack archive on the simulated disk (`classify_zip_file_at_depth`). */
export function classifyArchiveNode(node: FsNode, path: string): ClassificationResult {
    if (node.meta?.type === 'mrpack') return { item_type: 'modpack', file_path: path }
    return { item_type: 'unknown', reason: 'File is not a valid ZIP archive' }
}

// ── ZIP archives ────────────────────────────────────────────────────────────

class EntrySet {
    readonly names: string[]
    readonly files = new Set<string>()
    readonly dirs = new Set<string>()
    readonly encrypted: boolean

    constructor(archive: ZipArchive) {
        this.names = archive.entries.map((entry) => entry.name)
        this.encrypted = archive.entries.some((entry) => entry.encrypted)
        for (const entry of archive.entries) {
            const name = entry.name.replace(/\/+$/, '')
            if (!name) continue
            if (entry.directory) this.dirs.add(`${name}/`)
            else this.files.add(name)
            const parts = name.split('/')
            for (let index = 1; index < parts.length; index++) {
                this.dirs.add(`${parts.slice(0, index).join('/')}/`)
            }
        }
    }

    hasFile(base: string, file: string) {
        return this.files.has(`${base}${file}`)
    }

    childFolders(base: string): string[] {
        const seen = new Set<string>()
        for (const path of this.dirs) {
            if (!path.startsWith(base)) continue
            const first = path.slice(base.length).split('/')[0]
            if (first && !isNoise(first)) seen.add(first)
        }
        return [...seen].sort()
    }

    hasChildFolder(base: string, name: string) {
        return this.dirs.has(`${base}${name}/`)
    }

    hasVersionJson(base: string) {
        for (const path of this.files) {
            if (!path.startsWith(base)) continue
            const parts = path.slice(base.length).split('/')
            if (
                parts.length === 3 &&
                parts[0] === 'versions' &&
                parts[1] &&
                `${parts[1]}.json` === parts[2]
            ) {
                return true
            }
        }
        return false
    }

    hasDirectVersionJson(base: string) {
        const matching = this.childFolders(base).filter((child) =>
            this.hasFile(`${base}${child}/`, `${child}.json`),
        )
        if (matching.length === 0) return false
        const baseName = base.replace(/\/$/, '').split('/').pop() ?? ''
        if (baseName.toLowerCase() === 'versions') return true
        if (matching.length >= 2) return true
        return matching.some((child) => this.hasFile(`${base}${child}/`, `${child}.jar`))
    }

    hasMmcInstance(base: string) {
        for (const path of this.files) {
            if (!path.startsWith(base)) continue
            const parts = path.slice(base.length).split('/')
            if (
                parts.length === 3 &&
                parts[0] === 'instances' &&
                parts[2] === 'instance.cfg' &&
                parts[1]
            ) {
                return true
            }
        }
        return false
    }

    hasModsJar(base: string) {
        for (const path of this.files) {
            if (!path.startsWith(base)) continue
            const parts = path.slice(base.length).split('/')
            if (parts.length === 2 && parts[0] === 'mods' && /\.jar$/i.test(parts[1])) return true
        }
        return false
    }

    hasRootJarAndJson(base: string) {
        let jar = false
        let json = false
        for (const path of this.files) {
            if (!path.startsWith(base)) continue
            const rest = path.slice(base.length)
            if (rest.includes('/')) continue
            if (/\.jar$/i.test(rest)) jar = true
            if (/\.json$/i.test(rest)) json = true
        }
        return jar && json
    }

    hasMultipleInstanceChildren(base: string) {
        let count = 0
        for (const child of this.childFolders(base)) {
            const childBase = `${base}${child}/`
            if (
                this.hasVersionJson(childBase) ||
                this.hasDirectVersionJson(childBase) ||
                this.hasModsJar(childBase) ||
                this.hasRootJarAndJson(childBase)
            ) {
                count++
                if (count >= 2) return true
            }
        }
        return false
    }

    hasAxolotlConfig(base: string) {
        return (
            this.hasFile(base, 'axolotl_config.json') ||
            this.childFolders(base).some((child) =>
                this.hasFile(`${base}${child}/`, 'axolotl_config.json'),
            )
        )
    }

    hasPopulatedShadersDir(base: string) {
        const prefix = `${base}shaders/`
        for (const path of this.files) if (path.startsWith(prefix)) return true
        for (const path of this.dirs) if (path.startsWith(prefix) && path !== prefix) return true
        return false
    }

    nestedZipFiles(base: string): string[] {
        const zips: string[] = []
        for (const path of this.files) {
            if (!path.startsWith(base)) continue
            const rest = path.slice(base.length)
            if (!rest.includes('/') && isZipName(rest)) zips.push(rest)
        }
        return zips.sort()
    }
}

function innerBase(base: string): string | undefined {
    const trimmed = base.replace(/\/$/, '')
    return trimmed || undefined
}

/** `pack::detect::detect_at_base`: whether a modpack manifest sits at `base`. */
async function detectPack(archive: ZipArchive, entries: EntrySet, base: string) {
    const has = (name: string) => entries.names.includes(`${base}${name}`)
    if (has('mcbbs.packmeta')) return true
    if (has('mmc-pack.json')) return true
    if (has('modrinth.index.json')) return true
    if (has('manifest.json')) {
        try {
            await readZipJson(archive, `${base}manifest.json`)
            return true
        } catch {
            // An unreadable manifest.json does not abort detection of other formats.
        }
    }
    if (has('modpack.json')) return true
    return has('modpack.zip') || has('modpack.mrpack')
}

interface ZipWalk {
    resultPath: string
    allowNestedUnpack: boolean
    nestedBytes: number
}

async function classifyEntries(
    archive: ZipArchive,
    entries: EntrySet,
    depth: number,
    base: string,
    walk: ZipWalk,
): Promise<ClassificationResult> {
    if (await detectPack(archive, entries, base)) {
        return { item_type: 'modpack', file_path: walk.resultPath }
    }

    let launcherType: string | null = null
    if (entries.hasAxolotlConfig(base)) launcherType = 'Axolotl'
    else if (entries.hasFile(base, 'multimc.cfg')) launcherType = 'MultiMC'
    else if (entries.hasFile(base, 'prismlauncher.cfg')) launcherType = 'PrismLauncher'
    else if (entries.hasMmcInstance(base) || entries.hasFile(base, 'instance.cfg'))
        launcherType = 'MultiMC'
    else if (
        entries.hasVersionJson(base) ||
        entries.hasDirectVersionJson(base) ||
        entries.hasModsJar(base) ||
        entries.hasRootJarAndJson(base)
    )
        launcherType = 'Generic'
    else if (entries.hasMultipleInstanceChildren(base)) launcherType = 'Generic'
    else if (entries.hasFile(base, '.hmcl/config/launcher-settings.json')) launcherType = 'HMCL'
    if (launcherType) {
        return {
            item_type: 'launcher',
            launcher_type: launcherType,
            base_path: walk.resultPath,
            innerBase: innerBase(base),
        }
    }

    if (entries.hasFile(base, 'pack.mcmeta')) {
        const candidates: string[] = []
        if (entries.hasChildFolder(base, 'data')) candidates.push('data_pack')
        if (entries.hasChildFolder(base, 'assets')) candidates.push('resource_pack')
        if (candidates.length > 0) {
            return {
                item_type: 'resource_pack',
                file_path: walk.resultPath,
                candidates,
                innerBase: innerBase(base),
            }
        }
    }
    if (entries.hasPopulatedShadersDir(base)) {
        return { item_type: 'shader_pack', file_path: walk.resultPath, innerBase: innerBase(base) }
    }
    if (entries.hasFile(base, 'level.dat')) {
        return { item_type: 'world_save', file_path: walk.resultPath, innerBase: innerBase(base) }
    }

    if (depth >= MAX_NESTING_DEPTH) {
        return {
            item_type: 'unknown',
            reason: `Archive nesting is too deep to analyze (limit ${MAX_NESTING_DEPTH} levels)`,
        }
    }
    for (const child of entries.childFolders(base)) {
        const result = await classifyEntries(archive, entries, depth + 1, `${base}${child}/`, walk)
        if (result.item_type !== 'unknown') return result
    }
    for (const nested of entries.nestedZipFiles(base)) {
        const entry = archive.entries.find((item) => item.name === `${base}${nested}`)
        if (!entry) continue
        if (!walk.allowNestedUnpack) {
            walk.nestedBytes += entry.uncompressedSize
            continue
        }
        try {
            const bytes = await readZipEntry(archive, entry)
            const inner = await openZip(new Blob([bytes]))
            const result = await classifyEntries(inner, new EntrySet(inner), depth + 1, '', walk)
            if (result.item_type !== 'unknown') {
                // The result points back at the outer archive.
                return {
                    ...result,
                    file_path: result.file_path && walk.resultPath,
                    base_path: result.base_path && walk.resultPath,
                }
            }
        } catch {
            // An unreadable nested archive is skipped like the launcher does.
        }
    }

    if (walk.nestedBytes > 0) {
        return {
            item_type: 'unknown',
            reason: `Archive contains nested archives that must be unpacked to analyze (total ${walk.nestedBytes} bytes)`,
        }
    }
    if (entries.encrypted) {
        return {
            item_type: 'unknown',
            reason: 'Archive contains encrypted files and cannot be fully analyzed',
        }
    }
    return {
        item_type: 'unknown',
        reason: 'ZIP archive requires extraction to determine content type (large archives may take a while)',
    }
}

/** `classify_zip_path`. */
export async function classifyZip(
    source: Blob,
    path: string,
    allowNestedUnpack = false,
): Promise<ClassificationResult> {
    let archive: ZipArchive
    try {
        archive = await openZip(source)
    } catch (error) {
        return {
            item_type: 'unknown',
            reason: error instanceof ZipFormatError ? error.message : 'Cannot open ZIP file',
        }
    }
    return classifyEntries(archive, new EntrySet(archive), 0, '', {
        resultPath: path,
        allowNestedUnpack,
        nestedBytes: 0,
    })
}

/**
 * `classify_zip_with_extraction`: the archive's tree is classified with the folder
 * rules (as if extracted), which also catch `.minecraft` folders and loose game
 * directories the quick scan leaves to extraction.
 */
export async function classifyZipWithExtraction(
    source: Blob,
    path: string,
): Promise<ClassificationResult> {
    let archive: ZipArchive
    try {
        archive = await openZip(source)
    } catch {
        return { item_type: 'unknown', reason: 'File is not a valid ZIP archive' }
    }
    if (archive.entries.length === 0) return { item_type: 'unknown', reason: 'Empty zip file' }
    const root: FsNode = { name: '', kind: 'dir', modified: '', size: 0, children: [] }
    for (const entry of archive.entries) {
        const parts = entry.name.replace(/\/+$/, '').split('/').filter(Boolean)
        let node = root
        parts.forEach((part, index) => {
            const last = index === parts.length - 1
            let child = node.children!.find((item) => item.name === part)
            if (!child) {
                child =
                    last && !entry.directory
                        ? { name: part, kind: 'file', modified: '', size: entry.uncompressedSize }
                        : { name: part, kind: 'dir', modified: '', size: 0, children: [] }
                node.children!.push(child)
            }
            node = child
        })
    }
    const result = classifyFolderNode(root, path)
    if (
        result.item_type === 'launcher' ||
        result.item_type === 'world_save' ||
        result.item_type === 'resource_pack' ||
        result.item_type === 'shader_pack'
    ) {
        return {
            ...result,
            base_path: result.base_path && path,
            file_path: result.file_path && path,
        }
    }
    return result
}

/** `classify_jar`: an HMCL launcher jar, otherwise a mod. */
async function classifyJar(source: Blob, path: string): Promise<ClassificationResult> {
    try {
        const archive = await openZip(source)
        const manifest = await readZipText(archive, 'META-INF/MANIFEST.MF')
        const mainClass = manifest?.match(/^Main-Class:\s*(\S+)/m)?.[1]
        if (mainClass === 'org.jackhuang.hmcl.Main') {
            const parent = path.split('/').slice(0, -1).join('/')
            return { item_type: 'launcher', launcher_type: 'HMCL', base_path: parent }
        }
    } catch {
        // Unreadable manifests fall through to a mod.
    }
    return { item_type: 'mod', file_path: path }
}

/** `classify_dropped_item` for a file the visitor picked. */
export async function classifyPickedFile(source: Blob, path: string, allowNestedUnpack = false) {
    const name = baseName(path)
    const ext = name.includes('.') ? name.split('.').pop()!.toLowerCase() : ''
    if (ext === 'zip' || ext === 'mrpack') return classifyZip(source, path, allowNestedUnpack)
    if (ext === 'exe') {
        return {
            item_type: 'unknown',
            reason: `Unrecognised executable: ${path}`,
        } satisfies ClassificationResult
    }
    if (ext === 'jar') return classifyJar(source, path)
    if (ext === 'litematic' || ext === 'schematic') {
        return { item_type: 'litematic', file_path: path } satisfies ClassificationResult
    }
    return {
        item_type: 'unknown',
        reason: `Unrecognised file type: ${path}`,
    } satisfies ClassificationResult
}
