/**
 * Bridge helpers for the drop classification Tauri commands.
 *
 * These wrap the Tauri `invoke()` calls so the frontend can classify
 * dropped files, scan launcher instances, and detect file locks.
 */
import type { ClassificationChoice, ClassificationResult } from '@modrinth/ui'

import { invoke } from '@/helpers/invoke'

type NativeClassificationResult = Omit<
    ClassificationResult,
    'candidates' | 'choices' | 'item_type' | 'resolved_to'
> &
    (
        | { item_type: 'multiple'; candidates: ClassificationChoice[] }
        | {
              item_type: Exclude<ClassificationResult['item_type'], 'multiple'>
              candidates?: string[]
              resolved_to?: NativeClassificationResult
          }
    )

function normalizeClassification(result: NativeClassificationResult): ClassificationResult {
    if (result.item_type === 'multiple') {
        const { candidates, ...classification } = result
        const source = result.file_path?.replaceAll('\\', '/').replace(/\/$/, '')
        return {
            ...classification,
            choices: candidates.map((candidate) => {
                const path = candidate.filePath?.replaceAll('\\', '/')
                if (!source || !path?.startsWith(`${source}/`) || !candidate.innerBase)
                    return candidate
                // A candidate on disk has its own path. Keep only the part
                // of the base that describes a location inside its archive.
                const relative = path.slice(source.length + 1)
                if (candidate.innerBase === relative) return { ...candidate, innerBase: undefined }
                if (candidate.innerBase.startsWith(`${relative}/`)) {
                    return {
                        ...candidate,
                        innerBase: candidate.innerBase.slice(relative.length + 1),
                    }
                }
                return candidate
            }),
        }
    }
    const { resolved_to, ...classification } = result
    return resolved_to
        ? { ...classification, resolved_to: normalizeClassification(resolved_to) }
        : classification
}

/**
 * Information about a process that has a file handle open.
 */
export interface LockingProcess {
    pid: number
    name: string
    path: string
    start_time: string | null
}

/**
 * Classify a dropped file or folder by its path on disk.
 *
 * @param path Absolute filesystem path to the dropped item
 * @returns Classification result indicating what kind of content it is
 */
export async function classifyDroppedItem(
    path: string,
    allowNestedExtraction = false,
): Promise<ClassificationResult> {
    const result = await invoke<NativeClassificationResult>('plugin:drop|drop_classify', {
        path,
        allowNestedExtraction,
    })
    return normalizeClassification(result)
}

/**
 * Classify a dropped ZIP file by extracting it to a temporary directory first.
 *
 * This is a potentially **long-running** operation — the UI MUST prompt the
 * user before calling this, since extraction can take significant time for
 * large archives.
 *
 * Only call this when [`classifyDroppedItem`] returned `Unknown` with a reason
 * containing "extraction".
 *
 * @param path Absolute filesystem path to the ZIP file
 * @returns Classification result after extraction and analysis
 */
export async function classifyDroppedItemWithExtraction(
    path: string,
): Promise<ClassificationResult> {
    const result = await invoke<NativeClassificationResult>('plugin:drop|drop_classify_extract', {
        path,
    })
    return normalizeClassification(result)
}

/**
 * Extract a ZIP archive into a fresh temporary directory and return its path.
 *
 * Used for compressed launcher folders (e.g. a zipped `.minecraft`): the
 * instance scan and import then operate on the extraction, so the archive is
 * unpacked exactly once. Call [`removeTempDir`] when the flow finishes.
 *
 * @param zipPath Absolute path to the ZIP archive
 * @returns Absolute path of the extraction directory
 */
export function extractZipToTemp(zipPath: string): Promise<string> {
    return invoke('plugin:drop|drop_extract_zip_to_temp', { zipPath })
}

/**
 * Remove a temporary directory created by [`extractZipToTemp`].
 *
 * @param path Absolute path of the extraction directory
 */
export function removeTempDir(path: string): Promise<void> {
    return invoke('plugin:drop|drop_remove_temp_dir', { path })
}

/**
 * Metadata about a single importable instance within a launcher.
 */
export interface ScanInstance {
    name: string
    /** Resolved filesystem path (informational — the backend resolves it) */
    path: string
    /** Minecraft version, if known (empty string otherwise) */
    version: string
    /** Mod loader, if known (e.g. "fabric", "forge"; empty string otherwise) */
    loader: string
    /** Whether this instance qualifies for compatible mode import */
    compatibleMode?: boolean
    /** For compatible mode: path to the version subfolder containing the JSON */
    versionPath?: string
}

/**
 * Result of scanning a single launcher type for importable instances.
 */
export interface ScanResult {
    launcherName: string
    launcherType: string
    instances: ScanInstance[]
}

/**
 * Scan a launcher's data directory for importable Minecraft instances.
 *
 * @param launcherType Launcher type name (e.g. "MultiMC", "PrismLauncher", "HMCL")
 * @param basePath    Root directory of the launcher's data
 * @returns List of scan results (one entry per launcher type)
 */
export async function scanLauncherInstances(
    launcherType: string,
    basePath: string,
): Promise<ScanResult[]> {
    const instances: {
        name: string
        path: string
        compatibleMode?: boolean
        versionPath?: string
    }[] = await invoke('plugin:drop|drop_scan_launcher_instances', {
        launcherType,
        basePath,
    })

    return [
        {
            launcherName: launcherType,
            launcherType,
            instances: instances.map((inst) => ({
                name: inst.name,
                path: inst.path,
                version: '',
                loader: '',
                compatibleMode: inst.compatibleMode,
                versionPath: inst.versionPath,
            })),
        },
    ]
}

export interface ResolvedGamedir {
    path: string
    dialect: string
}

export function resolveGamedirs(
    launcherType: string,
    basePath: string,
): Promise<ResolvedGamedir[]> {
    return invoke('plugin:drop|drop_resolve_gamedirs', { launcherType, basePath })
}

/**
 * Detect processes holding a file lock on the given path.
 *
 * @param path Absolute path to the file to check
 * @returns List of locking processes (empty if unavailable or none found)
 */
export function detectFileLock(path: string): Promise<LockingProcess[]> {
    return invoke('plugin:drop|drop_detect_file_lock', { path })
}

/**
 * Extract mod metadata from a JAR file without installing it.
 *
 * @param path Absolute path to the JAR file
 * @returns JSON string of LocalModMetadata, or null if no metadata found
 */
export function extractModMetadata(path: string): Promise<string | null> {
    return invoke('plugin:drop|drop_extract_mod_metadata', { path })
}

/**
 * Metadata extracted from a mod JAR file.
 */
export interface LocalModMetadata {
    mod_id: string
    name?: string
    version?: string
    authors?: string[]
    description?: string
    url?: string
    icon_path?: string
    minecraft_version?: string
    loader_version?: string
    loader?: string
}

export interface ModrinthLookupResult {
    hash: string
    project_id: string
    version_id: string
    project_name?: string
    project_slug?: string
    version_number?: string
    game_versions: string[]
    loaders: string[]
}

export async function lookupModHash(path: string): Promise<ModrinthLookupResult | null> {
    return invoke('plugin:drop|drop_lookup_mod_hash', { path })
}
