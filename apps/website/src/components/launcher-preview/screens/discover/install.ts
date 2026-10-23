import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import {
    getLatestMatchingInstallVersion,
    getTargetInstallPreferences,
    usesTargetGameVersion,
} from '@modrinth/ui/src/layouts/shared/browse-tab/composables/install-logic.ts'
import { createContext } from '@modrinth/ui/src/providers/create-context.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { nextTick, ref, shallowRef } from 'vue'
import type { Router } from 'vue-router'

import type { ProjectCard, ProjectDetail, ProjectVersion } from '../../data'
import { projectIcon } from '../../data'
import { getDisplayInstanceIcon } from '../../lib/instance-icons'
import type { PreviewState } from '../../lib/state'
import type { GameInstance, InstanceLoader } from '../../lib/types'
import { messages } from './messages'
import type {
    ContentSelectionItem,
    ContentSelectionType,
    DiscoverDomain,
    ReviewedInstall,
} from './state'
import { makeContentSelectionKey } from './state'

/**
 * Port of the app's content install provider (apps/app-frontend/src/providers/content-install.ts
 * with content-install-compat.ts): single-project installs from search cards and project pages.
 * Modpacks go through "Install modpack" into a new instance; other content picks a target in
 * "Install project" (or uses the instance in context), may warn about incompatible versions,
 * and is queued after "Review content changes".
 */

export type ContentInstallCallback = (versionId?: string, installedProjectIds?: string[]) => void

export interface ContentInstallInstance {
    id: string
    name: string
    iconUrl?: string | null
    iconFrameless?: boolean
    installed: boolean
    compatible: boolean
    installing?: boolean
}

export interface ModpackInstallModalData {
    project: { id: string; title: string; icon_url: string }
    versions: ProjectVersion[]
    initialVersionId?: string | null
    existingInstancesByVersion: Record<string, { id: string; name: string }[]>
    instancePaths: string[]
}

export interface InstallModalRefs {
    contentInstall?: { show: () => void; hide: () => void } | null
    modpackInstall?: { show: (data: ModpackInstallModalData) => void } | null
    incompatibility?: { show: (versionId?: string) => void; hide: () => void } | null
    review?: {
        showInstallSelection: (input: {
            instance: GameInstance
            items: ContentSelectionItem[]
        }) => Promise<ReviewedInstall | null>
    } | null
}

const LOADER_ORDER = ['vanilla', 'fabric', 'quilt', 'neoforge', 'forge']
const SUPPORTED_LOADERS = new Set(['vanilla', 'forge', 'fabric', 'quilt', 'neoforge'])
const VANILLA_COMPATIBLE_LOADERS = new Set(['minecraft', 'datapack'])

export function sortLoaders(loaders: string[]) {
    return loaders.slice().sort((a, b) => {
        const aIdx = LOADER_ORDER.indexOf(a)
        const bIdx = LOADER_ORDER.indexOf(b)
        if (aIdx === -1 && bIdx === -1) return a.localeCompare(b)
        if (aIdx === -1) return 1
        if (bIdx === -1) return -1
        return aIdx - bIdx
    })
}

export function isVersionCompatible(
    version: ProjectVersion,
    project: { project_type: string },
    instance: GameInstance,
) {
    if (project.project_type === 'resourcepack') return true
    return (
        version.game_versions.includes(instance.game_version) &&
        (project.project_type === 'mod'
            ? version.loaders.includes(instance.loader) || version.loaders.includes('datapack')
            : true)
    )
}

export function findPreferredVersion(
    versions: ProjectVersion[],
    project: { project_type: string },
    instance: GameInstance,
) {
    const projectType = project.project_type ?? 'mod'
    if (projectType === 'resourcepack') return versions[0]
    return (
        versions.find(
            (version) =>
                version.game_versions.includes(instance.game_version) &&
                (projectType === 'mod' ? version.loaders.includes(instance.loader) : true),
        ) ?? versions.find((version) => isVersionCompatible(version, project, instance))
    )
}

/** `instance_matches_targets` of `get_install_candidates` (app-lib list_content.rs). */
function instanceMatchesTargets(
    projectType: string,
    instance: GameInstance,
    versions: ProjectVersion[],
) {
    if (projectType === 'resourcepack') return versions.length > 0
    return versions.some(
        (version) =>
            version.game_versions.includes(instance.game_version) &&
            (projectType !== 'mod' ||
                version.loaders.includes(instance.loader) ||
                version.loaders.includes('datapack')),
    )
}

export function sortVersionsByDate(versions: ProjectVersion[]) {
    return versions
        .slice()
        .sort((a, b) => Date.parse(b.date_published) - Date.parse(a.date_published))
}

export function contentTypeOf(projectType: string): ContentSelectionType {
    return projectType === 'resourcepack' || projectType === 'datapack' || projectType === 'shader'
        ? projectType
        : 'mod'
}

export function toProjectCard(detail: ProjectDetail): ProjectCard {
    return {
        id: detail.id,
        slug: detail.slug,
        title: detail.title,
        description: detail.description,
        categories: detail.categories,
        client_side: detail.client_side,
        server_side: detail.server_side,
        project_type: detail.project_type,
        downloads: detail.downloads,
        followers: detail.followers,
        license: detail.license,
        loaders: detail.loaders,
        game_versions: detail.game_versions,
        updated: detail.updated,
        date_modified: detail.date_modified,
    }
}

export function displayInstanceIcon(instance: Pick<GameInstance, 'icon_path' | 'loader'>) {
    return getDisplayInstanceIcon(instance.icon_path, instance.loader)
}

export function createContentInstall(options: {
    preview: PreviewState
    discover: DiscoverDomain
    router: Router
}) {
    const { preview, discover, router } = options
    const { formatMessage } = useVIntl()
    const { addNotification, handleError } = injectNotificationManager()

    const modals: InstallModalRefs = {}

    const instances = ref<ContentInstallInstance[]>([])
    const compatibleLoaders = ref<string[]>([])
    const gameVersions = ref<string[]>([])
    const releaseGameVersions = ref<Set<string>>(new Set())
    const loading = ref(false)
    const defaultTab = ref<'existing' | 'new'>('existing')
    const preferredLoader = ref<string | null>(null)
    const preferredGameVersion = ref<string | null>(null)

    const incompatibilityVersions = shallowRef<ProjectVersion[]>([])
    const incompatibilityGameVersion = ref('')
    const incompatibilityLoader = ref('')
    const incompatibilityProjectType = ref<string | undefined>(undefined)
    const incompatibilityProjectIcon = ref<string | undefined>(undefined)
    const incompatibilityProjectName = ref<string | undefined>(undefined)
    const incompatibilityMessage = ref<string | undefined>(undefined)
    const incompatibilityInstalling = ref(false)

    let currentProject: ProjectDetail | null = null
    let currentVersions: ProjectVersion[] = []
    let currentCallback: ContentInstallCallback = () => {}
    let contentInstallModalOpen = false
    let incompatibilityInstance: GameInstance | null = null
    let incompatibilityProject: ProjectDetail | null = null
    let incompatibilityCallback: ContentInstallCallback = () => {}
    let incompatibilityInstalled = false
    let pendingModpack: {
        project: ProjectDetail
        callback: ContentInstallCallback
        createInstanceCallback: (instanceId: string) => void
    } | null = null

    function settle(versionId?: string, installedProjectIds?: string[]) {
        const callback = currentCallback
        currentCallback = () => {}
        callback(versionId, installedProjectIds)
    }

    function hideContentInstallModal() {
        contentInstallModalOpen = false
        loading.value = false
        modals.contentInstall?.hide()
    }

    async function queueResourceVersion(
        instance: GameInstance,
        project: ProjectDetail,
        version: ProjectVersion,
        contentType?: ContentSelectionType,
    ) {
        if (!modals.review) throw new Error(formatMessage(messages.installFailed))
        const type = contentType ?? contentTypeOf(project.project_type)
        const key = makeContentSelectionKey('modrinth', project.id)
        const result = await modals.review.showInstallSelection({
            instance,
            items: [
                {
                    key,
                    provider: 'modrinth',
                    projectId: project.id,
                    providerProjectId: project.id,
                    versionId: version.id,
                    contentType: type,
                    title: project.title,
                    iconUrl: projectIcon(project.id),
                    slug: project.slug,
                    targetInstanceId: instance.id,
                },
            ],
        })
        if (!result) return null
        const primaryIncluded = result.activeRoots.includes(key)
        return {
            versionId: primaryIncluded ? version.id : undefined,
            installedProjectIds: primaryIncluded ? [project.id] : [],
        }
    }

    function candidateInstances(project: ProjectDetail, versions: ProjectVersion[]) {
        const installedIn = (instance: GameInstance) =>
            discover.installedProjectIds(instance.id).has(project.id)
        return preview.instances.instances.value
            .filter((instance) => instance.link?.type !== 'server_project')
            .slice()
            .sort((a, b) => a.name.localeCompare(b.name))
            .map((instance) => {
                const icon = displayInstanceIcon(instance)
                return {
                    id: instance.id,
                    name: instance.name,
                    iconUrl: icon.url,
                    iconFrameless: icon.frameless,
                    installed: installedIn(instance),
                    compatible: instanceMatchesTargets(project.project_type, instance, versions),
                    installing: false,
                }
            })
    }

    async function showModInstallModal(
        project: ProjectDetail,
        versions: ProjectVersion[],
        hints?: { preferredLoader?: string; preferredGameVersion?: string },
    ) {
        currentProject = project
        currentVersions = versions
        instances.value = []
        loading.value = true
        defaultTab.value = 'existing'

        const loaderSet = new Set<string>()
        const gameVersionSet = new Set<string>()
        for (const version of versions) {
            for (const loader of version.loaders) loaderSet.add(loader)
            for (const gameVersion of version.game_versions) gameVersionSet.add(gameVersion)
        }
        const mappedLoaders = new Set<string>()
        for (const loader of loaderSet) {
            if (SUPPORTED_LOADERS.has(loader)) mappedLoaders.add(loader)
            else if (VANILLA_COMPATIBLE_LOADERS.has(loader)) mappedLoaders.add('vanilla')
        }
        compatibleLoaders.value = sortLoaders([...mappedLoaders])
        const tags = await discover.ensureTags()
        const ordered: string[] = []
        const releases = new Set<string>()
        for (const tag of tags.gameVersions) {
            if (!gameVersionSet.has(tag.version)) continue
            ordered.push(tag.version)
            if (tag.version_type === 'release') releases.add(tag.version)
        }
        gameVersions.value = ordered.length ? ordered : [...gameVersionSet]
        releaseGameVersions.value = releases
        preferredLoader.value =
            hints?.preferredLoader && loaderSet.has(hints.preferredLoader)
                ? hints.preferredLoader
                : null
        preferredGameVersion.value =
            hints?.preferredGameVersion && gameVersionSet.has(hints.preferredGameVersion)
                ? hints.preferredGameVersion
                : null

        await nextTick()
        contentInstallModalOpen = true
        modals.contentInstall?.show()

        // `get_install_candidates` reads the instance database.
        preview.clock.after(260, () => {
            if (currentProject !== project) return
            const candidates = candidateInstances(project, versions)
            instances.value = candidates
            if (!candidates.some((instance) => instance.compatible && !instance.installed))
                defaultTab.value = 'new'
            loading.value = false
        })
    }

    async function handleInstallToInstance(target: ContentInstallInstance) {
        const instance = preview.instances.get(target.id)
        const project = currentProject
        const stored = instances.value.find((entry) => entry.id === target.id)
        if (!project || !instance) {
            handleError(formatMessage(messages.installFailed))
            settle()
            return
        }
        const version = findPreferredVersion(currentVersions, project, instance)
        if (!version) {
            if (currentVersions.length > 0) {
                await showIncompatibilityWarning(
                    instance,
                    project,
                    currentVersions,
                    currentVersions[0],
                    (versionId, ids) => {
                        if (versionId && stored) stored.installed = true
                        settle(versionId, ids)
                    },
                )
            } else {
                handleError(formatMessage(messages.missingVersion))
                settle()
            }
            return
        }
        if (stored) stored.installing = true
        try {
            const job = await queueResourceVersion(instance, project, version)
            if (!job) {
                if (stored) stored.installing = false
                return
            }
            if (stored && !job.installedProjectIds.length) stored.installing = false
            settle(job.versionId, job.installedProjectIds)
            hideContentInstallModal()
        } catch (error) {
            if (stored) stored.installing = false
            handleError(error)
            settle()
        }
    }

    async function showIncompatibilityWarning(
        instance: GameInstance,
        project: ProjectDetail,
        versions: ProjectVersion[],
        version: ProjectVersion,
        callback: ContentInstallCallback,
    ) {
        incompatibilityInstance = instance
        incompatibilityProject = project
        incompatibilityCallback = callback
        incompatibilityInstalled = false
        incompatibilityInstalling.value = false
        incompatibilityVersions.value = versions
        incompatibilityGameVersion.value = instance.game_version ?? ''
        incompatibilityLoader.value = instance.loader ?? ''
        incompatibilityProjectType.value = project.project_type
        incompatibilityProjectIcon.value = projectIcon(project.id)
        incompatibilityProjectName.value = project.title
        const compatibilityLabel =
            project.project_type === 'resourcepack' || project.project_type === 'datapack'
                ? (instance.game_version ?? '')
                : `${instance.loader ?? ''} ${instance.game_version ?? ''}`.trim()
        incompatibilityMessage.value = formatMessage(messages.noCompatibleVersions, {
            compatibilityLabel,
        })
        await nextTick()
        modals.incompatibility?.show(version.id)
    }

    async function handleIncompatibilityWarningInstall(version: ProjectVersion) {
        const instance = incompatibilityInstance
        const project = incompatibilityProject
        const callback = incompatibilityCallback
        if (!instance || !project) return
        incompatibilityInstalling.value = true
        let result: Awaited<ReturnType<typeof queueResourceVersion>> = null
        try {
            result = await queueResourceVersion(instance, project, version)
        } catch (error) {
            handleError(error)
            incompatibilityInstalling.value = false
            return
        }
        incompatibilityInstalling.value = false
        if (!result) return
        incompatibilityInstalled = result.installedProjectIds.length > 0
        callback(result.versionId, result.installedProjectIds)
        modals.incompatibility?.hide()
    }

    function handleIncompatibilityWarningCancel() {
        if (incompatibilityInstance && !incompatibilityInstalled) incompatibilityCallback()
        incompatibilityInstalled = false
    }

    let creating = false
    async function handleCreateAndInstall(data: {
        name: string
        iconPath: string | null
        iconPreviewUrl: string | null
        loader: string
        gameVersion: string
    }) {
        if (creating) return
        const project = currentProject
        const loaderCandidates =
            data.loader === 'vanilla' ? ['vanilla', 'datapack', 'minecraft'] : [data.loader]
        const version =
            currentVersions.find(
                (candidate) =>
                    candidate.game_versions.includes(data.gameVersion) &&
                    loaderCandidates.some((loader) => candidate.loaders.includes(loader)),
            ) ?? currentVersions[0]
        if (!project || !version) {
            handleError(formatMessage(messages.missingVersion))
            return
        }
        creating = true
        hideContentInstallModal()
        try {
            // `install_create_instance`: the launcher installs the game, then the instance exists.
            const created = preview.instances.create({
                name: data.name,
                gameVersion: data.gameVersion,
                loader: data.loader as InstanceLoader,
                loaderVersion: null,
                iconPath: data.iconPreviewUrl,
            })
            const instance = preview.instances.get(created.id) ?? created
            instance.install_stage = 'minecraft_installing'
            await new Promise<void>((resolve) => preview.clock.after(1400, resolve))
            instance.install_stage = 'installed'
            const job = await queueResourceVersion(instance, project, version)
            if (!job) {
                addNotification({
                    title: formatMessage(messages.retainedInstanceTitle),
                    text: formatMessage(messages.retainedInstanceDescription, { name: data.name }),
                    type: 'info',
                })
                settle()
                await router.push(`/instance/${encodeURIComponent(instance.id)}`)
                return
            }
            settle(job.versionId, job.installedProjectIds)
        } catch (error) {
            handleError(error)
            settle()
        } finally {
            creating = false
        }
    }

    function handleNavigate(target: ContentInstallInstance) {
        settle()
        hideContentInstallModal()
        void router.push(`/instance/${encodeURIComponent(target.id)}`)
    }

    function handleCancel() {
        if (!contentInstallModalOpen) return
        contentInstallModalOpen = false
        loading.value = false
        settle()
    }

    async function handleModpackInstall(versionId: string, name: string) {
        if (!pendingModpack) return
        const { project, callback, createInstanceCallback } = pendingModpack
        pendingModpack = null
        const version = project.versions.find((candidate) => candidate.id === versionId)
        if (!version) {
            callback()
            handleError(formatMessage(messages.missingVersion))
            return
        }
        const loader = (version.loaders.find((candidate) => SUPPORTED_LOADERS.has(candidate)) ??
            'vanilla') as InstanceLoader
        // `install_create_modpack_instance`: a new instance linked to the pack, filled by the job.
        const created = preview.instances.create({
            name,
            gameVersion: version.game_versions[0] ?? '',
            loader,
            loaderVersion: null,
            iconPath: projectIcon(project.id),
            link: { type: 'modrinth_modpack', project_id: project.id, version_id: version.id },
        })
        const instance = preview.instances.get(created.id) ?? created
        instance.install_stage = 'pack_installing'
        const job = await preview.downloads.enqueue({
            project: toProjectCard(project),
            instanceId: instance.id,
            versionId: version.id,
        })
        if (!job) {
            instance.install_stage = 'not_installed'
            callback()
            handleError(formatMessage(messages.installFailed))
            return
        }
        discover.trackModpackJob(job, instance.id)
        createInstanceCallback(instance.id)
        callback(versionId)
    }

    function handleModpackInstallCancel() {
        const callback = pendingModpack?.callback
        pendingModpack = null
        callback?.()
    }

    async function install(
        projectId: string,
        versionId?: string | null,
        instanceId?: string | null,
        _source = 'unknown',
        callback: ContentInstallCallback = () => {},
        createInstanceCallback: (instanceId: string) => void = () => {},
        hints?: { preferredLoader?: string; preferredGameVersion?: string },
    ) {
        if (!instanceId && contentInstallModalOpen) hideContentInstallModal()
        const resolvedId = await discover.resolveProjectId(projectId)
        const project = await discover.ensureDetail(resolvedId)
        if (!project) {
            handleError(formatMessage(messages.projectNotFound, { project: projectId }))
            callback()
            return
        }
        const versions = sortVersionsByDate(project.versions)

        if (project.project_type === 'modpack') {
            pendingModpack = { project, callback, createInstanceCallback }
            const packs = preview.instances.instances.value
            modals.modpackInstall?.show({
                project: {
                    id: project.id,
                    title: project.title,
                    icon_url: projectIcon(project.id),
                },
                versions,
                initialVersionId: versionId,
                existingInstancesByVersion: Object.fromEntries(
                    versions.map((version) => [
                        version.id,
                        packs
                            .filter(
                                (pack) =>
                                    pack.link?.type === 'modrinth_modpack' &&
                                    pack.link.project_id === project.id &&
                                    pack.link.version_id === version.id,
                            )
                            .map((pack) => ({ id: pack.id, name: pack.name })),
                    ]),
                ),
                instancePaths: packs.map((pack) => pack.path.split(/[\\/]/).pop() ?? pack.path),
            })
            return
        }

        if (instanceId) {
            const instance = preview.instances.get(instanceId)
            if (!instance) {
                callback()
                return
            }
            let version = versionId
                ? versions.find((candidate) => candidate.id === versionId)
                : findPreferredVersion(versions, project, instance)
            if (!version) version = versions[0]
            if (!version) {
                handleError(formatMessage(messages.missingVersion))
                callback()
                return
            }
            if (isVersionCompatible(version, project, instance)) {
                try {
                    const job = await queueResourceVersion(instance, project, version)
                    if (!job) {
                        callback()
                        return
                    }
                    callback(job.versionId, job.installedProjectIds)
                } catch (error) {
                    handleError(error)
                    callback()
                }
            } else {
                await showIncompatibilityWarning(instance, project, versions, version, callback)
            }
            return
        }

        currentCallback = callback
        const scoped = versionId ? versions.filter((version) => version.id === versionId) : versions
        await showModInstallModal(project, scoped, hints)
    }

    /** The automatic release of a cart item for an instance (resource operation planning). */
    function automaticVersion(
        project: ProjectDetail,
        instance: GameInstance,
        contentType: ContentSelectionType,
    ) {
        const preferences = getTargetInstallPreferences(
            { gameVersion: instance.game_version, loader: instance.loader },
            contentType,
        )
        return getLatestMatchingInstallVersion(
            project.versions as unknown as Parameters<typeof getLatestMatchingInstallVersion>[0],
            preferences,
        ) as unknown as ProjectVersion | undefined
    }

    return {
        modals,
        instances,
        compatibleLoaders,
        gameVersions,
        releaseGameVersions,
        loading,
        defaultTab,
        preferredLoader,
        preferredGameVersion,
        incompatibilityVersions,
        incompatibilityGameVersion,
        incompatibilityLoader,
        incompatibilityProjectType,
        incompatibilityProjectIcon,
        incompatibilityProjectName,
        incompatibilityMessage,
        incompatibilityInstalling,
        install,
        handleInstallToInstance,
        handleCreateAndInstall,
        handleNavigate,
        handleCancel,
        handleModpackInstall,
        handleModpackInstallCancel,
        handleIncompatibilityWarningInstall,
        handleIncompatibilityWarningCancel,
        automaticVersion,
        usesTargetGameVersion,
    }
}

export type ContentInstall = ReturnType<typeof createContentInstall>

export const [injectContentInstall, provideContentInstall] = createContext<ContentInstall>(
    'DiscoverPage',
    'launcherPreviewContentInstall',
)
