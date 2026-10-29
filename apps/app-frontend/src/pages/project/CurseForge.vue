<template>
    <template v-if="loading">
        <Teleport to="#sidebar-teleport-target">
            <ProjectPageSkeleton variant="sidebar" />
        </Teleport>
        <ProjectPageSkeleton :title="previewTitle" />
    </template>
    <div v-else-if="data">
        <UpgradeProjectReturnBar />
        <Teleport to="#sidebar-teleport-target">
            <ProjectSidebarCompatibility
                :project="data"
                :tags="{ loaders: allLoaders, gameVersions: allGameVersions }"
                :platform-action="(platform) => browseByProjectFilter('loader', platform)"
                class="project-sidebar-section"
            />
            <ProjectSidebarLinks
                link-target="_blank"
                :project="data"
                :mcmod-url="mcmodUrl"
                class="project-sidebar-section"
            />
            <ProjectSidebarTags
                :project="data"
                :tag-action="(tag) => browseByProjectFilter('category', tag)"
                class="project-sidebar-section"
            />
            <ProjectSidebarCreators
                :members="members"
                :org-link="() => data.links.website_url"
                :user-link="(username) => authorLinks[username] ?? data.links.website_url"
                link-target="_blank"
                class="project-sidebar-section"
            />
            <ProjectSidebarDetails
                :project="data"
                :resource-dates="{
                    createdAt: project?.dateCreated,
                    updatedAt: project?.dateModified,
                    latestPublishedAt: project?.dateReleased,
                }"
                :has-versions="versions.length > 0"
                hide-license
                link-target="_blank"
                class="project-sidebar-section"
            />
        </Teleport>

        <div class="flex flex-col gap-4 p-6">
            <Teleport
                v-if="themeStore.featureFlags.project_background"
                to="#background-teleport-target"
            >
                <ProjectBackgroundGradient :project="data" />
            </Teleport>
            <BrowseInstallHeader
                v-if="fromInstanceContent && cartInstallContext"
                :install-context="cartInstallContext"
            />
            <ProjectHeader
                :project="data"
                :show-followers="false"
                :translated-title="translationActive ? translations.title : undefined"
                :translated-description="translationActive ? translations.description : undefined"
                :translation-mode="translationMode"
                :translation-style="translationStyle"
            >
                <template #actions>
                    <Button
                        type="quiet"
                        size="xl"
                        :disabled="translationLoading || !descriptionReady"
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
                        v-if="managedProjectType || isWorldMap"
                        type="colored"
                        color="brand"
                        size="xl"
                        :disabled="installing || cartProjectInstalling || !filesReady"
                        @click="installSelected(null)"
                        ><SpinnerIcon v-if="installing" class="animate-spin" />
                        <PlusIcon v-else-if="isWorldMap || cartProjectSelected" />
                        <DownloadIcon v-else />
                        {{
                            formatMessage(
                                installing || cartProjectInstalling
                                    ? commonMessages.installingLabel
                                    : cartProjectSelected
                                      ? commonMessages.selectedLabel
                                      : isWorldMap
                                        ? instanceId
                                            ? commonMessages.installButton
                                            : messages.addToAnInstance
                                        : commonMessages.installButton,
                            )
                        }}
                    </Button>
                    <OverflowMenu
                        :tooltip="formatMessage(commonMessages.moreOptionsButton)"
                        :options="[
                            ...(favoriteSupported
                                ? [
                                      {
                                          id: 'save',
                                          disabled: favoritePending,
                                          tooltip: formatMessage(
                                              favoriteSaved
                                                  ? messages.removeFromFavorites
                                                  : messages.addToFavorites,
                                          ),
                                          action: () => toggleFavorite(),
                                      },
                                  ]
                                : []),
                            ...(data.site_url
                                ? [
                                      {
                                          id: 'open-in-browser',
                                          link: data.site_url,
                                          external: true,
                                      },
                                  ]
                                : []),
                            ...(mcmodUrl
                                ? [
                                      {
                                          id: 'open-in-mcmod',
                                          link: mcmodUrl,
                                          external: true,
                                      },
                                  ]
                                : []),
                        ]"
                        :aria-label="formatMessage(commonMessages.moreOptionsButton)"
                    >
                        <MoreVerticalIcon aria-hidden="true" />
                        <template #open-in-browser>
                            <ExternalIcon /> {{ formatMessage(commonMessages.openInBrowserButton) }}
                        </template>
                        <template #open-in-mcmod>
                            <BookOpenIcon /> {{ formatMessage(messages.openInMcmod) }}
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
                    </OverflowMenu>
                </template>
            </ProjectHeader>
            <SelectedProjectsFloatingBar
                v-if="cartInstallContext"
                :install-context="cartInstallContext"
            />
            <BrowseInstanceSelector
                ref="browseInstanceSelector"
                :instances="contentSelection.instances.value"
                :selected-instance="contentSelection.targetInstance.value"
                :selected-count="contentSelection.selectedCount.value"
                :install-current="contentSelection.installSelected"
                :clear-current="contentSelection.clear"
                @select="contentSelection.setTarget"
            />

            <div v-if="filesFailed || tagsFailed" role="alert" class="flex items-center gap-3">
                <span>{{ formatMessage(messages.additionalDetailsFailed) }}</span>
                <Button @click="retrySupplementalData">{{
                    formatMessage(commonMessages.retryButton)
                }}</Button>
            </div>
            <NavTabs
                :links="[
                    {
                        label: formatMessage(messages.description),
                        href: projectDescriptionHref,
                    },
                    {
                        label: formatMessage(messages.versions),
                        href: projectVersionsHref,
                    },
                    {
                        label: formatMessage(commonMessages.changelogLabel),
                        href: projectChangelogHref,
                    },
                    {
                        label: formatMessage(messages.gallery),
                        href: projectGalleryHref,
                        shown: data.gallery.length > 0,
                    },
                ]"
            />

            <Changelog v-if="activeTab === 'changelog'" :project="data" source="curseforge" />
            <Gallery
                v-else-if="activeTab === 'gallery'"
                :project="data"
                :translation-active="translationActive"
                :translations="translations"
                :translation-mode="translationMode"
                :translation-style="translationStyle"
            />
            <ProjectPageSkeleton
                v-else-if="activeTab === 'versions' && versionsLoading"
                variant="versions"
            />
            <ProjectPageVersions
                v-else-if="activeTab === 'versions' && !filesFailed"
                :loaders="allLoaders"
                :game-versions="allGameVersions"
                :versions="versions"
                :project="data"
                :show-environment-column="themeStore.featureFlags.show_version_environment_column"
            >
                <template #actions="{ version }">
                    <Button
                        v-tooltip="
                            formatMessage(
                                isWorldMap
                                    ? messages.addToAnInstance
                                    : commonMessages.installButton,
                            )
                        "
                        circular
                        icon-only
                        type="quiet"
                        :color="isWorldMap ? 'brand' : 'green'"
                        :disabled="installing"
                        @click.stop="installSelected(version.id)"
                    >
                        <PlusIcon v-if="isWorldMap" />
                        <DownloadIcon v-else />
                    </Button>
                </template>
            </ProjectPageVersions>
            <Card v-else-if="activeTab === 'description'">
                <ResourceDescription source="curseforge" :project-id="String(data.id)">
                    <ProjectPageSkeleton v-if="descriptionLoading" variant="description" />
                    <div v-else-if="descriptionFailed" role="alert" class="flex items-center gap-3">
                        <span>{{ formatMessage(messages.descriptionFailed) }}</span>
                        <Button
                            @click="loadDescription(Number(route.params.id), projectRequestVersion)"
                            >{{ formatMessage(commonMessages.retryButton) }}</Button
                        >
                    </div>
                    <TranslatedProjectDescription
                        v-else-if="data.body"
                        :description="data.body"
                        :active="translationActive"
                        :translations="translations"
                        :mode="translationMode"
                        :style="translationStyle"
                        format="html"
                    />
                    <p v-else class="m-0">{{ data.description }}</p>
                </ResourceDescription>
            </Card>
        </div>
    </div>
    <div v-else class="p-6">
        <Card>
            <h2>{{ formatMessage(messages.unavailableTitle) }}</h2>
            <p>{{ formatMessage(messages.unavailableDescription) }}</p>
            <Button @click="loadProject(Number(route.params.id))">{{
                formatMessage(commonMessages.retryButton)
            }}</Button>
        </Card>
    </div>
</template>

<script setup lang="ts">
import {
    BookmarkFilledIcon,
    BookmarkIcon,
    BookOpenIcon,
    DownloadIcon,
    ExternalIcon,
    LanguagesIcon,
    MoreVerticalIcon,
    PlusIcon,
    SpinnerIcon,
} from '@modrinth/assets'
import {
    BrowseInstallHeader,
    Button,
    Card,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    NavTabs,
    OverflowMenu,
    ProjectBackgroundGradient,
    ProjectHeader,
    ProjectPageSkeleton,
    ProjectPageVersions,
    ProjectSidebarCompatibility,
    ProjectSidebarCreators,
    ProjectSidebarDetails,
    ProjectSidebarLinks,
    ProjectSidebarTags,
    SelectedProjectsFloatingBar,
    usesTargetGameVersion,
    useVIntl,
} from '@modrinth/ui'
import { computed, onScopeDispose, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import BrowseInstanceSelector from '@/components/browse/BrowseInstanceSelector.vue'
import ResourceDescription from '@/components/ui/ResourceDescription.vue'
import TranslatedProjectDescription from '@/components/ui/TranslatedProjectDescription.vue'
import { useContentFavorites } from '@/composables/useContentFavorites'
import { isFavoriteContentType } from '@/helpers/content-favorites'
import { resolveMcmodUrl } from '@/helpers/content-search'
import {
    type CurseForgeFile,
    type CurseForgeProject,
    getCurseForgeDescription,
    getCurseForgeFiles,
    getCurseForgeFilesPage,
    getCurseForgeImageUrl,
    getCurseForgeProject,
} from '@/helpers/curseforge'
import { runAfterPageTransitionSettle } from '@/helpers/page-transition'
import { projectGalleryTranslationSegments } from '@/helpers/project-gallery'
import { createProjectBrowseLocation, type ProjectBrowseFilter } from '@/helpers/project-links'
import { readProjectPreviewTitle } from '@/helpers/project-preview'
import { get_game_versions, get_loaders } from '@/helpers/tags'
import {
    autoTranslateHintMessages,
    getTranslationErrorKind,
    getTranslationSettings,
    noteManualTranslateClick,
    prepareDescription,
    translateInBatches as translateContent,
    type TranslationStyle,
    validateTranslatedDescription,
} from '@/helpers/translation'
import i18n from '@/i18n.config'
import { injectContentInstall } from '@/providers/content-install'
import { curseForgeLoaderType } from '@/providers/content-install-compat'
import { injectContentSelection, makeContentSelectionKey } from '@/providers/content-selection'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { useTheming } from '@/store/state.js'

import Changelog from './Changelog.vue'
import Gallery from './Gallery.vue'
import UpgradeProjectReturnBar from './UpgradeProjectReturnBar.vue'

const route = useRoute()
const previewTitle = computed(() => readProjectPreviewTitle(route.path))
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
const themeStore = useTheming()
const { addNotification, handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { installCurseForge, installCurseForgeWorld } = injectContentInstall()
const contentSelection = injectContentSelection()
const contentFavorites = useContentFavorites()

void contentFavorites.load().catch(handleError)

const messages = defineMessages({
    additionalDetailsFailed: {
        id: 'app.project.additional-details-failed',
        defaultMessage: 'Some project details could not be loaded.',
    },
    descriptionFailed: {
        id: 'app.resources.description.failed',
        defaultMessage: 'The description could not be loaded.',
    },
    openInMcmod: {
        id: 'app.project.open-in-mcmod',
        defaultMessage: 'Open in MC Mod',
    },
    description: {
        id: 'project.description.title',
        defaultMessage: 'Description',
    },
    versions: {
        id: 'project.versions.title',
        defaultMessage: 'Versions',
    },
    gallery: {
        id: 'project.gallery.title',
        defaultMessage: 'Gallery',
    },
    translateProject: {
        id: 'app.project.translation.translate',
        defaultMessage: 'Translate',
    },
    showOriginal: {
        id: 'app.project.translation.show-original',
        defaultMessage: 'Show original',
    },
    translating: {
        id: 'app.project.translation.translating',
        defaultMessage: 'Translating…',
    },
    translationFailed: {
        id: 'app.project.translation.failed',
        defaultMessage: 'Translation failed. The original content was kept. Try again.',
    },
    translationFailedTitle: {
        id: 'app.project.translation.failed-title',
        defaultMessage: 'Translation failed',
    },
    translationRateLimited: {
        id: 'app.translation.error.rate-limited',
        defaultMessage:
            'The translation service is temporarily rate limited. Please try again later.',
    },
    translationAuthenticationFailed: {
        id: 'app.translation.error.authentication',
        defaultMessage: 'The translation service could not authenticate. Please try again later.',
    },
    translationContentTooLong: {
        id: 'app.translation.error.content-too-long',
        defaultMessage: 'This content is too long for the selected translation service.',
    },
    translationNetworkFailed: {
        id: 'app.translation.error.network',
        defaultMessage:
            'The translation service could not be reached. Check your network or proxy.',
    },
    unavailableTitle: {
        id: 'app.project.curseforge.unavailable-title',
        defaultMessage: 'Project unavailable',
    },
    unavailableDescription: {
        id: 'app.project.curseforge.unavailable-description',
        defaultMessage: 'The CurseForge project did not return any data.',
    },
    addToAnInstance: {
        id: 'app.browse.add-to-an-instance',
        defaultMessage: 'Add to an instance',
    },
    noCompatibleVersion: {
        id: 'app.project.install-button.no-compatible-version',
        defaultMessage: 'No compatible version was found for this instance.',
    },
    backToInstanceContent: {
        id: 'app.project.install-context.back-to-instance-content',
        defaultMessage: 'Back to instance content',
    },
    addToFavorites: {
        id: 'app.content-favorites.add',
        defaultMessage: 'Add to favorites',
    },
    removeFromFavorites: {
        id: 'app.content-favorites.remove',
        defaultMessage: 'Remove from favorites',
    },
    favoritesLoading: {
        id: 'app.content-favorites.loading',
        defaultMessage: 'Updating favorites…',
    },
})

const loading = ref(true)
const versionsLoading = ref(false)
const filesComplete = ref(false)
const installing = ref(false)
const browseInstanceSelector = ref()
const project = shallowRef<CurseForgeProject | null>(null)
const mcmodUrl = ref<string | null>(null)
const description = ref('')
const descriptionLoading = ref(false)
const descriptionReady = ref(false)
const descriptionFailed = ref(false)
const filesReady = ref(false)
const filesFailed = ref(false)
const tagsFailed = ref(false)
const files = shallowRef<CurseForgeFile[]>([])
const allLoaders = ref([])
const allGameVersions = ref([])
const translationActive = ref(false)
const translationLoading = ref(false)
const translations = ref<Record<string, string>>({})
const translationMode = ref<'bilingual' | 'translation-only'>('bilingual')
const translationStyle = ref<TranslationStyle>('weakened')
let projectRequestVersion = 0
let filesRequestVersion = 0
let translationRequestVersion = 0

const projectType = computed(() => {
    switch (project.value?.classId) {
        case 5:
            return 'plugin'
        case 6:
            return 'mod'
        case 12:
            return 'resourcepack'
        case 17:
            return 'world'
        case 6945:
            return 'datapack'
        case 4471:
            return 'modpack'
        case 6552:
            return 'shader'
        default:
            return 'mod'
    }
})

const favoriteSupported = computed(() => isFavoriteContentType(projectType.value))
const favoriteProjectId = computed(() => project.value?.id.toString() ?? '')
const favoriteSaved = computed(() =>
    favoriteProjectId.value
        ? contentFavorites.isFavorite('curseforge', favoriteProjectId.value)
        : false,
)
const favoritePending = computed(() =>
    favoriteProjectId.value
        ? contentFavorites.isPending('curseforge', favoriteProjectId.value)
        : false,
)

function toggleFavorite() {
    if (!favoriteSupported.value || !favoriteProjectId.value || favoritePending.value) return
    void contentFavorites
        .toggle({
            provider: 'curseforge',
            project_id: favoriteProjectId.value,
            content_type: projectType.value,
            slug: project.value?.slug,
        })
        .catch(handleError)
}

const managedProjectType = computed(() =>
    ['mod', 'resourcepack', 'shader', 'datapack', 'modpack'].includes(projectType.value),
)
const isWorldMap = computed(() => projectType.value === 'world')
const instanceId = computed(() => (typeof route.query.i === 'string' ? route.query.i : null))
const fromBrowse = computed(
    () => typeof route.query.b === 'string' && route.query.b.startsWith('/browse/'),
)
const fromInstanceContent = computed(
    () => route.query.from === 'instance-content' && instanceId.value !== null,
)
const instanceContentTarget = computed(() => {
    if (!fromInstanceContent.value) return null
    const target = contentSelection.targetInstance.value
    return target?.id === instanceId.value ? target : null
})
const instanceContentBackUrl = computed(() =>
    instanceContentTarget.value ? `/instance/${encodeURIComponent(instanceId.value!)}` : null,
)
const cartEligible = computed(
    () =>
        (fromBrowse.value || instanceContentTarget.value !== null) &&
        ['mod', 'resourcepack', 'shader', 'datapack', 'world'].includes(projectType.value),
)
const cartProjectKey = computed(() =>
    project.value ? makeContentSelectionKey('curseforge', project.value.id.toString()) : '',
)
const cartProjectSelected = computed(
    () => !!cartProjectKey.value && contentSelection.isSelected(cartProjectKey.value),
)
const cartProjectInstalling = computed(
    () => !!cartProjectKey.value && contentSelection.isInstalling(cartProjectKey.value),
)
const cartInstallContext = computed(() => {
    const target = contentSelection.targetInstance.value
    if (!cartEligible.value || !target) return null
    return {
        showInstallHeader: false,
        name: target.name,
        loader: target.loader,
        gameVersion: target.game_version,
        backUrl:
            instanceContentBackUrl.value ??
            (typeof route.query.b === 'string' ? route.query.b : `/browse/${projectType.value}`),
        backLabel: fromInstanceContent.value ? formatMessage(messages.backToInstanceContent) : '',
        heading: '',
        selectedProjects: contentSelection.selectedProjects.value,
        isInstallingSelected: ['validating', 'reviewing', 'queueing'].includes(
            contentSelection.state.value,
        ),
        installProgress: contentSelection.progress.value,
        clearSelected: contentSelection.clear,
        installSelected: contentSelection.installSelected,
    }
})

const platformNames = [
    'forge',
    'fabric',
    'quilt',
    'neoforge',
    'liteloader',
    'rift',
    'iris',
    'optifine',
]
const loaderTypes: Record<number, string> = { 1: 'forge', 4: 'fabric', 5: 'quilt', 6: 'neoforge' }

function getFilePlatforms(file: CurseForgeFile) {
    const platforms = file.gameVersions
        .map((version) => version.toLowerCase().replaceAll(' ', ''))
        .filter((version) => platformNames.includes(version))

    if (platforms.length === 0 && projectType.value === 'resourcepack') {
        return ['minecraft']
    }

    return platforms
}

const projectLoaders = computed(() => {
    const loaders = new Set<string>()
    for (const file of files.value) {
        for (const platform of getFilePlatforms(file)) loaders.add(platform)
    }
    for (const index of project.value?.latestFilesIndexes ?? []) {
        if (index.modLoader && loaderTypes[index.modLoader])
            loaders.add(loaderTypes[index.modLoader])
    }
    return [...loaders]
})

const minecraftVersions = computed(() => {
    const versions = new Set<string>()
    for (const file of files.value) {
        for (const version of file.gameVersions) {
            if (/^\d+\.\d+/.test(version)) versions.add(version)
        }
    }
    for (const index of project.value?.latestFilesIndexes ?? []) {
        if (/^\d+\.\d+/.test(index.gameVersion)) versions.add(index.gameVersion)
    }
    return [...versions]
})

const data = computed(() => {
    if (!project.value) return null
    const value = project.value
    return {
        id: value.id.toString(),
        slug: value.slug,
        title: value.name,
        description: value.summary,
        body: description.value,
        project_type: projectType.value,
        downloads: value.downloadCount,
        followers: 0,
        icon_url: getCurseForgeImageUrl(value.logo?.url ?? value.logo?.thumbnailUrl),
        color: null,
        status: 'approved',
        categories: value.categories.map((category) => category.slug),
        additional_categories: [],
        versions: files.value.map((file) => file.id.toString()),
        game_versions: minecraftVersions.value,
        loaders: projectLoaders.value,
        client_side: 'unknown',
        server_side: 'unknown',
        published: value.dateCreated,
        approved: value.dateReleased || value.dateCreated,
        updated: value.dateModified,
        queued: null,
        license: { id: 'LicenseRef-Unknown', name: 'Unknown', url: null },
        issues_url: value.links.issuesUrl ?? '',
        source_url: value.links.sourceUrl ?? '',
        wiki_url: value.links.wikiUrl ?? '',
        discord_url: '',
        site_url: value.links.websiteUrl ?? '',
        donation_urls: [],
        links: {
            website_url: value.links.websiteUrl ?? '',
        },
        gallery: value.screenshots.map((screenshot) => ({
            title: screenshot.title,
            description: '',
            created: value.dateModified,
            url: getCurseForgeImageUrl(screenshot.thumbnailUrl, 960),
            raw_url: getCurseForgeImageUrl(screenshot.url, 1920),
            featured: false,
        })),
    }
})

function browseByProjectFilter(filter: ProjectBrowseFilter, value: string) {
    if (!data.value?.project_type) return
    void router.push(createProjectBrowseLocation(data.value.project_type, filter, value))
}

const members = computed(() =>
    (project.value?.authors ?? []).map((author, index) => ({
        id: author.id.toString(),
        role: index === 0 ? 'Owner' : 'Author',
        is_owner: index === 0,
        accepted: true,
        user: {
            id: author.id.toString(),
            username: author.name,
            avatar_url: '',
        },
    })),
)

const authorLinks = computed(() =>
    Object.fromEntries((project.value?.authors ?? []).map((author) => [author.name, author.url])),
)

const versions = computed(() =>
    files.value.map((file) => {
        const loaders = getFilePlatforms(file)
        const gameVersions = file.gameVersions.filter((version) => /^\d+\.\d+/.test(version))
        return {
            id: file.id.toString(),
            project_id: project.value?.id.toString() ?? '',
            name: file.displayName,
            version_number: file.displayName,
            version_type:
                file.releaseType === 1 ? 'release' : file.releaseType === 2 ? 'beta' : 'alpha',
            date_published: file.fileDate,
            downloads: file.downloadCount,
            game_versions: gameVersions,
            loaders: loaders.length ? loaders : projectLoaders.value,
            files: [
                {
                    filename: file.fileName,
                    size: file.fileLength,
                    url: file.downloadUrl ?? '',
                    primary: true,
                    hashes: {},
                },
            ],
            featured: false,
            status: 'listed',
            changelog: '',
            dependencies: [],
            displayUrlEnding: file.id.toString(),
        }
    }),
)

const activeTab = computed(() => {
    if (route.path.endsWith('/changelog')) return 'changelog'
    if (route.path.endsWith('/versions')) return 'versions'
    if (route.path.endsWith('/gallery')) return 'gallery'
    return 'description'
})

function buildProjectHref(path: string) {
    const params = new URLSearchParams()
    for (const [key, value] of Object.entries(route.query)) {
        if (Array.isArray(value)) {
            for (const item of value) if (item) params.append(key, String(item))
        } else if (value) {
            params.append(key, String(value))
        }
    }
    const query = params.toString()
    return query ? `${path}?${query}` : path
}

const projectDescriptionHref = computed(() =>
    buildProjectHref(`/project/curseforge/${route.params.id}`),
)
const projectVersionsHref = computed(() =>
    buildProjectHref(`/project/curseforge/${route.params.id}/versions`),
)
const projectChangelogHref = computed(() =>
    buildProjectHref(`/project/curseforge/${route.params.id}/changelog`),
)
const projectGalleryHref = computed(() =>
    buildProjectHref(`/project/curseforge/${route.params.id}/gallery`),
)

async function loadProject(projectId: number) {
    const requestVersion = ++projectRequestVersion
    filesRequestVersion++
    translationRequestVersion++
    translationActive.value = false
    translationLoading.value = false
    translations.value = {}
    loading.value = true
    versionsLoading.value = true
    descriptionLoading.value = true
    filesComplete.value = false
    project.value = null
    mcmodUrl.value = null
    description.value = ''
    files.value = []
    allLoaders.value = []
    allGameVersions.value = []

    filesReady.value = false
    filesFailed.value = false
    tagsFailed.value = false
    descriptionReady.value = false
    const projectRequest = getCurseForgeProject(projectId)
    void loadDescription(projectId, requestVersion)
    void loadInitialFiles(projectId, requestVersion)
    void loadProjectTags(requestVersion)
    try {
        const projectData = await projectRequest
        if (requestVersion !== projectRequestVersion) return
        project.value = projectData
        breadcrumbs.setName('Project', projectData.name)
        breadcrumbs.setNameIcon(
            'Project',
            projectData.logo?.thumbnailUrl ?? projectData.logo?.url ?? null,
        )
        void resolveMcmodUrl(projectData.slug, 'curseforge').then((url) => {
            if (requestVersion === projectRequestVersion) mcmodUrl.value = url
        })
    } catch (error) {
        if (requestVersion === projectRequestVersion) handleError(error)
    } finally {
        if (requestVersion === projectRequestVersion) loading.value = false
    }
}

let descriptionRequestVersion = 0
async function loadDescription(projectId: number, projectVersion: number) {
    const requestVersion = ++descriptionRequestVersion
    descriptionLoading.value = true
    descriptionReady.value = false
    descriptionFailed.value = false
    const isCurrent = () =>
        projectVersion === projectRequestVersion && requestVersion === descriptionRequestVersion
    try {
        const result = await getCurseForgeDescription(projectId)
        if (!isCurrent()) return
        description.value = result
        descriptionReady.value = true
    } catch {
        if (isCurrent()) descriptionFailed.value = true
    } finally {
        if (isCurrent()) descriptionLoading.value = false
    }
}

async function loadInitialFiles(projectId: number, projectVersion: number) {
    const requestVersion = ++filesRequestVersion
    versionsLoading.value = true
    filesFailed.value = false
    try {
        const result = await getCurseForgeFilesPage(projectId, { index: 0, pageSize: 50 })
        if (projectVersion !== projectRequestVersion || requestVersion !== filesRequestVersion)
            return
        files.value = result.files
        filesComplete.value = result.files.length >= result.pagination.totalCount
        filesReady.value = true
    } catch {
        if (projectVersion === projectRequestVersion && requestVersion === filesRequestVersion)
            filesFailed.value = true
    } finally {
        if (projectVersion === projectRequestVersion && requestVersion === filesRequestVersion)
            versionsLoading.value = false
    }
}

let tagsRequestVersion = 0
async function loadProjectTags(projectVersion: number) {
    const requestVersion = ++tagsRequestVersion
    tagsFailed.value = false
    const result = await Promise.allSettled([get_loaders(), get_game_versions()])
    if (projectVersion !== projectRequestVersion || requestVersion !== tagsRequestVersion) return
    if (result[0].status === 'fulfilled') allLoaders.value = result[0].value
    if (result[1].status === 'fulfilled') allGameVersions.value = result[1].value
    tagsFailed.value = result.some((entry) => entry.status === 'rejected')
}
function retrySupplementalData() {
    if (filesFailed.value) {
        if (filesReady.value) void loadAllProjectFiles(Number(route.params.id))
        else void loadInitialFiles(Number(route.params.id), projectRequestVersion)
    }
    if (tagsFailed.value) void loadProjectTags(projectRequestVersion)
}

async function loadAllProjectFiles(projectId: number) {
    if (versionsLoading.value) return
    const projectVersion = projectRequestVersion
    const requestVersion = ++filesRequestVersion
    versionsLoading.value = true
    filesFailed.value = false

    try {
        const response = await getCurseForgeFiles(projectId, { index: 0, pageSize: 50 })
        if (projectVersion !== projectRequestVersion || requestVersion !== filesRequestVersion)
            return
        files.value = response.files
        filesComplete.value = true
    } catch (error) {
        if (projectVersion === projectRequestVersion && requestVersion === filesRequestVersion) {
            filesFailed.value = true
            handleError(error)
        }
    } finally {
        if (projectVersion === projectRequestVersion && requestVersion === filesRequestVersion) {
            versionsLoading.value = false
        }
    }
}

watch(
    () => Number(route.params.id),
    (projectId) => {
        if (Number.isFinite(projectId)) void loadProject(projectId)
    },
    { immediate: true },
)
onScopeDispose(() => {
    projectRequestVersion++
    filesRequestVersion++
    translationRequestVersion++
})

watch([activeTab, () => project.value?.id, filesReady], ([tab, projectId, ready]) => {
    if (tab === 'versions' && projectId && ready && !filesComplete.value) {
        void loadAllProjectFiles(projectId)
    }
})

watch(
    [fromBrowse, fromInstanceContent, instanceId],
    async ([fromBrowseEnabled, fromInstanceContentEnabled, preferredInstanceId]) => {
        if (fromBrowseEnabled || fromInstanceContentEnabled) {
            const version = projectRequestVersion
            const isCurrent = () =>
                version === projectRequestVersion && instanceId.value === preferredInstanceId
            await contentSelection.refreshInstances(preferredInstanceId, isCurrent)
            if (!isCurrent()) return
            if (fromInstanceContentEnabled && preferredInstanceId) {
                const preferredInstance = contentSelection.instances.value.find(
                    (instance) => instance.id === preferredInstanceId,
                )
                if (preferredInstance) contentSelection.setTarget(preferredInstance)
            }
        }
    },
    { immediate: true },
)

async function installSelected(fileId: string | null) {
    if (!project.value || !filesReady.value) return
    if (cartEligible.value && !contentSelection.targetInstance.value) {
        await contentSelection.refreshInstances()
        browseInstanceSelector.value?.show()
        return
    }
    if (cartEligible.value && contentSelection.targetInstance.value) {
        if (cartProjectSelected.value) {
            contentSelection.remove(cartProjectKey.value)
            return
        }
        const target = contentSelection.targetInstance.value
        const selectedProject = project.value
        const selectedKey = cartProjectKey.value
        const selectedType = projectType.value
        const requestVersion = projectRequestVersion
        const expectedLoader = { forge: 1, fabric: 4, quilt: 5, neoforge: 6 }[target.loader]
        let resolvedFileId: string | number | null = fileId
        if (!resolvedFileId && !usesTargetGameVersion(selectedType)) {
            resolvedFileId = files.value.find((file) => file.isAvailable)?.id ?? null
        } else if (!resolvedFileId) {
            resolvedFileId =
                selectedProject.latestFilesIndexes.find(
                    (index) =>
                        index.gameVersion === target.game_version &&
                        (selectedType !== 'mod' ||
                            !expectedLoader ||
                            index.modLoader === expectedLoader),
                )?.fileId ??
                files.value.find(
                    (file) =>
                        file.gameVersions.includes(target.game_version) &&
                        (selectedType !== 'mod' || getFilePlatforms(file).includes(target.loader)),
                )?.id ??
                null
            if (!resolvedFileId) {
                const compatibleFiles = await getCurseForgeFilesPage(selectedProject.id, {
                    gameVersion: target.game_version,
                    modLoaderType:
                        selectedType === 'mod' ? curseForgeLoaderType(target.loader) : undefined,
                    index: 0,
                    pageSize: 50,
                })
                resolvedFileId = compatibleFiles.files.find((file) => file.isAvailable)?.id ?? null
            }
        }
        if (
            requestVersion !== projectRequestVersion ||
            project.value?.id !== selectedProject.id ||
            contentSelection.targetInstance.value?.id !== target.id
        )
            return
        if (!resolvedFileId) {
            handleError(new Error(formatMessage(messages.noCompatibleVersion)))
            return
        }
        await contentSelection.add({
            key: selectedKey,
            provider: 'curseforge',
            projectId: selectedProject.id.toString(),
            providerProjectId: selectedProject.id.toString(),
            versionId: resolvedFileId.toString(),
            contentType: selectedType as 'mod' | 'resourcepack' | 'datapack' | 'shader' | 'world',
            title: selectedProject.name,
            iconUrl: getCurseForgeImageUrl(
                selectedProject.logo?.url ?? selectedProject.logo?.thumbnailUrl,
            ),
            slug: selectedProject.slug,
            preferences: {
                gameVersions: usesTargetGameVersion(selectedType) ? [target.game_version] : [],
                loaders: selectedType === 'mod' ? [target.loader] : [],
            },
        })
        return
    }
    if (isWorldMap.value) {
        installing.value = true
        await installCurseForgeWorld(
            project.value.id,
            fileId,
            instanceId.value,
            'ProjectPage',
            () => {
                installing.value = false
            },
        ).catch((error) => {
            installing.value = false
            handleError(error)
        })
        return
    }

    installing.value = true
    await installCurseForge(
        project.value.id.toString(),
        fileId,
        instanceId.value,
        'ProjectPage',
        () => {
            installing.value = false
        },
        (instanceId) => {
            router.push(`/instance/${instanceId}`)
        },
    ).catch((error) => {
        installing.value = false
        handleError(error)
    })
}

function translationFailureMessage(error: unknown) {
    return formatMessage(
        {
            'rate-limited': messages.translationRateLimited,
            authentication: messages.translationAuthenticationFailed,
            'content-too-long': messages.translationContentTooLong,
            network: messages.translationNetworkFailed,
            provider: messages.translationFailed,
        }[getTranslationErrorKind(error)],
    )
}

async function translateProject() {
    if (!data.value || translationLoading.value) return
    const requestVersion = ++translationRequestVersion
    const previousTranslationActive = translationActive.value
    const previousTranslations = translations.value
    translationLoading.value = true

    try {
        const settings = await getTranslationSettings()
        if (requestVersion !== translationRequestVersion) return
        translationMode.value = settings.mode
        translationStyle.value = settings.style
        const prepared = prepareDescription(data.value.body ?? '', 'html')
        const targetLanguage = settings.target_language || i18n.global.locale.value || 'en-US'
        const baseRequest = {
            source_language: 'auto',
            target_language: targetLanguage,
            context: {
                title: data.value.title,
                description: data.value.description,
            },
        }
        const allSegments = [
            { id: 'title', text: data.value.title, format: 'plain' },
            { id: 'description', text: data.value.description, format: 'plain' },
            ...projectGalleryTranslationSegments(data.value.gallery),
            ...prepared.segments,
        ]

        translationActive.value = true
        const accumulated = { ...translations.value }
        await translateContent({ ...baseRequest, segments: allSegments }, (response) => {
            if (requestVersion !== translationRequestVersion) return
            for (const segment of response.segments) accumulated[segment.id] = segment.text
            translations.value = { ...accumulated }
        })
        if (requestVersion !== translationRequestVersion) return

        validateTranslatedDescription(prepared, accumulated)
    } catch (error) {
        if (requestVersion === translationRequestVersion) {
            translationActive.value = previousTranslationActive
            translations.value = previousTranslations
            addNotification({
                title: formatMessage(messages.translationFailedTitle),
                text: translationFailureMessage(error),
                type: 'error',
            })
        }
    } finally {
        if (requestVersion === translationRequestVersion) translationLoading.value = false
    }
}

watch([() => project.value?.id, descriptionReady], ([projectId, ready]) => {
    const requestVersion = projectRequestVersion
    if (projectId && ready)
        runAfterPageTransitionSettle(() => {
            if (requestVersion === projectRequestVersion) void maybeAutoTranslate()
        })
})

async function maybeAutoTranslate() {
    const requestVersion = projectRequestVersion
    try {
        const settings = await getTranslationSettings()
        if (
            requestVersion === projectRequestVersion &&
            settings.auto_translate &&
            descriptionReady.value
        )
            await translateProject()
    } catch (error) {
        handleError(error)
    }
}

async function maybeHintAutoTranslate() {
    if (!(await noteManualTranslateClick())) return
    addNotification({
        title: formatMessage(autoTranslateHintMessages.title),
        text: formatMessage(autoTranslateHintMessages.text),
        type: 'info',
    })
}

function toggleTranslation() {
    if (translationActive.value) {
        translationRequestVersion++
        translationActive.value = false
        translationLoading.value = false
        return
    }
    void maybeHintAutoTranslate()
    void translateProject()
}
onScopeDispose(() => {
    projectRequestVersion++
    filesRequestVersion++
    descriptionRequestVersion++
    tagsRequestVersion++
    translationRequestVersion++
})
</script>

<style scoped>
.project-sidebar-section {
    @apply p-4 flex flex-col gap-2 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid;
}
</style>
