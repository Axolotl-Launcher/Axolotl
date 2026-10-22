import type { Labrinth } from '@modrinth/api-client'
import type { ContentInstallInstance, ContentInstallProjectInfo, ContentItem } from '@modrinth/ui'
import { useDebugLogger, useVIntl } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import dayjs from 'dayjs'
import { nextTick, ref } from 'vue'
import type { Router } from 'vue-router'

import type ResourceOperationPreviewModal from '@/components/ui/ResourceOperationPreviewModal.vue'
import { trackEvent } from '@/helpers/analytics'
import { get_organization, get_project, get_team, get_version_many } from '@/helpers/cache.js'
import {
    type CurseForgeFile,
    type CurseForgeInstallResult,
    type CurseForgeManualDownloadImport,
    type CurseForgeProject,
    getCurseForgeDownloadFailureDetails,
    getCurseForgeFile,
    getCurseForgeFiles,
    getCurseForgeProject,
    installCurseForgeModpack,
    queueCurseForgeWorld,
    summarizeCurseForgeInstall,
} from '@/helpers/curseforge'
import {
    type CurseForgeManualDownloadItem,
    setCurseForgeManualDownloads,
} from '@/helpers/curseforge-manual'
import { instance_listener } from '@/helpers/events.js'
import {
    install_create_instance,
    install_create_modpack_instance,
    installJobInstanceId,
    wait_for_install_job,
} from '@/helpers/install'
import { edit, get, get_install_candidates, list } from '@/helpers/instance'
import { getDisplayInstanceIcon } from '@/helpers/instance-icons'
import { type ResourceType, sourceResource } from '@/helpers/resources'
import { get_game_versions } from '@/helpers/tags'
import type { GameInstance, InstanceLoader } from '@/helpers/types'
import {
    findPreferredVersion,
    getInstallTargets,
    type InstallTargetInstance,
    isVersionCompatible,
    mapCurseForgeProject,
    mapCurseForgeVersion,
    sortLoaders,
    SUPPORTED_LOADERS,
    VANILLA_COMPATIBLE_LOADERS,
} from '@/providers/content-install-compat'
import {
    filterRemainingManualDownloads,
    formatAutomaticDownloadsFailedNotification,
    formatDependencyNotesNotification,
    formatImportedManualDownloadsNotification,
    formatManualDownloadsNotification,
    mapInstallResultToManualItems,
} from '@/providers/content-install-manual-downloads'
import {
    type ContentInstallCallback,
    type ContentInstallContext,
    type ContentInstallInstanceEvent,
    type ContentInstallTargetMode,
    type CurseForgeManualDownloadsModalRef,
    injectContentInstall,
    type InstallProvider,
    type ModalRef,
    type ModpackInstallModalRef,
    provideContentInstall,
} from '@/providers/content-install-types'
import { useTheming } from '@/store/state'

import { createContentInstallDepNotifications } from './content-install-dep-notifications'
import * as contentInstallMessages from './content-install-messages'
import { createInstallingItemsRegistry } from './content-install-registry'
import { createInstallSession } from './content-install-session'

export {
    type ContentInstallCallback,
    type ContentInstallContext,
    injectContentInstall,
    provideContentInstall,
}

const noCompatibleVersionsMessage = contentInstallMessages.noCompatibleVersionsMessage
const curseForgeWorldInvalidProjectMessage =
    contentInstallMessages.curseForgeWorldInvalidProjectMessage
const curseForgeWorldUnavailableMessage = contentInstallMessages.curseForgeWorldUnavailableMessage
const curseForgeWorldInstanceNotReadyMessage =
    contentInstallMessages.curseForgeWorldInstanceNotReadyMessage
const curseForgeWorldUnknownInstanceMessage =
    contentInstallMessages.curseForgeWorldUnknownInstanceMessage
const curseForgeNetworkFailureTitleMessage =
    contentInstallMessages.curseForgeNetworkFailureTitleMessage
const curseForgeNetworkFailureMessage = contentInstallMessages.curseForgeNetworkFailureMessage
const modpackInstalledTitleMessage = contentInstallMessages.modpackInstalledTitleMessage
const modpackInstalledBodyMessage = contentInstallMessages.modpackInstalledBodyMessage

export function createContentInstall(opts: {
    router: Router
    handleError: (err: unknown) => void
    addNotification: (notification: {
        title: string
        text?: string
        type?: 'error' | 'warning' | 'success' | 'info'
        supportData?: Record<string, unknown>
    }) => void
}): ContentInstallContext {
    const { formatMessage } = useVIntl()
    const themeStore = useTheming()
    const debugState = useDebugLogger('content-install')
    const instances = ref<ContentInstallInstance[]>([])
    const compatibleLoaders = ref<string[]>([])
    const gameVersions = ref<string[]>([])
    const loading = ref(false)
    const defaultTab = ref<'existing' | 'new'>('existing')
    const preferredLoader = ref<string | null>(null)
    const preferredGameVersion = ref<string | null>(null)
    const releaseGameVersions = ref<Set<string>>(new Set())

    const projectInfo = ref<ContentInstallProjectInfo | null>(null)
    const symlinkTarget = ref<string | null | undefined>(undefined)
    const installingItems = ref<Map<string, ContentItem[]>>(new Map())
    const pendingManualDownloadsByInstance = ref<Map<string, CurseForgeManualDownloadItem[]>>(
        new Map(),
    )
    const installRevisionByInstance = ref<Map<string, number>>(new Map())
    const installFailureRevisionByInstance = ref<Map<string, number>>(new Map())
    const incompatibilityWarningVersions = ref<Labrinth.Versions.v2.Version[]>([])
    const incompatibilityWarningCurrentGameVersion = ref('')
    const incompatibilityWarningCurrentLoader = ref('')
    const incompatibilityWarningProjectType = ref<string | undefined>(undefined)
    const incompatibilityWarningProjectIconUrl = ref<string | undefined>(undefined)
    const incompatibilityWarningProjectName = ref<string | undefined>(undefined)
    const incompatibilityWarningMessage = ref<string | undefined>(undefined)
    const incompatibilityWarningInstalling = ref(false)

    const { addInstallingItem, removeInstallingItems } = createInstallingItemsRegistry(
        installingItems,
        debugState,
    )

    const { notifyInstalledDependencies } = createContentInstallDepNotifications({
        formatMessage,
        addNotification: opts.addNotification,
    })

    function markInstanceContentChanged(instanceId: string) {
        const next = new Map(installRevisionByInstance.value)
        const newRev = (next.get(instanceId) ?? 0) + 1
        next.set(instanceId, newRev)
        installRevisionByInstance.value = next
        debugState('markInstanceContentChanged', { instanceId, revision: newRev })
    }

    function markInstanceContentInstallFailed(instanceId: string) {
        const next = new Map(installFailureRevisionByInstance.value)
        const newRev = (next.get(instanceId) ?? 0) + 1
        next.set(instanceId, newRev)
        installFailureRevisionByInstance.value = next
        debugState('markInstanceContentInstallFailed', { instanceId, revision: newRev })
    }

    void instance_listener((event: ContentInstallInstanceEvent) => {
        debugState('instance_listener event', event)
        if (event.event === 'content_install_finished') {
            markInstanceContentChanged(event.instance_id)
            removeInstallingItems(event.instance_id, event.project_ids ?? [])
            void notifyInstalledDependencies(event.instance_id, event.dependency_project_ids ?? [])
        } else if (event.event === 'content_install_failed') {
            removeInstallingItems(event.instance_id, event.project_ids ?? [])
            markInstanceContentInstallFailed(event.instance_id)
            markInstanceContentChanged(event.instance_id)
            opts.handleError(
                event.message ?? formatMessage(contentInstallMessages.errorMessages.installFailed),
            )
        }
    }).catch(opts.handleError)

    let modalRef: ModalRef | null = null
    function hideContentInstallModal() {
        contentInstallModalOpen = false
        loading.value = false
        modalRef?.hide()
    }
    let resourcePreview: InstanceType<typeof ResourceOperationPreviewModal> | null = null
    let modpackInstallModalRef: ModpackInstallModalRef | null = null
    let curseForgeManualDownloadsModalRef: CurseForgeManualDownloadsModalRef | null = null
    let incompatibilityWarningModalRef: ModalRef | null = null
    let currentProvider: InstallProvider = 'modrinth'
    let currentTargetMode: ContentInstallTargetMode = 'content'
    let currentProject: Labrinth.Projects.v2.Project | null = null
    let currentVersions: Labrinth.Versions.v2.Version[] = []
    let currentCurseForgeProject: CurseForgeProject | null = null
    let currentCurseForgeFiles = new Map<string, CurseForgeFile>()
    let currentWorldFileId: string | null = null
    let contentInstallModalOpen = false
    let instanceMap: Record<string, InstallTargetInstance> = {}
    let incompatibilityWarningInstance: InstallTargetInstance | null = null
    let incompatibilityWarningProject: Labrinth.Projects.v2.Project | null = null
    let incompatibilityWarningCallback: ContentInstallCallback = () => {}
    let incompatibilityWarningInstalled = false

    let pendingModpackInstall: {
        project: Labrinth.Projects.v2.Project
        version: string
        source: string
        callback: ContentInstallCallback
        createInstanceCallback: (instanceId: string) => void
        provider: InstallProvider
    } | null = null

    const installSession = createInstallSession()
    const {
        beginInstallSession,
        settleCurrentCallback,
        settleInstallSession,
        cancelCurrentSession,
        currentId: currentInstallSessionId,
        setCallback: setCurrentInstallCallback,
    } = installSession

    // Back-compat name used throughout this factory for the live session id.
    function currentSessionId() {
        return currentInstallSessionId()
    }

    async function guardInstallRequest<T>(
        request: () => Promise<T>,
        callback: ContentInstallCallback,
    ) {
        const previousSessionId = currentSessionId()
        const promise = request()
        const sessionId = currentSessionId() !== previousSessionId ? currentSessionId() : null
        try {
            return await promise
        } catch (error) {
            if (sessionId !== null) {
                if (sessionId === currentSessionId()) {
                    hideContentInstallModal()
                    settleInstallSession(sessionId)
                } else {
                    return
                }
            } else {
                callback()
            }
            throw error
        }
    }

    async function createAndInstallCurseForgeModpack(
        project: Labrinth.Projects.v2.Project,
        version: Labrinth.Versions.v2.Version,
        numericProjectId: number,
        gameVersion: string,
        loader: InstanceLoader,
        name: string,
        source: string,
        callback: ContentInstallCallback,
        createInstanceCallback: (instanceId: string) => void,
    ) {
        const curseForgeLink = {
            type: 'curseforge_modpack' as const,
            project_id: numericProjectId.toString(),
            version_id: version.id,
        }
        // Match Modrinth managed packs: associate the instance as soon as it is created
        // so Installation settings can show the linked-modpack controls immediately.
        const job = await install_create_instance({
            name,
            gameVersion,
            loader,
            loaderVersion: 'latest',
            iconPath: project.icon_url ?? null,
            link: curseForgeLink,
        })
        const createdInstanceId = installJobInstanceId(job)
        debugState('createAndInstallCurseForgeModpack', {
            jobId: job.job_id,
            createdInstanceId,
        })
        if (!createdInstanceId) return
        createInstanceCallback(createdInstanceId)
        addInstallingItem(createdInstanceId, project, version)
        try {
            await wait_for_install_job(job.job_id)
            debugState('wait_for_install_job done', { jobId: job.job_id, createdInstanceId })
            await edit(createdInstanceId, {
                link: curseForgeLink,
            })
            removeInstallingItems(createdInstanceId, [project.id])
            markInstanceContentChanged(createdInstanceId)
            trackEvent('PackInstall', {
                id: project.id,
                version_id: version.id,
                title: project.title,
                source,
            })
            callback(version.id, [project.id])
        } catch (err) {
            debugState('createAndInstallCurseForgeModpack ERR', {
                err: String(err),
                createdInstanceId,
            })
            // Best-effort: still keep the managed association for settings/update UI.
            await edit(createdInstanceId, {
                link: curseForgeLink,
            }).catch(() => {})
            removeInstallingItems(createdInstanceId, [project.id])
            markInstanceContentInstallFailed(createdInstanceId)
            throw err
        }
    }

    async function showModInstallModal(
        project: Labrinth.Projects.v2.Project,
        versions: Labrinth.Versions.v2.Version[],
        onInstall: ContentInstallCallback,
        hints?: {
            preferredLoader?: string
            preferredGameVersion?: string
            showProjectInfo?: boolean
        },
        modalAlreadyOpen = false,
    ) {
        const sessionId = currentSessionId()
        currentTargetMode = 'content'
        currentWorldFileId = null
        currentProject = project
        currentVersions = versions
        setCurrentInstallCallback(onInstall)

        instances.value = []
        loading.value = true
        defaultTab.value = 'existing'

        if (hints?.showProjectInfo) {
            projectInfo.value = {
                title: project.title,
                iconUrl: project.icon_url,
                link:
                    currentProvider === 'curseforge'
                        ? `/project/curseforge/${currentCurseForgeProject?.id}`
                        : `/project/${project.slug ?? project.id}`,
            }
            if (currentProvider === 'curseforge' && currentCurseForgeProject?.authors[0]) {
                const author = currentCurseForgeProject.authors[0]
                projectInfo.value = {
                    ...projectInfo.value,
                    owner: {
                        name: author.name,
                        circle: true,
                        link: () => openUrl(author.url),
                    },
                }
            } else if (project.organization) {
                get_organization(project.organization)
                    .then((org: { id: string; slug: string; name: string; icon_url?: string }) => {
                        if (sessionId === currentSessionId() && projectInfo.value) {
                            const orgSlug = org.slug ?? org.id
                            projectInfo.value = {
                                ...projectInfo.value,
                                owner: {
                                    name: org.name,
                                    iconUrl: org.icon_url,
                                    circle: false,
                                    link: () =>
                                        openUrl(`https://modrinth.com/organization/${orgSlug}`),
                                },
                            }
                        }
                    })
                    .catch(() => {})
            } else if (project.team) {
                get_team(project.team)
                    .then(
                        (
                            members: {
                                user: { id: string; username: string; avatar_url?: string }
                                is_owner: boolean
                            }[],
                        ) => {
                            const owner = members.find((m) => m.is_owner)
                            if (sessionId === currentSessionId() && owner && projectInfo.value) {
                                projectInfo.value = {
                                    ...projectInfo.value,
                                    owner: {
                                        name: owner.user.username,
                                        iconUrl: owner.user.avatar_url,
                                        circle: true,
                                        link: () =>
                                            openUrl(
                                                `https://modrinth.com/user/${owner.user.username}`,
                                            ),
                                    },
                                }
                            }
                        },
                    )
                    .catch(() => {})
            }
        } else {
            projectInfo.value = null
        }

        const loaderSet = new Set<string>()
        const gameVersionSet = new Set<string>()
        for (const v of versions) {
            for (const l of v.loaders) loaderSet.add(l)
            for (const gv of v.game_versions) gameVersionSet.add(gv)
        }
        const mappedLoaders = new Set<string>()
        for (const l of loaderSet) {
            if (SUPPORTED_LOADERS.has(l)) mappedLoaders.add(l)
            else if (VANILLA_COMPATIBLE_LOADERS.has(l)) mappedLoaders.add('vanilla')
        }
        compatibleLoaders.value = sortLoaders([...mappedLoaders])
        gameVersions.value = [...gameVersionSet]
        releaseGameVersions.value = new Set(gameVersionSet)

        preferredLoader.value =
            hints?.preferredLoader && loaderSet.has(hints.preferredLoader)
                ? hints.preferredLoader
                : null
        preferredGameVersion.value =
            hints?.preferredGameVersion && gameVersionSet.has(hints.preferredGameVersion)
                ? hints.preferredGameVersion
                : null

        if (!modalAlreadyOpen) {
            await nextTick()
            if (sessionId !== currentSessionId()) return
            contentInstallModalOpen = true
            modalRef?.show()
            trackEvent('ProjectInstallStart', { source: 'ProjectInstallModal' })
        }

        get_game_versions()
            .then((allGameVersions) => {
                if (sessionId !== currentSessionId()) return
                const releases = new Set<string>()
                const ordered: string[] = []
                for (const gv of allGameVersions) {
                    if (gameVersionSet.has(gv.version)) {
                        ordered.push(gv.version)
                        if (gv.version_type === 'release') {
                            releases.add(gv.version)
                        }
                    }
                }
                gameVersions.value = ordered
                releaseGameVersions.value = releases
            })
            .catch(() => {})

        try {
            const candidates = await get_install_candidates(
                project.id,
                project.project_type,
                getInstallTargets(versions),
            )
            if (sessionId !== currentSessionId()) return
            const newInstanceMap: Record<string, InstallTargetInstance> = {}
            const newInstances: ContentInstallInstance[] = candidates.map((instance) => {
                newInstanceMap[instance.id] = instance
                const displayIcon = getDisplayInstanceIcon(instance.icon_path, instance.loader)
                return {
                    id: instance.id,
                    name: instance.name,
                    iconUrl: displayIcon.url,
                    iconFrameless: displayIcon.frameless,
                    installed: instance.installed,
                    compatible: instance.compatible,
                    installing: false,
                }
            })

            instanceMap = newInstanceMap
            instances.value = newInstances

            if (!newInstances.some((i) => i.compatible && !i.installed)) {
                defaultTab.value = 'new'
            }
        } catch (err) {
            if (sessionId === currentSessionId()) opts.handleError(err)
        } finally {
            if (sessionId === currentSessionId()) loading.value = false
        }
    }

    async function showContentInstallLoading(sessionId: number) {
        if (sessionId !== currentSessionId()) return
        currentTargetMode = 'content'
        currentWorldFileId = null
        instances.value = []
        compatibleLoaders.value = []
        gameVersions.value = []
        releaseGameVersions.value = new Set()
        projectInfo.value = null
        loading.value = true
        defaultTab.value = 'existing'

        await nextTick()
        if (sessionId !== currentSessionId()) return
        contentInstallModalOpen = true
        modalRef?.show()
        trackEvent('ProjectInstallStart', { source: 'ProjectInstallModal' })
    }

    function rememberManualDownloads(
        instanceId: string,
        result: CurseForgeInstallResult,
    ): CurseForgeManualDownloadItem[] {
        const manualItems = mapInstallResultToManualItems(result)
        setCurseForgeManualDownloads(instanceId, manualItems)
        const next = new Map(pendingManualDownloadsByInstance.value)
        if (manualItems.length > 0) {
            next.set(instanceId, manualItems)
        } else {
            next.delete(instanceId)
        }
        pendingManualDownloadsByInstance.value = next
        debugState('rememberManualDownloads', {
            instanceId,
            count: manualItems.length,
            items: manualItems.map((m) => m.fileName),
        })
        return manualItems
    }

    function handleCurseForgeManualDownloadsImported(
        instanceId: string,
        imported: CurseForgeManualDownloadImport[],
    ) {
        if (imported.length === 0) return
        const remaining = filterRemainingManualDownloads(
            pendingManualDownloadsByInstance.value.get(instanceId) ?? [],
            imported,
        )
        setCurseForgeManualDownloads(instanceId, remaining)
        const next = new Map(pendingManualDownloadsByInstance.value)
        if (remaining.length > 0) next.set(instanceId, remaining)
        else next.delete(instanceId)
        pendingManualDownloadsByInstance.value = next
        markInstanceContentChanged(instanceId)
        opts.addNotification(
            formatImportedManualDownloadsNotification(formatMessage, imported.length),
        )
    }

    function showManualCurseForgeDownloads(instanceId: string, result: CurseForgeInstallResult) {
        const summary = summarizeCurseForgeInstall(result)
        const manualItems = rememberManualDownloads(instanceId, result)
        if (manualItems.length === 0) return

        opts.addNotification(
            formatManualDownloadsNotification(
                formatMessage,
                summary,
                manualItems.map((item) => item.fileName),
            ),
        )

        curseForgeManualDownloadsModalRef?.show({
            items: manualItems,
            installed: summary.installed,
            instanceId,
        })
    }

    function showFailedCurseForgeDownloads(result: CurseForgeInstallResult) {
        const failedDownloads = result.failedDownloads ?? []
        if (failedDownloads.length === 0) return

        opts.addNotification(
            formatAutomaticDownloadsFailedNotification(
                formatMessage,
                failedDownloads.map((item) => item.fileName),
            ),
        )
    }

    function showCurseForgeDependencyNotes(result: CurseForgeInstallResult) {
        const notification = formatDependencyNotesNotification(formatMessage, {
            optional: result.optionalDependencies?.length ?? 0,
            incompatible: result.incompatibleDependencies?.length ?? 0,
            skipped: result.skippedDependencies?.length ?? 0,
        })
        if (notification) opts.addNotification(notification)
    }

    function handleContentInstallError(error: unknown) {
        const technicalDetails = getCurseForgeDownloadFailureDetails(error)
        if (currentProvider === 'curseforge' && technicalDetails) {
            opts.addNotification({
                title: formatMessage(curseForgeNetworkFailureTitleMessage),
                text: formatMessage(curseForgeNetworkFailureMessage),
                type: 'error',
                supportData: {
                    provider: 'curseforge',
                    technicalDetails,
                },
            })
            return
        }

        opts.handleError(error)
    }

    async function installCurrentCurseForgeVersion(
        instance: InstallTargetInstance,
        project: Labrinth.Projects.v2.Project,
        version: Labrinth.Versions.v2.Version,
        curseForgeProject = currentCurseForgeProject,
        file = currentCurseForgeFiles.get(version.id),
    ) {
        if (!curseForgeProject || !file) {
            throw new Error(
                formatMessage(contentInstallMessages.errorMessages.missingCurseForgeFile),
            )
        }

        if (project.project_type !== 'modpack')
            throw new Error(formatMessage(contentInstallMessages.errorMessages.reviewRequired))
        const result = (
            await installCurseForgeModpack({
                instanceId: instance.id,
                projectId: curseForgeProject.id,
                fileId: file.id,
            })
        ).content

        debugState('installCurrentCFVersion: result', {
            instanceId: instance.id,
            installedCount: result.installed.length,
            manualCount: (result.manualDownloads ?? []).length,
            failedCount: (result.failed ?? []).length,
        })

        showManualCurseForgeDownloads(instance.id, result)
        showFailedCurseForgeDownloads(result)
        showCurseForgeDependencyNotes(result)
        const installedProjectIds = [
            ...new Set(result.installed.map((installed) => `curseforge:${installed.projectId}`)),
        ]
        const primaryInstalled =
            project.project_type === 'modpack' ||
            result.installed.some(
                (installed) =>
                    !installed.dependency && installed.projectId === curseForgeProject.id,
            )
        if (primaryInstalled && !installedProjectIds.includes(project.id)) {
            installedProjectIds.unshift(project.id)
        }
        markInstanceContentChanged(instance.id)
        if (project.project_type === 'modpack') {
            const summary = summarizeCurseForgeInstall(result)
            if (summary.manual === 0 && summary.failed === 0 && summary.installed > 0) {
                setCurseForgeManualDownloads(instance.id, [])
                const next = new Map(pendingManualDownloadsByInstance.value)
                next.delete(instance.id)
                pendingManualDownloadsByInstance.value = next
                opts.addNotification({
                    title: formatMessage(modpackInstalledTitleMessage),
                    text: formatMessage(modpackInstalledBodyMessage, {
                        count: summary.installed,
                    }),
                    type: 'success',
                })
            }
        }
        return { installedProjectIds, primaryInstalled }
    }

    async function queueResourceVersion(
        instance: InstallTargetInstance,
        project: Labrinth.Projects.v2.Project,
        version: Labrinth.Versions.v2.Version,
        provider: 'modrinth' | 'curseforge',
    ) {
        if (!resourcePreview)
            throw new Error(formatMessage(contentInstallMessages.errorMessages.previewUnavailable))
        const projectId =
            provider === 'curseforge' ? project.id.replace(/^curseforge:/, '') : project.id
        const resourceType = (
            {
                mod: 'mod',
                plugin: 'plugin',
                datapack: 'dataPack',
                resourcepack: 'resourcePack',
                shader: 'shader',
            } as Record<string, ResourceType>
        )[project.project_type]
        if (!resourceType)
            throw new Error(formatMessage(contentInstallMessages.errorMessages.unsupportedType))
        const result = await resourcePreview.showInstallSelection({
            instanceId: instance.id,
            instanceName: instance.name,
            installDependencies: themeStore.getFeatureFlag('auto_install_dependencies'),
            selections: [
                {
                    id: `${provider}:${projectId}`,
                    resource: sourceResource(provider, projectId),
                    release: {
                        kind: 'release',
                        reference: {
                            project: { source: provider, projectId },
                            releaseId: version.id,
                        },
                    },
                    resourceType,
                    scope: { kind: 'instance' },
                },
            ],
        })
        if (!result) return null
        const primaryIncluded = result.activeRoots.includes(`${provider}:${projectId}`)
        return {
            job: result.job,
            versionId: primaryIncluded ? version.id : undefined,
            installedProjectIds: primaryIncluded ? [project.id] : [],
        }
    }

    async function queueCurrentCurseForgeVersion(
        instance: InstallTargetInstance,
        project: Labrinth.Projects.v2.Project,
        version: Labrinth.Versions.v2.Version,
    ) {
        return queueResourceVersion(instance, project, version, 'curseforge')
    }

    async function queueCurrentCurseForgeWorld(
        instance: InstallTargetInstance & Partial<Pick<GameInstance, 'install_stage'>>,
    ) {
        const curseForgeProject = currentCurseForgeProject
        const file = currentWorldFileId ? currentCurseForgeFiles.get(currentWorldFileId) : null
        if (!curseForgeProject || !file || !currentProject) {
            throw new Error(formatMessage(curseForgeWorldUnavailableMessage))
        }
        if (curseForgeProject.classId !== 17) {
            throw new Error(formatMessage(curseForgeWorldInvalidProjectMessage))
        }
        if (instance.install_stage && instance.install_stage !== 'installed') {
            throw new Error(formatMessage(curseForgeWorldInstanceNotReadyMessage))
        }

        return await queueCurseForgeWorld(
            {
                instanceId: instance.id,
                projectId: curseForgeProject.id,
                fileId: file.id,
            },
            { title: currentProject.title, iconUrl: currentProject.icon_url },
        )
    }

    async function handleInstallToInstance(instance: ContentInstallInstance) {
        const selectedInstance = instanceMap[instance.id]
        const storeInstance = instances.value.find((i) => i.id === instance.id)
        if (!currentProject || !selectedInstance) {
            opts.handleError(formatMessage(contentInstallMessages.errorMessages.missingTarget))
            settleCurrentCallback()
            return
        }

        if (currentTargetMode === 'world') {
            if (storeInstance) storeInstance.installing = true
            try {
                await queueCurrentCurseForgeWorld(selectedInstance)
                trackEvent('ProjectInstall', {
                    loader: selectedInstance.loader,
                    game_version: selectedInstance.game_version,
                    id: currentProject.id,
                    version_id: currentWorldFileId ?? '',
                    project_type: 'world',
                    title: currentProject.title,
                    source: 'ProjectInstallModal',
                })
                settleCurrentCallback(currentWorldFileId ?? undefined, [currentProject.id])
                hideContentInstallModal()
            } catch (err) {
                if (storeInstance) storeInstance.installing = false
                handleContentInstallError(err)
                settleCurrentCallback()
            }
            return
        }

        const version = findPreferredVersion(currentVersions, currentProject, selectedInstance)
        if (!version) {
            if (currentVersions.length > 0 && incompatibilityWarningModalRef) {
                const onIncompatibleInstall = (versionId?: string) => {
                    if (versionId && storeInstance) {
                        storeInstance.installed = true
                    }
                    settleCurrentCallback(
                        versionId,
                        versionId && currentProject ? [currentProject.id] : undefined,
                    )
                }
                await showIncompatibilityWarning(
                    selectedInstance,
                    currentProject,
                    currentVersions,
                    currentVersions[0],
                    onIncompatibleInstall,
                )
            } else {
                opts.handleError(formatMessage(contentInstallMessages.errorMessages.missingVersion))
                settleCurrentCallback()
            }
            return
        }

        if (currentProvider === 'modrinth') {
            if (storeInstance) storeInstance.installing = true
            try {
                const job = await queueResourceVersion(
                    selectedInstance,
                    currentProject,
                    version,
                    'modrinth',
                )
                if (!job) {
                    if (storeInstance) storeInstance.installing = false
                    settleCurrentCallback()
                    return
                }
                trackEvent('ProjectInstall', {
                    loader: selectedInstance.loader,
                    game_version: selectedInstance.game_version,
                    id: currentProject.id,
                    version_id: version.id,
                    project_type: currentProject.project_type,
                    title: currentProject.title,
                    source: 'ProjectInstallModal',
                })
                if (storeInstance && !job.installedProjectIds.length)
                    storeInstance.installing = false
                settleCurrentCallback(job.versionId, job.installedProjectIds)
                hideContentInstallModal()
            } catch (err) {
                if (storeInstance) storeInstance.installing = false
                handleContentInstallError(err)
                settleCurrentCallback()
            }
            return
        }

        if (currentProvider === 'curseforge') {
            if (storeInstance) storeInstance.installing = true
            try {
                const job = await queueCurrentCurseForgeVersion(
                    selectedInstance,
                    currentProject,
                    version,
                )
                if (!job) {
                    if (storeInstance) storeInstance.installing = false
                    settleCurrentCallback()
                    return
                }
                trackEvent('ProjectInstall', {
                    loader: selectedInstance.loader,
                    game_version: selectedInstance.game_version,
                    id: currentProject.id,
                    version_id: version.id,
                    project_type: currentProject.project_type,
                    title: currentProject.title,
                    source: 'ProjectInstallModal',
                })
                if (storeInstance && !job.installedProjectIds.length)
                    storeInstance.installing = false
                settleCurrentCallback(job.versionId, job.installedProjectIds)
                hideContentInstallModal()
            } catch (err) {
                if (storeInstance) storeInstance.installing = false
                handleContentInstallError(err)
                settleCurrentCallback()
            }
            return
        }

        if (storeInstance) storeInstance.installing = true

        const installedProjectIds: string[] = [currentProject.id]
        const plannedProjectIds: string[] = [currentProject.id]
        addInstallingItem(instance.id, currentProject, version)

        try {
            let primaryInstalled = true
            if (currentProvider === 'curseforge') {
                const result = await installCurrentCurseForgeVersion(
                    selectedInstance,
                    currentProject,
                    version,
                )
                installedProjectIds.splice(
                    0,
                    installedProjectIds.length,
                    ...result.installedProjectIds,
                )
                primaryInstalled = result.primaryInstalled
                removeInstallingItems(instance.id, plannedProjectIds)
            } else {
                throw new Error(
                    formatMessage(contentInstallMessages.errorMessages.unsupportedProvider),
                )
            }
            if (storeInstance) {
                storeInstance.installed = primaryInstalled
                storeInstance.installing = false
            }
            trackEvent('ProjectInstall', {
                loader: selectedInstance.loader,
                game_version: selectedInstance.game_version,
                id: currentProject!.id,
                version_id: version.id,
                project_type: currentProject!.project_type,
                title: currentProject!.title,
                source: 'ProjectInstallModal',
            })
            settleCurrentCallback(primaryInstalled ? version.id : undefined, installedProjectIds)
        } catch (err) {
            if (storeInstance) storeInstance.installing = false
            removeInstallingItems(instance.id, plannedProjectIds)
            markInstanceContentInstallFailed(instance.id)
            handleContentInstallError(err)
            settleCurrentCallback()
        }
    }

    async function showIncompatibilityWarning(
        instance: InstallTargetInstance,
        project: Labrinth.Projects.v2.Project,
        versions: Labrinth.Versions.v2.Version[],
        version: Labrinth.Versions.v2.Version,
        callback: ContentInstallCallback,
    ) {
        incompatibilityWarningInstance = instance
        incompatibilityWarningProject = project
        incompatibilityWarningCallback = callback
        incompatibilityWarningInstalled = false
        incompatibilityWarningInstalling.value = false
        incompatibilityWarningVersions.value = versions
        incompatibilityWarningCurrentGameVersion.value = instance.game_version ?? ''
        incompatibilityWarningCurrentLoader.value = instance.loader ?? ''
        incompatibilityWarningProjectType.value = project.project_type
        incompatibilityWarningProjectIconUrl.value = project.icon_url ?? undefined
        incompatibilityWarningProjectName.value = project.title

        const compatibilityLabel =
            project.project_type === 'resourcepack' || project.project_type === 'datapack'
                ? (instance.game_version ?? '')
                : `${instance.loader ?? ''} ${instance.game_version ?? ''}`.trim()
        incompatibilityWarningMessage.value = formatMessage(noCompatibleVersionsMessage, {
            compatibilityLabel,
        })

        await nextTick()
        incompatibilityWarningModalRef?.show(version.id)
        trackEvent('ProjectInstallStart', { source: 'ProjectIncompatibilityWarningModal' })
    }

    async function handleIncompatibilityWarningInstall(version: Labrinth.Versions.v2.Version) {
        const instance = incompatibilityWarningInstance
        const project = incompatibilityWarningProject
        const callback = incompatibilityWarningCallback
        if (!instance || !project) return

        incompatibilityWarningInstalling.value = true
        addInstallingItem(instance.id, project, version)
        let result: Awaited<ReturnType<typeof queueResourceVersion>> = null
        try {
            if (currentProvider === 'curseforge') {
                result = await queueCurrentCurseForgeVersion(instance, project, version)
                if (!result) {
                    incompatibilityWarningInstalling.value = false
                    removeInstallingItems(instance.id, [project.id])
                    return
                }
            } else {
                result = await queueResourceVersion(instance, project, version, 'modrinth')
                if (!result) {
                    incompatibilityWarningInstalling.value = false
                    removeInstallingItems(instance.id, [project.id])
                    return
                }
            }
        } catch (err) {
            handleContentInstallError(err)
            incompatibilityWarningInstalling.value = false
            removeInstallingItems(instance.id, [project.id])
            markInstanceContentInstallFailed(instance.id)
            return
        }

        incompatibilityWarningInstalling.value = false
        incompatibilityWarningInstalled = result.installedProjectIds.length > 0
        if (!incompatibilityWarningInstalled) removeInstallingItems(instance.id, [project.id])
        callback(result.versionId, result.installedProjectIds)
        markInstanceContentChanged(instance.id)
        incompatibilityWarningModalRef?.hide()
        removeInstallingItems(instance.id, [project.id])

        trackEvent('ProjectInstall', {
            loader: instance.loader ?? '',
            game_version: instance.game_version ?? '',
            id: project.id,
            version_id: version.id,
            project_type: project.project_type,
            title: project.title,
            source: 'ProjectIncompatibilityWarningModal',
        })
    }

    function handleIncompatibilityWarningCancel() {
        if (incompatibilityWarningInstance && !incompatibilityWarningInstalled) {
            incompatibilityWarningCallback()
        }
        incompatibilityWarningInstalled = false
    }

    let creatingSession: number | null = null
    async function handleCreateAndInstall(data: {
        name: string
        iconPath: string | null
        iconPreviewUrl: string | null
        loader: string
        gameVersion: string
    }) {
        const sessionId = currentSessionId()
        if (creatingSession === sessionId) return
        const project = currentProject
        const provider = currentProvider
        const curseForgeProject = currentCurseForgeProject
        const loaderCandidates =
            data.loader === 'vanilla' ? ['vanilla', 'datapack', 'minecraft'] : [data.loader]
        const version =
            currentVersions.find(
                (v) =>
                    v.game_versions.includes(data.gameVersion) &&
                    loaderCandidates.some((l) => v.loaders.includes(l)),
            ) ?? currentVersions[0]
        if (!project || !version) {
            handleContentInstallError(
                formatMessage(contentInstallMessages.errorMessages.missingVersion),
            )
            return
        }
        const file = currentCurseForgeFiles.get(version.id)
        const isCurrent = () => sessionId === currentSessionId()
        creatingSession = sessionId
        hideContentInstallModal()
        let createdInstanceId: string | null = null
        const retainInstance = async () => {
            opts.addNotification({
                title: formatMessage(contentInstallMessages.retainedInstanceMessages.title),
                text: formatMessage(contentInstallMessages.retainedInstanceMessages.description, {
                    name: data.name,
                }),
                type: 'info',
            })
            if (isCurrent() && createdInstanceId) {
                settleInstallSession(sessionId)
                await opts.router.push(`/instance/${encodeURIComponent(createdInstanceId)}`)
            }
        }
        try {
            const job = await install_create_instance({
                name: data.name,
                gameVersion: data.gameVersion,
                loader: data.loader as InstanceLoader,
                loaderVersion: 'latest',
                iconPath: data.iconPath,
            })
            const id = installJobInstanceId(job)
            if (!id) {
                settleInstallSession(sessionId)
                return
            }
            createdInstanceId = id
            if (!isCurrent()) {
                await retainInstance()
                return
            }
            await wait_for_install_job(job.job_id)
            if (!isCurrent()) {
                await retainInstance()
                return
            }
            const target = {
                id,
                name: data.name,
                icon_path: data.iconPath ?? undefined,
                game_version: data.gameVersion,
                loader: data.loader as InstanceLoader,
            }
            let installedProjectIds: string[]
            let installedVersionId: string | undefined
            if (provider === 'modrinth' || project.project_type !== 'modpack') {
                const contentJob = await queueResourceVersion(target, project, version, provider)
                if (!contentJob) {
                    await retainInstance()
                    return
                }
                if (!isCurrent()) return
                installedProjectIds = contentJob.installedProjectIds
                installedVersionId = contentJob.versionId
            } else {
                addInstallingItem(id, project, version)
                const result = await installCurrentCurseForgeVersion(
                    target,
                    project,
                    version,
                    curseForgeProject,
                    file,
                )
                removeInstallingItems(id, [project.id])
                if (!isCurrent()) return
                installedProjectIds = result.installedProjectIds
                installedVersionId = version.id
                await opts.router.push('/downloads')
                if (!isCurrent()) return
            }
            trackEvent('InstanceCreate', { source: 'ProjectInstallModal' })
            trackEvent('ProjectInstall', {
                loader: data.loader,
                game_version: data.gameVersion,
                id: project.id,
                version_id: version.id,
                project_type: project.project_type,
                title: project.title,
                source: 'ProjectInstallModal',
            })
            settleInstallSession(sessionId, installedVersionId, installedProjectIds)
        } catch (err) {
            if (createdInstanceId) {
                removeInstallingItems(createdInstanceId, [project.id])
                markInstanceContentInstallFailed(createdInstanceId)
            }
            if (isCurrent()) {
                settleInstallSession(sessionId)
                handleContentInstallError(err)
            }
        } finally {
            if (creatingSession === sessionId) creatingSession = null
        }
    }

    function handleNavigate(instance: ContentInstallInstance) {
        settleCurrentCallback()
        hideContentInstallModal()
        opts.router.push(`/instance/${encodeURIComponent(instance.id)}`)
    }

    function handleCancel() {
        if (!contentInstallModalOpen) return
        contentInstallModalOpen = false
        loading.value = false
        cancelCurrentSession()
    }

    async function installInternal(
        projectId: string,
        versionId?: string | null,
        instanceId?: string | null,
        source: string = 'unknown',
        callback: ContentInstallCallback = () => {},
        createInstanceCallback: (instanceId: string) => void = () => {},
        hints?: {
            preferredLoader?: string
            preferredGameVersion?: string
            showProjectInfo?: boolean
        },
    ) {
        const modalSessionId = instanceId ? null : beginInstallSession(callback)
        const requestCallback: ContentInstallCallback =
            modalSessionId === null
                ? callback
                : (...args) => settleInstallSession(modalSessionId, ...args)
        if (modalSessionId !== null && contentInstallModalOpen) hideContentInstallModal()
        currentProvider = 'modrinth'
        currentTargetMode = 'content'
        currentWorldFileId = null
        currentCurseForgeProject = null
        currentCurseForgeFiles = new Map()
        const shouldShowInstallTargetModal = !instanceId
        const project: Labrinth.Projects.v2.Project = await get_project(projectId)
        if (modalSessionId !== null && modalSessionId !== currentSessionId()) return
        if (!project) {
            if (modalSessionId === currentSessionId()) {
                hideContentInstallModal()
                settleCurrentCallback()
            }
            opts.handleError(
                formatMessage(contentInstallMessages.errorMessages.projectNotFound, {
                    project: projectId,
                }),
            )
            requestCallback()
            return
        }

        if (project.project_type === 'modpack') {
            if (shouldShowInstallTargetModal) hideContentInstallModal()
            const [versions, packs] = await Promise.all([
                get_version_many(project.versions, 'must_revalidate') as Promise<
                    Labrinth.Versions.v2.Version[]
                >,
                list(),
            ])
            if (modalSessionId !== null && modalSessionId !== currentSessionId()) return
            const sortedVersions = versions.sort(
                (a, b) => dayjs(b.date_published).valueOf() - dayjs(a.date_published).valueOf(),
            )
            pendingModpackInstall = {
                project,
                version: versionId ?? sortedVersions[0]?.id ?? '',
                source,
                callback: requestCallback,
                createInstanceCallback,
                provider: 'modrinth',
            }
            const existingInstancesByVersion = Object.fromEntries(
                sortedVersions.map((version) => [
                    version.id,
                    packs
                        .filter(
                            (pack) =>
                                pack.link?.project_id === project.id &&
                                pack.link?.version_id === version.id,
                        )
                        .map((pack) => ({ id: pack.id, name: pack.name })),
                ]),
            )
            modpackInstallModalRef?.show({
                project,
                versions: sortedVersions,
                initialVersionId: versionId,
                existingInstancesByVersion,
                instancePaths: packs.map((pack) => pack.path),
            })
            return
        }

        if (modalSessionId !== null) {
            await showContentInstallLoading(modalSessionId)
            if (modalSessionId !== currentSessionId()) return
        }

        if (instanceId) {
            const [instanceOrNull, versions] = await Promise.all([
                get(instanceId),
                get_version_many(
                    versionId ? [versionId] : project.versions,
                    'must_revalidate',
                ) as Promise<Labrinth.Versions.v2.Version[]>,
            ])
            if (!instanceOrNull) {
                callback()
                return
            }

            const instance = instanceOrNull
            const projectVersions = versions.sort(
                (a, b) => dayjs(b.date_published).valueOf() - dayjs(a.date_published).valueOf(),
            )
            let version = versionId
                ? projectVersions.find((v) => v.id === versionId)
                : findPreferredVersion(projectVersions, project, instance)
            if (!version) version = projectVersions[0]

            if (isVersionCompatible(version, project, instance)) {
                const job = await queueResourceVersion(instance, project, version, 'modrinth')
                if (!job) {
                    callback()
                    return
                }
                trackEvent('ProjectInstall', {
                    loader: instance.loader,
                    game_version: instance.game_version,
                    id: project.id,
                    project_type: project.project_type,
                    version_id: version.id,
                    title: project.title,
                    source,
                })
                callback(job.versionId, job.installedProjectIds)
            } else {
                await showIncompatibilityWarning(
                    instance,
                    project,
                    projectVersions,
                    version,
                    callback,
                )
            }
            return
        }

        let versions = (
            (await get_version_many(
                versionId ? [versionId] : project.versions,
            )) as Labrinth.Versions.v2.Version[]
        ).sort((a, b) => dayjs(b.date_published).valueOf() - dayjs(a.date_published).valueOf())
        if (modalSessionId !== currentSessionId()) return
        if (versionId) versions = versions.filter((v) => v.id === versionId)
        await showModInstallModal(project, versions, callback, hints, true)
    }

    async function installCurseForgeInternal(
        projectId: string,
        versionId?: string | null,
        instanceId?: string | null,
        source: string = 'unknown',
        callback: ContentInstallCallback = () => {},
        createInstanceCallback: (instanceId: string) => void = () => {},
        hints?: {
            preferredLoader?: string
            preferredGameVersion?: string
            showProjectInfo?: boolean
        },
    ) {
        const modalSessionId = instanceId ? null : beginInstallSession(callback)
        const requestCallback: ContentInstallCallback =
            modalSessionId === null
                ? callback
                : (...args) => settleInstallSession(modalSessionId, ...args)
        if (modalSessionId !== null && contentInstallModalOpen) hideContentInstallModal()
        currentTargetMode = 'content'
        currentWorldFileId = null
        const numericProjectId = Number(projectId.replace(/^curseforge:/, ''))
        if (!Number.isFinite(numericProjectId)) {
            throw new Error(
                formatMessage(contentInstallMessages.errorMessages.invalidCurseForgeProject),
            )
        }
        const shouldShowInstallTargetModal = !instanceId
        const [curseForgeProject, fileResponse] = await Promise.all([
            getCurseForgeProject(numericProjectId),
            versionId
                ? getCurseForgeFile(numericProjectId, Number(versionId)).then((file) => ({
                      files: [file],
                  }))
                : getCurseForgeFiles(numericProjectId, {}),
        ])
        if (modalSessionId !== null && modalSessionId !== currentSessionId()) return
        const availableFiles = fileResponse.files.filter((file) => file.isAvailable)
        const project = mapCurseForgeProject(curseForgeProject, availableFiles)
        let versions = availableFiles
            .map((file) => mapCurseForgeVersion(file, numericProjectId, project.project_type))
            .sort((a, b) => dayjs(b.date_published).valueOf() - dayjs(a.date_published).valueOf())
        if (versionId && project.project_type !== 'modpack') {
            versions = versions.filter((version) => version.id === versionId)
        }
        if (versions.length === 0) {
            if (shouldShowInstallTargetModal) hideContentInstallModal()
            throw new Error(formatMessage(contentInstallMessages.errorMessages.noCurseForgeFiles))
        }

        currentProvider = 'curseforge'
        currentProject = project
        currentVersions = versions
        currentCurseForgeProject = curseForgeProject
        currentCurseForgeFiles = new Map(availableFiles.map((file) => [file.id.toString(), file]))

        if (project.project_type === 'modpack') {
            if (shouldShowInstallTargetModal) hideContentInstallModal()
            const packs = await list()
            if (modalSessionId !== null && modalSessionId !== currentSessionId()) return
            pendingModpackInstall = {
                project,
                version: versionId ?? versions[0]?.id ?? '',
                source,
                callback: requestCallback,
                createInstanceCallback,
                provider: 'curseforge',
            }
            const existingInstancesByVersion = Object.fromEntries(
                versions.map((version) => [
                    version.id,
                    packs
                        .filter(
                            (pack) =>
                                pack.link?.type === 'curseforge_modpack' &&
                                pack.link.project_id === numericProjectId.toString() &&
                                pack.link.version_id === version.id,
                        )
                        .map((pack) => ({ id: pack.id, name: pack.name })),
                ]),
            )
            modpackInstallModalRef?.show({
                project,
                versions,
                initialVersionId: versionId,
                existingInstancesByVersion,
                instancePaths: packs.map((pack) => pack.path),
            })
            return
        }

        if (modalSessionId !== null) {
            await showContentInstallLoading(modalSessionId)
            if (modalSessionId !== currentSessionId()) return
        }

        if (instanceId) {
            const instance = await get(instanceId)
            if (!instance) {
                callback()
                return
            }
            let version = versionId
                ? versions.find((candidate) => candidate.id === versionId)
                : findPreferredVersion(versions, project, instance)
            if (!version) version = versions[0]
            if (isVersionCompatible(version, project, instance)) {
                const job = await queueCurrentCurseForgeVersion(instance, project, version)
                if (!job) {
                    callback()
                    return
                }
                trackEvent('ProjectInstall', {
                    loader: instance.loader,
                    game_version: instance.game_version,
                    id: project.id,
                    project_type: project.project_type,
                    version_id: version.id,
                    title: project.title,
                    source,
                })
                callback(job.versionId, job.installedProjectIds)
            } else {
                await showIncompatibilityWarning(instance, project, versions, version, callback)
            }
        } else {
            await showModInstallModal(project, versions, callback, hints, true)
        }
    }

    function setCurseForgeWorldInstallState(
        curseForgeProject: CurseForgeProject,
        file: CurseForgeFile,
        callback: ContentInstallCallback,
    ) {
        const project = mapCurseForgeProject(curseForgeProject, [file])
        currentProvider = 'curseforge'
        currentTargetMode = 'world'
        currentProject = project
        currentVersions = [mapCurseForgeVersion(file, curseForgeProject.id, project.project_type)]
        currentCurseForgeProject = curseForgeProject
        currentCurseForgeFiles = new Map([[file.id.toString(), file]])
        currentWorldFileId = file.id.toString()
        setCurrentInstallCallback(callback)
    }

    async function showCurseForgeWorldInstallModal(
        curseForgeProject: CurseForgeProject,
        file: CurseForgeFile,
        callback: ContentInstallCallback,
    ) {
        const sessionId = currentSessionId()
        setCurseForgeWorldInstallState(curseForgeProject, file, callback)
        instances.value = []
        compatibleLoaders.value = []
        gameVersions.value = []
        releaseGameVersions.value = new Set()
        preferredLoader.value = null
        preferredGameVersion.value = null
        loading.value = true
        defaultTab.value = 'existing'
        projectInfo.value = null

        await nextTick()
        if (sessionId !== currentSessionId()) return
        contentInstallModalOpen = true
        modalRef?.show()
        trackEvent('ProjectInstallStart', { source: 'ProjectInstallModal' })

        try {
            const candidates = (await list()).filter(
                (candidate) => candidate.install_stage === 'installed',
            )
            if (
                sessionId !== currentSessionId() ||
                currentTargetMode !== 'world' ||
                currentCurseForgeProject?.id !== curseForgeProject.id ||
                currentWorldFileId !== file.id.toString()
            ) {
                return
            }
            const newInstanceMap: Record<string, InstallTargetInstance> = {}
            instances.value = candidates.map((candidate) => {
                newInstanceMap[candidate.id] = candidate
                const displayIcon = getDisplayInstanceIcon(candidate.icon_path, candidate.loader)
                return {
                    id: candidate.id,
                    name: candidate.name,
                    iconUrl: displayIcon.url,
                    iconFrameless: displayIcon.frameless,
                    installed: false,
                    compatible: true,
                    installing: false,
                }
            })
            instanceMap = newInstanceMap
        } catch (err) {
            if (sessionId === currentSessionId()) opts.handleError(err)
        } finally {
            if (sessionId === currentSessionId()) loading.value = false
        }
    }

    async function installCurseForgeWorldInternal(
        projectId: string | number,
        fileId?: string | number | null,
        instanceId?: string | null,
        source: string = 'unknown',
        callback: ContentInstallCallback = () => {},
    ) {
        const modalSessionId = instanceId ? null : beginInstallSession(callback)
        const requestCallback: ContentInstallCallback =
            modalSessionId === null
                ? callback
                : (...args) => settleInstallSession(modalSessionId, ...args)
        const numericProjectId = Number(String(projectId).replace(/^curseforge:/, ''))
        if (!Number.isSafeInteger(numericProjectId) || numericProjectId <= 0) {
            throw new Error(formatMessage(curseForgeWorldInvalidProjectMessage))
        }
        const numericFileId = fileId == null ? null : Number(fileId)
        if (numericFileId != null && (!Number.isSafeInteger(numericFileId) || numericFileId <= 0)) {
            throw new Error(formatMessage(curseForgeWorldUnavailableMessage))
        }

        const curseForgeProject = await getCurseForgeProject(numericProjectId)
        if (curseForgeProject.classId !== 17) {
            throw new Error(formatMessage(curseForgeWorldInvalidProjectMessage))
        }
        const file = numericFileId
            ? await getCurseForgeFile(numericProjectId, numericFileId)
            : (await getCurseForgeFiles(numericProjectId, {})).files.find(
                  (candidate) => candidate.isAvailable,
              )
        if (!file?.isAvailable) {
            throw new Error(formatMessage(curseForgeWorldUnavailableMessage))
        }
        if (modalSessionId !== null && modalSessionId !== currentSessionId()) return

        if (!instanceId) {
            await showCurseForgeWorldInstallModal(curseForgeProject, file, callback)
            return
        }

        setCurseForgeWorldInstallState(curseForgeProject, file, requestCallback)
        const instance = await get(instanceId)
        if (!instance) throw new Error(formatMessage(curseForgeWorldUnknownInstanceMessage))
        await queueCurrentCurseForgeWorld(instance)
        trackEvent('ProjectInstall', {
            loader: instance.loader,
            game_version: instance.game_version,
            id: currentProject!.id,
            version_id: file.id.toString(),
            project_type: 'world',
            title: currentProject!.title,
            source,
        })
        callback(file.id.toString(), [currentProject!.id])
    }

    function install(
        projectId: string,
        versionId?: string | null,
        instanceId?: string | null,
        source: string = 'unknown',
        callback: ContentInstallCallback = () => {},
        createInstanceCallback: (instanceId: string) => void = () => {},
        hints?: {
            preferredLoader?: string
            preferredGameVersion?: string
            showProjectInfo?: boolean
        },
    ) {
        return guardInstallRequest(
            () =>
                installInternal(
                    projectId,
                    versionId,
                    instanceId,
                    source,
                    callback,
                    createInstanceCallback,
                    hints,
                ),
            callback,
        )
    }

    function installCurseForge(
        projectId: string,
        versionId?: string | null,
        instanceId?: string | null,
        source: string = 'unknown',
        callback: ContentInstallCallback = () => {},
        createInstanceCallback: (instanceId: string) => void = () => {},
        hints?: {
            preferredLoader?: string
            preferredGameVersion?: string
            showProjectInfo?: boolean
        },
    ) {
        return guardInstallRequest(
            () =>
                installCurseForgeInternal(
                    projectId,
                    versionId,
                    instanceId,
                    source,
                    callback,
                    createInstanceCallback,
                    hints,
                ),
            callback,
        )
    }

    function installCurseForgeWorld(
        projectId: string | number,
        fileId?: string | number | null,
        instanceId?: string | null,
        source: string = 'unknown',
        callback: ContentInstallCallback = () => {},
    ) {
        return guardInstallRequest(
            () => installCurseForgeWorldInternal(projectId, fileId, instanceId, source, callback),
            callback,
        )
    }

    return {
        instances,
        compatibleLoaders,
        gameVersions,
        loading,
        defaultTab,
        preferredLoader,
        preferredGameVersion,
        releaseGameVersions,
        projectInfo,
        symlinkTarget,
        handleInstallToInstance,
        handleCreateAndInstall,
        handleNavigate,
        handleCancel,
        setResourceInstallPreview(modal) {
            resourcePreview = modal
        },
        setContentInstallModal(ref: ModalRef) {
            modalRef = ref
        },
        setModpackInstallModal(ref: ModpackInstallModalRef) {
            modpackInstallModalRef = ref
        },
        setCurseForgeManualDownloadsModal(ref: CurseForgeManualDownloadsModalRef) {
            curseForgeManualDownloadsModalRef = ref
        },
        showCurseForgeManualDownloads(instanceId: string, items: CurseForgeManualDownloadItem[]) {
            setCurseForgeManualDownloads(instanceId, items)
            const next = new Map(pendingManualDownloadsByInstance.value)
            next.set(instanceId, items)
            pendingManualDownloadsByInstance.value = next
            curseForgeManualDownloadsModalRef?.show({ items, instanceId })
        },
        handleCurseForgeManualDownloadsImported,
        async handleModpackInstall(versionId: string, name: string) {
            if (!pendingModpackInstall) return
            const { project, source, callback, createInstanceCallback, provider } =
                pendingModpackInstall
            pendingModpackInstall = null
            if (provider === 'curseforge') {
                const numericProjectId = Number(project.id.replace(/^curseforge:/, ''))
                const selectedVersion =
                    currentVersions.find((candidate) => candidate.id === versionId) ??
                    currentVersions[0]
                if (!selectedVersion || !Number.isFinite(numericProjectId)) {
                    callback()
                    throw new Error(
                        formatMessage(contentInstallMessages.errorMessages.reinstallUnavailable),
                    )
                }
                const gameVersion = selectedVersion.game_versions[0]
                if (!gameVersion) {
                    callback()
                    throw new Error(
                        formatMessage(contentInstallMessages.errorMessages.missingGameVersion),
                    )
                }
                const loader =
                    (selectedVersion.loaders.find((candidate) =>
                        SUPPORTED_LOADERS.has(candidate),
                    ) as InstanceLoader | undefined) ?? 'vanilla'
                await createAndInstallCurseForgeModpack(
                    project,
                    selectedVersion,
                    numericProjectId,
                    gameVersion,
                    loader,
                    name,
                    source,
                    callback,
                    createInstanceCallback,
                ).catch((error) => {
                    callback()
                    throw error
                })
                return
            }
            const job = await install_create_modpack_instance(
                {
                    type: 'fromVersionId',
                    project_id: project.id,
                    version_id: versionId,
                    title: name,
                    icon_url: project.icon_url,
                },
                { name },
            ).catch((error) => {
                callback()
                throw error
            })
            const instanceId = installJobInstanceId(job)
            if (instanceId) {
                createInstanceCallback(instanceId)
            }
            trackEvent('PackInstall', {
                id: project.id,
                version_id: versionId,
                title: project.title,
                source,
            })
            callback(versionId)
        },
        handleModpackInstallCancel() {
            const callback = pendingModpackInstall?.callback
            pendingModpackInstall = null
            callback?.()
        },
        setIncompatibilityWarningModal(ref: ModalRef) {
            incompatibilityWarningModalRef = ref
        },
        incompatibilityWarningVersions,
        incompatibilityWarningCurrentGameVersion,
        incompatibilityWarningCurrentLoader,
        incompatibilityWarningProjectType,
        incompatibilityWarningProjectIconUrl,
        incompatibilityWarningProjectName,
        incompatibilityWarningMessage,
        incompatibilityWarningInstalling,
        handleIncompatibilityWarningInstall,
        handleIncompatibilityWarningCancel,
        install,
        installCurseForge,
        installCurseForgeWorld,
        installingItems,
        pendingManualDownloadsByInstance,
        installRevisionByInstance,
        installFailureRevisionByInstance,
    }
}
