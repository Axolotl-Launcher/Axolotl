<script setup lang="ts">
import BoxIcon from '@modrinth/assets/icons/box.svg?component'
import BoxesIcon from '@modrinth/assets/icons/boxes.svg?component'
import DropdownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import EditIcon from '@modrinth/assets/icons/edit.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import GlobeIcon from '@modrinth/assets/icons/globe.svg?component'
import ImageIcon from '@modrinth/assets/icons/image.svg?component'
import MoreVerticalIcon from '@modrinth/assets/icons/more-vertical.svg?component'
import PackageIcon from '@modrinth/assets/icons/package.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SettingsIcon from '@modrinth/assets/icons/settings.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import TerminalSquareIcon from '@modrinth/assets/icons/terminal-square.svg?component'
import Badge from '@modrinth/ui/src/components/base/Badge.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import ButtonGroup from '@modrinth/ui/src/components/base/buttons/ButtonGroup.vue'
import NavTabs from '@modrinth/ui/src/components/base/NavTabs.vue'
import OverflowMenu from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, onUnmounted, ref, shallowRef, watch } from 'vue'
import { type RouteLocationNormalizedLoaded, RouterView, useRoute, useRouter } from 'vue-router'

import { loadProject, type ProjectDetail, projectIcon } from '../../data'
import { instanceServerProjectId, isServerProjectLink } from '../../lib/instance-project'
import { createShortcutNotice, revealInFolder } from '../../lib/platform/opener'
import { instanceUpgradeNotices } from '../../lib/post-upgrade-notice'
import { usePreview } from '../../lib/state'
import { injectPreviewWindow } from '../../lib/window'
import PreviewContextMenu from '../../overlays/PreviewContextMenu.vue'
import InstanceIcon from '../../shell/InstanceIcon.vue'
import { useHomeState } from '../home/state'
import { getLoaderIcon } from './components/loader-icons'
import ServerProjectStats from './components/ServerProjectStats.vue'
import { useSymlinkWarningDismiss } from './components/symlink-warning'
import SymlinkInstanceWarning from './components/SymlinkInstanceWarning.vue'
import ExportModal from './export/ExportModal.vue'
import { instancePageMessages as messages, previewMessages } from './messages'
import InstanceSettingsModal from './settings/InstanceSettingsModal.vue'

/**
 * Port of `apps/app-frontend/src/pages/instance/Index.vue`: the instance header
 * (icon, name, version and loader, play time, play/stop with the launch timer,
 * settings and overflow actions), the tab bar and the active tab.
 */
defineOptions({ name: 'InstancePage' })

const { formatMessage } = useVIntl()
const route = useRoute()
const router = useRouter()
const preview = usePreview()
const { layer } = injectPreviewWindow()

/** The page is keyed by instance id in the shell, so the id is fixed for its lifetime. */
const pageId = String(route.params.id ?? '')
const displayedInstanceRoute = shallowRef<RouteLocationNormalizedLoaded>(router.currentRoute.value)

watch(
    () => router.currentRoute.value,
    (nextRoute) => {
        if (nextRoute.path.startsWith('/instance') && nextRoute.params.id === pageId) {
            displayedInstanceRoute.value = nextRoute
        }
    },
    { immediate: true },
)

const instance = computed(() => preview.instances.get(pageId))
const homeState = useHomeState()
const offline = false
const linkedServerProject = shallowRef<ProjectDetail | null>(null)
const isServerInstance = computed(
    () =>
        isServerProjectLink(instance.value?.link) || !!linkedServerProject.value?.minecraft_server,
)
const notices = instanceUpgradeNotices(preview)
const postUpgradeNotice = computed(() => notices.get(pageId))
const canUpgradeInstance = computed(() => {
    const current = instance.value
    return (
        current?.install_stage === 'installed' &&
        (!current.link || current.link.type === 'shared_instance' || !!current.symlink_target)
    )
})
watch(
    () => instanceServerProjectId(instance.value?.link),
    async (id) => {
        linkedServerProject.value = null
        if (!id) return
        const project = await loadProject(id).catch(() => null)
        if (instanceServerProjectId(instance.value?.link) !== id) return
        linkedServerProject.value = project?.minecraft_server ? project : null
        const address = linkedServerProject.value?.minecraft_java_server?.address
        if (
            address &&
            !homeState.worlds.value.some(
                (world) =>
                    world.instance_id === pageId &&
                    world.type === 'server' &&
                    world.address === address,
            )
        ) {
            const index = homeState.worlds.value.filter(
                (world) => world.instance_id === pageId && world.type === 'server',
            ).length
            homeState.worlds.value.push({
                type: 'server',
                instance_id: pageId,
                name: project!.title,
                address,
                index,
                path: `servers.dat#${index}`,
                pack_status: 'prompt',
                icon: projectIcon(id),
                display_status: 'normal',
            })
        }
    },
    { immediate: true },
)
const symlinkWarning = useSymlinkWarningDismiss(() => instance.value?.id)

// `get(id)` fails with "not managed" for an unknown or deleted instance, and
// the app sends the user back home.
watch(
    instance,
    (value) => {
        if (!value && router.currentRoute.value.params.id === pageId)
            void router.push({ path: '/' })
    },
    { immediate: true },
)

const showInstancePlayTime = computed(
    () => preview.settings.feature_flags.show_instance_play_time ?? true,
)
const loaderIcon = computed(() => getLoaderIcon(instance.value?.loader))

/* --------------------------------------------------------------- play state */

const process = computed(
    () => preview.processes.processes.value.find((entry) => entry.instance.id === pageId) ?? null,
)
/** Waiting for another instance's launch preparation to finish first. */
const queued = ref(false)
const playing = computed(
    () =>
        !!process.value &&
        (process.value.stage === 'running' || process.value.stage === 'stopping'),
)
const loading = computed(() => queued.value || (!!process.value && !playing.value))
const stopping = computed(() => process.value?.stage === 'stopping')

const launchElapsedSeconds = ref(0)
let launchTimer: (() => void) | undefined

function startLaunchTimer() {
    launchTimer?.()
    launchElapsedSeconds.value = 0
    let elapsed = 0
    launchTimer = preview.clock.every((delta) => {
        elapsed += delta
        launchElapsedSeconds.value = Math.floor(elapsed / 1000)
    })
}

function stopLaunchTimer() {
    launchTimer?.()
    launchTimer = undefined
}

watch(loading, (value) => {
    if (!value) stopLaunchTimer()
})

let stopQueueWatch: (() => void) | undefined

function startInstance(_context: string) {
    const current = instance.value
    if (!current || loading.value || playing.value) return
    startLaunchTimer()
    if (preview.processes.launch(current)) return
    // The launcher prepares one launch at a time; this one starts after it.
    queued.value = true
    stopQueueWatch?.()
    stopQueueWatch = watch(
        () => preview.processes.pendingInstanceId.value,
        (pending) => {
            if (pending) return
            stopQueueWatch?.()
            stopQueueWatch = undefined
            queued.value = false
            if (instance.value) preview.processes.launch(instance.value)
        },
    )
}

function handlePlayServer() {
    const address = linkedServerProject.value?.minecraft_java_server?.address
    if (!address || loading.value || playing.value) return
    homeState.pendingJoins.set(pageId, {
        instanceId: pageId,
        type: 'server',
        identifier: address,
    })
    startInstance('InstancePageServer')
}

function stopInstance(_context: string) {
    if (process.value) preview.processes.stop(process.value.uuid)
}

/** The subpages emit `play` after starting a world; the state is derived from the processes. */
function updatePlayState() {}

/* ------------------------------------------------------------- time played */

const timePlayed = computed(() =>
    instance.value ? Math.floor(instance.value.submitted_time_played / 1000) : 0,
)

const timePlayedHumanized = computed(() => {
    const hours = Math.floor(timePlayed.value / 3600)
    if (hours >= 1) return formatMessage(previewMessages.timePlayedHours, { count: hours })
    const minutes = Math.floor(timePlayed.value / 60)
    if (minutes >= 1) return formatMessage(previewMessages.timePlayedMinutes, { count: minutes })
    return formatMessage(previewMessages.timePlayedSeconds, { count: timePlayed.value })
})

/* -------------------------------------------------------------------- tabs */

const basePath = computed(
    () =>
        `/instance/${encodeURIComponent(String(displayedInstanceRoute.value.params.id ?? pageId))}`,
)

/**
 * `renderMode: 'fixed'` routes (the log console) pin the header and tabs and
 * scroll only the tab body, so the console resolves a bounded height.
 */
/** The Files tab's Studio takes the whole page: the header and tabs are hidden and the body is fixed. */
const isStudioMode = computed(() => displayedInstanceRoute.value.name === 'FileStudio')
const isUpgradeMode = computed(() => displayedInstanceRoute.value.path.includes('/upgrade'))
const isFixedRender = computed(
    () => displayedInstanceRoute.value.name === 'Logs' || isStudioMode.value,
)

const tabs = computed(() => [
    { label: formatMessage(messages.contentTab), href: `${basePath.value}`, icon: BoxesIcon },
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
    { label: formatMessage(messages.worldsTab), href: `${basePath.value}/worlds`, icon: GlobeIcon },
    {
        label: formatMessage(messages.logsTab),
        href: `${basePath.value}/logs`,
        icon: TerminalSquareIcon,
    },
])

/* ------------------------------------------------------------- breadcrumbs */

function displayIconUrl() {
    const current = instance.value
    if (!current) return null
    if (current.icon_path) return current.icon_path
    const loaderIcons: Record<string, string> = {
        fabric: '/launcher-preview/loaders/fabric.webp',
        forge: '/launcher-preview/loaders/forge.webp',
        neoforge: '/launcher-preview/loaders/neoforge.webp',
        quilt: '/launcher-preview/loaders/quilt.webp',
    }
    return loaderIcons[current.loader] ?? '/launcher-preview/instance-icons/grass-block.webp'
}

function updateBreadcrumbs() {
    if (!instance.value) return
    preview.breadcrumbs.setRootContext({
        name: instance.value.name,
        link: displayedInstanceRoute.value.path,
        query: displayedInstanceRoute.value.query as Record<string, string>,
        iconUrl: displayIconUrl(),
    })
}

updateBreadcrumbs()
watch(
    () => [instance.value?.id, instance.value?.name, instance.value?.icon_path],
    () => updateBreadcrumbs(),
)

/* ----------------------------------------------------------------- actions */

const settingsModal = ref<InstanceType<typeof InstanceSettingsModal>>()
const exportModal = ref<InstanceType<typeof ExportModal>>()
const options = ref<InstanceType<typeof PreviewContextMenu> | null>(null)

/** `showInstanceInFolder`: Finder belongs to the desktop app. */
function showInstanceInFolder() {
    if (!instance.value) return
    revealInFolder(instance.value.path)
}

/** `createInstanceShortcut`: shortcut files are written by the desktop app. */
function createShortcut() {
    if (!instance.value) return
    createShortcutNotice()
}

async function copyPath() {
    if (!instance.value) return
    try {
        await navigator.clipboard?.writeText(instance.value.path)
    } catch {
        // Clipboard access is permission-gated in the browser; the app copies silently too.
    }
}

function handleRightClick(event: MouseEvent) {
    const current = instance.value
    if (!current) return
    const baseOptions = [
        { name: 'add_content', section: 'manage' as const, shown: !current.link },
        { type: 'divider' },
        { name: 'edit', section: 'manage' as const },
        { name: 'open_folder', section: 'navigate' as const },
        { name: 'copy_path', section: 'navigate' as const },
    ]
    options.value?.showMenu(
        event,
        current,
        playing.value
            ? [{ name: 'stop', section: 'primary', color: 'danger' }, ...baseOptions]
            : [{ name: 'play', section: 'primary', color: 'primary' }, ...baseOptions],
    )
}

function handleOptionsClick(args: { option: string; item: unknown }) {
    switch (args.option) {
        case 'play':
            startInstance('InstancePageContextMenu')
            break
        case 'stop':
            stopInstance('InstancePageContextMenu')
            break
        case 'add_content':
            void router.push({
                path: `/browse/${instance.value?.loader === 'vanilla' ? 'datapack' : 'mod'}`,
                query: { i: pageId },
            })
            break
        case 'edit':
            settingsModal.value?.show()
            break
        case 'open_folder':
            showInstanceInFolder()
            break
        case 'copy_path':
            void copyPath()
            break
    }
}

function repairInstance() {
    const current = instance.value
    if (!current) return
    current.install_stage = 'minecraft_installing'
    preview.clock.after(2400, () => {
        if (instance.value === current) current.install_stage = 'installed'
    })
}

onUnmounted(() => {
    stopLaunchTimer()
    stopQueueWatch?.()
})
</script>

<template>
    <div
        v-if="instance"
        :class="{
            'flex h-full min-h-0 flex-col': isFixedRender,
            'instance-fixed-render': isFixedRender,
        }"
    >
        <div
            :class="['p-6 pb-4 pr-2', { 'shrink-0': isFixedRender, hidden: isStudioMode }]"
            @contextmenu.prevent.stop="(event) => handleRightClick(event)"
        >
            <ExportModal ref="exportModal" :instance="instance" />
            <InstanceSettingsModal
                :key="instance.id"
                ref="settingsModal"
                :instance="instance"
                :offline="offline"
            />
            <!-- ContentPageHeader, with the window's md breakpoint baked in -->
            <div class="flex flex-col gap-2 border-0 border-b border-solid border-divider pb-4">
                <div class="flex flex-wrap items-start gap-4">
                    <div class="flex min-w-0 flex-1 gap-4">
                        <InstanceIcon
                            :icon-path="instance.icon_path"
                            :instance-id="instance.id"
                            :loader="instance.loader"
                            :alt="instance.name"
                            size="64px"
                        />
                        <div class="flex min-w-0 flex-col justify-center gap-2">
                            <div class="flex flex-col justify-center gap-1.5">
                                <div class="flex flex-wrap items-center gap-2">
                                    <h1
                                        class="m-0 text-2xl font-semibold leading-none text-[var(--color-text-primary)]"
                                    >
                                        {{ instance.name }}
                                    </h1>
                                </div>
                            </div>
                            <div class="flex flex-wrap gap-3 empty:hidden">
                                <div class="flex flex-wrap items-center gap-2">
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

                                        <div class="h-1.5 w-1.5 rounded-full bg-surface-5"></div>

                                        <div
                                            class="flex items-center gap-1.5 font-medium capitalize"
                                        >
                                            <component
                                                :is="loaderIcon"
                                                v-if="loaderIcon"
                                                class="h-4 w-4"
                                            />
                                            {{ instance.loader }}
                                            {{
                                                instance.loader_version
                                                    ? instance.loader_version
                                                    : ''
                                            }}
                                        </div>

                                        <template v-if="showInstancePlayTime">
                                            <div
                                                class="h-1.5 w-1.5 rounded-full bg-surface-5"
                                            ></div>

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
                                    <template v-else-if="linkedServerProject">
                                        <ServerProjectStats :project="linkedServerProject" />
                                        <div class="flex items-center gap-1.5 font-medium">
                                            {{ formatMessage(messages.linkedTo) }}
                                            <img
                                                :src="projectIcon(linkedServerProject.id)"
                                                :alt="linkedServerProject.title"
                                                class="h-6 w-6 rounded"
                                                draggable="false"
                                            />
                                            <router-link
                                                :to="`/project/${linkedServerProject.id}`"
                                                class="hover:underline"
                                                >{{ linkedServerProject.title }}</router-link
                                            >
                                        </div>
                                    </template>

                                    <div
                                        v-if="
                                            instance.symlink_target && symlinkWarning.isHidden.value
                                        "
                                        class="h-1.5 w-1.5 rounded-full bg-surface-5"
                                    ></div>
                                    <SymlinkInstanceWarning
                                        v-if="
                                            instance.symlink_target && symlinkWarning.isHidden.value
                                        "
                                        :symlink-target="instance.symlink_target"
                                        badge-only
                                    />
                                </div>
                            </div>
                        </div>
                    </div>
                    <div class="flex flex-wrap items-center gap-2">
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
                                type="colored"
                                color="brand"
                                size="xl"
                                @click="repairInstance()"
                                ><DownloadIcon />
                                {{ formatMessage(commonMessages.repairButton) }}
                            </Button>
                            <Button
                                v-else-if="playing"
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
                                v-else-if="!loading && !isServerInstance"
                                type="colored"
                                color="brand"
                                size="xl"
                                @click="startInstance('InstancePage')"
                                ><PlayIcon />
                                {{ formatMessage(commonMessages.playButton) }}
                            </Button>
                            <ButtonGroup v-else-if="!loading && isServerInstance">
                                <Button
                                    type="colored"
                                    color="brand"
                                    size="xl"
                                    :disabled="!linkedServerProject?.minecraft_java_server?.address"
                                    @click="handlePlayServer"
                                >
                                    <PlayIcon />{{ formatMessage(commonMessages.playButton) }}
                                </Button>
                                <OverflowMenu
                                    :container="layer ?? undefined"
                                    :options="[
                                        { id: 'join-server', action: handlePlayServer },
                                        {
                                            id: 'launch-instance',
                                            action: () => startInstance('InstancePage'),
                                        },
                                    ]"
                                >
                                    <DropdownIcon />
                                    <template #join-server
                                        ><PlayIcon />{{
                                            formatMessage(messages.joinServer)
                                        }}</template
                                    >
                                    <template #launch-instance
                                        ><PlayIcon />{{
                                            formatMessage(messages.launchInstance)
                                        }}</template
                                    >
                                </OverflowMenu>
                            </ButtonGroup>
                            <Button v-else type="colored" color="brand" size="xl" disabled
                                ><SpinnerIcon class="animate-spin" />
                                {{
                                    formatMessage(messages.starting, {
                                        seconds: launchElapsedSeconds,
                                    })
                                }}
                            </Button>
                            <Button
                                v-tooltip="formatMessage(messages.instanceSettings)"
                                size="xl"
                                circular
                                icon-only
                                :label="formatMessage(messages.instanceSettings)"
                                @click="settingsModal?.show()"
                                ><SettingsIcon />
                            </Button>
                            <OverflowMenu
                                :container="layer ?? undefined"
                                :options="[
                                    { id: 'open-folder', action: () => showInstanceInFolder() },
                                    { id: 'export-mrpack', action: () => exportModal?.show() },
                                    { id: 'create-shortcut', action: () => createShortcut() },
                                    {
                                        id: 'upgrade-instance',
                                        shown: canUpgradeInstance,
                                        action: () => router.push(`${basePath}/upgrade`),
                                    },
                                ]"
                            >
                                <MoreVerticalIcon />
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
                                <template #upgrade-instance
                                    ><DownloadIcon />{{
                                        formatMessage(messages.upgradeInstance)
                                    }}</template
                                >
                            </OverflowMenu>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        <div
            data-onboarding-id="instance-tabs"
            :class="['px-6', { 'shrink-0': isFixedRender, hidden: isStudioMode || isUpgradeMode }]"
        >
            <SymlinkInstanceWarning
                v-if="instance.symlink_target && !symlinkWarning.isHidden.value"
                :symlink-target="instance.symlink_target"
                class="mb-3"
                dismissible
            />
            <NavTabs :links="tabs" />
        </div>
        <div
            :class="['p-6 pt-4', { 'flex min-h-0 flex-1 flex-col overflow-hidden': isFixedRender }]"
        >
            <RouterView v-slot="{ Component }" :key="instance.id" :route="displayedInstanceRoute">
                <template v-if="Component">
                    <Suspense :key="instance.id">
                        <component
                            :is="Component"
                            :instance="instance"
                            :options="options"
                            :offline="offline"
                            :playing="playing"
                            :installed="instance.install_stage !== 'installed'"
                            :is-server-instance="isServerInstance"
                            :server-project="linkedServerProject"
                            :open-settings="() => settingsModal?.show(1)"
                            @play="updatePlayState"
                            @stop="() => stopInstance('InstanceSubpage')"
                        ></component>
                    </Suspense>
                </template>
            </RouterView>
        </div>
        <PreviewContextMenu ref="options" @option-clicked="handleOptionsClick">
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
        </PreviewContextMenu>
    </div>
</template>

<style>
.launcher-preview-window .app-viewport:has(.instance-fixed-render) {
    scrollbar-gutter: auto;
}

.launcher-preview-window .app-viewport:has(.instance-fixed-render) .page-transition-grid,
.launcher-preview-window .app-viewport:has(.instance-fixed-render) .page-transition-layer {
    height: 100%;
    min-height: 0;
}
</style>
