import type {
    AbstractPopupNotificationManager,
    AbstractWebNotificationManager,
    CreationFlowContextValue,
    CreationFlowModal,
    SymlinkMethodChoice,
    SymlinkMethodInstance,
} from '@modrinth/ui'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { join } from '@tauri-apps/api/path'
import { inject, provide, ref, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'
import { useRouter } from 'vue-router'

import type UnknownPackWarningModal from '@/components/ui/install_flow/UnknownPackWarningModal.vue'
import type ModpackAlreadyInstalledModal from '@/components/ui/modal/ModpackAlreadyInstalledModal.vue'
import { trackEvent } from '@/helpers/analytics'
import { get_project_versions, get_search_results } from '@/helpers/cache.js'
import { getCurseForgeFiles, hasCompatibleCurseForgeFile } from '@/helpers/curseforge'
import { install_job_listener } from '@/helpers/events.js'
import { import_instance } from '@/helpers/import.js'
import {
    type CreatePackLocation,
    install_create_instance,
    install_create_modpack_instance,
    install_get_modpack_preview,
    type InstallJobSnapshot,
    wait_for_install_job,
} from '@/helpers/install'
import { list } from '@/helpers/instance'
import { get_loader_versions as getLoaderManifest } from '@/helpers/metadata.js'
import type { InstanceLoader } from '@/helpers/types'

const modpackMessages = defineMessages({
    installing: {
        id: 'app.drop.modpack-installing',
        defaultMessage: 'Installing modpack...',
    },
    installed: {
        id: 'app.drop.modpack-installed-success',
        defaultMessage: 'Modpack installed successfully',
    },
    installingFile: {
        id: 'app.drop.installing-file',
        defaultMessage: 'Installing {name}...',
    },
    unknownFileType: {
        id: 'app.drop.unknown-force-analysis-title',
        defaultMessage: 'Unable to identify file type',
    },
    unknownFileTypeText: {
        id: 'app.drop.unknown-force-analysis-text',
        defaultMessage:
            'This archive needs to be extracted and deeply analyzed to determine its content type. This may take a while. Force analysis?',
    },
    forceAnalysis: {
        id: 'app.drop.unknown-force-analysis-button',
        defaultMessage: 'Force analysis',
    },
    analyzing: {
        id: 'app.drop.unknown-force-analyzing',
        defaultMessage: 'Force analyzing archive...',
    },
    couldNotIdentify: {
        id: 'app.drop.unknown-force-analysis-failed-title',
        defaultMessage: 'Analysis failed',
    },
    couldNotIdentifyText: {
        id: 'app.drop.unknown-force-analysis-failed-text',
        defaultMessage: 'Could not identify the file type even after deep analysis.',
    },
    unexpectedType: {
        id: 'app.drop.unexpected-type',
        defaultMessage: 'Unexpected type: {type}',
    },
})

const OPTIFABRIC_CURSEFORGE_PROJECT_ID = 322385

export function setupCreationModal(
    notificationManager: AbstractWebNotificationManager,
    _popupNotificationManager: AbstractPopupNotificationManager,
) {
    const { formatMessage } = useVIntl()
    const { handleError } = notificationManager
    const router = useRouter()

    const installationModal =
        useTemplateRef<ComponentExposed<typeof CreationFlowModal>>('installationModal')
    const unknownPackWarningModal =
        useTemplateRef<InstanceType<typeof UnknownPackWarningModal>>('unknownPackWarningModal')
    const modpackAlreadyInstalledModal = ref<InstanceType<typeof ModpackAlreadyInstalledModal>>()

    function setModpackAlreadyInstalledModal(
        modal: InstanceType<typeof ModpackAlreadyInstalledModal>,
    ) {
        modpackAlreadyInstalledModal.value = modal
    }

    async function fetchExistingInstanceNames(): Promise<string[]> {
        const instances = await list().catch(handleError)
        return instances?.map((i) => i.name) ?? []
    }

    provide('showCreationModal', () => {
        installationModal.value?.show()
    })

    provide(
        'showCreationModalWithOptions',
        (options?: {
            skipSetupType?: boolean
            initialMode?: 'custom' | 'import'
            onBack?: () => void
        }) => {
            installationModal.value?.show(options)
        },
    )

    async function proceedWithModpackCreation(
        projectId: string,
        versionId: string,
        name: string,
        iconUrl?: string,
    ) {
        await install_create_modpack_instance(
            {
                type: 'fromVersionId',
                project_id: projectId,
                version_id: versionId,
                title: name,
                icon_url: iconUrl,
            },
            { name },
        ).catch(handleError)
        trackEvent('InstanceCreate', { source: 'CreationModalModpack' })
    }

    async function handleCreate(config: CreationFlowContextValue) {
        try {
            installationModal.value?.hide()

            if (config.isImportMode.value) {
                // Collect all instances to import
                const instanceEntries: Array<{
                    launcherType: string
                    launcherName: string
                    path: string
                    instanceName: string
                    instancePath: string
                }> = []
                for (const [launcherName, instanceSet] of Object.entries(
                    config.importSelectedInstances.value,
                )) {
                    const launcher = config.importLaunchers.value.find(
                        (l) => l.name === launcherName,
                    )
                    if (!launcher || instanceSet.size === 0) continue
                    for (const name of instanceSet) {
                        const instanceData = launcher.instances.find((i) => i.name === name)
                        instanceEntries.push({
                            launcherType: launcher.launcherType ?? launcher.name,
                            launcherName: launcher.name,
                            path: launcher.path,
                            instanceName: name,
                            instancePath: instanceData?.path ?? '',
                        })
                    }
                }

                if (instanceEntries.length === 0) return

                const chooseImportMethod: (options: {
                    instances: SymlinkMethodInstance[]
                }) => Promise<SymlinkMethodChoice[]> = inject('chooseImportMethod')!

                const choices = await chooseImportMethod({
                    instances: instanceEntries.map((entry) => ({
                        name: entry.instanceName,
                        path: entry.instancePath,
                        launcherType: entry.launcherType,
                        basePath: entry.path,
                    })),
                })

                if (choices.length === 0) return

                const choiceByInstanceName = new Map(
                    choices.map((choice) => [choice.instanceName, choice]),
                )

                for (const entry of instanceEntries) {
                    const choice = choiceByInstanceName.get(entry.instanceName)
                    try {
                        const job = await import_instance(
                            entry.launcherType,
                            entry.path,
                            entry.instanceName,
                            false,
                            entry.instancePath,
                            choice?.gameVersion ?? undefined,
                            choice?.loader ?? undefined,
                            choice?.loaderVersion ?? undefined,
                            null,
                        )
                        await wait_for_install_job(job.job_id)
                    } catch (error) {
                        handleError(error)
                    }
                }
                trackEvent('InstanceCreate', { source: 'CreationModalImport' })
                return
            }

            if (config.modpackSelection.value) {
                const { projectId, versionId, name, iconUrl } = config.modpackSelection.value
                await proceedWithModpackCreation(projectId, versionId, name, iconUrl)
                return
            }

            if (config.modpackFilePath.value) {
                // Fallback: called when modpack is imported via the creation flow
                // (not via onImportFileReceived, which has its own install path).
                const splitPath = config.modpackFilePath.value.split(/[\\/]/)
                const fileName = splitPath
                    ? splitPath[splitPath.length - 1]
                    : config.modpackFilePath.value
                await installModpackFromPath(config.modpackFilePath.value, fileName)
                trackEvent('InstanceCreate', { source: 'CreationModalModpackFile' })
                return
            }

            // Custom/vanilla setup
            const loader = config.hideLoaderChips.value
                ? 'vanilla'
                : (config.selectedLoader.value ?? 'vanilla')
            const loaderVersion = config.hideLoaderVersion.value
                ? null
                : (config.selectedLoaderVersion.value ?? config.loaderVersionType.value)
            const iconPath = config.instanceIconPath.value ?? null
            const name = config.instanceName.value.trim() || config.autoInstanceName.value
            // Game directory: `gameDirOverride` holds the picked `.minecraft`
            // root. Builtin keeps the managed folder (null); external resolves to
            // `<root>/versions/<name>` when version-isolated, or the `.minecraft`
            // root itself when not.
            const mode = config.gameDirOverrideMode.value
            const gameRoot = config.gameDirOverride.value ?? null
            const gameDirOverride =
                mode === 'builtin'
                    ? null
                    : gameRoot
                      ? mode === 'isolated'
                          ? await join(gameRoot, 'versions', name)
                          : gameRoot
                      : null

            await install_create_instance({
                name,
                gameVersion: config.selectedGameVersion.value!,
                loader: loader as InstanceLoader,
                loaderVersion,
                adjuncts: config.selectedAdjuncts.value.map((kind) => ({
                    instanceId: '',
                    kind,
                    version: null,
                    role: 'adjunct',
                })),
                iconPath,
                gameDirOverride,
            }).catch(handleError)

            trackEvent('InstanceCreate', {
                source: 'CreationModal',
            })
        } catch (err) {
            handleError(err as Error)
        }
    }

    const pendingModpackCreation = ref<{
        projectId: string
        versionId: string
        name: string
        iconUrl?: string
    } | null>(null)

    async function doInstallModpackFile(
        location: CreatePackLocation,
        options: { autoCloseMs?: number | null; existingNotify?: { id: number } } = {},
    ) {
        const existingNotify = options.existingNotify
        const installingNotify =
            existingNotify ??
            notificationManager.addNotification({
                title: formatMessage(modpackMessages.installing),
                type: 'info',
                autoCloseMs: options.autoCloseMs ?? 1000 * 10,
            })

        const job = await install_create_modpack_instance(location).catch((e) => {
            notificationManager.removeNotification(installingNotify.id)
            handleError(e)
            return null
        })
        if (!job) return

        // Single-use listener that auto-cleans up when the job reaches a terminal state
        const unlisten = await install_job_listener((updatedJob: InstallJobSnapshot) => {
            if (updatedJob.job_id !== job.job_id) return

            if (updatedJob.status === 'succeeded') {
                notificationManager.removeNotification(installingNotify.id)
                notificationManager.addNotification({
                    title: formatMessage(modpackMessages.installed),
                    type: 'success',
                })
                unlisten()
            } else if (['failed', 'canceled', 'interrupted'].includes(updatedJob.status)) {
                notificationManager.removeNotification(installingNotify.id)
                unlisten()
            }
        })
    }

    async function handleModpackDuplicateCreateAnyway() {
        if (!pendingModpackCreation.value) return
        const { projectId, versionId, name, iconUrl } = pendingModpackCreation.value
        pendingModpackCreation.value = null
        await proceedWithModpackCreation(projectId, versionId, name, iconUrl)
    }

    function handleModpackDuplicateGoToInstance(instanceId: string) {
        pendingModpackCreation.value = null
        router.push(`/instance/${encodeURIComponent(instanceId)}/`)
    }

    function handleBrowseModpacks() {
        installationModal.value?.hide()
        router.push('/browse/modpack')
    }

    async function searchModpacks(query: string, limit: number = 10) {
        const params = [`facets=[["project_type:modpack"]]`, `limit=${limit}`]
        if (query) {
            params.push(`query=${encodeURIComponent(query)}`)
        }
        const raw = await get_search_results(`?${params.join('&')}`)
        if (raw?.result) return raw.result
        return { hits: [], offset: 0, limit, total_hits: 0 }
    }

    async function getProjectVersions(projectId: string) {
        const versions = await get_project_versions(projectId, 'must_revalidate')
        return versions ?? []
    }

    async function hasCompatibleOptiFabric(gameVersion: string) {
        const response = await getCurseForgeFiles(OPTIFABRIC_CURSEFORGE_PROJECT_ID, {
            index: 0,
            pageSize: 50,
        })
        return hasCompatibleCurseForgeFile(response.files, gameVersion)
    }

    let _currentFlowCtx: CreationFlowContextValue | null = null

    /** Install a modpack file with continuous feedback notifications. */
    async function installModpackFromPath(
        filePath: string,
        fileName: string,
        options: { persistUntilDone?: boolean } = {},
    ) {
        const persistUntilDone = options.persistUntilDone === true
        const currentNotify = notificationManager.addNotification({
            title: formatMessage(modpackMessages.installingFile, { name: fileName }),
            type: 'info',
            autoCloseMs: persistUntilDone ? null : 1000 * 10,
        })

        const location: CreatePackLocation = { type: 'fromFile', path: filePath }

        try {
            const isMrpack = fileName?.toLowerCase().endsWith('.mrpack')

            if (!isMrpack) {
                // .zip needs preview to determine manifest
                const preview = await install_get_modpack_preview(location).catch((e) => {
                    notificationManager.removeNotification(currentNotify.id)
                    handleError(e)
                    return null
                })
                if (!preview) return

                if (preview.unknownFile) {
                    notificationManager.removeNotification(currentNotify.id)
                    unknownPackWarningModal.value?.show(async () => {
                        if (persistUntilDone) {
                            const installingNotify = notificationManager.addNotification({
                                title: formatMessage(modpackMessages.installing),
                                type: 'info',
                                autoCloseMs: null,
                            })
                            await doInstallModpackFile(location, {
                                existingNotify: installingNotify,
                            })
                        } else {
                            await doInstallModpackFile(location)
                        }
                    }, fileName)
                    return
                }
            }

            if (persistUntilDone) {
                await doInstallModpackFile(location, { existingNotify: currentNotify })
            } else {
                await doInstallModpackFile(location)
            }
        } catch (e) {
            notificationManager.removeNotification(currentNotify?.id)
            handleError(e as Error)
        }
    }

    provide('setCreationFlowCtx', (ctx: CreationFlowContextValue) => {
        _currentFlowCtx = ctx
    })

    return {
        installationModal,
        unknownPackWarningModal,
        fetchExistingInstanceNames,
        handleCreate,
        handleBrowseModpacks,
        searchModpacks,
        getProjectVersions,
        hasCompatibleOptiFabric,
        getLoaderManifest,
        installModpackFromPath,
        setModpackAlreadyInstalledModal,
        handleModpackDuplicateCreateAnyway,
        handleModpackDuplicateGoToInstance,
    }
}
