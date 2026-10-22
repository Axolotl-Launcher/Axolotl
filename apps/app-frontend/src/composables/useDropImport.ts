/**
 * Composable encapsulating the entire drag-and-drop import system.
 *
 * Handles single-file drops, batch drops, launcher instance imports,
 * modpack installs, content installation, and all related UI flows.
 */

import type {
    BatchDropGroup,
    BatchDropItem,
    BatchDropPhase,
    ClassificationResult,
    SymlinkMethodChoice,
} from '@modrinth/ui'
import { useDebugLogger, useGlobalDrop, useInstanceContext, useVIntl } from '@modrinth/ui'
import { join } from '@tauri-apps/api/path'
import { computed, type ComputedRef, nextTick, ref } from 'vue'
import type { Router } from 'vue-router'

import { syncConfiguredDirectLinks } from '@/helpers/direct-link-sync'
import {
    classifyDroppedItem,
    classifyDroppedItemWithExtraction,
    detectFileLock,
    extractModMetadata,
    extractZipToTemp,
    lookupModHash,
    type ModrinthLookupResult,
    removeTempDir,
    resolveGamedirs,
    scanLauncherInstances,
    type ScanResult,
} from '@/helpers/drop'
import { import_instance } from '@/helpers/import.js'
import {
    install_create_modpack_instance,
    install_job_cancel,
    wait_for_install_job,
} from '@/helpers/install'
import {
    add_project_from_path,
    check_symlink_capability,
    get as getInstance,
    import_world_save,
    install_datapack_to_world,
    list as listInstances,
} from '@/helpers/instance'
import { getDisplayInstanceIcon } from '@/helpers/instance-icons'
import { assessLocalModCompatibility } from '@/helpers/version-compatibility'
import type { AppNotificationManager } from '@/providers/app-notifications'
import type { AppPopupNotificationManager } from '@/providers/app-popup-notifications'
import type { ContentInstallContext } from '@/providers/content-install'

// Re-export types for external use
export type { ClassificationResult, ModrinthLookupResult, ScanResult }

export type ContentFileProjectType =
    'mod' | 'resourcepack' | 'datapack' | 'shaderpack' | 'schematic'

export interface PendingInstall {
    type: string
    filePath: string
    innerBase?: string
}

export interface PendingDropIncompatibility {
    filePath: string
    instId: string
    type: string
    instVersion: string | undefined
    instLoader: string | undefined
    meta: { name?: string; mod_id?: string } | null
    modrinthLookup: ModrinthLookupResult | null
    innerBase?: string
}

export interface SelectedInstance {
    launcherType: string
    basePath: string
    name: string
    path: string
    compatibleMode?: boolean
    versionPath?: string
}

export interface ImportContext {
    launcherType: string
    basePath: string
}

export interface BatchTargetInstanceInfo {
    id: string
    name: string
    iconUrl?: string | null
    gameVersion?: string | null
    loader?: string | null
}

export interface DropImportOptions {
    /** Notification manager instance */
    notificationManager: AppNotificationManager
    /** Popup notification manager instance */
    popupNotificationManager: AppPopupNotificationManager
    /** Install modpack from path function */
    installModpackFromPath: (
        path: string,
        name: string,
        options: { persistUntilDone: boolean },
    ) => Promise<void>
    /** Content install provider functions */
    contentInstall: ContentInstallContext
    /** File drop handler from providers */
    fileDrop: (paths: string[]) => void
    /** Whether currently on skins page */
    onSkinsPage: ComputedRef<boolean>
    /** Whether currently on schematic workshop page */
    onSchematicWorkshopPage: ComputedRef<boolean>
    /** Whether currently on the settings page */
    onSettingsPage: ComputedRef<boolean>
    /** Check if path is a schematic file */
    isSchematicFile: (path: string) => boolean
    /** Track analytics event */
    trackEvent: (name: string, properties?: Record<string, unknown>) => void
    /** Route to push */
    router: Router
}

/**
 * Composable that manages the entire drag-and-drop import system.
 *
 * Usage:
 * ```ts
 * const dropImport = useDropImport({
 *   notificationManager,
 *   popupNotificationManager,
 *   handleError,
 *   installModpackFromPath,
 *   contentInstall,
 *   fileDrop,
 *   onSkinsPage,
 *   onSchematicWorkshopPage,
 *   onSettingsPage,
 *   isSchematicFile,
 *   trackEvent,
 *   router,
 *   route,
 * })
 * ```
 */
export function useDropImport(options: DropImportOptions) {
    const {
        notificationManager,
        popupNotificationManager,
        installModpackFromPath,
        contentInstall,
        fileDrop,
        onSkinsPage,
        onSchematicWorkshopPage,
        onSettingsPage,
        isSchematicFile,
        trackEvent,
        router,
    } = options

    const { formatMessage } = useVIntl()
    const dropDebug = useDebugLogger('DropFlow')
    const { addNotification } = notificationManager
    const { addPopupNotification } = popupNotificationManager

    const isSettingsImagePath = (path: string) =>
        /\.(png|jpe?g|webp|gif|avif|bmp)$/i.test(path.split(/[/\\]/).pop() ?? '')

    // ── Instance context ──────────────────────────────────────────────────
    const { isInInstance, instanceId } = useInstanceContext()

    // ── Modal refs (to be bound externally) ──────────────────────────────
    const confirmDropModal = ref<{ show: () => void; hide: () => void } | null>(null)
    const genericInstallModal = ref<{
        show: (options: {
            contentType: string
            fileName: string
            instances: BatchTargetInstanceInfo[]
        }) => void
        hide: () => void
    } | null>(null)
    const launcherImportModal = ref<{
        show: (results: ScanResult[]) => void
        hide: () => void
    } | null>(null)
    const symlinkCardsModal = ref<{
        show: (options: {
            instances: Array<{
                name: string
                path?: string
                launcherType?: string
                basePath?: string
                compatibleMode?: boolean
            }>
            symlinkCapable: string
        }) => void
        hide: () => void
    } | null>(null)
    const dataPackWorldModal = ref<{
        show: (instanceId?: string) => void
    } | null>(null)
    const compatibleModeConfirmModal = ref<{ show: () => void } | null>(null)
    const incompatibilityWarningModal = ref<{ show: () => void } | null>(null)

    // ── Content file project type map ────────────────────────────────────
    const contentFileProjectTypeMap: Record<string, ContentFileProjectType | undefined> = {
        mod: 'mod',
        resource_pack: 'resourcepack',
        data_pack: 'datapack',
        shader_pack: 'shaderpack',
        litematic: 'schematic',
        schematic: 'schematic',
    }

    // ── Single-file drop state ───────────────────────────────────────────
    const dropClassification = ref<ClassificationResult | null>(null)
    const dropFileName = ref('')
    const dropFilePath = ref('')
    const lastDroppedPath = ref('')
    const scanningInstances = ref(false)
    const pendingInstall = ref<PendingInstall | null>(null)
    const pendingDropIncompatibility = ref<PendingDropIncompatibility | null>(null)
    const selectedInstances = ref<SelectedInstance[]>([])
    const currentImportContext = ref<ImportContext | null>(null)
    const compatibleModeResults = ref<ScanResult[] | null>(null)
    const compatibleModeGameDir = ref<string | null>(null)
    const compatibleModeLauncherType = ref<string>('Generic')
    const launcherZipTempDir = ref<string | null>(null)
    const dropProcessingNotificationId = ref<string | number | null>(null)

    // ── Batch drop state ─────────────────────────────────────────────────
    const batchPhase = ref<BatchDropPhase>('idle')
    const batchActive = computed(() => batchPhase.value !== 'idle')
    const batchItems = ref<BatchDropItem[]>([])
    const batchOriginalCount = ref(0)
    const batchScanDone = ref(0)
    const batchGroups = ref<BatchDropGroup[]>([])
    const batchCurrentGroup = ref<BatchDropGroup | null>(null)
    const batchTargetInstances = ref<BatchTargetInstanceInfo[]>([])
    const batchTargetInstanceId = ref('')
    const batchWorldPath = ref('')
    const batchTempDirs = ref<string[]>([])

    let batchScanCancelled = false
    let batchInstallCancelled = false
    let batchScanPromise: Promise<void> | null = null
    let batchConfirmIndex = 0
    let batchSymlinkMode = false
    let batchTargetPickMode = false
    let batchWorldMode = false
    let batchGroupMode = false
    const batchGroupKey = ref(0)
    const incompatWarningKey = ref(0)
    let batchCompatResolve: ((result: 'installed' | 'skipped' | 'failed') => void) | null = null
    let batchCurrentInstallJobId: string | null = null

    // ── Content install incompatibility state ─────────────────────────────
    const contentInstallIncompatibilityWarningVersions = ref([])
    const contentInstallIncompatibilityWarningCurrentGameVersion = ref('')
    const contentInstallIncompatibilityWarningCurrentLoader = ref('')
    const contentInstallIncompatibilityWarningProjectType = ref('')
    const contentInstallIncompatibilityWarningProjectIconUrl = ref('')
    const contentInstallIncompatibilityWarningProjectName = ref('')
    const contentInstallIncompatibilityWarningMessage = ref('')
    const contentInstallIncompatibilityWarningInstalling = ref(false)

    // ── Symlink choice state ─────────────────────────────────────────────
    let symlinkChoiceResolve: ((choices: SymlinkMethodChoice[]) => void) | null = null

    // ── Messages ─────────────────────────────────────────────────────────
    const messages = defineMessages({
        unknownCompatibility: {
            id: 'app.drop.compatibility.unknown',
            defaultMessage: 'Unknown',
        },
        dropOverlayTitle: {
            id: 'app.drop.overlay-title',
            defaultMessage: 'Drop to import',
        },
        dropOverlaySubtitle: {
            id: 'app.drop.overlay-subtitle',
            defaultMessage: 'Release to analyze',
        },
        dropProcessing: {
            id: 'app.drop.processing',
            defaultMessage: 'Processing {name}...',
        },
        dropMultipleFilesTitle: {
            id: 'app.drop.error.multiple-files-title',
            defaultMessage: 'Cannot import multiple files',
        },
        dropMultipleFilesText: {
            id: 'app.drop.error.multiple-files-text',
            defaultMessage: 'Please drop one file at a time.',
        },
        dropShortcutFailedTitle: {
            id: 'app.drop.error.shortcut-title',
            defaultMessage: 'Shortcut resolution failed',
        },
        dropShortcutFailedText: {
            id: 'app.drop.error.shortcut-text',
            defaultMessage: 'Could not resolve the shortcut target.',
        },
        dropUnknownTitle: {
            id: 'app.drop.error.unknown-title',
            defaultMessage: 'Unknown file type',
        },
        dropUnknownText: {
            id: 'app.drop.error.unknown-text',
            defaultMessage: 'Could not determine what kind of file this is.',
        },
        dropUnknownDepthText: {
            id: 'app.drop.error.unknown-depth-text',
            defaultMessage:
                'The archive is nested too deeply to analyze. Unpack it to a folder and try again.',
        },
        dropUnknownEncryptedText: {
            id: 'app.drop.error.unknown-encrypted-text',
            defaultMessage: 'The archive contains encrypted files and cannot be analyzed.',
        },
        dropNestedUnpackTitle: {
            id: 'app.drop.nested-unpack-title',
            defaultMessage: 'Nested archives detected',
        },
        dropNestedUnpackText: {
            id: 'app.drop.nested-unpack-text',
            defaultMessage:
                'This archive contains nested archives ({size}) that must be unpacked to analyze. This may take some time. Continue?',
        },
        dropNestedUnpackButton: {
            id: 'app.drop.nested-unpack-button',
            defaultMessage: 'Continue analysis',
        },
        dropErrorTitle: {
            id: 'app.drop.error.title',
            defaultMessage: 'Drop error',
        },
        dropWorldImportedTitle: {
            id: 'app.drop.world-imported-title',
            defaultMessage: 'World imported',
        },
        dropWorldImportedText: {
            id: 'app.drop.world-imported-text',
            defaultMessage: 'World save has been imported successfully.',
        },
        dropContentInstalledTitle: {
            id: 'app.drop.content-installed-title',
            defaultMessage: 'Content installed',
        },
        dropContentInstalledText: {
            id: 'app.drop.content-installed-text',
            defaultMessage: 'File has been installed to the instance.',
        },
        dropInstallFailedTitle: {
            id: 'app.drop.install-failed-title',
            defaultMessage: 'Installation failed',
        },
        dropInstanceImportedTitle: {
            id: 'app.drop.instance-imported-title',
            defaultMessage: 'Instance imported',
        },
        dropInstanceImportedText: {
            id: 'app.drop.instance-imported-text',
            defaultMessage: '{name} imported successfully.',
        },
        dropImportFailedTitle: {
            id: 'app.drop.import-failed-title',
            defaultMessage: 'Import failed',
        },
        dropImportFailedText: {
            id: 'app.drop.import-failed-text',
            defaultMessage: 'Failed to import {name}: {error}',
        },
        dropNoInstances: {
            id: 'app.drop.no-instances',
            defaultMessage: 'No instances found',
        },
        dropScanning: {
            id: 'app.drop.scanning',
            defaultMessage: 'Scanning for instances',
        },
        dropScanFailed: {
            id: 'app.drop.scan-failed',
            defaultMessage: 'Failed to scan for instances',
        },
        dropExtractFailed: {
            id: 'app.drop.extract-failed',
            defaultMessage: 'Failed to extract archive',
        },
        dropProcessFailedTitle: {
            id: 'app.drop.process-failed-title',
            defaultMessage: 'Failed to process file',
        },
        dropTemporaryFileTitle: {
            id: 'app.drop.temporary-file-title',
            defaultMessage: 'Temporary file detected',
        },
        dropTemporaryFileText: {
            id: 'app.drop.temporary-file-text',
            defaultMessage:
                'The file "{file}" appears to be a temporary copy. Try dragging the file from its original folder instead of from a browser, archive, or cloud storage.',
        },
        dropImportProgressTitle: {
            id: 'app.drop.import-progress-title',
            defaultMessage: 'Importing instances…',
        },
        dropImportProgressText: {
            id: 'app.drop.import-progress-text',
            defaultMessage: '{current} / {total} instances imported',
        },
        dropImportCompletedTitle: {
            id: 'app.drop.import-completed-title',
            defaultMessage: 'Import completed',
        },
        dropImportCompletedText: {
            id: 'app.drop.import-completed-text',
            defaultMessage: 'Successfully imported {count} instances',
        },
        dropImportCompletedPartialText: {
            id: 'app.drop.import-completed-partial-text',
            defaultMessage: 'Imported {completed} of {total} instances ({failed} failed)',
        },
        dropImportCancelledTitle: {
            id: 'app.drop.batch.import-cancelled-title',
            defaultMessage: 'Import cancelled',
        },
        dropImportCancelledText: {
            id: 'app.drop.batch.import-cancelled-text',
            defaultMessage: 'Nothing was imported.',
        },
        dropBatchNothingImportableTitle: {
            id: 'app.drop.batch.nothing-importable-title',
            defaultMessage: 'Nothing to import',
        },
        dropBatchNothingImportableText: {
            id: 'app.drop.batch.nothing-importable-text',
            defaultMessage:
                '{count, plural, one {# file} other {# files}} could not be recognized.',
        },
        dropBatchCompletedTitle: {
            id: 'app.drop.batch.completed-title',
            defaultMessage: 'Import finished',
        },
        dropBatchCompletedText: {
            id: 'app.drop.batch.completed-text',
            defaultMessage: 'Imported {succeeded} of {total} ({failed} failed, {skipped} skipped).',
        },
        dropBatchTargetLabel: {
            id: 'app.drop.batch.target-label',
            defaultMessage: 'Select target instance for this batch',
        },
        dropBatchGroupFileLabel: {
            id: 'app.drop.batch.group-file-label',
            defaultMessage: '{count, plural, one {# file} other {# files}}: {names}',
        },
        dropModpackInstallFailed: {
            id: 'app.drop.modpack-install-failed',
            defaultMessage: 'Failed to install modpack',
        },
        dropUnknownForceAnalysisTitle: {
            id: 'app.drop.unknown-force-analysis-title',
            defaultMessage: 'Unable to identify file type',
        },
        dropUnknownForceAnalysisText: {
            id: 'app.drop.unknown-force-analysis-text',
            defaultMessage:
                'This archive needs to be extracted and deeply analyzed to determine its content type. This may take a while. Force analysis?',
        },
        dropUnknownForceAnalysisButton: {
            id: 'app.drop.unknown-force-analysis-button',
            defaultMessage: 'Force analysis',
        },
        dropUnknownForceAnalyzing: {
            id: 'app.drop.unknown-force-analyzing',
            defaultMessage: 'Force analyzing archive...',
        },
        dropUnknownForceAnalysisFailedTitle: {
            id: 'app.drop.unknown-force-analysis-failed-title',
            defaultMessage: 'Analysis failed',
        },
        dropUnknownForceAnalysisFailedText: {
            id: 'app.drop.unknown-force-analysis-failed-text',
            defaultMessage: 'Could not identify the file type even after deep analysis.',
        },
        dropInstallModWarning: {
            id: 'app.drop.compatibility-review-warning',
            defaultMessage:
                '{file} targets {modVersion} ({modLoader}); the instance is {instVersion} ({instLoader}). Minecraft: {gameState, select, compatible {compatible} incompatible {incompatible} other {unknown}}; loader: {loaderState, select, compatible {compatible} incompatible {incompatible} other {unknown}}. Confirm to import this file with these compatibility exceptions.',
        },
        dropCompatibleModeTitle: {
            id: 'app.drop.compatible-mode-title',
            defaultMessage: 'This appears to be a pre-version-isolation instance',
        },
        dropCompatibleModeDesc: {
            id: 'app.drop.compatible-mode-desc',
            defaultMessage: 'Would you like to import this as a compatible mode instance?',
        },
        dropCompatibleModeImport: {
            id: 'app.drop.compatible-mode-import',
            defaultMessage: 'Compatible Import',
        },
        dropCompatibleModeOldWay: {
            id: 'app.drop.compatible-mode-old-way',
            defaultMessage: 'Import as Old Version',
        },
        dropCompatibleModeCancel: {
            id: 'app.drop.compatible-mode-cancel',
            defaultMessage: 'Cancel',
        },
    })

    // ── Helper functions ─────────────────────────────────────────────────

    function clearDropProcessingNotification() {
        if (dropProcessingNotificationId.value !== null) {
            notificationManager.removeNotification(dropProcessingNotificationId.value)
            dropProcessingNotificationId.value = null
        }
    }

    function isZipPath(path: string): boolean {
        return /\.zip$/i.test(path)
    }

    function unknownReasonMessage(reason: string | undefined): string {
        const normalized = reason?.toLowerCase() ?? ''
        if (normalized.includes('too deep') || normalized.includes('nesting')) {
            return formatMessage(messages.dropUnknownDepthText)
        }
        if (normalized.includes('encrypted')) {
            return formatMessage(messages.dropUnknownEncryptedText)
        }
        return reason ? reason : formatMessage(messages.dropUnknownText)
    }

    async function cleanupLauncherZipTemp() {
        const tempDir = launcherZipTempDir.value
        if (!tempDir) return
        launcherZipTempDir.value = null
        try {
            await removeTempDir(tempDir)
            dropDebug('handleDropConfirm: launcher zip temp cleaned', { tempDir })
        } catch (error) {
            dropDebug('handleDropConfirm: launcher zip temp cleanup failed', error)
        }
    }

    // ── Classification ───────────────────────────────────────────────────

    async function classifyDropPath(path: string): Promise<ClassificationResult> {
        lastDroppedPath.value = path
        if (onSkinsPage.value) {
            return { item_type: 'unknown' as const, file_path: path, reason: 'skipped' }
        }
        if (onSchematicWorkshopPage.value && isSchematicFile(path)) {
            return { item_type: 'unknown' as const, file_path: path, reason: 'skipped' }
        }
        if (onSettingsPage.value && isSettingsImagePath(path)) {
            return { item_type: 'unknown' as const, file_path: path, reason: 'skipped' }
        }
        const folderName = path.split(/[\\/]/).filter(Boolean).at(-1)
        if (folderName?.toLowerCase() === '.minecraft') {
            dropDebug('classifyDropPath: routing .minecraft folder as GameDir', { path })
            return {
                item_type: 'launcher',
                launcher_type: 'Generic',
                base_path: path,
                innerBase: undefined,
            }
        }
        return classifyDroppedItem(path)
    }

    function resolveBatchClassification(
        result: ClassificationResult,
        depth = 0,
    ): ClassificationResult {
        if (result.item_type === 'shortcut_resolved' && result.resolved_to && depth < 3) {
            return resolveBatchClassification(result.resolved_to, depth + 1)
        }
        return result
    }

    // ── Compatible mode ──────────────────────────────────────────────────

    async function handleCompatibleModeConfirm(choice: 'compatible' | 'old-way' | 'cancel') {
        if (choice === 'cancel') {
            currentImportContext.value = null
            return
        }

        const gameDir = compatibleModeGameDir.value ?? dropFilePath.value!
        const launcherType = compatibleModeLauncherType.value
        const scanResults = compatibleModeResults.value
        const instanceName = scanResults?.[0]?.instances[0]?.name ?? ''
        // Strip launcher prefix and "versions/" prefix if present
        // (e.g. ".minecraft:versions/1.12.2" → "1.12.2")
        let versionName = instanceName
        const colonIdx = versionName.lastIndexOf(':')
        if (colonIdx >= 0) {
            versionName = versionName.slice(colonIdx + 1)
        }
        if (versionName.startsWith('versions/')) {
            versionName = versionName.slice('versions/'.length)
        }
        dropDebug('handleCompatibleModeConfirm', {
            choice,
            gameDir,
            instanceName,
            versionName,
            launcherType,
        })

        if (choice === 'compatible') {
            const single = scanResults?.[0]?.instances[0]
            if (!single?.versionPath) {
                currentImportContext.value = null
                dropDebug('handleCompatibleModeConfirm: missing versionPath', {
                    instanceName,
                })
                addNotification({
                    title: formatMessage(messages.dropNoInstances),
                    type: 'error',
                })
                return
            }
            selectedInstances.value = [
                {
                    launcherType,
                    basePath: gameDir,
                    name: versionName,
                    path: gameDir,
                    compatibleMode: true,
                    versionPath: single.versionPath,
                },
            ]
            const cap = await check_symlink_capability()
            symlinkCardsModal.value?.show({
                instances: [
                    {
                        name: versionName,
                        path: gameDir,
                        launcherType,
                        basePath: gameDir,
                        compatibleMode: true,
                        versionPath: single.versionPath,
                    },
                ],
                symlinkCapable: cap,
            })
        } else if (choice === 'old-way') {
            const single = scanResults![0].instances[0]
            selectedInstances.value = [
                {
                    launcherType,
                    basePath: gameDir,
                    name: versionName,
                    path: single.path,
                },
            ]
            const cap = await check_symlink_capability()
            symlinkCardsModal.value?.show({
                instances: [
                    {
                        name: versionName,
                        path: single.path,
                        launcherType,
                        basePath: gameDir,
                    },
                ],
                symlinkCapable: cap,
            })
        }
    }

    // ── Single-file drop flow ────────────────────────────────────────────

    function handleDropCancel() {
        clearDropProcessingNotification()
        dropClassification.value = null
    }

    function handleConfirmDropCancel() {
        if (batchGroupMode) {
            batchGroupMode = false
            confirmDropModal.value?.hide()
            void cancelBatch('group-cancel')
            return
        }
        handleDropCancel()
    }

    async function handleConfirmDropConfirm(type: string, innerBase?: string) {
        if (batchGroupMode) {
            batchGroupMode = false
            confirmDropModal.value?.hide()
            onBatchGroupConfirm(type)
            return
        }
        await handleDropConfirm(type, innerBase)
    }

    async function handleConfirmDropHelp() {
        if (batchGroupMode) {
            batchGroupMode = false
            confirmDropModal.value?.hide()
            void cancelBatch('group-help')
        }
        await handleDropHelp()
    }

    async function routeToGamedirFlow(paths: string[]) {
        dropDebug('gamedir flow: attempting direct-link sync', { paths })
        const roots = paths
            .filter((path) => path.trim())
            .map((path) => ({ path: path.trim(), mode: 'automatic' as const }))
        if (roots.length === 0) return false
        try {
            const raw = localStorage.getItem('axolotl-minecraft-directories')
            const existing = raw ? JSON.parse(raw) : []
            const merged = [...(Array.isArray(existing) ? existing : []), ...roots]
            const unique = [...new Map(merged.map((root) => [root.path, root])).values()]
            localStorage.setItem('axolotl-minecraft-directories', JSON.stringify(unique))
            const report = await syncConfiguredDirectLinks(unique)
            dropDebug('gamedir flow: direct-link sync completed', { roots: unique, report })
            if (report.errors.length > 0) {
                throw new Error(report.errors.join('\n'))
            }
            addNotification({
                title: formatMessage(messages.dropInstanceImportedTitle),
                text: formatMessage(messages.dropInstanceImportedText, { name: paths.join(', ') }),
                type: 'success',
            })
            return true
        } catch (error) {
            addNotification({
                title: formatMessage(messages.dropScanFailed),
                text: error instanceof Error ? error.message : String(error),
                type: 'error',
            })
            return false
        }
    }

    async function handleDropConfirm(type: string, innerBase?: string) {
        const classification = dropClassification.value
        dropClassification.value = null
        confirmDropModal.value?.hide()

        dropDebug('handleDropConfirm: entry', {
            type,
            classification_item_type: classification?.item_type,
            file_path: classification?.file_path,
        })

        const isLauncherImport =
            classification?.item_type === 'launcher' ||
            classification?.item_type === 'hmcl_launcher'

        if (!isLauncherImport && !classification?.file_path && !dropFilePath.value) {
            dropDebug(
                'handleDropConfirm: no filePath available (classification and dropFilePath both empty), aborting',
            )
            return
        }

        const filePath = classification?.file_path ?? dropFilePath.value
        const fileName =
            filePath?.split(/[/\\]/).pop() ??
            classification?.base_path?.split(/[/\\]/).pop() ??
            'file'
        dropDebug('handleDropConfirm: routing decision', {
            type,
            isLauncherImport,
            item_type: classification?.item_type,
        })

        if (type === 'dot_minecraft') {
            await routeToGamedirFlow([dropFilePath.value ?? ''])
            return
        }

        if (isLauncherImport && type === 'instance') {
            const launcherType =
                classification!.item_type === 'hmcl_launcher'
                    ? 'HMCL'
                    : classification!.launcher_type!
            const basePath =
                classification!.item_type === 'hmcl_launcher'
                    ? classification!.launcher_dir!
                    : classification!.base_path!
            dropDebug('handleDropConfirm: launcher import branch', { launcherType, basePath })
            dropDebug('handleDropConfirm: launcher classified for gamedir routing', {
                launcherType,
                basePath,
                configPath:
                    launcherType === 'HMCL'
                        ? `${basePath}/.hmcl/hmcl.json`
                        : 'PCL global registry / PCL-CE config',
            })
            if (['Generic', 'PCL2', 'PCL2CE', 'HMCL'].includes(launcherType)) {
                try {
                    const resolved = await resolveGamedirs(
                        launcherType,
                        classification!.base_path ?? classification!.launcher_dir ?? filePath!,
                    )
                    dropDebug('handleDropConfirm: resolved launcher gamedirs', {
                        launcherType,
                        resolved,
                    })
                    if (resolved.length === 0) {
                        throw new Error(formatMessage(messages.dropNoInstances))
                    }
                    await routeToGamedirFlow(resolved.map((entry) => entry.path))
                } catch (error) {
                    addNotification({
                        title: formatMessage(messages.dropScanFailed),
                        text: error instanceof Error ? error.message : String(error),
                        type: 'error',
                    })
                }
                return
            }

            let scanBasePath = basePath
            if (isZipPath(basePath)) {
                scanningInstances.value = true
                try {
                    const tempDir = await extractZipToTemp(basePath)
                    launcherZipTempDir.value = tempDir
                    scanBasePath = classification!.innerBase
                        ? await join(tempDir, classification!.innerBase)
                        : tempDir
                    dropDebug('handleDropConfirm: extracted launcher zip', {
                        tempDir,
                        innerBase: classification!.innerBase,
                        scanBasePath,
                    })
                } catch (error) {
                    launcherZipTempDir.value = null
                    const errorDetail = error instanceof Error ? error.message : String(error)
                    console.error(
                        '[DropFlow] launcher zip extraction failed:',
                        errorDetail,
                        basePath,
                    )
                    dropDebug('handleDropConfirm: launcher zip extraction failed', error)
                    addNotification({
                        title: formatMessage(messages.dropExtractFailed),
                        text: errorDetail,
                        type: 'error',
                    })
                    return
                } finally {
                    scanningInstances.value = false
                }
            }

            currentImportContext.value = { launcherType, basePath: scanBasePath }
            scanningInstances.value = true
            let results: ScanResult[]
            try {
                results = await scanLauncherInstances(launcherType, scanBasePath)
            } catch (error) {
                currentImportContext.value = null
                dropDebug('handleDropConfirm: launcher scan failed', error)
                addNotification({ title: formatMessage(messages.dropScanFailed), type: 'error' })
                cleanupLauncherZipTemp()
                return
            } finally {
                scanningInstances.value = false
            }
            const totalInstances = results.reduce((s, r) => s + r.instances.length, 0)
            dropDebug('handleDropConfirm: launcher scan result', { totalInstances, results })

            if (totalInstances === 0) {
                currentImportContext.value = null
                dropDebug('handleDropConfirm: no instances found')
                addNotification({ title: formatMessage(messages.dropNoInstances), type: 'warning' })
                cleanupLauncherZipTemp()
                return
            }

            if (totalInstances === 1 && results[0]?.instances[0]) {
                const single = results[0].instances[0]
                dropDebug('handleDropConfirm: single instance, showing symlink modal', {
                    name: single.name,
                    path: single.path,
                })
                selectedInstances.value = [
                    {
                        launcherType,
                        basePath: single.compatibleMode ? single.path : scanBasePath,
                        name: single.name,
                        path: single.compatibleMode
                            ? (single.versionPath ?? single.path)
                            : single.path,
                        compatibleMode: single.compatibleMode,
                        versionPath: single.versionPath,
                    },
                ]
                if (launcherZipTempDir.value) {
                    dropDebug('handleDropConfirm: zip source, importing as copy')
                    await onSymlinkMethodConfirmed(false)
                    return
                }
                const cap = await check_symlink_capability()
                symlinkCardsModal.value?.show({
                    instances: [
                        {
                            name: single.name,
                            path: single.compatibleMode
                                ? (single.versionPath ?? single.path)
                                : single.path,
                            launcherType,
                            basePath: single.compatibleMode ? single.path : scanBasePath,
                            compatibleMode: single.compatibleMode,
                            versionPath: single.versionPath,
                        },
                    ],
                    symlinkCapable: cap,
                })
                return
            }

            dropDebug('handleDropConfirm: multiple instances, showing launcher import modal')
            launcherImportModal.value?.show(results)
            return
        }

        if (type === 'modpack') {
            dropDebug('handleDropConfirm: modpack branch', { filePath, fileName })

            if (!filePath) {
                dropDebug('handleDropConfirm: modpack — no filePath, aborting')
                addNotification({
                    title: formatMessage(messages.dropModpackInstallFailed),
                    type: 'error',
                })
                return
            }

            clearDropProcessingNotification()
            await installModpackFromPath(filePath, fileName, { persistUntilDone: true })
            trackEvent('InstanceCreate', { source: 'DropConfirmModpack' })
            await router.push('/library')
            return
        }

        const contentTypes = [
            'mod',
            'resource_pack',
            'data_pack',
            'shader_pack',
            'world_save',
            'litematic',
            'schematic',
        ]
        if (!contentTypes.includes(type)) {
            dropDebug('handleDropConfirm: type not in contentTypes — FALLTHROUGH, no handler!', {
                type,
                contentTypes,
            })
            return
        }

        dropDebug('handleDropConfirm: content install branch', {
            type,
            isInInstance: isInInstance.value,
            hasInstanceId: !!instanceId.value,
        })

        if (type === 'data_pack') {
            dropDebug('handleDropConfirm: data pack requires a world target', {
                filePath,
            })
            pendingInstall.value = { type, filePath, innerBase }
            dataPackWorldModal.value?.show(
                isInInstance.value ? (instanceId.value ?? undefined) : undefined,
            )
            return
        }

        if (isInInstance.value && instanceId.value) {
            dropDebug('handleDropConfirm: installing directly to current instance', {
                instanceId: instanceId.value,
            })
            await installContentDirectly(type, filePath, instanceId.value, innerBase)
        } else {
            dropDebug(
                'handleDropConfirm: storing pending install, showing instance selection modal',
            )
            pendingInstall.value = { type, filePath, innerBase }

            let instances: BatchTargetInstanceInfo[] = []
            try {
                const allInstances = await listInstances()
                instances = allInstances.map((inst) => ({
                    id: inst.id,
                    name: inst.name,
                    iconUrl: getDisplayInstanceIcon(inst.icon_path, inst.loader).url,
                    gameVersion: inst.game_version || null,
                    loader: inst.loader || null,
                }))
            } catch {
                // If listing fails, show empty list
            }
            genericInstallModal.value?.show({
                contentType: type,
                fileName,
                instances,
            })
        }
    }

    async function installContentDirectly(
        type: string,
        filePath: string,
        instId: string,
        innerBase?: string,
        throwOnError = false,
    ) {
        const throwIfBatchCancelled = () => {
            if (throwOnError && batchInstallCancelled) {
                throw new Error('Batch import cancelled')
            }
        }

        try {
            throwIfBatchCancelled()
            if (type === 'world_save') {
                await import_world_save(instId, filePath, innerBase)
                addNotification({
                    title: formatMessage(messages.dropWorldImportedTitle),
                    text: formatMessage(messages.dropWorldImportedText),
                    type: 'success',
                })
                return
            }

            if (type === 'mod') {
                let meta: {
                    minecraft_version?: string
                    loader?: string
                    name?: string
                    mod_id?: string
                } | null = null
                let modrinthLookup: ModrinthLookupResult | null = null

                const metaStr = await extractModMetadata(filePath)
                throwIfBatchCancelled()
                dropDebug('installContentDirectly: mod metadata extraction', {
                    filePath,
                    hasMeta: !!metaStr,
                })

                if (metaStr) {
                    try {
                        meta = JSON.parse(metaStr)
                        dropDebug('installContentDirectly: parsed mod metadata', { meta })
                    } catch (e) {
                        dropDebug('installContentDirectly: failed to parse mod metadata', {
                            error: e,
                        })
                    }
                }

                try {
                    modrinthLookup = await lookupModHash(filePath)
                    throwIfBatchCancelled()
                    dropDebug('installContentDirectly: modrinth hash lookup', {
                        found: !!modrinthLookup,
                    })
                } catch (e) {
                    dropDebug('installContentDirectly: hash lookup failed', { error: e })
                }

                const inst = await getInstance(instId)
                throwIfBatchCancelled()
                dropDebug('installContentDirectly: instance details', {
                    inst: inst?.id,
                    game_version: inst?.game_version,
                    loader: inst?.loader,
                })

                if (inst) {
                    const instVersion = inst.game_version
                    const instLoader = inst.loader
                    const modMcVersion = meta?.minecraft_version
                    const modLoader = meta?.loader

                    const assessment = assessLocalModCompatibility(meta, inst)
                    const versionMismatch = assessment.gameVersion !== 'compatible'
                    const loaderMismatch = assessment.loader !== 'compatible'

                    dropDebug('installContentDirectly: compatibility check', {
                        versionMismatch,
                        loaderMismatch,
                        modMcVersion,
                        instVersion,
                        modLoader,
                        instLoader,
                    })

                    if (versionMismatch || loaderMismatch) {
                        throwIfBatchCancelled()
                        pendingDropIncompatibility.value = {
                            filePath,
                            instId,
                            type,
                            instVersion,
                            instLoader,
                            meta,
                            modrinthLookup,
                            innerBase,
                        }
                        const warning = formatMessage(messages.dropInstallModWarning, {
                            gameState: assessment.gameVersion,
                            loaderState: assessment.loader,
                            file: filePath.split(/[/\\]/).pop() || filePath,
                            modVersion:
                                modMcVersion ?? formatMessage(messages.unknownCompatibility),
                            modLoader: modLoader ?? formatMessage(messages.unknownCompatibility),
                            instVersion: instVersion ?? 'any',
                            instLoader: instLoader ?? 'none',
                        })
                        contentInstall.incompatibilityWarningVersions.value = []
                        contentInstall.incompatibilityWarningCurrentGameVersion.value =
                            instVersion ?? ''
                        contentInstall.incompatibilityWarningCurrentLoader.value = instLoader ?? ''
                        contentInstall.incompatibilityWarningProjectType.value = 'mod'
                        contentInstall.incompatibilityWarningProjectName.value = meta?.name ?? 'Mod'
                        contentInstall.incompatibilityWarningMessage.value = warning
                        contentInstall.incompatibilityWarningInstalling.value = false
                        incompatWarningKey.value++
                        await nextTick()
                        throwIfBatchCancelled()
                        incompatibilityWarningModal.value?.show()
                        return
                    }
                } else {
                    dropDebug('installContentDirectly: skipping version check', {
                        hasInstance: !!inst,
                        hasModVersion: !!meta?.minecraft_version,
                    })
                }
            }

            const projectType = contentFileProjectTypeMap[type]
            await add_project_from_path(instId, filePath, projectType, innerBase)
            addNotification({
                title: formatMessage(messages.dropContentInstalledTitle),
                text: formatMessage(messages.dropContentInstalledText),
                type: 'success',
            })
        } catch (e) {
            if (throwOnError && batchInstallCancelled) throw e
            let errMsg =
                e instanceof Error ? e.message : typeof e === 'string' ? e : JSON.stringify(e)
            try {
                const lockInfo = await detectFileLock(filePath)
                if (lockInfo.length > 0) {
                    const lockLines = lockInfo
                        .map((p) => `  PID ${p.pid}: ${p.name} (${p.path})`)
                        .join('\n')
                    errMsg += `\n\nFile locked by:\n${lockLines}`
                }
            } catch {
                // Lock detection is best-effort
            }
            addNotification({
                title: formatMessage(messages.dropInstallFailedTitle),
                text: errMsg,
                type: 'error',
            })
            if (throwOnError) throw e
        }
    }

    async function handleDropHelp() {
        await router.push('/help/drop')
        await confirmDropModal.value?.hide()
    }

    // ── Force analysis & nested unpack prompts ───────────────────────────

    function showForceAnalysisPrompt(classification: ClassificationResult) {
        const filePath = dropFilePath.value
        if (!filePath) return

        dropDebug('showForceAnalysisPrompt: showing force-analysis prompt', {
            reason: classification.reason,
            filePath,
        })

        addPopupNotification({
            title: formatMessage(messages.dropUnknownForceAnalysisTitle),
            text: formatMessage(messages.dropUnknownForceAnalysisText),
            type: 'info',
            autoCloseMs: null,
            buttons: [
                {
                    label: formatMessage(messages.dropUnknownForceAnalysisButton),
                    action: async () => {
                        const analyzingNotification = addNotification({
                            title: formatMessage(messages.dropUnknownForceAnalyzing),
                            type: 'info',
                            autoCloseMs: null,
                        })

                        try {
                            const result = await classifyDroppedItemWithExtraction(filePath)
                            notificationManager.removeNotification(analyzingNotification.id)

                            if (result.item_type === 'unknown') {
                                addNotification({
                                    title: formatMessage(
                                        messages.dropUnknownForceAnalysisFailedTitle,
                                    ),
                                    text: formatMessage(
                                        messages.dropUnknownForceAnalysisFailedText,
                                    ),
                                    type: 'error',
                                })
                                return
                            }

                            await continueWithClassification(result, filePath)
                        } catch (e) {
                            notificationManager.removeNotification(analyzingNotification.id)
                            addNotification({
                                title: formatMessage(messages.dropUnknownForceAnalysisFailedTitle),
                                text: e instanceof Error ? e.message : String(e),
                                type: 'error',
                            })
                        }
                    },
                    color: 'brand',
                },
            ],
        })
    }

    async function continueWithClassification(
        result: ClassificationResult,
        fallbackFileName: string,
    ) {
        if (result.item_type === 'unknown') {
            addNotification({
                title: formatMessage(messages.dropUnknownTitle),
                text: unknownReasonMessage(result.reason),
                type: 'error',
            })
            return
        }
        dropClassification.value = result
        dropFilePath.value = result.file_path ?? result.base_path ?? ''
        dropFileName.value =
            result.file_path?.split(/[/\\]/).pop() ??
            result.base_path?.split(/[/\\]/).pop() ??
            fallbackFileName

        switch (result.item_type) {
            case 'modpack':
                await handleDropConfirm('modpack')
                break
            case 'world_save':
                await handleDropConfirm('world_save')
                break
            case 'launcher':
            case 'hmcl_launcher':
                await handleDropConfirm('instance')
                break
            default:
                if (result.item_type === 'resource_pack' || result.item_type === 'multiple') {
                    confirmDropModal.value?.show()
                    return
                }
                await handleDropConfirm(result.item_type)
                break
        }
    }

    function showNestedUnpackPrompt(classification: ClassificationResult) {
        const filePath = dropFilePath.value
        if (!filePath) return

        dropDebug('showNestedUnpackPrompt: nested archives need unpacking', {
            reason: classification.reason,
            filePath,
        })

        const sizeBytes = Number(classification.reason?.match(/total (\d+) bytes/i)?.[1] ?? 0)
        const sizeLabel =
            sizeBytes > 0
                ? sizeBytes >= 1024 * 1024
                    ? `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`
                    : `${Math.max(1, Math.round(sizeBytes / 1024))} KB`
                : '?'

        addPopupNotification({
            title: formatMessage(messages.dropNestedUnpackTitle),
            text: formatMessage(messages.dropNestedUnpackText, { size: sizeLabel }),
            type: 'info',
            autoCloseMs: null,
            buttons: [
                {
                    label: formatMessage(messages.dropNestedUnpackButton),
                    action: async () => {
                        try {
                            const result = await classifyDroppedItem(filePath, true)
                            await continueWithClassification(result, dropFileName.value || 'file')
                        } catch (e) {
                            addNotification({
                                title: formatMessage(messages.dropProcessFailedTitle),
                                text: e instanceof Error ? e.message : String(e),
                                type: 'error',
                            })
                        }
                    },
                },
            ],
        })
    }

    // ── Generic install handlers ─────────────────────────────────────────

    async function handleGenericInstall(targetInstanceId: string) {
        if (batchTargetPickMode) {
            batchTargetPickMode = false
            genericInstallModal.value?.hide()
            try {
                await proceedBatchWithTargetInstance(targetInstanceId)
            } catch (error) {
                await failBatch(error)
            }
            return
        }
        genericInstallModal.value?.hide()
        const pending = pendingInstall.value
        pendingInstall.value = null
        if (!pending) return

        await installContentDirectly(
            pending.type,
            pending.filePath,
            targetInstanceId,
            pending.innerBase,
        )
    }

    function handleGenericInstallCancel() {
        if (batchTargetPickMode) {
            batchTargetPickMode = false
            genericInstallModal.value?.hide()
            void cancelBatch('target-instance-cancel')
            return
        }
        dropClassification.value = null
    }

    async function handleGenericInstallNavigateCreate() {
        if (batchTargetPickMode) {
            batchTargetPickMode = false
            genericInstallModal.value?.hide()
            void cancelBatch('target-instance-navigate-create')
        }
        router.push('/create')
    }

    async function handleDatapackWorldSelect(target: { instanceId: string; worldPath: string }) {
        const pending = pendingInstall.value
        pendingInstall.value = null
        if (!pending) return
        clearDropProcessingNotification()
        try {
            await install_datapack_to_world(
                target.instanceId,
                target.worldPath,
                pending.filePath,
                pending.innerBase,
            )
            addNotification({
                title: formatMessage(messages.dropContentInstalledTitle),
                text: formatMessage(messages.dropContentInstalledText),
                type: 'success',
            })
        } catch (e) {
            addNotification({
                title: formatMessage(messages.dropInstallFailedTitle),
                text:
                    e instanceof Error ? e.message : typeof e === 'string' ? e : JSON.stringify(e),
                type: 'error',
            })
        }
    }

    // ── Launcher import handlers ─────────────────────────────────────────

    function onLauncherImportCancelled() {
        launcherImportModal.value?.hide()
        cleanupLauncherZipTemp()
    }

    function chooseImportMethod(options: {
        instanceNames: string[]
        symlinkCapable: 'supported' | 'requires_admin' | 'unsupported'
    }): Promise<SymlinkMethodChoice[]> {
        return new Promise((resolve) => {
            symlinkChoiceResolve = resolve
            symlinkCardsModal.value?.show({
                instances: options.instanceNames.map((name) => ({ name })),
                symlinkCapable: options.symlinkCapable,
            })
        })
    }

    async function onImportSelected(
        selections: Array<{
            launcherType: string
            launcherName: string
            instances: Array<{
                name: string
                path: string
                compatibleMode?: boolean
                versionPath?: string
            }>
        }>,
    ) {
        const allSelected: SelectedInstance[] = []
        for (const sel of selections) {
            for (const inst of sel.instances) {
                allSelected.push({
                    launcherType: sel.launcherType,
                    basePath: inst.compatibleMode ? inst.path : '',
                    name: inst.name,
                    path: inst.compatibleMode ? (inst.versionPath ?? inst.path) : inst.path,
                    compatibleMode: inst.compatibleMode,
                    versionPath: inst.versionPath,
                })
            }
        }
        if (allSelected.length === 0) return
        selectedInstances.value = allSelected

        if (launcherZipTempDir.value) {
            dropDebug('onImportSelected: zip source, importing as copy', {
                count: allSelected.length,
            })
            await onSymlinkMethodConfirmed(false)
            return
        }

        const cap = await check_symlink_capability()
        symlinkCardsModal.value?.show({
            instances: allSelected.map((i) => ({
                name: i.name,
                path: i.path,
                launcherType: i.launcherType,
                basePath: i.basePath || i.path,
                compatibleMode: i.compatibleMode,
                versionPath: i.versionPath,
            })),
            symlinkCapable: cap,
        })
    }

    function onSymlinkMethodCancelled() {
        if (symlinkChoiceResolve) {
            symlinkChoiceResolve([])
            symlinkChoiceResolve = null
        }
        symlinkCardsModal.value?.hide()
        if (batchSymlinkMode) {
            batchSymlinkMode = false
            void cancelBatch('symlink-cancel')
            return
        }
        cleanupLauncherZipTemp()
    }

    function resolvedInstancePath(
        inst: SelectedInstance,
        _ctx: ImportContext | null,
    ): string | undefined {
        if (inst.compatibleMode) return inst.versionPath
        return inst.path
    }

    async function onSymlinkMethodConfirmed(choices: SymlinkMethodChoice[] | boolean) {
        if (symlinkChoiceResolve) {
            symlinkChoiceResolve(Array.isArray(choices) ? choices : [])
            symlinkChoiceResolve = null
            return
        }

        if (batchSymlinkMode) {
            const group = batchCurrentGroup.value
            batchSymlinkMode = false
            symlinkCardsModal.value?.hide()
            if (group) {
                const choiceArray = Array.isArray(choices) ? choices : []
                for (const item of group.items) {
                    const choice = choiceArray.find(
                        (c) =>
                            c.instanceName === item.name &&
                            (c.instancePath ?? undefined) === (item.instancePath ?? undefined),
                    )
                    if (choice) {
                        item.symlink = choice.symlink
                        item.gameVersion = choice.gameVersion
                        item.loader = choice.loader
                        item.loaderVersion = choice.loaderVersion
                        item.gameDirOverride = choice.gameDirOverride
                    }
                }
            }
            batchConfirmIndex++
            void showNextBatchGroup()
            return
        }

        const instances = selectedInstances.value
        selectedInstances.value = []
        const ctx = currentImportContext.value
        currentImportContext.value = null
        if (instances.length === 0) {
            cleanupLauncherZipTemp()
            return
        }

        if (instances.length === 1) {
            const inst = instances[0]
            const choice = Array.isArray(choices)
                ? choices.find(
                      (c) =>
                          c.instanceName === inst.name &&
                          (c.instancePath ?? undefined) === (inst.path ?? undefined),
                  )
                : undefined
            try {
                const job = await import_instance(
                    ctx?.launcherType ?? inst.launcherType,
                    inst.compatibleMode ? inst.basePath : (ctx?.basePath ?? inst.path),
                    inst.name,
                    choice?.symlink ?? (Array.isArray(choices) ? false : choices),
                    resolvedInstancePath(inst),
                    inst.compatibleMode ? undefined : (choice?.gameVersion ?? undefined),
                    inst.compatibleMode ? undefined : (choice?.loader ?? undefined),
                    inst.compatibleMode ? undefined : (choice?.loaderVersion ?? undefined),
                    choice?.gameDirOverride ?? null,
                )
                await wait_for_install_job(job.job_id)
                addNotification({
                    title: formatMessage(messages.dropInstanceImportedTitle),
                    text: formatMessage(messages.dropInstanceImportedText, { name: inst.name }),
                    type: 'success',
                })
            } catch (e) {
                addNotification({
                    title: formatMessage(messages.dropImportFailedTitle),
                    text: formatMessage(messages.dropImportFailedText, {
                        name: inst.name,
                        error: String(e),
                    }),
                    type: 'error',
                })
            } finally {
                cleanupLauncherZipTemp()
            }
            return
        }

        const total = instances.length
        let completed = 0
        let failedCount = 0

        let progressNotif = addNotification({
            title: formatMessage(messages.dropImportProgressTitle),
            text: formatMessage(messages.dropImportProgressText, { current: 0, total }),
            type: 'info',
            autoCloseMs: null,
        })

        for (let i = 0; i < instances.length; i++) {
            const inst = instances[i]
            const choice = Array.isArray(choices)
                ? choices.find(
                      (c) =>
                          c.instanceName === inst.name &&
                          (c.instancePath ?? undefined) === (inst.path ?? undefined),
                  )
                : undefined

            notificationManager.removeNotification(progressNotif.id)
            progressNotif = addNotification({
                title: formatMessage(messages.dropImportProgressTitle),
                text: formatMessage(messages.dropImportProgressText, {
                    current: i + 1,
                    total,
                }),
                type: 'info',
                autoCloseMs: null,
            })

            try {
                const job = await import_instance(
                    ctx?.launcherType ?? inst.launcherType,
                    inst.compatibleMode ? inst.basePath : (ctx?.basePath ?? inst.path),
                    inst.name,
                    choice?.symlink ?? (Array.isArray(choices) ? false : choices),
                    resolvedInstancePath(inst),
                    inst.compatibleMode ? undefined : (choice?.gameVersion ?? undefined),
                    inst.compatibleMode ? undefined : (choice?.loader ?? undefined),
                    inst.compatibleMode ? undefined : (choice?.loaderVersion ?? undefined),
                    choice?.gameDirOverride ?? null,
                )
                await wait_for_install_job(job.job_id)
                completed++
            } catch (e) {
                failedCount++
                addNotification({
                    title: formatMessage(messages.dropImportFailedTitle),
                    text: formatMessage(messages.dropImportFailedText, {
                        name: inst.name,
                        error: String(e),
                    }),
                    type: 'error',
                })
            }
        }

        cleanupLauncherZipTemp()

        notificationManager.removeNotification(progressNotif.id)
        if (failedCount === 0) {
            addNotification({
                title: formatMessage(messages.dropImportCompletedTitle),
                text: formatMessage(messages.dropImportCompletedText, { count: total }),
                type: 'success',
            })
        } else {
            addNotification({
                title: formatMessage(messages.dropImportCompletedTitle),
                text: formatMessage(messages.dropImportCompletedPartialText, {
                    completed,
                    failed: failedCount,
                    total,
                }),
                type: 'warning',
            })
        }
    }

    // ── Batch drop flow ──────────────────────────────────────────────────

    async function startBatchImport(paths: string[]) {
        console.log('[BatchDrop] startBatchImport paths=', paths.length, paths)
        if (batchPhase.value !== 'idle') return
        batchPhase.value = 'scanning'
        batchOriginalCount.value = paths.length
        batchScanDone.value = 0
        batchScanCancelled = false
        batchInstallCancelled = false
        batchConfirmIndex = 0
        batchTempDirs.value = []
        batchTargetInstanceId.value = ''
        batchWorldPath.value = ''
        batchGroups.value = []
        batchCurrentGroup.value = null
        batchItems.value = paths.map((path, index) => ({
            id: `batch-${Date.now()}-${index}`,
            sourcePath: path,
            name: path.split(/[/\\]/).pop() || path,
            scanState: 'pending',
            selected: true,
        }))
        const scanPromise = runBatchScan()
        batchScanPromise = scanPromise
        try {
            await scanPromise
        } catch (error) {
            await failBatch(error)
        } finally {
            if (batchScanPromise === scanPromise) batchScanPromise = null
        }
    }

    async function runBatchScan() {
        const items = batchItems.value
        const total = items.length
        let cursor = 0
        console.log(`[BatchDrop] runBatchScan total=${total}`)
        const workers = Array.from({ length: Math.min(3, total) }, async () => {
            while (!batchScanCancelled) {
                const index = cursor++
                if (index >= total) return
                const item = items[index]
                item.scanState = 'scanning'
                console.log(`[BatchDrop] scan start idx=${index} path=${item.sourcePath}`)
                try {
                    const raw = await classifyDropPath(item.sourcePath)
                    const resolved = resolveBatchClassification(raw)
                    if (resolved.item_type === 'shortcut_resolved') {
                        item.scanState = 'skipped'
                        item.reason = 'shortcut-exceeded'
                    } else {
                        await applyBatchClassification(item, resolved)
                    }
                    console.log(
                        `[BatchDrop] scan done idx=${index} type=${item.itemType} state=${item.scanState}`,
                    )
                } catch (error) {
                    item.scanState = 'error'
                    item.reason = error instanceof Error ? error.message : String(error)
                    console.log(`[BatchDrop] scan ERROR idx=${index}`, error)
                } finally {
                    batchScanDone.value++
                }
            }
        })
        await Promise.all(workers)
        if (batchScanCancelled) {
            await finishBatchCancellation()
            return
        }
        await finishBatchScan()
    }

    async function applyBatchClassification(item: BatchDropItem, resolved: ClassificationResult) {
        console.log(
            '[BatchDrop] applyBatchClassification item=',
            item.name,
            'resolved=',
            resolved.item_type,
        )
        if (resolved.item_type === 'unknown') {
            item.scanState = 'skipped'
            item.reason = resolved.reason ?? 'unknown'
            return
        }

        if (resolved.item_type === 'launcher' || resolved.item_type === 'hmcl_launcher') {
            const launcherType =
                resolved.item_type === 'hmcl_launcher'
                    ? 'HMCL'
                    : ((resolved as { launcher_type?: string }).launcher_type ?? 'Generic')
            if (['Generic', 'PCL2', 'PCL2CE', 'HMCL'].includes(launcherType)) {
                const basePath =
                    resolved.item_type === 'hmcl_launcher'
                        ? ((resolved as { launcher_dir?: string }).launcher_dir ?? '')
                        : ((resolved as { base_path?: string }).base_path ?? '')
                const resolvedDirs = await resolveGamedirs(launcherType, basePath)
                item.scanState = 'done'
                item.itemType = 'gamedir'
                item.basePath = resolvedDirs.map((entry) => entry.path).join('|')
                if (resolvedDirs.length === 0) item.reason = 'No game directories found'
                return
            }
            await expandBatchLauncher(item, resolved)
            return
        }

        item.scanState = 'done'
        item.classification = resolved
        item.innerBase = (resolved as { innerBase?: string }).innerBase

        switch (resolved.item_type) {
            case 'mod':
            case 'shader_pack':
            case 'world_save':
            case 'litematic':
                item.itemType = resolved.item_type
                break
            case 'modpack':
                item.itemType = 'modpack'
                break
            case 'resource_pack': {
                const candidates = (resolved as { candidates?: string[] }).candidates ?? []
                if (candidates.length === 1) {
                    item.itemType = candidates[0]
                } else if (candidates.length > 1) {
                    item.itemType = 'ambiguous'
                    item.candidates = candidates
                } else {
                    item.itemType = 'resource_pack'
                }
                break
            }
            case 'multiple':
                item.itemType = 'ambiguous'
                item.choices = (resolved as { choices?: Array<{ itemType: string }> }).choices
                item.candidates = [
                    ...new Set(
                        ((resolved as { choices?: Array<{ itemType: string }> }).choices ?? [])
                            .map((choice) => choice.itemType)
                            .filter(Boolean),
                    ),
                ]
                break
        }
    }

    async function expandBatchLauncher(item: BatchDropItem, resolved: ClassificationResult) {
        const launcherType =
            resolved.item_type === 'hmcl_launcher'
                ? 'HMCL'
                : ((resolved as { launcher_type?: string }).launcher_type ?? 'Generic')
        const rawBasePath =
            resolved.item_type === 'hmcl_launcher'
                ? ((resolved as { launcher_dir?: string }).launcher_dir ?? '')
                : ((resolved as { base_path?: string }).base_path ?? '')

        if (!rawBasePath) {
            item.scanState = 'skipped'
            item.reason = 'No launcher path'
            return
        }

        let scanBasePath = rawBasePath
        const fromZip = isZipPath(rawBasePath)
        if (fromZip) {
            try {
                const tempDir = await extractZipToTemp(rawBasePath)
                batchTempDirs.value.push(tempDir)
                const innerBase = (resolved as { innerBase?: string }).innerBase
                scanBasePath = innerBase ? await join(tempDir, innerBase) : tempDir
            } catch (error) {
                item.scanState = 'error'
                item.reason = error instanceof Error ? error.message : String(error)
                return
            }
        }

        try {
            const results = await scanLauncherInstances(launcherType, scanBasePath)
            const instances = results.flatMap((result) => result.instances)
            console.log(
                `[BatchDrop] expand launcher type=${launcherType} base=${scanBasePath} instances=${instances.length}`,
            )
            if (instances.length === 0) {
                item.scanState = 'skipped'
                item.reason = 'No importable instances found'
                return
            }
            item.scanState = 'done'
            item.itemType = 'launcher_container'
            for (const inst of instances) {
                const compatible = inst.compatibleMode
                batchItems.value.push({
                    id: `batch-${Date.now()}-${batchItems.value.length}`,
                    sourcePath: item.sourcePath,
                    name: inst.name,
                    sourceLabel: item.name,
                    scanState: 'done',
                    itemType: 'instance',
                    launcherType,
                    basePath: compatible ? inst.path : scanBasePath,
                    instanceFolder: inst.name,
                    instancePath: compatible ? inst.versionPath : inst.path,
                    fromZip: fromZip || undefined,
                    selected: true,
                })
            }
        } catch (error) {
            item.scanState = 'error'
            item.reason = error instanceof Error ? error.message : String(error)
        }
    }

    async function finishBatchScan() {
        const hasImportable = batchItems.value.some(
            (item) =>
                item.itemType &&
                item.itemType !== 'launcher_container' &&
                item.scanState === 'done',
        )
        console.log('[BatchDrop] finishBatchScan hasImportable=', hasImportable)
        if (!hasImportable) {
            await showBatchSummaryFromScan()
            return
        }

        const needsInstance = batchItems.value.some(
            (item) =>
                item.itemType &&
                [
                    'mod',
                    'resource_pack',
                    'shader_pack',
                    'world_save',
                    'litematic',
                    'data_pack',
                    'ambiguous',
                ].includes(item.itemType),
        )
        if (needsInstance) {
            if (isInInstance.value && instanceId.value) {
                console.log('[BatchDrop] in-instance context, using current instance directly')
                await proceedBatchWithTargetInstance(instanceId.value)
                return
            }
            console.log(
                '[BatchDrop] finishBatchScan needsInstance=true (show target instance picker)',
            )
            batchPhase.value = 'picking-instance'
            await loadBatchTargetInstances()
            return
        }

        console.log('[BatchDrop] finishBatchScan needsInstance=false (go to group confirms)')
        await startBatchGroupConfirms()
    }

    async function proceedBatchWithTargetInstance(targetInstanceId: string) {
        batchTargetInstanceId.value = targetInstanceId
        const needsWorld = batchItems.value.some(
            (item) =>
                item.itemType === 'data_pack' ||
                (item.itemType === 'ambiguous' && (item.candidates ?? []).includes('data_pack')),
        )
        if (needsWorld) {
            batchPhase.value = 'picking-world'
            batchWorldMode = true
            dataPackWorldModal.value?.show(targetInstanceId)
            console.log('[BatchDrop] dataPackWorldModal.show (batch world) called')
        } else {
            await startBatchGroupConfirms()
        }
    }

    async function loadBatchTargetInstances() {
        try {
            const all = await listInstances()
            batchTargetInstances.value = all.map((inst) => ({
                id: inst.id,
                name: inst.name,
                iconUrl: getDisplayInstanceIcon(inst.icon_path, inst.loader).url,
                gameVersion: inst.game_version || null,
                loader: inst.loader || null,
            }))
            console.log(
                '[BatchDrop] loadBatchTargetInstances count=',
                batchTargetInstances.value.length,
            )
        } catch (error) {
            batchTargetInstances.value = []
            console.log('[BatchDrop] loadBatchTargetInstances ERROR', error)
        }
        await nextTick()
        genericInstallModal.value?.show({
            contentType: 'mod',
            fileName: formatMessage(messages.dropBatchTargetLabel),
            instances: batchTargetInstances.value,
        })
        batchTargetPickMode = true
        console.log('[BatchDrop] genericInstallModal.show (batch target) called')
    }

    async function handleBatchOrDatapackWorldSelect(target: {
        instanceId: string
        worldPath: string
    }) {
        if (batchWorldMode) {
            batchWorldMode = false
            batchTargetInstanceId.value = target.instanceId
            batchWorldPath.value = target.worldPath
            console.log('[BatchDrop] batch world selected world=', target.worldPath)
            try {
                await startBatchGroupConfirms()
            } catch (error) {
                await failBatch(error)
            }
            return
        }
        await handleDatapackWorldSelect(target)
    }

    function handleBatchWorldAfterHide() {
        if (batchWorldMode) {
            batchWorldMode = false
            void cancelBatch('world-cancel')
        }
    }

    async function startBatchGroupConfirms() {
        const groups: BatchDropGroup[] = []
        const typeOrder = [
            'modpack',
            'ambiguous',
            'mod',
            'resource_pack',
            'shader_pack',
            'world_save',
            'litematic',
            'data_pack',
            'instance',
            'gamedir',
        ]
        const byType = new Map<string, BatchDropItem[]>()
        for (const item of batchItems.value) {
            if (item.selected === false || !item.itemType || item.itemType === 'launcher_container')
                continue
            if (item.scanState !== 'done') continue
            const groupKey =
                item.itemType === 'ambiguous'
                    ? `${item.itemType}:${[
                          ...new Set(
                              (
                                  item.choices ??
                                  (item.candidates ?? []).map((itemType) => ({ itemType }))
                              ).map(({ itemType }) => itemType),
                          ),
                      ]
                          .sort()
                          .join(',')}`
                    : item.itemType
            const list = byType.get(groupKey) ?? []
            list.push(item)
            byType.set(groupKey, list)
        }
        for (const type of typeOrder) {
            for (const [groupKey, list] of byType) {
                if (list[0]?.itemType === type && list.length) {
                    groups.push({ id: groupKey, type, items: list })
                }
            }
        }
        batchGroups.value = groups
        batchConfirmIndex = 0
        batchPhase.value = 'confirming'
        console.log(
            '[BatchDrop] startBatchGroupConfirms groups=',
            groups.map((g) => `${g.type}:${g.items.length}`),
        )
        await showNextBatchGroup()
    }

    async function showNextBatchGroup() {
        try {
            if (batchConfirmIndex >= batchGroups.value.length) {
                console.log('[BatchDrop] showNextBatchGroup done all groups, run install')
                await runBatchInstall()
                return
            }
            const group = batchGroups.value[batchConfirmIndex]
            batchCurrentGroup.value = group
            console.log(
                `[BatchDrop] showNextBatchGroup idx=${batchConfirmIndex} type=${group.type} count=${group.items.length}`,
            )
            if (group.type === 'instance') {
                if (group.items.some((item) => item.fromZip)) {
                    for (const item of group.items) item.symlink = false
                    batchConfirmIndex++
                    await showNextBatchGroup()
                    return
                }
                const cap = await check_symlink_capability()
                batchSymlinkMode = true
                symlinkCardsModal.value?.show({
                    instances: group.items.map((item) => ({
                        name: item.name,
                        path: item.instancePath,
                        launcherType: item.launcherType,
                        basePath: item.basePath,
                    })),
                    symlinkCapable: cap,
                })
            } else if (group.type === 'gamedir') {
                await routeToGamedirFlow(
                    group.items.flatMap((item) => (item.basePath ?? '').split('|')),
                )
                batchConfirmIndex++
                await showNextBatchGroup()
            } else {
                await showBatchGroupConfirmModal(group)
            }
        } catch (error) {
            await failBatch(error)
        }
    }

    async function showBatchGroupConfirmModal(group: BatchDropGroup) {
        batchGroupKey.value++
        await nextTick()

        let classification: ClassificationResult
        if (group.type === 'ambiguous') {
            const choices = group.items
                .flatMap((item) =>
                    item.choices?.length
                        ? item.choices
                        : (item.candidates ?? []).map((candidate) => ({ itemType: candidate })),
                )
                .filter(
                    (choice, index, all) =>
                        all.findIndex((c) => c.itemType === choice.itemType) === index,
                )
            classification = {
                item_type: 'multiple',
                file_path: group.items[0]?.sourcePath,
                choices,
            } as unknown as ClassificationResult
        } else {
            classification = { item_type: group.type } as unknown as ClassificationResult
        }
        dropClassification.value = classification
        dropFileName.value = formatMessage(messages.dropBatchGroupFileLabel, {
            count: group.items.length,
            names: group.items.map((item) => item.name).join(', '),
        })
        batchGroupMode = true
        confirmDropModal.value?.show()
    }

    function onBatchGroupConfirm(type: string) {
        const group = batchCurrentGroup.value
        if (!group) return
        console.log('[BatchDrop] onBatchGroupConfirm group=', group.type, 'type=', type)
        for (const item of group.items) {
            item.selected = true
            item.confirmedType = type
            item.itemType = type
            const choice = item.choices?.find((c) => c.itemType === type)
            if (choice?.innerBase !== undefined) {
                item.innerBase = choice.innerBase
            }
        }
        batchConfirmIndex++
        void showNextBatchGroup()
    }

    function cancelBatchScan() {
        console.log('[BatchDrop] cancelBatchScan clicked')
        batchScanCancelled = true
        void cancelBatch('scan-button')
    }

    async function cancelBatchInstallJob() {
        const jobId = batchCurrentInstallJobId
        if (!jobId) return
        try {
            await install_job_cancel(jobId)
        } catch (error) {
            console.warn('[BatchDrop] failed to cancel active install job', jobId, error)
        }
        try {
            await wait_for_install_job(jobId)
        } catch {
            // A canceled job is expected to reject from wait_for_install_job.
        }
    }

    async function cancelBatch(source = 'unknown') {
        console.log(
            `[BatchDrop] cancelBatch INVOKED source=${source} phase=${batchPhase.value}`,
            new Error(source).stack,
        )
        if (batchPhase.value === 'idle' || batchPhase.value === 'cancelled') return
        batchScanCancelled = true
        batchInstallCancelled = true
        batchSymlinkMode = false
        batchTargetPickMode = false
        batchWorldMode = false
        batchGroupMode = false
        const compatResolve = batchCompatResolve
        batchCompatResolve = null
        pendingDropIncompatibility.value = null
        compatResolve?.('skipped')
        confirmDropModal.value?.hide()
        symlinkCardsModal.value?.hide()
        if (batchPhase.value === 'scanning' && batchScanPromise) {
            batchPhase.value = 'cancelled'
            return
        }
        if (batchPhase.value === 'installing') {
            batchPhase.value = 'cancelled'
            await cancelBatchInstallJob()
            return
        }
        await finishBatchCancellation()
    }

    async function finishBatchCancellation() {
        await cleanupBatchTempDirs()
        addNotification({
            title: formatMessage(messages.dropImportCancelledTitle),
            text: formatMessage(messages.dropImportCancelledText),
            type: 'info',
        })
        finishBatch()
        batchPhase.value = 'idle'
    }

    async function failBatch(error: unknown) {
        console.error('[BatchDrop] batch failed', error)
        addNotification({
            title: formatMessage(messages.dropImportFailedTitle),
            text: error instanceof Error ? error.message : String(error),
            type: 'error',
        })
        batchPhase.value = 'idle'
        batchScanCancelled = true
        batchInstallCancelled = true
        batchTargetPickMode = false
        batchWorldMode = false
        batchGroupMode = false
        confirmDropModal.value?.hide()
        symlinkCardsModal.value?.hide()
        await cleanupBatchTempDirs()
        finishBatch()
    }

    async function cleanupBatchTempDirs() {
        const dirs = [...batchTempDirs.value]
        batchTempDirs.value = []
        for (const dir of dirs) {
            try {
                await removeTempDir(dir)
            } catch {
                // Best-effort cleanup; stale dirs are swept by the backend.
            }
        }
    }

    async function showBatchSummaryFromScan() {
        const skipped = batchItems.value.filter(
            (item) => item.scanState === 'skipped' || item.scanState === 'error',
        )
        if (skipped.length > 0) {
            addNotification({
                title: formatMessage(messages.dropBatchNothingImportableTitle),
                text: formatMessage(messages.dropBatchNothingImportableText, {
                    count: skipped.length,
                }),
                type: 'warning',
            })
        }
        await finishBatchImport()
    }

    async function runBatchInstall() {
        batchPhase.value = 'installing'
        batchInstallCancelled = false
        console.log('[BatchDrop] runBatchInstall start')
        const typeOrder = [
            'modpack',
            'instance',
            'mod',
            'resource_pack',
            'shader_pack',
            'world_save',
            'litematic',
            'data_pack',
        ]
        const queue = typeOrder.flatMap((type) =>
            batchItems.value.filter(
                (item) =>
                    item.selected !== false && item.scanState === 'done' && item.itemType === type,
            ),
        )
        for (const item of batchItems.value) {
            if (
                item.selected !== false &&
                item.scanState === 'done' &&
                item.itemType &&
                !typeOrder.includes(item.itemType)
            ) {
                queue.push(item)
            }
        }

        let succeeded = 0
        let failed = 0
        let skipped = 0
        for (const item of queue) {
            if (batchInstallCancelled) {
                addNotification({
                    title: formatMessage(messages.dropImportCancelledTitle),
                    text: formatMessage(messages.dropImportCancelledText),
                    type: 'info',
                })
                break
            }
            item.installState = 'processing'
            console.log(`[BatchDrop] install start type=${item.itemType} name=${item.name}`)
            try {
                await installBatchItem(item)
                if (batchInstallCancelled) break
                if (pendingDropIncompatibility.value) {
                    const result = await new Promise<'installed' | 'skipped' | 'failed'>(
                        (resolve) => {
                            batchCompatResolve = resolve
                        },
                    )
                    if (result === 'installed') {
                        item.installState = 'success'
                        succeeded++
                        console.log(`[BatchDrop] install SUCCESS (compat) name=${item.name}`)
                    } else if (result === 'skipped') {
                        item.installState = 'skipped'
                        skipped++
                        console.log(
                            `[BatchDrop] install SKIPPED (compat cancelled) name=${item.name}`,
                        )
                    } else {
                        item.installState = 'failed'
                        failed++
                    }
                } else {
                    item.installState = 'success'
                    succeeded++
                    console.log(`[BatchDrop] install SUCCESS name=${item.name}`)
                }
            } catch (error) {
                if (batchInstallCancelled) break
                item.installState = 'failed'
                failed++
                console.log(`[BatchDrop] install FAILED name=${item.name}`, error)
            }
        }
        console.log(
            `[BatchDrop] runBatchInstall finished queue=${queue.length} success=${succeeded} failed=${failed} skipped=${skipped}`,
        )
        if (batchInstallCancelled) {
            await finishBatchCancellation()
            return
        }

        addNotification({
            title: formatMessage(messages.dropBatchCompletedTitle),
            text: formatMessage(messages.dropBatchCompletedText, {
                succeeded,
                failed,
                skipped,
                total: queue.length,
            }),
            type: failed > 0 || skipped > 0 ? 'warning' : 'success',
        })
        await finishBatchImport()
    }

    async function installBatchItem(item: BatchDropItem) {
        console.log(
            `[BatchDrop] installBatchItem type=${item.itemType} name=${item.name} path=${item.sourcePath}`,
        )
        switch (item.itemType) {
            case 'modpack': {
                const job = await install_create_modpack_instance({
                    type: 'fromFile',
                    path: item.sourcePath,
                })
                batchCurrentInstallJobId = job.job_id
                try {
                    if (batchInstallCancelled) await cancelBatchInstallJob()
                    else await wait_for_install_job(job.job_id)
                } finally {
                    if (batchCurrentInstallJobId === job.job_id) batchCurrentInstallJobId = null
                }
                return
            }
            case 'instance': {
                const job = await import_instance(
                    item.launcherType,
                    item.basePath,
                    item.instanceFolder,
                    item.symlink ?? false,
                    item.instancePath,
                    item.gameVersion ?? undefined,
                    item.loader ?? undefined,
                    item.loaderVersion ?? undefined,
                    item.gameDirOverride ?? null,
                )
                batchCurrentInstallJobId = job.job_id
                try {
                    if (batchInstallCancelled) await cancelBatchInstallJob()
                    else await wait_for_install_job(job.job_id)
                } finally {
                    if (batchCurrentInstallJobId === job.job_id) batchCurrentInstallJobId = null
                }
                return
            }
            case 'data_pack':
                if (!batchTargetInstanceId.value || !batchWorldPath.value) {
                    throw new Error('Missing target instance or world for datapack')
                }
                await install_datapack_to_world(
                    batchTargetInstanceId.value,
                    batchWorldPath.value,
                    item.sourcePath,
                    item.innerBase,
                )
                return
            case 'mod':
            case 'resource_pack':
            case 'shader_pack':
            case 'world_save':
            case 'litematic':
            case 'schematic':
                await installContentDirectly(
                    item.itemType,
                    item.sourcePath,
                    batchTargetInstanceId.value,
                    item.innerBase,
                    true,
                )
                return
            default:
                throw new Error(`Unsupported batch item type: ${item.itemType}`)
        }
    }

    async function finishBatchImport() {
        await cleanupBatchTempDirs()
        batchPhase.value = 'idle'
        batchItems.value = []
        finishBatch()
    }

    // ── Incompatibility warning handlers ─────────────────────────────────

    async function handleIncompatibilityWarningUpdate(
        version: { id: string; project_id: string },
        _event: MouseEvent,
    ) {
        const decision = batchCompatResolve
        batchCompatResolve = null
        const pending = pendingDropIncompatibility.value
        if (pending) {
            pendingDropIncompatibility.value = null
            const projectType = contentFileProjectTypeMap[pending.type]
            try {
                await add_project_from_path(
                    pending.instId,
                    pending.filePath,
                    projectType,
                    pending.innerBase,
                )
                addNotification({
                    title: formatMessage(messages.dropContentInstalledTitle),
                    text: formatMessage(messages.dropContentInstalledText),
                    type: 'success',
                })
                decision?.('installed')
            } catch (e) {
                addNotification({
                    title: formatMessage(messages.dropInstallFailedTitle),
                    text:
                        e instanceof Error
                            ? e.message
                            : typeof e === 'string'
                              ? e
                              : JSON.stringify(e),
                    type: 'error',
                })
                decision?.('failed')
            }
            return
        }
        // Delegate to content install provider for normal flow
        await contentInstall.install(version.project_id, version.id, null, 'IncompatibilityWarning')
    }

    function handleIncompatibilityWarningCancel() {
        const decision = batchCompatResolve
        batchCompatResolve = null
        pendingDropIncompatibility.value = null
        if (decision) {
            decision('skipped')
            return
        }
    }

    function handleDropInstallSearchCompat() {
        const decision = batchCompatResolve
        batchCompatResolve = null
        const pending = pendingDropIncompatibility.value
        if (!pending) return
        const searchName = pending.meta?.name ?? pending.meta?.mod_id ?? 'mod'
        const searchUrl = pending.modrinthLookup
            ? `/project/${pending.modrinthLookup.project_id}`
            : `/browse/mod?q=${encodeURIComponent(searchName)}&i=${pending.instId}`
        pendingDropIncompatibility.value = null
        decision?.('skipped')
        router.push(searchUrl)
    }

    // ── Global drop setup ────────────────────────────────────────────────

    const { isDragging, isProcessing, finishBatch } = useGlobalDrop(
        {
            classifyFile: classifyDropPath,
            onClassifyStart: (fileName) => {
                if (onSkinsPage.value) return
                if (onSchematicWorkshopPage.value && isSchematicFile(fileName)) return
                if (onSettingsPage.value && isSettingsImagePath(fileName)) return
                dropProcessingNotificationId.value = addNotification({
                    title: formatMessage(messages.dropProcessing, { name: fileName }),
                    type: 'info',
                    autoCloseMs: null,
                }).id
            },
            onImportStart: (type, classification) => {
                if (type === 'unknown' && classification?.reason === 'skipped') return
                dropClassification.value = classification
                dropFilePath.value =
                    classification.file_path ?? classification.base_path ?? lastDroppedPath.value
                dropFileName.value =
                    classification.file_path?.split(/[/\\]/).pop() ??
                    classification.base_path?.split(/[/\\]/).pop() ??
                    (lastDroppedPath.value.split(/[/\\]/).pop() || 'file')

                if (
                    type === 'unknown' &&
                    classification?.reason?.toLowerCase().includes('nested')
                ) {
                    clearDropProcessingNotification()
                    showNestedUnpackPrompt(classification)
                    return
                }

                if (
                    type === 'unknown' &&
                    classification?.reason?.toLowerCase().includes('extraction')
                ) {
                    clearDropProcessingNotification()
                    showForceAnalysisPrompt(classification)
                    return
                }

                if (type === 'unknown') {
                    clearDropProcessingNotification()
                    const unknownFile =
                        classification?.file_path?.split(/[/\\]/).pop() ??
                        classification?.base_path?.split(/[/\\]/).pop() ??
                        ''

                    const isTempFile =
                        unknownFile.startsWith('.tmp') || unknownFile.startsWith('tmp')
                    if (isTempFile) {
                        addNotification({
                            title: formatMessage(messages.dropTemporaryFileTitle),
                            text: formatMessage(messages.dropTemporaryFileText, {
                                file: unknownFile,
                            }),
                            type: 'warning',
                        })
                    } else {
                        addNotification({
                            title: formatMessage(messages.dropUnknownTitle),
                            text: unknownReasonMessage(classification?.reason),
                            type: 'error',
                        })
                    }
                    return
                }

                confirmDropModal.value?.show()
            },
            onImportEnd: () => {},
            onBatchStart: (paths) => {
                void startBatchImport(paths)
            },
            onError: (reason) => {
                clearDropProcessingNotification()

                if (reason === 'multiple-files') {
                    addNotification({
                        title: formatMessage(messages.dropMultipleFilesTitle),
                        text: formatMessage(messages.dropMultipleFilesText),
                        type: 'error',
                    })
                } else if (reason === 'shortcut-exceeded') {
                    addNotification({
                        title: formatMessage(messages.dropShortcutFailedTitle),
                        text: formatMessage(messages.dropShortcutFailedText),
                        type: 'error',
                    })
                } else if (reason === 'unknown') {
                    addNotification({
                        title: formatMessage(messages.dropUnknownTitle),
                        text: formatMessage(messages.dropUnknownText),
                        type: 'error',
                    })
                } else {
                    addNotification({
                        title: formatMessage(messages.dropErrorTitle),
                        text: reason,
                        type: 'error',
                    })
                }
            },
        },
        fileDrop,
    )

    // ── Return public API ────────────────────────────────────────────────

    return {
        // State
        isDragging,
        isProcessing,
        batchActive,
        batchPhase,
        batchItems,
        batchOriginalCount,
        batchScanDone,
        batchGroups,
        batchCurrentGroup,
        batchTargetInstances,
        batchTargetInstanceId,
        batchWorldPath,
        dropClassification,
        dropFileName,
        dropFilePath,
        dropProcessingNotificationId,
        scanningInstances,
        pendingInstall,
        pendingDropIncompatibility,
        selectedInstances,
        currentImportContext,
        compatibleModeResults,
        compatibleModeGameDir,
        compatibleModeLauncherType,
        contentInstallIncompatibilityWarningVersions,
        contentInstallIncompatibilityWarningCurrentGameVersion,
        contentInstallIncompatibilityWarningCurrentLoader,
        contentInstallIncompatibilityWarningProjectType,
        contentInstallIncompatibilityWarningProjectIconUrl,
        contentInstallIncompatibilityWarningProjectName,
        contentInstallIncompatibilityWarningMessage,
        contentInstallIncompatibilityWarningInstalling,
        batchGroupKey,
        incompatWarningKey,

        // Modal refs (to be bound in template)
        confirmDropModal,
        genericInstallModal,
        launcherImportModal,
        symlinkCardsModal,
        dataPackWorldModal,
        compatibleModeConfirmModal,
        incompatibilityWarningModal,

        // Handlers
        handleConfirmDropCancel,
        handleConfirmDropConfirm,
        handleConfirmDropHelp,
        handleCompatibleModeConfirm,
        handleDropConfirm,
        handleDropCancel,
        handleDropHelp,
        handleGenericInstall,
        handleGenericInstallCancel,
        handleGenericInstallNavigateCreate,
        handleBatchOrDatapackWorldSelect,
        handleBatchWorldAfterHide,
        onLauncherImportCancelled,
        onImportSelected,
        onSymlinkMethodCancelled,
        onSymlinkMethodConfirmed,
        chooseImportMethod,
        cancelBatchScan,
        handleIncompatibilityWarningUpdate,
        handleIncompatibilityWarningCancel,
        handleDropInstallSearchCompat,

        // Utility
        contentFileProjectTypeMap,
        clearDropProcessingNotification,
        showNestedUnpackPrompt,
        showForceAnalysisPrompt,
        unknownReasonMessage,
    }
}

// Need to import defineMessages from vue-i18n or similar
function defineMessages<T extends Record<string, { id: string; defaultMessage: string }>>(
    messages: T,
): T {
    return messages
}
