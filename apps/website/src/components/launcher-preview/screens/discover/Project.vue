<script setup lang="ts">
import SwapIcon from '@modrinth/assets/icons/arrow-left-right.svg?component'
import BookOpenIcon from '@modrinth/assets/icons/book-open.svg?component'
import BookmarkIcon from '@modrinth/assets/icons/bookmark.svg?component'
import BookmarkFilledIcon from '@modrinth/assets/icons/bookmark-filled.svg?component'
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import GlobeIcon from '@modrinth/assets/icons/globe.svg?component'
import HeartIcon from '@modrinth/assets/icons/heart.svg?component'
import LanguagesIcon from '@modrinth/assets/icons/languages.svg?component'
import MoreVerticalIcon from '@modrinth/assets/icons/more-vertical.svg?component'
import PackageIcon from '@modrinth/assets/icons/package.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import ReportIcon from '@modrinth/assets/icons/report.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import NavTabs from '@modrinth/ui/src/components/base/NavTabs.vue'
import OverflowMenu from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import ProjectPageSkeleton from '@modrinth/ui/src/components/project/ProjectPageSkeleton.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import {
    getLatestMatchingInstallVersion,
    getTargetInstallPreferences,
    usesTargetGameVersion,
} from '@modrinth/ui/src/layouts/shared/browse-tab/composables/install-logic.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import {
    commonMessages,
    commonProjectSettingsMessages,
    formatProjectTypeSentence,
} from '@modrinth/ui/src/utils/common-messages.ts'
import { formatDependencyProjectFilterOption } from '@modrinth/ui/src/utils/search.ts'
import { computed, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import type { ProjectDetail, ProjectVersion } from '../../data'
import { projectIcon } from '../../data'
import { resolveMcmodUrl } from '../../lib/content-search'
import { instanceServerProjectId } from '../../lib/instance-project'
import { usePreview } from '../../lib/state'
import type { GameInstance } from '../../lib/types'
import { injectPreviewWindow } from '../../lib/window'
import PreviewContextMenu from '../../overlays/PreviewContextMenu.vue'
import { provideHomeRuntime } from '../home/runtime'
import { useHomeState } from '../home/state'
import { instancePageMessages } from '../instance/messages'
import type { BrowseInstallContext } from './browse-context'
import BrowseInstallHeader from './components/BrowseInstallHeader.vue'
import BrowseInstanceSelector from './components/BrowseInstanceSelector.vue'
import InstallModals from './components/InstallModals.vue'
import ProjectChangelog from './components/project/ProjectChangelog.vue'
import ProjectDescription from './components/project/ProjectDescription.vue'
import ProjectGallery from './components/project/ProjectGallery.vue'
import ProjectHeader from './components/project/ProjectHeader.vue'
import ProjectSidebar from './components/project/ProjectSidebar.vue'
import ProjectVersionPage from './components/project/ProjectVersion.vue'
import ProjectVersions from './components/project/ProjectVersions.vue'
import SelectedProjectsFloatingBar from './components/SelectedProjectsFloatingBar.vue'
import {
    createContentInstall,
    displayInstanceIcon,
    provideContentInstall,
    sortVersionsByDate,
} from './install'
import { messages } from './messages'
import { projectCategories } from './search-engine'
import { isFavoriteContentType, makeContentSelectionKey, useDiscover } from './state'
import type { GameVersionTag } from './versions'

/**
 * The project page (apps/app-frontend/src/pages/project/Index.vue) for a Modrinth project of
 * the snapshot: the sidebar, the install context header, the project header with its
 * install/translate/more actions, the tabs and the tab body for the child route.
 */
const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const route = useRoute()
const router = useRouter()
const preview = usePreview()
const homeRuntime = provideHomeRuntime(useHomeState())
const discover = useDiscover()
const selection = discover.selection
const { layer } = injectPreviewWindow()

const contentInstall = createContentInstall({ preview, discover, router })
provideContentInstall(contentInstall)

const loading = ref(true)
const loadFailed = ref(false)
const installing = ref(false)
const browseInstanceSelector = ref<InstanceType<typeof BrowseInstanceSelector>>()
const data = shallowRef<ProjectDetail | null>(null)
const mcmodUrl = ref<string | null>(null)
const versions = shallowRef<ProjectVersion[]>([])
const instance = ref<GameInstance | null>(null)
const installed = ref(false)
const installedVersion = ref<string | null>(null)
const gameVersions = shallowRef<GameVersionTag[]>([])
const translationActive = ref(false)
const translationLoading = ref(false)
let translationRequestVersion = 0
let cancelTranslation: (() => void) | null = null

const favoriteSupported = computed(() => isFavoriteContentType(data.value?.project_type ?? ''))
const favoritePending = computed(() =>
    data.value ? discover.isFavoritePending('modrinth', data.value.id) : false,
)
const favoriteSaved = computed(() =>
    data.value ? discover.isFavorite('modrinth', data.value.id) : false,
)

async function toggleFavorite() {
    const project = data.value
    if (!project || !favoriteSupported.value || favoritePending.value) return
    if (!isFavoriteContentType(project.project_type)) return
    await discover.toggleFavorite({
        provider: 'modrinth',
        project_id: project.id,
        content_type: project.project_type,
        slug: project.slug,
    })
}

/** helpers/project-links.ts `createProjectBrowseLocation`. */
function browseByProjectFilter(filter: 'category' | 'loader', value: string) {
    const projectType = data.value?.project_type
    if (!projectType) return
    void router.push({
        path: `/browse/${projectType}`,
        query: filter === 'loader' ? { g: `categories:${value}` } : { f: `categories:${value}` },
    })
}

const instanceFilters = computed(() => {
    if (!instance.value || !data.value) return {}
    const loaders: string[] = []
    if (data.value.project_type === 'mod') {
        if (instance.value.loader !== 'vanilla') loaders.push(instance.value.loader)
        if (instance.value.loader === 'vanilla' || data.value.loaders.includes('datapack'))
            loaders.push('datapack')
    }
    return {
        l: loaders,
        g: usesTargetGameVersion(data.value.project_type) ? instance.value.game_version : undefined,
    }
})

function buildProjectHref(path: string, extraQuery: Record<string, unknown> = {}) {
    const params = new URLSearchParams()
    for (const [key, value] of Object.entries({ ...route.query, ...extraQuery })) {
        if (Array.isArray(value)) {
            for (const entry of value) if (entry != null) params.append(key, String(entry))
        } else if (value) {
            params.append(key, String(value))
        }
    }
    const query = params.toString()
    return query ? `${path}?${query}` : path
}

function buildBrowseHref(path: string) {
    const params = new URLSearchParams()
    for (const [key, value] of Object.entries(route.query)) {
        if (key === 'b') continue
        if (Array.isArray(value)) {
            for (const entry of value) if (entry != null) params.append(key, entry)
        } else if (value) {
            params.append(key, String(value))
        }
    }
    const query = params.toString()
    return query ? `${path}?${query}` : path
}

const projectDescriptionHref = computed(() => buildProjectHref(`/project/${route.params.id}`))
const versionsHref = computed(() =>
    buildProjectHref(`/project/${route.params.id}/versions`, instanceFilters.value),
)
const projectChangelogHref = computed(() =>
    buildProjectHref(`/project/${route.params.id}/changelog`),
)
const projectGalleryHref = computed(() => buildProjectHref(`/project/${route.params.id}/gallery`))

const instanceContentBackUrl = computed(() => {
    if (route.query.from !== 'instance-content' || typeof route.query.i !== 'string') return null
    if (instance.value?.id !== route.query.i) return null
    return `/instance/${encodeURIComponent(route.query.i)}`
})
const projectBrowseBackUrl = computed(() => {
    if (instanceContentBackUrl.value) return instanceContentBackUrl.value
    const browsePath = route.query.b
    if (typeof browsePath === 'string' && browsePath.startsWith('/browse/')) return browsePath
    return buildBrowseHref(`/browse/${data.value?.project_type ?? 'mod'}`)
})
const projectBackLabel = computed(() =>
    formatMessage(
        instanceContentBackUrl.value ? messages.backToInstanceContent : messages.backToBrowse,
    ),
)
const fromBrowse = computed(
    () => typeof route.query.b === 'string' && route.query.b.startsWith('/browse/'),
)
const cartEligible = computed(
    () =>
        fromBrowse.value &&
        !!data.value &&
        ['mod', 'resourcepack', 'datapack', 'shader'].includes(data.value.project_type),
)
const cartTarget = computed(() => selection.targetInstance.value)
const cartProjectKey = computed(() =>
    data.value ? makeContentSelectionKey('modrinth', data.value.id) : '',
)
const cartProjectSelected = computed(
    () => !!cartProjectKey.value && selection.isSelected(cartProjectKey.value),
)

function installSelected() {
    return selection.installSelected(() =>
        addNotification({ title: formatMessage(messages.selectionTargetChanged), type: 'error' }),
    )
}

const projectInstallContext = computed<BrowseInstallContext | null>(() => {
    if (cartEligible.value && cartTarget.value) {
        const target = cartTarget.value
        const icon = displayInstanceIcon(target)
        return {
            showInstallHeader: !!instance.value,
            name: target.name,
            loader: target.loader,
            gameVersion: target.game_version,
            iconSrc: icon.url,
            iconFrameless: icon.frameless,
            backUrl: projectBrowseBackUrl.value,
            backLabel: projectBackLabel.value,
            heading: formatMessage(messages.installContentToInstance),
            selectedProjects: selection.selectedProjects.value,
            isInstallingSelected: ['validating', 'reviewing', 'queueing'].includes(
                selection.state.value,
            ),
            installProgress: selection.progress.value,
            clearSelected: selection.clear,
            installSelected,
        }
    }
    if (instance.value) {
        const icon = displayInstanceIcon(instance.value)
        return {
            name: instance.value.name,
            loader: instance.value.loader,
            gameVersion: instance.value.game_version,
            iconSrc: icon.url,
            iconFrameless: icon.frameless,
            backUrl: projectBrowseBackUrl.value,
            backLabel: projectBackLabel.value,
            heading: formatMessage(messages.installContentToInstance),
        }
    }
    return null
})

const installButtonLoading = computed(
    () =>
        installing.value ||
        (cartProjectKey.value ? selection.isInstalling(cartProjectKey.value) : false),
)
const installButtonInstalled = computed(
    () =>
        installed.value ||
        (!!data.value &&
            (!route.query.i || cartTarget.value?.id === route.query.i) &&
            selection.isInstalledIdentity('modrinth', data.value.id)),
)
const installButtonDisabled = computed(
    () => installButtonInstalled.value || installButtonLoading.value,
)
const installButtonLabel = computed(() => {
    if (installButtonInstalled.value) return formatMessage(commonMessages.installedLabel)
    if (installButtonLoading.value) return formatMessage(commonMessages.installingLabel)
    if (cartProjectSelected.value) return formatMessage(commonMessages.selectedLabel)
    return formatMessage(commonMessages.installButton)
})
const installButtonTooltip = computed(() =>
    installButtonInstalled.value ? formatMessage(messages.alreadyInstalled) : null,
)

const showSwitchVersion = computed(() => !!instance.value && installed.value)
const onVersionsPage = computed(() => route.name === 'Versions')

function getDependentSearchTypes() {
    const project = data.value
    if (!project) return []
    if (project.project_type !== 'mod') return [project.project_type]
    const loaders = project.loaders ?? []
    const types: string[] = []
    if (loaders.some((loader) => ['fabric', 'forge', 'neoforge', 'quilt'].includes(loader)))
        types.push('mod')
    if (loaders.some((loader) => ['paper', 'purpur', 'spigot', 'folia'].includes(loader)))
        types.push('plugin')
    if (loaders.includes('datapack')) types.push('datapack')
    return types.length ? types : ['mod']
}

function getDependentSearchActions() {
    const project = data.value
    if (!project) return []
    const types = getDependentSearchTypes()
    const dependency = encodeURIComponent(
        formatDependencyProjectFilterOption(project.id, ['required']),
    )
    return [
        ...types.map((projectType) => ({
            id: formatMessage(
                types.length === 1 ? messages.viewDependents : messages.viewProjectTypeDependents,
                {
                    projectType: formatProjectTypeSentence(formatMessage, projectType),
                },
            ),
            icon: SearchIcon,
            link: `/browse/${projectType}?dep=${dependency}`,
        })),
        ...(project.project_type !== 'modpack' && types.length !== 1
            ? [
                  {
                      id: formatMessage(messages.viewModpacks),
                      icon: PackageIcon,
                      link: `/browse/modpack?dep=${dependency}`,
                  },
              ]
            : []),
    ]
}

const overflowOptions = computed(() => {
    const project = data.value
    if (!project) return []
    return [
        {
            id: 'follow',
            disabled: true,
            tooltip: formatMessage(messages.comingSoon),
            action: () => {},
        },
        ...(favoriteSupported.value
            ? [
                  {
                      id: 'save',
                      disabled: favoritePending.value,
                      tooltip: formatMessage(
                          favoriteSaved.value
                              ? messages.removeFromFavorites
                              : messages.addToFavorites,
                      ),
                      action: () => void toggleFavorite(),
                  },
              ]
            : []),
        ...getDependentSearchActions(),
        {
            id: 'open-in-browser',
            link: `https://modrinth.com/${project.project_type}/${project.slug}`,
            external: true,
        },
        ...(mcmodUrl.value ? [{ id: 'open-in-mcmod', link: mcmodUrl.value, external: true }] : []),
        { divider: true },
        {
            id: 'report',
            color: 'red',
            hoverFilled: true,
            link: `https://modrinth.com/report?item=project&itemID=${project.id}`,
        },
    ]
})

function goToVersions() {
    void router.push(versionsHref.value)
}

const headerProject = computed(() =>
    data.value ? { ...data.value, categories: projectCategories(data.value) } : null,
)

// Loading ---------------------------------------------------------------------------------------------
let projectLoadVersion = 0

async function fetchProjectData() {
    const requestedProjectId = String(route.params.id)
    const requestedInstanceId = route.query.i
    const requestVersion = projectLoadVersion
    const isCurrent = () =>
        requestVersion === projectLoadVersion &&
        String(route.params.id) === requestedProjectId &&
        route.query.i === requestedInstanceId
    translationRequestVersion++
    cancelTranslation?.()
    translationActive.value = false
    translationLoading.value = false

    const projectId = await discover.resolveProjectId(requestedProjectId)
    const project = await discover.ensureDetail(projectId)
    if (!isCurrent()) return
    if (!project) {
        throw new Error('Project not found')
    }
    data.value = project
    versions.value = sortVersionsByDate(project.versions)
    mcmodUrl.value = null
    void resolveMcmodUrl(project.slug).then((url) => {
        if (isCurrent()) mcmodUrl.value = url
    })
    if (typeof requestedInstanceId === 'string') {
        instance.value = preview.instances.get(requestedInstanceId) ?? null
        if (instance.value) {
            await preview.downloads.ensureContents()
            if (!isCurrent()) return
            const installedVersionId = discover.installedVersionId(instance.value.id, project.id)
            if (discover.installedProjectIds(instance.value.id).has(project.id)) {
                installed.value = true
                installedVersion.value = installedVersionId
            }
        }
    }
    preview.breadcrumbs.setName('Project', project.title)
    preview.breadcrumbs.setNameIcon('Project', projectIcon(project.id))
}

function syncContentSelectionTarget() {
    if (!fromBrowse.value && !route.query.i) return
    selection.refreshInstances(instance.value?.id ?? null)
}

async function loadProjectPage() {
    const loadVersion = ++projectLoadVersion
    loading.value = true
    loadFailed.value = false
    data.value = null
    versions.value = []
    instance.value = null
    installed.value = false
    installedVersion.value = null
    try {
        const tags = await discover.ensureTags()
        gameVersions.value = tags.gameVersions as GameVersionTag[]
        await fetchProjectData()
        if (loadVersion !== projectLoadVersion) return
        syncContentSelectionTarget()
    } catch (error) {
        if (loadVersion !== projectLoadVersion) return
        if (data.value) handleError(error)
        else loadFailed.value = true
    } finally {
        if (loadVersion === projectLoadVersion) loading.value = false
    }
}

void loadProjectPage()

watch([() => route.params.id, () => route.query.i], () => {
    if (route.params.id && route.path.startsWith('/project')) void loadProjectPage()
})

/**
 * The app's breadcrumb store keeps `?Project` while the trail has no crumbs (Changelog); the
 * window's store drops it, so the page names itself again when its tab changes.
 */
watch(
    () => route.name,
    () => {
        if (!data.value) return
        preview.breadcrumbs.setName('Project', data.value.title)
        preview.breadcrumbs.setNameIcon('Project', projectIcon(data.value.id))
    },
)

// Translation -------------------------------------------------------------------------------------------
/**
 * Translating sends the project text to the app's translation service. The window has no
 * route to it, so a request ends the way the app reports an unreachable service.
 */
function translateProject() {
    if (!data.value || translationLoading.value) return
    const requestVersion = ++translationRequestVersion
    const previousTranslationActive = translationActive.value
    translationLoading.value = true
    translationActive.value = true
    cancelTranslation = preview.clock.after(1400, () => {
        cancelTranslation = null
        if (requestVersion !== translationRequestVersion) return
        translationActive.value = previousTranslationActive
        translationLoading.value = false
        addNotification({
            title: formatMessage(messages.translationFailedTitle),
            text: formatMessage(messages.translationNetworkFailed),
            type: 'error',
        })
    })
}

function toggleTranslation() {
    if (translationActive.value && !translationLoading.value) {
        translationRequestVersion++
        translationActive.value = false
        translationLoading.value = false
        return
    }
    if (discover.noteManualTranslateClick()) {
        addNotification({
            title: formatMessage(messages.autoTranslateHintTitle),
            text: formatMessage(messages.autoTranslateHintText),
            type: 'info',
        })
    }
    translateProject()
}

// Installing -------------------------------------------------------------------------------------------
async function install(version: string | null) {
    const project = data.value
    if (!project) return
    if (project.minecraft_java_server) {
        const current = preview.instances.instances.value.find(
            (entry) => instanceServerProjectId(entry.link) === project.id,
        )
        const target =
            current ??
            preview.instances.create({
                name: project.title,
                gameVersion:
                    project.minecraft_java_server.content.recommended_game_version ??
                    project.game_versions[0],
                loader: 'vanilla',
                loaderVersion: null,
                iconPath: projectIcon(project.id),
                link: { type: 'server_project', project_id: project.id },
            })
        void homeRuntime.startJoinServer(target, project.minecraft_java_server.address)
        await router.push(`/instance/${encodeURIComponent(target.id)}`)
        return
    }
    if (cartEligible.value && !cartTarget.value) {
        selection.refreshInstances()
        browseInstanceSelector.value?.show()
        return
    }
    if (cartEligible.value && cartTarget.value) {
        if (cartProjectSelected.value) {
            selection.remove(cartProjectKey.value)
            return
        }
        installing.value = true
        try {
            const target = cartTarget.value
            const preferences = getTargetInstallPreferences(
                { gameVersion: target.game_version, loader: target.loader },
                project.project_type,
            )
            const selectedVersion =
                version ??
                (
                    getLatestMatchingInstallVersion(
                        versions.value as unknown as Parameters<
                            typeof getLatestMatchingInstallVersion
                        >[0],
                        preferences,
                    ) as unknown as ProjectVersion | undefined
                )?.id
            if (!selectedVersion)
                throw new Error(formatMessage(messages.projectNoCompatibleVersion))
            selection.add(
                {
                    key: cartProjectKey.value,
                    provider: 'modrinth',
                    projectId: project.id,
                    providerProjectId: project.id,
                    versionId: selectedVersion,
                    contentType: project.project_type as
                        'mod' | 'resourcepack' | 'datapack' | 'shader',
                    title: project.title,
                    iconUrl: projectIcon(project.id),
                    slug: project.slug,
                    preferences,
                },
                (title) =>
                    addNotification({
                        title: formatMessage(messages.selectionDuplicate, { project: title }),
                        type: 'error',
                    }),
            )
        } catch (error) {
            handleError(error)
        } finally {
            installing.value = false
        }
        return
    }

    installing.value = true
    await contentInstall
        .install(
            project.id,
            version,
            instance.value ? instance.value.id : null,
            'ProjectPage',
            (installedVersionId, installedProjectIds) => {
                installing.value = false
                const installedIds = installedProjectIds ?? [project.id]
                if (instance.value && installedVersionId && installedIds.includes(project.id)) {
                    installed.value = true
                    installedVersion.value = installedVersionId
                }
            },
            (profile) => {
                void router.push(`/instance/${encodeURIComponent(profile)}`)
            },
        )
        .catch(handleError)
}

async function selectTargetInstance(target: GameInstance) {
    selection.setTarget(target)
}

// Context menu ---------------------------------------------------------------------------------------
const options = ref<InstanceType<typeof PreviewContextMenu>>()

function handleRightClick(event: MouseEvent) {
    options.value?.showMenu(event, data.value, [
        { name: 'install' },
        { type: 'divider' },
        { name: 'open_link' },
        { name: 'copy_link' },
    ])
}

function handleOptionsClick(args: { item: unknown; option: string }) {
    const item = args.item as ProjectDetail
    const url = `https://modrinth.com/${item.project_type}/${item.slug}`
    switch (args.option) {
        case 'install':
            void install(null)
            break
        case 'open_link':
            window.open(url, '_blank', 'noopener,noreferrer')
            break
        case 'copy_link':
            void navigator.clipboard?.writeText(url).catch(() => {})
            break
    }
}

const tabLinks = computed(() => [
    {
        label: formatMessage(commonProjectSettingsMessages.description),
        href: projectDescriptionHref.value,
    },
    {
        label: formatMessage(commonProjectSettingsMessages.versions),
        href: versionsHref.value,
        subpages: ['version'],
        shown: !data.value?.minecraft_server,
    },
    {
        label: formatMessage(commonMessages.changelogLabel),
        href: projectChangelogHref.value,
        shown: !data.value?.minecraft_server,
    },
    {
        label: formatMessage(commonProjectSettingsMessages.gallery),
        href: projectGalleryHref.value,
        shown: (data.value?.gallery.length ?? 0) > 0,
    },
])
</script>

<template>
    <template v-if="loading">
        <Teleport defer to="#sidebar-teleport-target">
            <ProjectPageSkeleton variant="sidebar" />
        </Teleport>
        <ProjectPageSkeleton />
    </template>
    <EmptyState
        v-else-if="loadFailed"
        type="error"
        class="p-6"
        :heading="formatMessage(messages.loadFailed)"
    >
        <template #actions>
            <Button type="colored" color="brand" @click="loadProjectPage">
                {{ formatMessage(commonMessages.retryButton) }}
            </Button>
            <Button type="outlined" @click="router.push(projectBrowseBackUrl)">
                {{ formatMessage(commonMessages.backButton) }}
            </Button>
        </template>
    </EmptyState>
    <div v-else-if="data">
        <Teleport defer to="#sidebar-teleport-target">
            <ProjectSidebar :project="data" :tags="discover.tags.value" :mcmod-url="mcmodUrl" />
        </Teleport>
        <div class="flex flex-col gap-4 p-6">
            <div
                v-if="projectInstallContext && projectInstallContext.showInstallHeader !== false"
                class="sticky top-0 z-20 -mx-6 -mt-6 rounded-tl-[--radius-xl] border-0 border-b border-solid border-surface-5 bg-surface-1 p-3"
            >
                <BrowseInstallHeader :install-context="projectInstallContext" />
            </div>
            <ProjectHeader
                :project="headerProject ?? data"
                :icon-url="projectIcon(data.id)"
                translation-style="weakened"
                @category="(category: string) => browseByProjectFilter('category', category)"
                @contextmenu.prevent.stop="handleRightClick"
            >
                <template #actions>
                    <Button
                        type="quiet"
                        size="xl"
                        :disabled="translationLoading"
                        @click="toggleTranslation"
                        ><SpinnerIcon v-if="translationLoading" class="animate-spin" />
                        <LanguagesIcon v-else />
                        {{
                            formatMessage(
                                translationLoading
                                    ? messages.translating
                                    : translationActive
                                      ? messages.showOriginal
                                      : messages.translateProject,
                            )
                        }}
                    </Button>
                    <Button
                        v-if="data.minecraft_server"
                        type="colored"
                        color="brand"
                        size="xl"
                        @click="install(null)"
                        ><PlayIcon />{{ formatMessage(instancePageMessages.joinServer) }}</Button
                    >
                    <Button
                        v-else-if="showSwitchVersion && onVersionsPage"
                        v-tooltip="installButtonTooltip"
                        size="xl"
                        disabled
                        ><CheckIcon />
                        {{ formatMessage(commonMessages.installedLabel) }}
                    </Button>
                    <Button v-else-if="showSwitchVersion" size="xl" @click="goToVersions"
                        ><SwapIcon />
                        {{ formatMessage(messages.switchVersion) }}
                    </Button>
                    <Button
                        v-else
                        v-tooltip="installButtonTooltip"
                        type="colored"
                        color="brand"
                        size="xl"
                        :disabled="installButtonDisabled"
                        @click="install(null)"
                        ><SpinnerIcon
                            v-if="installButtonLoading && !installButtonInstalled"
                            class="animate-spin"
                        />
                        <DownloadIcon v-else-if="!installButtonInstalled && !cartProjectSelected" />
                        <CheckIcon v-else />
                        {{ installButtonLabel }}
                    </Button>
                    <OverflowMenu
                        :tooltip="formatMessage(commonMessages.moreOptionsButton)"
                        :container="layer ?? undefined"
                        :options="overflowOptions"
                        :aria-label="formatMessage(commonMessages.moreOptionsButton)"
                    >
                        <MoreVerticalIcon aria-hidden="true" />
                        <template #open-in-browser>
                            <ExternalIcon />
                            {{ formatMessage(commonMessages.openInBrowserButton) }}
                        </template>
                        <template #open-in-mcmod>
                            <BookOpenIcon /> {{ formatMessage(messages.openInMcmod) }}
                        </template>
                        <template #follow>
                            <HeartIcon /> {{ formatMessage(commonMessages.followButton) }}
                        </template>
                        <template v-if="favoriteSupported" #save>
                            <BookmarkFilledIcon v-if="favoriteSaved" class="text-brand" />
                            <BookmarkIcon v-else />
                            {{
                                formatMessage(
                                    favoritePending
                                        ? messages.favoritesLoading
                                        : favoriteSaved
                                          ? messages.removeFromFavorites
                                          : messages.addToFavorites,
                                )
                            }}
                        </template>
                        <template #report>
                            <ReportIcon /> {{ formatMessage(commonMessages.reportButton) }}
                        </template>
                    </OverflowMenu>
                </template>
            </ProjectHeader>
            <NavTabs :links="tabLinks" />
            <ProjectDescription v-if="route.name === 'Project'" :project="data" />
            <ProjectVersions
                v-else-if="route.name === 'Versions'"
                :project="data"
                :versions="versions"
                :game-versions="gameVersions"
                :install="install"
                :installed="installed"
                :installing="installing"
                :installed-version="installedVersion"
            />
            <ProjectVersionPage
                v-else-if="route.name === 'Version'"
                :key="String(route.params.version)"
                :project="data"
                :versions="versions"
                :game-versions="gameVersions"
                :install="install"
                :installed="installed"
                :installing="installing"
                :installed-version="installedVersion"
            />
            <ProjectChangelog
                v-else-if="route.name === 'Changelog'"
                :project="data"
                :versions="versions"
            />
            <ProjectGallery v-else-if="route.name === 'Gallery'" :project="data" />
        </div>
        <SelectedProjectsFloatingBar
            v-if="projectInstallContext"
            :install-context="projectInstallContext"
        />
        <BrowseInstanceSelector
            ref="browseInstanceSelector"
            :instances="selection.instances.value"
            :selected-instance="selection.targetInstance.value"
            :selected-count="selection.selectedCount.value"
            :install-current="installSelected"
            :clear-current="selection.clear"
            @select="selectTargetInstance"
        />
        <PreviewContextMenu ref="options" @option-clicked="handleOptionsClick">
            <template #install>
                <DownloadIcon /> {{ formatMessage(commonMessages.installButton) }}
            </template>
            <template #open_link>
                <GlobeIcon /> {{ formatMessage(commonMessages.openInModrinthButton) }}
                <ExternalIcon />
            </template>
            <template #copy_link>
                <ClipboardCopyIcon /> {{ formatMessage(commonMessages.copyLinkButton) }}
            </template>
        </PreviewContextMenu>
    </div>
    <InstallModals />
</template>
