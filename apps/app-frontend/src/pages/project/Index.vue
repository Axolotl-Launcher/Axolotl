<template>
    <template v-if="loading">
        <Teleport to="#sidebar-teleport-target">
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
        <UpgradeProjectReturnBar />
        <Teleport to="#sidebar-teleport-target">
            <ProjectSidebarCompatibility
                v-if="!isServerProject"
                :project="data"
                :tags="{ loaders: allLoaders, gameVersions: allGameVersions }"
                :project-v3="projectV3"
                :platform-action="(platform) => browseByProjectFilter('loader', platform)"
                class="project-sidebar-section"
            />
            <ProjectSidebarServerInfo
                v-if="isServerProject"
                :project-v3="projectV3"
                :tags="{ loaders: allLoaders, gameVersions: allGameVersions }"
                :required-content="serverRequiredContent"
                :recommended-version="serverRecommendedVersion"
                :supported-versions="serverSupportedVersions"
                :loaders="serverModpackLoaders"
                :ping="serverPing"
                :status-online="serverStatusOnline"
                class="project-sidebar-section"
            />
            <ProjectSidebarLinks
                link-target="_blank"
                :project="data"
                :project-v3="projectV3"
                :mcmod-url="mcmodUrl"
                class="project-sidebar-section"
            />
            <ProjectSidebarTags
                :project="data"
                :tag-action="(tag) => browseByProjectFilter('category', tag)"
                class="project-sidebar-section"
            />
            <ProjectSidebarCreators
                :organization="organization"
                :members="members"
                :org-link="(slug) => `https://modrinth.com/organization/${slug}`"
                :user-link="(username) => `https://modrinth.com/user/${username}`"
                link-target="_blank"
                class="project-sidebar-section"
            />
            <ProjectSidebarDetails
                :project="data"
                :resource-dates="data.resourceDates"
                :has-versions="versions.length > 0"
                :link-target="`_blank`"
                :hide-license="isServerProject"
                :show-followers="isServerProject"
                class="project-sidebar-section"
            />
        </Teleport>
        <div class="flex flex-col gap-4 p-6">
            <div
                v-if="projectInstallContext && projectInstallContext.showInstallHeader !== false"
                class="sticky top-0 z-20 -mx-6 -mt-6 rounded-tl-[--radius-xl] border-0 border-b border-solid border-surface-5 bg-[color-mix(in_srgb,var(--surface-1-opaque,var(--surface-1))_20%,transparent)] p-3 backdrop-blur-xl"
            >
                <BrowseInstallHeader :install-context="projectInstallContext" />
            </div>
            <InstanceIndicator v-if="instance && !projectInstallContext" :instance="instance" />
            <template v-if="data">
                <Teleport
                    v-if="themeStore.featureFlags.project_background"
                    to="#background-teleport-target"
                >
                    <ProjectBackgroundGradient :project="data" />
                </Teleport>
                <ProjectHeader
                    v-else
                    :project="data"
                    :project-v3="projectV3"
                    :ping="serverPing"
                    :translated-title="translationActive ? translations.title : undefined"
                    :translated-description="
                        translationActive ? translations.description : undefined
                    "
                    :translation-mode="translationMode"
                    :translation-style="translationStyle"
                    @contextmenu.prevent.stop="handleRightClick"
                >
                    <template v-if="isServerProject" #actions>
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
                            v-if="serverPlaying"
                            type="colored"
                            color="red"
                            size="xl"
                            @click="handleStopServer"
                            ><StopCircleIcon />
                            {{ formatMessage(commonMessages.stopButton) }}
                        </Button>
                        <Button
                            v-else
                            type="colored"
                            color="brand"
                            size="xl"
                            :disabled="data && installingServerProjects.includes(data.id)"
                            @click="handleClickPlay"
                            ><PlayIcon />
                            {{
                                data && installingServerProjects.includes(data.id)
                                    ? formatMessage(commonMessages.installingLabel)
                                    : formatMessage(commonMessages.playButton)
                            }}
                        </Button>
                        <Button
                            v-tooltip="formatMessage(commonMessages.addServerToInstanceButton)"
                            size="xl"
                            circular
                            icon-only
                            @click="handleAddServerToInstance"
                            ><PlusIcon />
                        </Button>
                        <OverflowMenu
                            :tooltip="formatMessage(commonMessages.moreOptionsButton)"
                            :options="[
                                {
                                    id: 'open-in-browser',
                                    link: `https://modrinth.com/project/${data.slug}`,
                                    external: true,
                                },
                                ...(mcmodUrl
                                    ? [
                                          {
                                              id: 'open-in-mcmod',
                                              link: mcmodUrl,
                                              external: true,
                                          },
                                      ]
                                    : []),
                                {
                                    divider: true,
                                },
                                {
                                    id: 'report',
                                    color: 'red',
                                    hoverFilled: true,
                                    link: `https://modrinth.com/report?item=project&itemID=${data.id}`,
                                },
                            ]"
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
                            <template #report>
                                <ReportIcon /> {{ formatMessage(commonMessages.reportButton) }}
                            </template>
                        </OverflowMenu>
                    </template>
                    <template v-else #actions>
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
                            v-if="showSwitchVersion && onVersionsPage"
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
                            <DownloadIcon
                                v-else-if="
                                    !installButtonInstalled &&
                                    !serverProjectSelected &&
                                    !cartProjectSelected
                                "
                            />
                            <CheckIcon v-else />
                            {{ installButtonLabel }}
                        </Button>
                        <!-- 开服功能暂有问题，隐藏该按钮
						<Transition name="start-server">
							<Button
								v-if="serverCapableModpack"
								key="modpack-start-server"
								size="xl"
								type="outlined"

									v-tooltip="formatMessage(messages.startServer)"
									native-type="button"
									@click="openModpackServerFlow"
								>
									<ServerIcon />
									{{ formatMessage(messages.startServer) }}
								</Button>
						</Transition>
						-->
                        <OverflowMenu
                            :tooltip="formatMessage(commonMessages.moreOptionsButton)"
                            :options="[
                                {
                                    id: 'follow',
                                    disabled: true,
                                    tooltip: formatMessage(messages.comingSoon),
                                    action: () => {},
                                },
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
                                              action: () => void toggleFavorite(),
                                          },
                                      ]
                                    : []),
                                ...getDependentSearchActions(),
                                {
                                    id: 'open-in-browser',
                                    link: `https://modrinth.com/${data.project_type}/${data.slug}`,
                                    external: true,
                                },
                                ...(mcmodUrl
                                    ? [
                                          {
                                              id: 'open-in-mcmod',
                                              link: mcmodUrl,
                                              external: true,
                                          },
                                      ]
                                    : []),
                                {
                                    divider: true,
                                },
                                {
                                    id: 'report',
                                    color: 'red',
                                    hoverFilled: true,
                                    link: `https://modrinth.com/report?item=project&itemID=${data.id}`,
                                },
                            ]"
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
                <NavTabs
                    :links="[
                        {
                            label: formatMessage(commonProjectSettingsMessages.description),
                            href: projectDescriptionHref,
                        },
                        {
                            label: formatMessage(commonProjectSettingsMessages.versions),
                            href: versionsHref,
                            subpages: ['version'],
                            shown: projectV3?.minecraft_server == null,
                        },
                        {
                            label: formatMessage(commonMessages.changelogLabel),
                            href: projectChangelogHref,
                            shown: projectV3?.minecraft_server == null,
                        },
                        {
                            label: formatMessage(commonProjectSettingsMessages.gallery),
                            href: projectGalleryHref,
                            shown: data.gallery.length > 0,
                        },
                    ]"
                />
                <ProjectPageSkeleton
                    v-if="versionsLoading && (onVersionsPage || route.params.version)"
                    variant="versions"
                />
                <RouterView
                    v-else-if="route.path.startsWith('/project')"
                    :project="data"
                    :versions="versions"
                    :members="members"
                    :instance="instance"
                    :install="install"
                    :installed="installed"
                    :installing="installing || instanceContextLoading"
                    :installed-version="installedVersion"
                    :translation-active="translationActive"
                    :translations="translations"
                    :translation-mode="translationMode"
                    :translation-style="translationStyle"
                    :start-server="(version) => openModpackServerFlow(version)"
                />
            </template>
            <template v-else> {{ formatMessage(messages.loadFailed) }} </template>
        </div>
        <SelectedProjectsFloatingBar
            v-if="projectInstallContext"
            :install-context="projectInstallContext"
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
        <ContextMenu ref="options" @option-clicked="handleOptionsClick">
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
        </ContextMenu>
        <CreationFlowModal
            v-if="serverInstallContent.isServerContext.value && data?.project_type === 'modpack'"
            ref="serverSetupModalRef"
            :type="
                serverInstallContent.serverFlowFrom.value === 'reset-server'
                    ? 'reset-server'
                    : 'server-onboarding'
            "
            :available-loaders="[
                'vanilla',
                'fabric',
                'neoforge',
                'forge',
                'quilt',
                'paper',
                'purpur',
            ]"
            :show-snapshot-toggle="true"
            :on-back="serverInstallContent.onServerFlowBack"
            :search-modpacks="serverInstallContent.searchServerModpacks"
            :get-project-versions="serverInstallContent.getServerProjectVersions"
            :get-loader-manifest="getLoaderManifest"
            @hide="() => {}"
            @browse-modpacks="() => {}"
            @create="serverInstallContent.handleServerModpackFlowCreate"
        />
        <CreateModpackServerModal ref="modpackServerModal" @created="handleModpackServerCreated" />
    </div>
</template>

<script setup>
import {
    BookmarkFilledIcon,
    BookmarkIcon,
    BookOpenIcon,
    CheckIcon,
    ClipboardCopyIcon,
    DownloadIcon,
    ExternalIcon,
    GlobeIcon,
    HeartIcon,
    LanguagesIcon,
    MoreVerticalIcon,
    PackageIcon,
    PlayIcon,
    PlusIcon,
    ReportIcon,
    SearchIcon,
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- used by temporarily commented start-server button
    ServerIcon,
    SpinnerIcon,
    StopCircleIcon,
} from '@modrinth/assets'
import {
    BrowseInstallHeader,
    Button,
    commonMessages,
    commonProjectSettingsMessages,
    CreationFlowModal,
    defineMessages,
    EmptyState,
    formatDependencyProjectFilterOption,
    formatProjectTypeSentence,
    getLatestMatchingInstallVersion,
    getTargetInstallPreferences,
    injectNotificationManager,
    injectPopupNotificationManager,
    NavTabs,
    OverflowMenu,
    ProjectBackgroundGradient,
    ProjectHeader,
    ProjectPageSkeleton,
    ProjectSidebarCompatibility,
    ProjectSidebarCreators,
    ProjectSidebarDetails,
    ProjectSidebarLinks,
    ProjectSidebarServerInfo,
    ProjectSidebarTags,
    requestInstall,
    SelectedProjectsFloatingBar,
    usesTargetGameVersion,
    useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import { computed, onUnmounted, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { SwapIcon } from '@/assets/icons/index.js'
import BrowseInstanceSelector from '@/components/browse/BrowseInstanceSelector.vue'
import CreateModpackServerModal from '@/components/multiplayer/servers/modpack/CreateModpackServerModal.vue'
import ContextMenu from '@/components/ui/ContextMenu.vue'
import InstanceIndicator from '@/components/ui/InstanceIndicator.vue'
import {
    fetchCachedServerStatus,
    getFreshCachedServerStatus,
} from '@/composables/instances/use-server-status-query'
import { useContentFavorites } from '@/composables/useContentFavorites'
import {
    get_organization,
    get_project,
    get_project_v3,
    get_team,
    get_version,
    get_version_many,
} from '@/helpers/cache.js'
import { isFavoriteContentType } from '@/helpers/content-favorites'
import { resolveMcmodUrl } from '@/helpers/content-search'
import { resolveDownloadUrl } from '@/helpers/download-url'
import { retiredDownloadMessage } from '@/helpers/download-url-messages'
import { process_listener } from '@/helpers/events'
import {
    get as getInstance,
    get_content_snapshot,
    kill,
    list as listInstances,
} from '@/helpers/instance'
import { getDisplayInstanceIcon } from '@/helpers/instance-icons'
import { get_loader_versions as getLoaderManifest } from '@/helpers/metadata'
import { get_by_instance_id } from '@/helpers/process'
import { projectGalleryTranslationSegments } from '@/helpers/project-gallery'
import { createProjectBrowseLocation } from '@/helpers/project-links'
import { get_game_versions, get_loaders } from '@/helpers/tags'
import {
    autoTranslateHintMessages,
    getTranslationErrorKind,
    getTranslationSettings,
    noteManualTranslateClick,
    prepareDescription,
    translateInBatches as translateContent,
    validateTranslatedDescription,
} from '@/helpers/translation'
import { getServerAddress } from '@/helpers/worlds'
import i18n from '@/i18n.config'
import { injectContentInstall } from '@/providers/content-install'
import { injectContentSelection, makeContentSelectionKey } from '@/providers/content-selection'
import { injectServerInstall } from '@/providers/server-install'
import { createServerInstallContent } from '@/providers/setup/server-install-content'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { useTheming } from '@/store/state.js'

import UpgradeProjectReturnBar from './UpgradeProjectReturnBar.vue'

dayjs.extend(relativeTime)

const { addNotification, handleError } = injectNotificationManager()
const popupNotificationManager = injectPopupNotificationManager()
const { install: installVersion } = injectContentInstall()
const contentSelection = injectContentSelection()
const route = useRoute()
const router = useRouter()
const queryClient = useQueryClient()
const breadcrumbs = useBreadcrumbs()
const themeStore = useTheming()
const { formatMessage } = useVIntl()
const contentFavorites = useContentFavorites()

void contentFavorites.load().catch(handleError)

const messages = defineMessages({
    comingSoon: { id: 'app.project.coming-soon', defaultMessage: 'Coming soon' },
    loadFailed: {
        id: 'app.project.load-failed',
        defaultMessage: 'The project could not be loaded. Try again.',
    },
    backToBrowse: {
        id: 'app.project.install-context.back-to-browse',
        defaultMessage: 'Back to discover',
    },
    backToInstanceContent: {
        id: 'app.project.install-context.back-to-instance-content',
        defaultMessage: 'Back to instance content',
    },
    installContentToInstance: {
        id: 'app.project.install-context.install-content-to-instance',
        defaultMessage: 'Install content to instance',
    },
    alreadyInstalled: {
        id: 'app.project.install-button.already-installed',
        defaultMessage: 'This project is already installed',
    },
    switchVersion: {
        id: 'app.project.install-button.switch-version',
        defaultMessage: 'Switch version',
    },
    noCompatibleVersion: {
        id: 'app.project.install-button.no-compatible-version',
        defaultMessage: 'No compatible version was found for this instance.',
    },
    openInMcmod: {
        id: 'app.project.open-in-mcmod',
        defaultMessage: 'Open in MC Mod',
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
    viewDependents: {
        id: 'project.actions.view-dependents',
        defaultMessage: 'View dependents',
    },
    viewProjectTypeDependents: {
        id: 'project.actions.view-project-type-dependents',
        defaultMessage: 'View {projectType} dependents',
    },
    viewModpacks: {
        id: 'project.actions.view-modpacks',
        defaultMessage: 'View modpacks',
    },
    startServer: {
        id: 'app.project.modpack-server.start',
        defaultMessage: 'Start server',
    },
    serverCreated: {
        id: 'app.project.modpack-server.created-title',
        defaultMessage: 'Server created',
    },
    serverCreatedDescription: {
        id: 'app.project.modpack-server.created-description',
        defaultMessage: '{name} is ready. Configure and start it in Multiplayer.',
    },
    openServer: {
        id: 'app.project.modpack-server.open',
        defaultMessage: 'Open server',
    },
})

const { installingServerProjects, playServerProject, showAddServerToInstanceModal } =
    injectServerInstall()
const loading = ref(true)
const loadFailed = ref(false)
const installing = ref(false)
const browseInstanceSelector = ref()
const data = shallowRef(null)
const mcmodUrl = ref(null)
const versions = shallowRef([])
const members = shallowRef([])
const organization = shallowRef(null)
const instance = ref(null)
const versionsLoading = ref(false)
const instanceContextLoading = ref(false)

const favoriteSupported = computed(() => isFavoriteContentType(data.value?.project_type ?? ''))
const favoritePending = computed(() =>
    data.value ? contentFavorites.isPending('modrinth', data.value.id) : false,
)
const favoriteSaved = computed(() =>
    data.value ? contentFavorites.isFavorite('modrinth', data.value.id) : false,
)

// Used by the temporarily commented start-server button.
// eslint-disable-next-line @typescript-eslint/no-unused-vars
const serverCapableModpack = computed(
    () => data.value?.project_type === 'modpack' && data.value.server_side !== 'unsupported',
)
const modpackServerModal = ref()

async function openModpackServerFlow(targetVersion) {
    if (!data.value) return
    // The version handed in by the UI can be an incomplete object that is missing
    // `id`/`loaders`/`game_versions`/`files`. Re-fetch a complete version from the
    // API so the modpack server flow has the data it needs (otherwise it misdetects
    // the loader as Vanilla and fails with "No server launcher available").
    let version = targetVersion
    const isComplete = (v) => !!v && Array.isArray(v.game_versions) && Array.isArray(v.files)
    if (!isComplete(version)) {
        const versionId = version?.id ?? data.value.versions?.[0]
        version = versionId ? await get_version(versionId, 'bypass').catch(() => null) : null
    }
    if (!version) return
    modpackServerModal.value?.show(data.value, version)
}

function handleModpackServerCreated(serverId) {
    popupNotificationManager.addPopupNotification({
        title: formatMessage(messages.serverCreated),
        text: formatMessage(messages.serverCreatedDescription, { name: data.value?.title ?? '' }),
        type: 'success',
        buttons: [
            {
                label: formatMessage(messages.openServer),
                color: 'brand',
                action: () => {
                    void router.push(`/multiplayer/servers/${encodeURIComponent(serverId)}`)
                },
            },
        ],
    })
}

async function toggleFavorite() {
    if (!data.value || !favoriteSupported.value || favoritePending.value) return
    await contentFavorites
        .toggle({
            provider: 'modrinth',
            project_id: data.value.id,
            content_type: data.value.project_type,
            slug: data.value.slug,
        })
        .catch(handleError)
}

function browseByProjectFilter(filter, value) {
    const projectType = isServerProject.value ? 'server' : data.value?.project_type
    if (!projectType) return
    void router.push(createProjectBrowseLocation(projectType, filter, value))
}

const installed = ref(false)
const installedVersion = ref(null)
const isServerProject = ref(false)
const projectV3 = shallowRef(null)
const serverRequiredContent = shallowRef(null)
const serverRecommendedVersion = shallowRef(null)
const serverSupportedVersions = shallowRef([])
const serverModpackLoaders = shallowRef([])
const serverPing = ref(undefined)
const serverStatusOnline = ref(false)
const serverInstancePath = ref(null)
const serverPlaying = ref(false)
const serverSetupModalRef = ref(null)
const serverInstallContent = createServerInstallContent({ serverSetupModalRef })
const translationActive = ref(false)
const translationLoading = ref(false)
const translations = ref({})
const translationMode = ref('bilingual')
const translationStyle = ref('weakened')
let translationRequestVersion = 0

serverInstallContent.watchServerContextChanges()
void serverInstallContent.initServerContext().catch(handleError)

const instanceFilters = computed(() => {
    if (!instance.value) {
        return {}
    }

    const loaders = []
    if (data.value.project_type === 'mod') {
        if (instance.value.loader !== 'vanilla') {
            loaders.push(instance.value.loader)
        }
        if (instance.value.loader === 'vanilla' || data.value.loaders.includes('datapack')) {
            loaders.push('datapack')
        }
    }

    return {
        l: loaders,
        g: usesTargetGameVersion(data.value.project_type) ? instance.value.game_version : undefined,
    }
})

function buildProjectHref(path, extraQuery = {}) {
    const params = new URLSearchParams()
    for (const [key, val] of Object.entries({ ...route.query, ...extraQuery })) {
        if (Array.isArray(val)) {
            for (const v of val) params.append(key, v)
        } else if (val) {
            params.append(key, String(val))
        }
    }
    const qs = params.toString()
    return qs ? `${path}?${qs}` : path
}

function buildBrowseHref(path) {
    const params = new URLSearchParams()
    for (const [key, val] of Object.entries(route.query)) {
        if (key === 'b') continue
        if (Array.isArray(val)) {
            for (const v of val) params.append(key, v)
        } else if (val) {
            params.append(key, String(val))
        }
    }
    const qs = params.toString()
    return qs ? `${path}?${qs}` : path
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
    const type = data.value?.project_type ? `${data.value.project_type}` : 'mod'
    return buildBrowseHref(`/browse/${type}`)
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
const cartTarget = computed(() => contentSelection.targetInstance.value)
const cartProjectKey = computed(() =>
    data.value ? makeContentSelectionKey('modrinth', data.value.id) : '',
)
const cartProjectSelected = computed(
    () => !!cartProjectKey.value && contentSelection.isSelected(cartProjectKey.value),
)

async function syncContentSelectionTarget(isCurrent = () => true) {
    if (!fromBrowse.value && !route.query.i) return
    await contentSelection.refreshInstances(instance.value?.id ?? null)
    if (!isCurrent()) return
    await contentSelection.refreshInstalledIdentities()
}

const projectInstallContext = computed(() => {
    const serverData = serverInstallContent.serverContextServerData.value
    if (serverData) {
        return {
            name: serverData.name,
            loader: serverData.loader ?? '',
            gameVersion: serverData.mc_version ?? '',
            serverId: serverInstallContent.serverIdQuery.value,
            upstream: serverData.upstream,
            iconSrc: null,
            isMedal: serverData.is_medal,
            backUrl: projectBrowseBackUrl.value,
            backLabel: projectBackLabel.value,
            heading: serverInstallContent.serverBrowseHeading.value,
            queuedCount: serverInstallContent.queuedServerInstallCount.value,
            selectedProjects: serverInstallContent.selectedServerInstallProjects.value,
            isInstallingSelected: serverInstallContent.isInstallingQueuedServerInstalls.value,
            installProgress: serverInstallContent.queuedInstallProgress.value,
            clearQueued: serverInstallContent.clearQueuedServerInstalls,
            clearSelected: serverInstallContent.clearQueuedServerInstalls,
            discardSelectedAndBack: serverInstallContent.discardQueuedServerInstallsAndBack,
            installSelected: serverInstallContent.installQueuedServerInstallsAndBack,
        }
    }

    if (cartEligible.value && cartTarget.value) {
        const target = cartTarget.value
        const displayIcon = getDisplayInstanceIcon(target.icon_path, target.loader)
        return {
            showInstallHeader: !!instance.value,
            name: target.name,
            loader: target.loader,
            gameVersion: target.game_version,
            iconSrc: displayIcon.url,
            iconFrameless: displayIcon.frameless,
            backUrl: projectBrowseBackUrl.value,
            backLabel: projectBackLabel.value,
            heading: formatMessage(messages.installContentToInstance),
            selectedProjects: contentSelection.selectedProjects.value,
            isInstallingSelected: ['validating', 'reviewing', 'queueing'].includes(
                contentSelection.state.value,
            ),
            installProgress: contentSelection.progress.value,
            clearSelected: contentSelection.clear,
            installSelected: contentSelection.installSelected,
        }
    }
    if (instance.value) {
        const displayIcon = getDisplayInstanceIcon(instance.value.icon_path, instance.value.loader)
        return {
            name: instance.value.name,
            loader: instance.value.loader,
            gameVersion: instance.value.game_version,
            iconSrc: displayIcon.url,
            iconFrameless: displayIcon.frameless,
            backUrl: projectBrowseBackUrl.value,
            backLabel: projectBackLabel.value,
            heading: formatMessage(messages.installContentToInstance),
        }
    }

    return null
})

const serverProjectInstallContext = computed(
    () =>
        !!serverInstallContent.serverContextServerData.value &&
        ['modpack', 'mod', 'plugin', 'datapack'].includes(data.value?.project_type),
)
const serverProjectSelected = computed(
    () =>
        !!data.value && serverInstallContent.queuedServerInstallProjectIds.value.has(data.value.id),
)
const serverProjectInstalled = computed(
    () =>
        !!data.value &&
        (serverInstallContent.serverContentProjectIds.value.has(data.value.id) ||
            serverInstallContent.serverContextServerData.value?.upstream?.project_id ===
                data.value.id),
)
const installButtonLoading = computed(
    () =>
        installing.value ||
        versionsLoading.value ||
        instanceContextLoading.value ||
        serverInstallContent.isInstallingQueuedServerInstalls.value ||
        (cartProjectKey.value ? contentSelection.isInstalling(cartProjectKey.value) : false),
)
const installButtonValidating = computed(
    () =>
        serverProjectInstallContext.value &&
        installing.value &&
        data.value?.project_type !== 'modpack' &&
        !serverInstallContent.isInstallingQueuedServerInstalls.value,
)
const installButtonInstalled = computed(() =>
    serverProjectInstallContext.value
        ? serverProjectInstalled.value
        : installed.value ||
          (!!data.value &&
              (!route.query.i || cartTarget.value?.id === route.query.i) &&
              contentSelection.isInstalledIdentity('modrinth', data.value.id, data.value.slug)),
)
const installButtonDisabled = computed(
    () => installButtonInstalled.value || installButtonLoading.value,
)
const installButtonLabel = computed(() => {
    if (installButtonInstalled.value) return formatMessage(commonMessages.installedLabel)
    if (installButtonValidating.value) return formatMessage(commonMessages.validatingLabel)
    if (installButtonLoading.value) return formatMessage(commonMessages.installingLabel)
    if (serverProjectSelected.value) return formatMessage(commonMessages.selectedLabel)
    if (cartProjectSelected.value) return formatMessage(commonMessages.selectedLabel)
    return formatMessage(commonMessages.installButton)
})
const installButtonTooltip = computed(() => {
    if (installButtonInstalled.value) return formatMessage(messages.alreadyInstalled)
    return null
})

const showSwitchVersion = computed(() => !!instance.value && installed.value)
const onVersionsPage = computed(() => route.name === 'Versions')

function getDependentSearchTypes() {
    if (!data.value) return []
    if (data.value.project_type !== 'mod')
        return [isServerProject.value ? 'server' : data.value.project_type]
    const loaders = data.value.loaders ?? []
    const types = []
    if (loaders.some((loader) => ['fabric', 'forge', 'neoforge', 'quilt'].includes(loader)))
        types.push('mod')
    if (loaders.some((loader) => ['paper', 'purpur', 'spigot', 'folia'].includes(loader)))
        types.push('plugin')
    if (loaders.some((loader) => ['datapack'].includes(loader))) types.push('datapack')
    return types.length ? types : ['mod']
}

function getDependentSearchActions() {
    if (!data.value) return []
    const types = getDependentSearchTypes()
    return [
        ...types.map((projectType) => ({
            id: formatMessage(
                types.length === 1 ? messages.viewDependents : messages.viewProjectTypeDependents,
                { projectType: formatProjectTypeSentence(formatMessage, projectType) },
            ),
            icon: SearchIcon,
            link: `/browse/${projectType}?dep=${encodeURIComponent(formatDependencyProjectFilterOption(data.value.id, ['required']))}`,
        })),
        ...(data.value.project_type !== 'modpack' && types.length !== 1
            ? [
                  {
                      id: formatMessage(messages.viewModpacks),
                      icon: PackageIcon,
                      link: `/browse/modpack?dep=${encodeURIComponent(formatDependencyProjectFilterOption(data.value.id, ['required']))}`,
                  },
              ]
            : []),
    ]
}

function goToVersions() {
    router.push(versionsHref.value)
}

const allLoaders = ref([])
const allGameVersions = ref([])
void Promise.all([get_loaders().catch(handleError), get_game_versions().catch(handleError)]).then(
    ([loaders, gameVersions]) => {
        allLoaders.value = loaders ?? []
        allGameVersions.value = gameVersions ?? []
    },
)

async function handleClickPlay() {
    if (!isServerProject.value) return
    await playServerProject(data.value.id).catch(handleError)
    await updateServerPlayState()
}

async function updateServerPlayState() {
    if (!isServerProject.value || !data.value) return
    const packs = await listInstances()
    const inst = packs.find((p) => p.link?.project_id === data.value.id)
    if (inst) {
        serverInstancePath.value = inst.id
        const processes = await get_by_instance_id(inst.id).catch(() => [])
        serverPlaying.value = Array.isArray(processes) && processes.length > 0
    } else {
        serverInstancePath.value = null
        serverPlaying.value = false
    }
}

async function handleStopServer() {
    if (!serverInstancePath.value) return
    await kill(serverInstancePath.value).catch(() => {})
    serverPlaying.value = false
}

function handleAddServerToInstance() {
    const address = getServerAddress(projectV3.value?.minecraft_java_server)
    if (!address || !data.value) return
    showAddServerToInstanceModal(data.value.title, address)
}

async function fetchProjectData() {
    const requestedProjectId = String(route.params.id)
    const requestedInstanceId = route.query.i
    const requestVersion = projectLoadVersion
    const isCurrent = () =>
        requestVersion === projectLoadVersion &&
        String(route.params.id) === requestedProjectId &&
        route.query.i === requestedInstanceId
    translationRequestVersion++
    translationActive.value = false
    translationLoading.value = false
    translations.value = {}
    const [project, projectV3Result] = await Promise.all([
        get_project(requestedProjectId, 'revalidate'),
        get_project_v3(requestedProjectId, 'revalidate').catch(handleError),
    ])
    if (!isCurrent()) return
    projectV3.value = projectV3Result

    if (!project) {
        throw new Error('Project not found')
    }

    data.value = project
    mcmodUrl.value = null
    void resolveMcmodUrl(project.slug, 'modrinth').then((url) => {
        if (isCurrent()) mcmodUrl.value = url
    })
    isServerProject.value = projectV3.value?.minecraft_server != null
    serverStatusOnline.value = !!projectV3.value?.minecraft_java_server?.ping?.data

    breadcrumbs.setName('Project', data.value.title)
    breadcrumbs.setNameIcon('Project', data.value.icon_url)

    void loadSupplementalProjectData(project, requestedInstanceId, isCurrent).catch(handleError)
    fetchDeferredServerData(project)
    void maybeAutoTranslate()
}

async function loadSupplementalProjectData(project, requestedInstanceId, isCurrent) {
    const updates = [
        get_version_many(project.versions, 'must_revalidate')
            .catch(handleError)
            .then((result) => {
                if (!isCurrent()) return
                versions.value = (result ?? []).sort(
                    (a, b) => dayjs(b.date_published) - dayjs(a.date_published),
                )
            })
            .finally(() => {
                if (isCurrent()) versionsLoading.value = false
            }),
        get_team(project.team)
            .catch(handleError)
            .then((result) => {
                if (isCurrent()) members.value = result ?? []
            }),
    ]

    if (requestedInstanceId) {
        updates.push(
            Promise.all([
                getInstance(requestedInstanceId),
                get_content_snapshot(requestedInstanceId, true).catch((error) => {
                    if (isCurrent()) handleError(error)
                    return null
                }),
            ])
                .then(async ([target, snapshot]) => {
                    if (!isCurrent()) return
                    instance.value = target
                    const installedFile = snapshot?.items.find(
                        (item) =>
                            item.materializationState === 'present' &&
                            ((item.provider === 'modrinth' &&
                                item.providerProjectId === project.id) ||
                                item.content?.provider_refs.some(
                                    (reference) =>
                                        reference.provider === 'modrinth' &&
                                        String(reference.project_id) === project.id,
                                )),
                    )
                    installed.value = !!installedFile
                    installedVersion.value =
                        (installedFile?.provider === 'modrinth'
                            ? installedFile.providerReleaseId
                            : installedFile?.content?.provider_refs.find(
                                  (reference) =>
                                      reference.provider === 'modrinth' &&
                                      String(reference.project_id) === project.id,
                              )?.version_id) ?? null
                    await syncContentSelectionTarget(isCurrent)
                })
                .catch((error) => {
                    if (isCurrent()) handleError(error)
                })
                .finally(() => {
                    if (isCurrent()) instanceContextLoading.value = false
                }),
        )
    } else {
        void syncContentSelectionTarget().catch(handleError)
    }
    if (project.organization) {
        updates.push(
            get_organization(project.organization)
                .catch(handleError)
                .then((result) => {
                    if (isCurrent()) organization.value = result
                }),
        )
    }

    await Promise.all(updates)
}

function translationFailureMessage(error) {
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
        translationMode.value = settings.mode
        translationStyle.value = settings.style
        const prepared = prepareDescription(data.value.body ?? '')
        const targetLanguage = settings.target_language || i18n.global.locale.value || 'en-US'
        const baseRequest = {
            source_language: 'auto',
            target_language: targetLanguage,
            context: {
                title: data.value.title ?? '',
                description: data.value.description ?? '',
            },
        }
        const allSegments = [
            { id: 'title', text: data.value.title ?? '', format: 'plain' },
            { id: 'description', text: data.value.description ?? '', format: 'plain' },
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

async function maybeAutoTranslate() {
    try {
        const settings = await getTranslationSettings()
        if (settings.auto_translate) await translateProject()
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

function fetchDeferredServerData(project) {
    const serverAddress = projectV3.value?.minecraft_java_server?.address
    if (serverAddress) {
        const cachedStatus = getFreshCachedServerStatus(queryClient, serverAddress)
        if (cachedStatus) {
            serverPing.value = cachedStatus.ping
            serverStatusOnline.value = true
        } else {
            serverPing.value = undefined
        }

        fetchCachedServerStatus(queryClient, serverAddress)
            .then((status) => {
                if (projectV3.value?.minecraft_java_server?.address !== serverAddress) return
                serverPing.value = status.ping
                serverStatusOnline.value = true
            })
            .catch((error) => {
                console.error(`Failed to ping server ${serverAddress}:`, error)
            })
    }

    const content = projectV3.value?.minecraft_java_server?.content
    if (content?.kind === 'modpack' && content.version_id) {
        get_version(content.version_id, 'bypass')
            .catch(handleError)
            .then(async (modpackVersion) => {
                if (!modpackVersion) return
                serverRecommendedVersion.value = modpackVersion.game_versions?.[0] ?? null
                serverModpackLoaders.value = modpackVersion.mrpack_loaders ?? []
                if (modpackVersion.project_id) {
                    const modpackProject = await get_project_v3(
                        modpackVersion.project_id,
                        'must_revalidate',
                    ).catch(handleError)
                    if (modpackProject) {
                        const primaryFile =
                            modpackVersion.files?.find((f) => f.primary) ??
                            modpackVersion.files?.[0]

                        serverRequiredContent.value = {
                            name: modpackProject.name,
                            versionNumber: modpackVersion.version_number ?? '',
                            icon: modpackProject.icon_url,
                            onclickName:
                                modpackProject.id !== project.id
                                    ? () => router.push(`/project/${modpackProject.id}`)
                                    : undefined,
                            onclickVersion:
                                modpackProject.id !== project.id
                                    ? () =>
                                          router.push(
                                              `/project/${modpackProject.id}/version/${modpackVersion.id}`,
                                          )
                                    : undefined,
                            onclickDownload: primaryFile?.url
                                ? () => {
                                      const url = resolveDownloadUrl(primaryFile.url)
                                      if (url) return openUrl(url)
                                      handleError(new Error(formatMessage(retiredDownloadMessage)))
                                  }
                                : undefined,
                            showCustomModpackTooltip: modpackProject.id === project.id,
                        }
                    }
                }
            })
    } else if (content?.kind === 'vanilla') {
        serverRecommendedVersion.value = content.recommended_game_version ?? null
        const supported = content.supported_game_versions ?? []
        serverSupportedVersions.value = supported.filter((v) => !!v)
    }

    updateServerPlayState()
}

let projectLoadVersion = 0

async function loadProjectPage() {
    const loadVersion = ++projectLoadVersion
    loading.value = true
    loadFailed.value = false
    data.value = null
    projectV3.value = null
    versions.value = []
    members.value = []
    organization.value = null
    instance.value = null
    versionsLoading.value = true
    instanceContextLoading.value = !!route.query.i
    installed.value = false
    installedVersion.value = null

    try {
        await fetchProjectData()
    } catch (error) {
        if (loadVersion !== projectLoadVersion) return
        if (data.value) handleError(error)
        else loadFailed.value = true
    } finally {
        if (loadVersion === projectLoadVersion) {
            loading.value = false
            if (!data.value) {
                versionsLoading.value = false
                instanceContextLoading.value = false
            }
        }
    }
}

void loadProjectPage()

let unlistenProcesses
process_listener((e) => {
    if (
        e.event === 'finished' &&
        serverInstancePath.value &&
        e.instance_id === serverInstancePath.value
    ) {
        serverPlaying.value = false
    }
}).then((unlisten) => {
    unlistenProcesses = unlisten
})

onUnmounted(() => {
    projectLoadVersion++
    unlistenProcesses?.()
})

watch([() => route.params.id, () => route.query.i], () => {
    if (route.params.id && route.path.startsWith('/project')) {
        void loadProjectPage()
    }
})

async function install(version) {
    if (serverProjectInstallContext.value && data.value) {
        if (serverProjectSelected.value) {
            serverInstallContent.removeQueuedServerInstall(data.value.id)
            return
        }
        if (installButtonDisabled.value) return

        installing.value = true
        try {
            const contentType = data.value.project_type
            await requestInstall({
                project: {
                    ...data.value,
                    project_id: data.value.id,
                    icon_url: data.value.icon_url,
                },
                contentType,
                mode: contentType === 'modpack' ? 'immediate' : 'queue',
                selectedFilters: [],
                providedFilters: [],
                overriddenProvidedFilterTypes: [],
                targetPreferences: getTargetInstallPreferences(
                    {
                        gameVersion: serverInstallContent.serverContextServerData.value?.mc_version,
                        loader: serverInstallContent.serverContextServerData.value?.loader,
                    },
                    contentType,
                ),
                getProjectVersions: async () => versions.value,
                queue: {
                    get: serverInstallContent.getQueuedServerInstallPlans,
                    set: serverInstallContent.setQueuedServerInstallPlans,
                },
                install: (plan) =>
                    serverInstallContent.openServerModpackInstallFlow({
                        projectId: plan.projectId,
                        versionId: plan.versionId,
                        name: plan.project.title ?? plan.project.name ?? data.value.title,
                        iconUrl: plan.project.icon_url ?? undefined,
                    }),
            })
        } catch (err) {
            handleError(err)
        } finally {
            installing.value = false
        }
        return
    }
    if (cartEligible.value && !cartTarget.value) {
        await contentSelection.refreshInstances()
        browseInstanceSelector.value?.show()
        return
    }
    if (cartEligible.value && data.value && cartTarget.value) {
        if (cartProjectSelected.value) {
            contentSelection.remove(cartProjectKey.value)
            return
        }
        installing.value = true
        try {
            const preferences = getTargetInstallPreferences(
                { gameVersion: cartTarget.value.game_version, loader: cartTarget.value.loader },
                data.value.project_type,
            )
            const selectedVersion =
                version ?? getLatestMatchingInstallVersion(versions.value, preferences)?.id
            if (!selectedVersion) throw new Error(formatMessage(messages.noCompatibleVersion))
            await contentSelection.add({
                key: cartProjectKey.value,
                provider: 'modrinth',
                projectId: data.value.id,
                providerProjectId: data.value.id,
                versionId: selectedVersion,
                contentType: data.value.project_type,
                title: data.value.title,
                iconUrl: data.value.icon_url,
                slug: data.value.slug,
                preferences,
            })
        } catch (error) {
            handleError(error)
        } finally {
            installing.value = false
        }
        return
    }

    installing.value = true
    await installVersion(
        data.value.id,
        version,
        instance.value ? instance.value.id : null,
        'ProjectPage',
        (version, installedProjectIds) => {
            installing.value = false

            const installedIds = installedProjectIds ?? [data.value.id]
            if (instance.value && version && installedIds.includes(data.value.id)) {
                installed.value = true
                installedVersion.value = version
            }
        },
        (profile) => {
            router.push(`/instance/${profile}`)
        },
    ).catch(handleError)
}

const options = ref(null)
const handleRightClick = (event) => {
    options.value.showMenu(event, data.value, [
        {
            name: 'install',
        },
        {
            type: 'divider',
        },
        {
            name: 'open_link',
        },
        {
            name: 'copy_link',
        },
    ])
}
const handleOptionsClick = (args) => {
    switch (args.option) {
        case 'install':
            install(null)
            break
        case 'open_link':
            openUrl(`https://modrinth.com/${args.item.project_type}/${args.item.slug}`)
            break
        case 'copy_link':
            navigator.clipboard.writeText(
                `https://modrinth.com/${args.item.project_type}/${args.item.slug}`,
            )
            break
    }
}
</script>

<style scoped lang="scss">
.start-server-enter-active {
    transition:
        opacity 0.28s ease,
        transform 0.28s cubic-bezier(0.22, 1, 0.36, 1);
}

.start-server-enter-from {
    opacity: 0;
    transform: translateY(6px) scale(0.96);
}

.start-server-leave-active {
    transition:
        opacity 0.16s ease,
        transform 0.16s ease;
}

.start-server-leave-to {
    opacity: 0;
    transform: translateY(4px) scale(0.97);
}

.project-sidebar-section {
    @apply p-4 flex flex-col gap-2 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid;
}
</style>
