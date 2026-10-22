<template>
    <div
        v-if="instance"
        :class="{
            'flex h-full min-h-0 flex-col': isFixedRender,
            'instance-fixed-render': isFixedRender,
        }"
    >
        <div
            :class="['p-6 pr-2 pb-4', { 'shrink-0': isFixedRender, hidden: isStudioMode }]"
            @contextmenu.prevent.stop="(event) => handleRightClick(event)"
        >
            <ExportModal ref="exportModal" :instance="instance" />
            <InstanceSettingsModal
                :key="instance.id"
                ref="settingsModal"
                :instance="instance"
                :offline="offline"
                @unlinked="fetchInstance"
                @updated="instance = $event"
            />
            <ContentPageHeader>
                <template #icon>
                    <InstanceIcon
                        :icon-path="instance.icon_path"
                        :instance-id="instance.id"
                        :loader="instance.loader"
                        :alt="instance.name"
                        size="64px"
                    />
                </template>
                <template #title>
                    {{ instance.name }}
                </template>
                <template #stats>
                    <div class="flex items-center flex-wrap gap-2">
                        <template v-if="!isServerInstance">
                            <div class="flex items-center gap-1.5 font-medium">
                                <BoxIcon class="h-4 w-4" />
                                {{ instance.game_version }}
                            </div>
                            <Badge
                                v-if="postUpgradeNotice"
                                color="green"
                                :type="
                                    formatMessage(messages.upgradedTo, {
                                        version: postUpgradeNotice.targetGameVersion,
                                    })
                                "
                            />

                            <div class="w-1.5 h-1.5 rounded-full bg-surface-5"></div>

                            <div class="flex items-center gap-1.5 font-medium capitalize">
                                <component :is="loaderIcon" v-if="loaderIcon" class="h-4 w-4" />
                                {{ instance.loader }}
                                {{ instance.loader_version ? instance.loader_version : '' }}
                            </div>

                            <template v-if="showInstancePlayTime">
                                <div class="w-1.5 h-1.5 rounded-full bg-surface-5"></div>

                                <div class="flex items-center gap-2 font-medium">
                                    <template v-if="timePlayed > 0">
                                        {{ timePlayedHumanized }}
                                    </template>
                                    <template v-else>{{
                                        formatMessage(messages.neverPlayed)
                                    }}</template>
                                </div>
                            </template>
                        </template>

                        <template v-else>
                            <template v-if="loadingServerPing">
                                <ServerOnlinePlayers
                                    v-if="playersOnline !== undefined"
                                    :online="playersOnline"
                                    :status-online="statusOnline"
                                    hide-label
                                />
                                <ServerRecentPlays :recent-plays="recentPlays ?? 0" hide-label />
                                <div
                                    v-if="
                                        (playersOnline !== undefined ||
                                            recentPlays !== undefined) &&
                                        (minecraftServer?.region || ping)
                                    "
                                    class="w-1.5 h-1.5 rounded-full bg-surface-5"
                                ></div>
                                <ServerPing v-if="ping" :ping="ping" />
                            </template>

                            <ServerRegion
                                v-if="minecraftServer?.region"
                                :region="minecraftServer?.region"
                            />

                            <div
                                v-if="minecraftServer?.region || ping"
                                class="w-1.5 h-1.5 rounded-full bg-surface-5"
                            ></div>

                            <div
                                v-if="linkedProjectV3"
                                class="flex gap-1.5 items-center font-medium text-[var(--color-text-default)]"
                            >
                                {{ formatMessage(messages.linkedTo) }}
                                <Avatar
                                    :src="linkedProjectV3.icon_url"
                                    :alt="linkedProjectV3.name"
                                    :tint-by="instance.id"
                                    size="24px"
                                />
                                <router-link
                                    :to="`/project/${linkedProjectV3.slug ?? linkedProjectV3.id}`"
                                    class="hover:underline text-[var(--color-text-default)] truncate"
                                >
                                    {{ linkedProjectV3.name }}
                                </router-link>
                            </div>
                        </template>

                        <div
                            v-if="instance?.symlink_target && symlinkWarning.isHidden.value"
                            class="w-1.5 h-1.5 rounded-full bg-surface-5"
                        ></div>
                        <SymlinkInstanceWarning
                            v-if="instance?.symlink_target && symlinkWarning.isHidden.value"
                            :symlink-target="instance.symlink_target"
                            badge-only
                        />
                    </div>
                </template>
                <template #actions>
                    <div data-onboarding-id="instance-actions" class="flex gap-2">
                        <Button
                            v-if="
                                [
                                    'installing',
                                    'pack_installing',
                                    'pack_installed',
                                    'not_installed',
                                    'minecraft_installing',
                                ].includes(instance.install_stage)
                            "
                            type="colored"
                            color="brand"
                            size="xl"
                            disabled
                            >{{ formatMessage(commonMessages.installingLabel) }}</Button
                        >
                        <Button
                            v-else-if="instance.install_stage !== 'installed'"
                            v-tooltip="
                                offline ? formatMessage(messages.offlineInstalledOnly) : undefined
                            "
                            type="colored"
                            color="brand"
                            size="xl"
                            :disabled="offline"
                            @click="repairInstance()"
                            ><DownloadIcon />
                            {{ formatMessage(commonMessages.repairButton) }}
                        </Button>
                        <Button
                            v-else-if="playing === true"
                            type="colored"
                            color="red"
                            size="xl"
                            :disabled="stopping"
                            @click="stopInstance('InstancePage')"
                            ><StopCircleIcon />
                            {{
                                stopping
                                    ? formatMessage(messages.stopping)
                                    : formatMessage(commonMessages.stopButton)
                            }}
                        </Button>
                        <Button
                            v-else-if="playing === false && loading === false && !isServerInstance"
                            type="colored"
                            color="brand"
                            size="xl"
                            @click="startInstance('InstancePage')"
                            ><PlayIcon />
                            {{ formatMessage(commonMessages.playButton) }}
                        </Button>
                        <ButtonGroup
                            v-else-if="playing === false && loading === false && isServerInstance"
                        >
                            <Button
                                type="colored"
                                color="brand"
                                size="xl"
                                @click="handlePlayServer()"
                                ><PlayIcon />
                                {{ formatMessage(commonMessages.playButton) }}
                            </Button>
                            <OverflowMenu
                                :options="[
                                    {
                                        id: 'join_server',
                                        action: () => handlePlayServer(),
                                    },
                                    {
                                        id: 'launch_instance',
                                        action: () => startInstance('InstancePage'),
                                    },
                                ]"
                            >
                                <div class="w-0 text-xl relative top-0.5 right-2.5">
                                    <DropdownIcon />
                                </div>

                                <template #join_server>
                                    <PlayIcon />
                                    {{ formatMessage(messages.joinServer) }}
                                </template>
                                <template #launch_instance>
                                    <PlayIcon />
                                    {{ formatMessage(messages.launchInstance) }}
                                </template>
                            </OverflowMenu>
                        </ButtonGroup>
                        <Button
                            v-else-if="loading === true && playing === false"
                            type="colored"
                            color="brand"
                            size="xl"
                            disabled
                            ><SpinnerIcon class="animate-spin" />
                            {{
                                formatMessage(messages.starting, { seconds: launchElapsedSeconds })
                            }}
                        </Button>
                        <Button
                            v-tooltip="formatMessage(messages.instanceSettings)"
                            size="xl"
                            circular
                            icon-only
                            @click="settingsModal?.show()"
                            ><SettingsIcon />
                        </Button>
                        <OverflowMenu
                            :options="[
                                {
                                    id: 'open-folder',
                                    action: () => {
                                        if (instance) showInstanceInFolder(instance.id)
                                    },
                                },
                                {
                                    id: 'export-mrpack',
                                    action: () => exportModal?.show(),
                                },
                                {
                                    id: 'create-shortcut',
                                    action: () => createShortcut(),
                                },
                                ...(canUpgradeInstance
                                    ? [{ id: 'upgrade-instance', action: () => openUpgrade() }]
                                    : []),
                            ]"
                        >
                            <MoreVerticalIcon />
                            <template #share-instance>
                                <UserPlusIcon /> {{ formatMessage(messages.shareInstance) }}
                            </template>
                            <template #host-a-server>
                                <ServerIcon /> {{ formatMessage(messages.createServer) }}
                            </template>
                            <template #open-folder>
                                <FolderOpenIcon />
                                {{ formatMessage(commonMessages.openFolderButton) }}
                            </template>
                            <template #export-mrpack>
                                <PackageIcon /> {{ formatMessage(messages.exportModpack) }}
                            </template>
                            <template #create-shortcut>
                                <ExternalIcon /> {{ formatMessage(messages.createShortcut) }}
                            </template>
                            <template #upgrade-instance>
                                <UpdatedIcon /> {{ formatMessage(messages.upgradeInstance) }}
                            </template>
                        </OverflowMenu>
                    </div>
                </template>
            </ContentPageHeader>
        </div>
        <div
            data-onboarding-id="instance-tabs"
            :class="['px-6', { 'shrink-0': isFixedRender, hidden: isStudioMode }]"
        >
            <SymlinkInstanceWarning
                v-if="instance?.symlink_target && !symlinkWarning.isHidden.value"
                :symlink-target="instance.symlink_target"
                class="mb-3"
                dismissible
            />
            <NavTabs v-if="!hideInstanceTabs" :links="tabs" />
        </div>
        <div
            :class="['p-6 pt-4', { 'flex min-h-0 flex-1 flex-col overflow-hidden': isFixedRender }]"
        >
            <RouterView v-slot="{ Component }" :key="instance.id" :route="displayedInstanceRoute">
                <template v-if="Component">
                    <Suspense
                        :key="instance.id"
                        @pending="subpagePending = true"
                        @resolve="subpagePending = false"
                    >
                        <component
                            :is="Component"
                            :instance="instance"
                            :options="options"
                            :offline="offline"
                            :playing="playing"
                            :installed="instance.install_stage !== 'installed'"
                            :is-server-instance="isServerInstance"
                            :open-settings="() => settingsModal?.show(1)"
                            @play="updatePlayState"
                            @stop="() => stopInstance('InstanceSubpage')"
                        ></component>
                    </Suspense>
                </template>
            </RouterView>
        </div>
        <ContextMenu ref="options" @option-clicked="handleOptionsClick">
            <template #play> <PlayIcon /> {{ formatMessage(commonMessages.playButton) }} </template>
            <template #stop>
                <StopCircleIcon /> {{ formatMessage(commonMessages.stopButton) }}
            </template>
            <template #add_content>
                <PlusIcon /> {{ formatMessage(messages.addContent) }}
            </template>
            <template #edit> <EditIcon /> {{ formatMessage(commonMessages.editButton) }} </template>
            <template #copy_path>
                <ClipboardCopyIcon /> {{ formatMessage(messages.copyPath) }}
            </template>
            <template #open_folder>
                <FolderOpenIcon /> {{ formatMessage(commonMessages.openFolderButton) }}
            </template>
            <template #copy_link>
                <ClipboardCopyIcon /> {{ formatMessage(commonMessages.copyLinkButton) }}
            </template>
            <template #open_link>
                <GlobeIcon /> {{ formatMessage(commonMessages.openInModrinthButton) }}
                <ExternalIcon />
            </template>
            <template #copy_names><EditIcon />{{ formatMessage(messages.copyNames) }}</template>
            <template #copy_slugs><HashIcon />{{ formatMessage(messages.copySlugs) }}</template>
            <template #copy_links><GlobeIcon />{{ formatMessage(messages.copyLinks) }}</template>
            <template #toggle><EditIcon />{{ formatMessage(messages.toggleSelected) }}</template>
            <template #disable><XIcon />{{ formatMessage(messages.disableSelected) }}</template>
            <template #enable
                ><CheckCircleIcon />{{ formatMessage(messages.enableSelected) }}</template
            >
            <template #hide_show
                ><EyeIcon />{{ formatMessage(messages.hideShowUnselected) }}</template
            >
            <template #update_all
                ><UpdatedIcon />{{
                    selected.length > 0
                        ? formatMessage(messages.updateSelected)
                        : formatMessage(messages.updateAll)
                }}</template
            >
            <template #filter_update
                ><UpdatedIcon />{{ formatMessage(messages.selectUpdatable) }}</template
            >
        </ContextMenu>
    </div>
</template>
<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
    BoxesIcon,
    BoxIcon,
    CheckCircleIcon,
    ClipboardCopyIcon,
    DownloadIcon,
    DropdownIcon,
    EditIcon,
    ExternalIcon,
    EyeIcon,
    FolderOpenIcon,
    getLoaderIcon,
    GlobeIcon,
    HashIcon,
    ImageIcon,
    MoreVerticalIcon,
    PackageIcon,
    PlayIcon,
    PlusIcon,
    ServerIcon,
    SettingsIcon,
    SpinnerIcon,
    StopCircleIcon,
    TerminalSquareIcon,
    UpdatedIcon,
    UserPlusIcon,
    XIcon,
} from '@modrinth/assets'
import {
    Avatar,
    Badge,
    Button,
    ButtonGroup,
    commonMessages,
    ContentPageHeader,
    defineMessages,
    injectNotificationManager,
    NavTabs,
    OverflowMenu,
    ServerOnlinePlayers,
    ServerPing,
    ServerRecentPlays,
    ServerRegion,
    useLoadingBarToken,
    useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import dayjs from 'dayjs'
import duration from 'dayjs/plugin/duration'
import relativeTime from 'dayjs/plugin/relativeTime'
import { computed, onUnmounted, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import ContextMenu from '@/components/ui/ContextMenu.vue'
import ExportModal from '@/components/ui/ExportModal.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import InstanceSettingsModal from '@/components/ui/modal/InstanceSettingsModal.vue'
import SymlinkInstanceWarning from '@/components/ui/SymlinkInstanceWarning.vue'
import {
    fetchCachedServerStatus,
    getFreshCachedServerStatus,
} from '@/composables/instances/use-server-status-query'
import { useInstanceConsole } from '@/composables/useInstanceConsole'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { postUpgradeNoticeQueryKey, usePostUpgradeNotice } from '@/composables/usePostUpgradeNotice'
import { useSymlinkWarningDismiss } from '@/composables/useSymlinkWarningDismiss'
import { trackEvent } from '@/helpers/analytics'
import { get_project_v3 } from '@/helpers/cache.js'
import { instance_listener, process_listener } from '@/helpers/events'
import {
    install_existing_instance,
    install_job_list,
    install_pack_to_existing_instance,
} from '@/helpers/install'
import {
    allow_symlink_target,
    gcReportFellBack,
    get,
    get_full_path,
    kill,
    run,
} from '@/helpers/instance'
import { getDisplayInstanceIcon } from '@/helpers/instance-icons'
import { get_by_instance_id } from '@/helpers/process'
import { createRequestGeneration } from '@/helpers/request-generation'
import type { GameInstance } from '@/helpers/types'
import { createInstanceShortcut, showInstanceInFolder } from '@/helpers/utils.js'
import { refreshWorlds, type ServerStatus } from '@/helpers/worlds'
import { injectServerInstall } from '@/providers/server-install'
import { handleSevereError } from '@/store/error.js'
import { useBreadcrumbs, useTheming } from '@/store/state'

import { isActiveUpgradeJobForInstance, isUnmanagedUpgradeEligible } from './upgrade/entry'

dayjs.extend(duration)
dayjs.extend(relativeTime)

const { addNotification, handleError } = injectNotificationManager()
const { playServerProject } = injectServerInstall()
const queryClient = useQueryClient()
const route = useRoute()
const props = defineProps<{
    id: string
}>()
const { formatMessage } = useVIntl()

const messages = defineMessages({
    neverPlayed: { id: 'app.instance.never-played', defaultMessage: 'Never played' },
    upgradedTo: {
        id: 'app.instance.post-upgrade-status',
        defaultMessage: 'Upgraded to {version}',
    },
    linkedTo: { id: 'app.instance.linked-to', defaultMessage: 'Linked to' },
    stopping: { id: 'app.instance.stopping', defaultMessage: 'Stopping...' },
    joinServer: { id: 'app.instance.join-server', defaultMessage: 'Join server' },
    launchInstance: { id: 'app.instance.launch-instance', defaultMessage: 'Launch instance' },
    starting: { id: 'app.instance.starting', defaultMessage: 'Starting... {seconds, number}s' },
    instanceSettings: { id: 'app.instance.settings', defaultMessage: 'Instance settings' },
    shareInstance: { id: 'app.instance.share', defaultMessage: 'Share instance' },
    createServer: { id: 'app.instance.create-server', defaultMessage: 'Create a server' },
    exportModpack: { id: 'app.instance.export-modpack', defaultMessage: 'Export modpack' },
    createShortcut: { id: 'app.instance.create-shortcut', defaultMessage: 'Create shortcut' },
    upgradeInstance: { id: 'app.instance.upgrade-instance', defaultMessage: 'Upgrade instance' },
    addContent: { id: 'app.instances.add-content', defaultMessage: 'Add content' },
    copyPath: { id: 'app.instances.copy-path', defaultMessage: 'Copy path' },
    copyNames: { id: 'app.instance.copy-names', defaultMessage: 'Copy names' },
    copySlugs: { id: 'app.instance.copy-slugs', defaultMessage: 'Copy slugs' },
    copyLinks: { id: 'app.instance.copy-links', defaultMessage: 'Copy links' },
    toggleSelected: { id: 'app.instance.toggle-selected', defaultMessage: 'Toggle selected' },
    disableSelected: { id: 'app.instance.disable-selected', defaultMessage: 'Disable selected' },
    enableSelected: { id: 'app.instance.enable-selected', defaultMessage: 'Enable selected' },
    hideShowUnselected: {
        id: 'app.instance.hide-show-unselected',
        defaultMessage: 'Show/hide unselected',
    },
    updateSelected: { id: 'app.instance.update-selected', defaultMessage: 'Update selected' },
    updateAll: { id: 'app.instance.update-all', defaultMessage: 'Update all' },
    selectUpdatable: { id: 'app.instance.select-updatable', defaultMessage: 'Select updatable' },
    contentTab: { id: 'app.instance.tabs.content', defaultMessage: 'Content' },
    filesTab: { id: 'app.instance.tabs.files', defaultMessage: 'Files' },
    worldsTab: { id: 'app.instance.tabs.worlds', defaultMessage: 'Worlds' },
    screenshotsTab: { id: 'app.instance.tabs.screenshots', defaultMessage: 'Screenshots' },
    logsTab: { id: 'app.instance.tabs.logs', defaultMessage: 'Logs' },
    offlineInstalledOnly: {
        id: 'app.instance.offline-installed-only',
        defaultMessage: 'Offline mode can only launch fully downloaded instances.',
    },
    gcFallbackNotice: {
        id: 'app.gc-notice.fallback',
        defaultMessage: 'Java did not accept {preferred}; using {chosen}.',
    },
})

const router = useRouter()
const displayedInstanceRoute = shallowRef(router.currentRoute.value)
const breadcrumbs = useBreadcrumbs()
const themeStore = useTheming()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const showInstancePlayTime = computed(() => themeStore.getFeatureFlag('show_instance_play_time'))
const { offline } = useNetworkStatus()

const instance = ref<GameInstance>()
const instanceId = computed(() => instance.value?.id)
const postUpgradeNoticeQuery = usePostUpgradeNotice(() => instance.value?.id ?? props.id)
const postUpgradeNotice = computed(() => postUpgradeNoticeQuery.data.value ?? null)
const symlinkWarning = useSymlinkWarningDismiss(instanceId)
const playing = ref(false)
const loading = ref(false)
const launchElapsedSeconds = ref(0)
const subpagePending = ref(false)
const stopping = ref(false)
const exportModal = ref<InstanceType<typeof ExportModal>>()
let launchElapsedTimer: ReturnType<typeof setInterval> | undefined

useLoadingBarToken(subpagePending)

const isServerInstance = ref(false)
const linkedProjectV3 = ref<Labrinth.Projects.v3.Project>()
const selected = ref<unknown[]>([])
const canUpgradeInstance = computed(() =>
    instance.value ? isUnmanagedUpgradeEligible(instance.value) : false,
)

const minecraftServer = computed(() => linkedProjectV3.value?.minecraft_server)
const javaServerPingData = computed(() => linkedProjectV3.value?.minecraft_java_server?.ping?.data)
const liveServerStatusOnline = ref(false)
const statusOnline = computed(() => liveServerStatusOnline.value || !!javaServerPingData.value)
const recentPlays = computed(
    () => linkedProjectV3.value?.minecraft_java_server?.verified_plays_2w ?? undefined,
)
const playersOnline = ref<number | undefined>(undefined)
const ping = ref<number | undefined>(undefined)
const loadingServerPing = ref(false)
const activeInstanceId = ref<string>()
const loaderIcon = computed(() =>
    instance.value?.loader ? getLoaderIcon(instance.value.loader) : undefined,
)

watch(
    () => router.currentRoute.value,
    (nextRoute) => {
        if (nextRoute.path.startsWith('/instance') && nextRoute.params.id === props.id) {
            displayedInstanceRoute.value = nextRoute
        }
    },
    { immediate: true },
)

function applyServerStatus(status: ServerStatus) {
    playersOnline.value = status.players?.online
    ping.value = status.ping
    liveServerStatusOnline.value = true
    loadingServerPing.value = true
}

function resetServerStatus() {
    ping.value = undefined
    playersOnline.value = undefined
    liveServerStatusOnline.value = false
    loadingServerPing.value = false
}

let instanceGeneration = 0
let instancePageClosed = false
const instanceReads = createRequestGeneration()
const processReads = createRequestGeneration()
async function fetchInstance() {
    const generation = ++instanceGeneration
    const requestedId = props.id
    const read = instanceReads.begin(requestedId)
    const isCurrent = () =>
        !instancePageClosed && instanceGeneration === generation && props.id === requestedId
    if (activeInstanceId.value !== requestedId) {
        isServerInstance.value = false
        linkedProjectV3.value = undefined
        resetServerStatus()
    }

    const nextInstance = await get(requestedId).catch((error) => {
        if (isCurrent() && instanceReads.isCurrent(read, props.id)) {
            if (String(error).includes('not managed')) void router.push({ path: '/' })
            else handleError(error)
        }
    })
    if (!isCurrent() || !instanceReads.isCurrent(read, props.id)) return
    if (!nextInstance) return
    const linkChanged =
        instance.value?.id !== nextInstance?.id ||
        instance.value?.link?.type !== nextInstance?.link?.type ||
        instance.value?.link?.project_id !== nextInstance?.link?.project_id
    if (linkChanged) {
        linkedProjectV3.value = undefined
        resetServerStatus()
    }
    instance.value = nextInstance ?? undefined
    activeInstanceId.value = nextInstance?.id
    isServerInstance.value =
        nextInstance?.link?.type === 'server_project_modpack' ||
        linkedProjectV3.value?.minecraft_server != null

    if (nextInstance?.symlink_target) {
        void allow_symlink_target(nextInstance.symlink_target).catch(() => {})
    }

    if (
        !offline.value &&
        !linkedProjectV3.value &&
        nextInstance?.link?.project_id &&
        ['modrinth_modpack', 'server_project_modpack'].includes(nextInstance.link.type)
    ) {
        void get_project_v3(nextInstance.link.project_id, 'stale_while_revalidate')
            .then((project) => {
                if (!isCurrent()) return
                if (
                    instance.value?.link?.type !== nextInstance.link?.type ||
                    instance.value?.link?.project_id !== nextInstance.link?.project_id
                )
                    return
                linkedProjectV3.value = project
                isServerInstance.value =
                    nextInstance.link?.type === 'server_project_modpack' ||
                    project?.minecraft_server != null
                fetchDeferredData(nextInstance.id)
            })
            .catch((error) => {
                if (isCurrent()) console.warn('Linked project metadata unavailable', error)
            })
    }

    fetchDeferredData(nextInstance?.id)

    if (nextInstance) {
        queryClient.prefetchQuery({
            queryKey: ['worlds', nextInstance.id],
            queryFn: () => refreshWorlds(nextInstance.id),
            staleTime: 30_000,
        })
    }
}

function fetchDeferredData(instanceId?: string) {
    const serverAddress = linkedProjectV3.value?.minecraft_java_server?.address
    if (isServerInstance.value && serverAddress) {
        const cachedStatus = getFreshCachedServerStatus(queryClient, serverAddress)
        if (cachedStatus) {
            applyServerStatus(cachedStatus)
        } else {
            playersOnline.value = undefined
            ping.value = undefined
            loadingServerPing.value = false
        }

        fetchCachedServerStatus(queryClient, serverAddress)
            .then((status) => {
                if (
                    activeInstanceId.value !== instanceId ||
                    linkedProjectV3.value?.minecraft_java_server?.address !== serverAddress
                )
                    return
                applyServerStatus(status)
            })
            .catch((error) => {
                console.error(`Failed to fetch server status for ${serverAddress}:`, error)
            })
            .finally(() => {
                if (activeInstanceId.value !== instanceId) return
                loadingServerPing.value = true
            })
    } else {
        loadingServerPing.value = true
    }

    updatePlayState()
}

async function updatePlayState() {
    const requestedId = props.id
    const request = processReads.begin(requestedId)
    const generation = instanceGeneration
    const runningProcesses = await get_by_instance_id(requestedId).catch((error) => {
        if (
            !instancePageClosed &&
            processReads.isCurrent(request, props.id) &&
            instanceGeneration === generation
        )
            handleError(error)
    })
    if (
        instancePageClosed ||
        !processReads.isCurrent(request, props.id) ||
        instanceGeneration !== generation
    )
        return

    playing.value = Array.isArray(runningProcesses) && runningProcesses.length > 0
}

await fetchInstance()
watch(
    () => props.id,
    async () => {
        if (route.path.startsWith('/instance')) {
            await fetchInstance()
        }
    },
)

const basePath = computed(
    () => `/instance/${encodeURIComponent(displayedInstanceRoute.value.params.id as string)}`,
)

/**
 * Per-route layout mode.
 * - `'scroll'` (default): the whole instance page scrolls inside `.app-viewport`. This lets
 *   `position: sticky` children (and the viewport-rooted `IntersectionObserver` used by
 *   `useStickyObserver`) work correctly.
 * - `'fixed'`: the header + tabs are pinned and only the tab body scrolls in its own container.
 *   Used by tabs whose content (e.g. the log console) needs a bounded height to resolve `h-full`.
 */
const renderMode = computed<'scroll' | 'fixed'>(() =>
    displayedInstanceRoute.value.meta.renderMode === 'fixed' ? 'fixed' : 'scroll',
)
const isFixedRender = computed(() => renderMode.value === 'fixed')
const hideInstanceTabs = computed(() =>
    displayedInstanceRoute.value.matched.some((record) => record.meta.hideInstanceTabs === true),
)
const isStudioMode = computed(() => displayedInstanceRoute.value.name === 'FileStudio')
const tabs = computed(() => [
    {
        label: formatMessage(messages.contentTab),
        href: `${basePath.value}`,
        icon: BoxesIcon,
    },
    {
        label: formatMessage(messages.filesTab),
        href: `${basePath.value}/files`,
        icon: FolderOpenIcon,
    },
    {
        label: formatMessage(messages.screenshotsTab),
        href: `${basePath.value}/screenshots`,
        icon: ImageIcon,
    },
    {
        label: formatMessage(messages.worldsTab),
        href: `${basePath.value}/worlds`,
        icon: GlobeIcon,
    },
    {
        label: formatMessage(messages.logsTab),
        href: `${basePath.value}/logs`,
        icon: TerminalSquareIcon,
    },
])

function updateBreadcrumbs() {
    if (instance.value) {
        breadcrumbs.setRootContext({
            name: instance.value.name,
            link: displayedInstanceRoute.value.path,
            query: displayedInstanceRoute.value.query,
            iconUrl: getDisplayInstanceIcon(instance.value.icon_path, instance.value.loader).url,
        })
    }
}

updateBreadcrumbs()

watch(
    () => instance.value?.id,
    () => {
        updateBreadcrumbs()
    },
)

const options = ref<InstanceType<typeof ContextMenu> | null>(null)

const startInstance = async (context: string) => {
    if (!instance.value) return

    loading.value = true
    launchElapsedSeconds.value = 0
    launchElapsedTimer = setInterval(() => {
        launchElapsedSeconds.value += 1
    }, 1000)
    try {
        const result = await run(props.id)
        const gcNotice = result.gc_notice
        if (gcNotice && gcReportFellBack(gcNotice)) {
            addNotification({
                type: 'warning',
                title: formatMessage(messages.gcFallbackNotice, {
                    preferred: gcNotice.preferred_strategy || 'auto',
                    chosen: gcNotice.chosen_strategy || 'JVM default GC',
                }),
            })
        }
        playing.value = true
    } catch (err) {
        const handled = await handleMinecraftLaunchError(err, {
            instance_id: props.id,
            instance_name: instance.value.name,
        })
        if (!handled) {
            handleSevereError(err, { instanceId: props.id })
        }
    } finally {
        clearInterval(launchElapsedTimer)
        launchElapsedTimer = undefined
        loading.value = false
    }

    trackEvent('InstanceStart', {
        loader: instance.value.loader,
        game_version: instance.value.game_version,
        source: context,
    })
}

const stopInstance = async (context: string) => {
    stopping.value = true
    await kill(props.id).catch(handleError)
    stopping.value = false
    playing.value = false

    if (!instance.value) return
    trackEvent('InstanceStop', {
        loader: instance.value.loader,
        game_version: instance.value.game_version,
        source: context,
    })
}

const handlePlayServer = async () => {
    if (!instance.value?.link?.project_id) return
    loading.value = true
    try {
        await playServerProject(instance.value.link.project_id)
    } finally {
        await updatePlayState()
        loading.value = false
    }
}

const repairInstance = async () => {
    if (
        instance.value.install_stage !== 'pack_installed' &&
        (instance.value.link?.type === 'modrinth_modpack' ||
            instance.value.link?.type === 'server_project_modpack')
    ) {
        await install_pack_to_existing_instance(instance.value.id, {
            type: 'fromVersionId',
            project_id:
                instance.value.link.project_id ?? instance.value.link.server_project_id ?? '',
            version_id:
                instance.value.link.version_id ?? instance.value.link.content_version_id ?? '',
            title: instance.value.name,
        }).catch(handleError)
    } else {
        await install_existing_instance(instance.value.id, false).catch(handleError)
    }
}

const createShortcut = async () => {
    if (!instance.value) return
    try {
        const shortcutPath = await createInstanceShortcut(instance.value.name, instance.value.id)
        if (!shortcutPath) return

        addNotification({
            type: 'success',
            title: 'Shortcut created',
        })
    } catch (error: unknown) {
        addNotification({
            type: 'error',
            title: `Error creating shortcut`,
            text: `${error}`,
        })
    }
}

const openUpgrade = async () => {
    if (!instance.value) return
    const active = (await install_job_list(true).catch(() => [])).find((job) =>
        isActiveUpgradeJobForInstance(job, instance.value!.id),
    )
    if (active) {
        await router.push({ path: '/downloads', query: { job: active.job_id } })
        return
    }
    await router.push(`/instance/${encodeURIComponent(instance.value.id)}/upgrade`)
}

const handleRightClick = (event: MouseEvent) => {
    const baseOptions = [
        { name: 'add_content', section: 'manage', shown: !instance.value.link },
        { type: 'divider' },
        { name: 'edit', section: 'manage' },
        { name: 'open_folder', section: 'navigate' },
        { name: 'copy_path', section: 'navigate' },
    ]

    options.value?.showMenu(
        event,
        instance.value,
        playing.value
            ? [
                  {
                      name: 'stop',
                      section: 'primary',
                      color: 'danger',
                  },
                  ...baseOptions,
              ]
            : [
                  {
                      name: 'play',
                      section: 'primary',
                      color: 'primary',
                  },
                  ...baseOptions,
              ],
    )
}

const handleOptionsClick = async (args: { option: string; item: unknown }) => {
    switch (args.option) {
        case 'play':
            await startInstance('InstancePageContextMenu')
            break
        case 'stop':
            await stopInstance('InstancePageContextMenu')
            break
        case 'add_content':
            await router.push({
                path: `/browse/${instance.value?.loader === 'vanilla' ? 'datapack' : 'mod'}`,
                query: { i: props.id },
            })
            break
        case 'edit':
            settingsModal.value?.show()
            break
        case 'open_folder':
            if (instance.value) await showInstanceInFolder(instance.value.id)
            break
        case 'copy_path': {
            if (instance.value) {
                const fullPath = await get_full_path(instance.value.id)
                await navigator.clipboard.writeText(fullPath)
            }
            break
        }
    }
}

const unlistenInstances = await instance_listener(
    async (event: { instance_id: string; event: string }) => {
        if (event.instance_id !== props.id) return
        if (event.event === 'removed' || route.path === '/') {
            if (route.path !== '/') {
                await router.push({ path: '/' })
            }
            return
        }
        const requestedId = props.id
        await fetchInstance()
        void queryClient.invalidateQueries({ queryKey: postUpgradeNoticeQueryKey(requestedId) })
    },
)

const unlistenProcesses = await process_listener((e: { event: string; instance_id: string }) => {
    if (e.event === 'finished' && e.instance_id === props.id) {
        processReads.invalidate()
        playing.value = false
        void queryClient.invalidateQueries({ queryKey: postUpgradeNoticeQueryKey(props.id) })
    }
})

const settingsModal = ref<InstanceType<typeof InstanceSettingsModal>>()

const timePlayed = computed(() => {
    return instance.value
        ? instance.value.recent_time_played + instance.value.submitted_time_played
        : 0
})

const timePlayedHumanized = computed(() => {
    const duration = dayjs.duration(timePlayed.value, 'seconds')
    const hours = Math.floor(duration.asHours())
    if (hours >= 1) {
        return hours + ' hour' + (hours > 1 ? 's' : '')
    }

    const minutes = Math.floor(duration.asMinutes())
    if (minutes >= 1) {
        return minutes + ' minute' + (minutes > 1 ? 's' : '')
    }

    const seconds = Math.floor(duration.asSeconds())
    return seconds + ' second' + (seconds > 1 ? 's' : '')
})

onUnmounted(() => {
    instanceReads.invalidate()
    processReads.invalidate()
    instancePageClosed = true
    instanceGeneration++
    clearInterval(launchElapsedTimer)
    unlistenProcesses()
    unlistenInstances()
    const instanceId = displayedInstanceRoute.value.params.id
    if (instanceId) {
        const { destroy } = useInstanceConsole(instanceId)
        destroy()
    }
})
</script>

<style>
.app-viewport:has(.instance-fixed-render) {
    scrollbar-gutter: auto;
}

.app-viewport:has(.instance-fixed-render) .page-transition-grid,
.app-viewport:has(.instance-fixed-render) .page-transition-layer {
    height: 100%;
    min-height: 0;
}
</style>
