import { reactive } from 'vue'

import { usePreviewDomain } from '../../../../lib/state/index'
import { type AllSettings, PREVIEW_APP_DIRECTORY, useSettingsPage } from '../../state'
import type { ExternalMinecraftRoot, ExternalMinecraftRootMode } from './direct-links'
import {
    APP_DB_BACKUPS_DIR,
    canonicalize,
    createDirAll,
    DOWNLOADS_DIR,
    useMacFileSystem,
} from './mac-fs'
import { theseusError } from './proxy'

/** `SourceId` values of the resource service's catalogue. */
export type ResourceSourceId = 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'

/** `MissingContentScannerSettings` (helpers/downloads-scanner.ts). */
export interface MissingContentScannerSettings {
    enabled: boolean
    directory: string | null
}

/**
 * The merged settings object. The `.minecraft` roots are kept there under
 * `minecraft_directories` (the app's `axolotl-minecraft-directories` local
 * storage entry), where the creation flow reads them.
 */
type SettingsWithRoots = AllSettings & { minecraft_directories?: unknown }

interface ResourceDomain {
    /** `resource_preferences.preferred_source`. */
    preferredSource: ResourceSourceId
    /** The `axolotl-missing-content-scanner` local storage entry. */
    scanner: MissingContentScannerSettings
    /** The directory the manual-download file watcher watches, or null. */
    watchedDirectory: string | null
    /** Roots added by the import flow that the list has already taken in. */
    adoptedImportRoots: string[]
    /** `curseforge_api_key_override`: the key entered in Settings, kept in the window. */
    curseforgeApiKey: string | null
}

/** `CurseForgeCapability` (helpers/curseforge.ts). */
export interface CurseForgeCapability {
    status: 'missing_key' | 'ready' | 'unauthorized'
    configured: boolean
    key_source: 'override' | 'launcher' | 'missing'
}

function normalizeMode(mode: unknown): ExternalMinecraftRootMode {
    return mode === 'shared' ? 'shared' : mode === 'automatic' ? 'automatic' : 'isolated'
}

export function useResourceStore() {
    const { settings, preview, simulate } = useSettingsPage()
    const fs = useMacFileSystem()
    const merged = settings as SettingsWithRoots
    const domain = usePreviewDomain<ResourceDomain>('settings:resources', () => {
        // `move_launcher_directory` at startup: the launcher records the
        // directory it runs from, so the field shows the current location.
        if (!settings.custom_dir) settings.custom_dir = PREVIEW_APP_DIRECTORY
        if (!settings.prev_custom_dir) settings.prev_custom_dir = settings.custom_dir
        return {
            value: reactive({
                preferredSource: 'modrinth' as ResourceSourceId,
                scanner: { enabled: true, directory: null },
                watchedDirectory: DOWNLOADS_DIR,
                adoptedImportRoots: [],
                curseforgeApiKey: null,
            }),
        }
    })

    /** `resource_preferred_source`. */
    async function resourcePreferredSource(): Promise<ResourceSourceId> {
        await simulate(30)
        return domain.preferredSource
    }

    /** `resource_set_preferred_source`. */
    async function resourceSetPreferredSource(source: ResourceSourceId): Promise<void> {
        await simulate(140)
        domain.preferredSource = source
    }

    /** `getMissingContentScannerSettings`. */
    function getMissingContentScannerSettings(): MissingContentScannerSettings {
        return {
            enabled: domain.scanner.enabled !== false,
            directory:
                typeof domain.scanner.directory === 'string' && domain.scanner.directory.trim()
                    ? domain.scanner.directory
                    : null,
        }
    }

    /** `setMissingContentScannerSettings`. */
    function setMissingContentScannerSettings(value: MissingContentScannerSettings) {
        domain.scanner = {
            enabled: value.enabled,
            directory: value.directory?.trim() || null,
        }
    }

    /** `curseforge_configure_manual_download_watcher`: returns the watched directory. */
    async function configureCurseForgeManualDownloadWatcher(
        enabled: boolean,
        scanDirectory?: string | null,
    ): Promise<string | null> {
        await simulate(50)
        if (!enabled) {
            domain.watchedDirectory = null
            return null
        }
        const disk = await fs.load()
        const resolved = canonicalize(disk, scanDirectory ?? DOWNLOADS_DIR)
        if (!resolved) throw theseusError('I/O (std) error: No such file or directory (os error 2)')
        if (resolved.node.kind !== 'dir') {
            throw theseusError('Invalid input: Manual import watch path is not a directory')
        }
        domain.watchedDirectory = resolved.path
        return resolved.path
    }

    /** `show_app_db_backups_folder`: creates the folder if needed, then reveals it in Finder. */
    async function appDbBackupsFolder(): Promise<string> {
        await simulate(60)
        createDirAll(await fs.load(), APP_DB_BACKUPS_DIR)
        return APP_DB_BACKUPS_DIR
    }

    /**
     * The stored roots (`localStorage['axolotl-minecraft-directories']`), with
     * the roots the gamedir import flow added since the list last read them.
     */
    function readStoredRoots(): unknown[] {
        const stored = Array.isArray(merged.minecraft_directories)
            ? [...(merged.minecraft_directories as unknown[])]
            : []
        const createArea = preview.domains.get('create')?.value as
            { configuredMinecraftDirectories?: { value?: unknown } } | undefined
        const imported = createArea?.configuredMinecraftDirectories?.value
        if (Array.isArray(imported)) {
            const known = new Set(
                stored.map((entry) =>
                    typeof entry === 'string'
                        ? entry.trim()
                        : String((entry as { path?: unknown })?.path ?? '').trim(),
                ),
            )
            for (const entry of imported as { path?: unknown; mode?: unknown }[]) {
                if (typeof entry?.path !== 'string' || !entry.path.trim()) continue
                const path = entry.path.trim()
                if (domain.adoptedImportRoots.includes(path)) continue
                domain.adoptedImportRoots.push(path)
                if (!known.has(path)) stored.push({ path, mode: entry.mode })
            }
        }
        return stored
    }

    /** `loadMinecraftDirectories` of ResourceManagementSettings.vue. */
    function loadMinecraftDirectories(): ExternalMinecraftRoot[] {
        const directories = readStoredRoots().flatMap((value): ExternalMinecraftRoot[] => {
            if (typeof value === 'string') return [{ path: value, mode: 'automatic' }]
            if (
                value &&
                typeof value === 'object' &&
                typeof (value as { path?: unknown }).path === 'string'
            ) {
                const entry = value as { path: string; mode?: unknown }
                return [{ path: entry.path, mode: normalizeMode(entry.mode) }]
            }
            return []
        })
        return [
            ...new Map(directories.map((directory) => [directory.path.trim(), directory])).values(),
        ]
    }

    /** `persistMinecraftDirectories`. */
    function persistMinecraftDirectories(values: ExternalMinecraftRoot[]) {
        const validValues = [
            ...new Map(
                values
                    .map((value) => ({ path: value.path.trim(), mode: normalizeMode(value.mode) }))
                    .map((value) => [value.path, value]),
            ).values(),
        ]
        merged.minecraft_directories = validValues
    }

    /** `curseforge_capability`: release builds carry the launcher's own key. */
    async function getCurseForgeCapability(): Promise<CurseForgeCapability> {
        await simulate(40)
        return {
            status: 'ready',
            configured: true,
            key_source: domain.curseforgeApiKey ? 'override' : 'launcher',
        }
    }

    async function getCurseForgeApiKeyOverride(): Promise<string | null> {
        await simulate(20)
        return domain.curseforgeApiKey
    }

    /** `curseforge_set_api_key`: the key stays in this window and is sent nowhere. */
    async function setCurseForgeApiKeyOverride(apiKey: string | null) {
        await simulate(120)
        domain.curseforgeApiKey = apiKey
        return getCurseForgeCapability()
    }

    return {
        getCurseForgeCapability,
        getCurseForgeApiKeyOverride,
        setCurseForgeApiKeyOverride,
        resourcePreferredSource,
        resourceSetPreferredSource,
        getMissingContentScannerSettings,
        setMissingContentScannerSettings,
        configureCurseForgeManualDownloadWatcher,
        loadMinecraftDirectories,
        persistMinecraftDirectories,
        appDbBackupsFolder,
    }
}
