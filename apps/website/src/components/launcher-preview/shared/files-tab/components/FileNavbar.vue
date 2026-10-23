<template>
    <header
        class="flex select-none flex-col gap-4 @container"
        :aria-label="formatMessage(messages.fileNavigation)"
    >
        <div v-if="!isEditing" class="flex items-center gap-2 @[800px]:hidden">
            <StyledInput
                :model-value="searchQuery"
                :icon="SearchIcon"
                type="search"
                name="search"
                autocomplete="off"
                :placeholder="formatMessage(messages.searchFiles)"
                class="!h-10"
                input-class="!h-10"
                wrapper-class="flex-1 min-w-0"
                @update:model-value="onSearchInput"
            />
        </div>
        <div class="flex items-center justify-between gap-2">
            <nav
                :aria-label="formatMessage(messages.breadcrumbNavigation)"
                class="m-0 flex min-w-0 flex-shrink items-center p-0 text-[var(--color-text-primary)]"
            >
                <ol class="m-0 flex min-w-0 flex-shrink list-none items-center p-0">
                    <li class="mr-4 flex-shrink-0">
                        <Button
                            v-tooltip="formatMessage(messages.backToHome)"
                            circular
                            icon-only
                            class="!size-10 bg-surface-4 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
                            @click="$emit('navigateHome')"
                            @mouseenter="$emit('prefetchHome')"
                            ><HomeIcon />
                            <span class="sr-only">{{ formatMessage(messages.home) }}</span>
                        </Button>
                    </li>
                    <li class="m-0 -ml-2 min-w-0 flex-shrink p-0">
                        <ol
                            ref="breadcrumbOuter"
                            class="m-0 flex min-w-0 flex-shrink items-center overflow-hidden p-0"
                            :class="{ 'breadcrumb-fade-mask': isBreadcrumbOverflowing }"
                            :style="
                                isBreadcrumbOverflowing
                                    ? { '--scroll-distance': `-${breadcrumbOverflowAmount}px` }
                                    : undefined
                            "
                            @mouseenter="onBreadcrumbMouseEnter"
                            @mouseleave="onBreadcrumbMouseLeave"
                        >
                            <TransitionGroup
                                ref="breadcrumbInner"
                                name="breadcrumb"
                                tag="span"
                                class="relative flex w-fit items-center"
                                :class="{ 'breadcrumbs-scroll': isBreadcrumbAnimating }"
                                @animationiteration="onBreadcrumbAnimationIteration"
                            >
                                <li
                                    v-for="(segment, index) in breadcrumbs"
                                    :key="`${segment || index}-group`"
                                    class="relative flex shrink-0 items-center text-sm"
                                >
                                    <div class="flex shrink-0 items-center">
                                        <Button
                                            type="quiet"
                                            class="cursor-pointer whitespace-nowrap focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
                                            :aria-current="
                                                !isEditing && index === breadcrumbs.length - 1
                                                    ? 'location'
                                                    : undefined
                                            "
                                            :class="{
                                                '!text-[var(--color-text-primary)]':
                                                    !isEditing && index === breadcrumbs.length - 1,
                                            }"
                                            @click="$emit('navigate', index)"
                                            >{{ segment || '' }}
                                        </Button>
                                        <ChevronRightIcon
                                            v-if="index < breadcrumbs.length - 1 || isEditing"
                                            class="size-4 flex-shrink-0 text-[var(--color-text-tertiary)]"
                                            aria-hidden="true"
                                        />
                                    </div>
                                </li>
                            </TransitionGroup>
                            <li
                                v-if="isEditing && editingFileName"
                                class="flex items-center px-3 text-base"
                            >
                                <span
                                    class="font-semibold !text-[var(--color-text-primary)]"
                                    aria-current="location"
                                >
                                    {{ editingFileName }}
                                </span>
                            </li>
                        </ol>
                    </li>
                </ol>
            </nav>

            <div v-if="!isEditing" class="flex flex-shrink-0 items-center gap-2">
                <StyledInput
                    id="search-folder"
                    :model-value="searchQuery"
                    :icon="SearchIcon"
                    type="search"
                    name="search"
                    autocomplete="off"
                    :placeholder="formatMessage(messages.searchFiles)"
                    class="hidden !h-10 @[800px]:inline-flex"
                    input-class="!h-10"
                    wrapper-class="w-full sm:w-[280px]"
                    @update:model-value="onSearchInput"
                />

                <slot name="before-refresh" />

                <Button
                    v-if="showRefreshButton"
                    type="outlined"
                    class="flex !h-10 items-center gap-2"
                    :disabled="refreshing"
                    @click="handleRefresh"
                    ><RefreshCwIcon
                        aria-hidden="true"
                        class="h-5 w-5 transition-transform"
                        :class="refreshing ? 'animate-spin' : ''"
                    />
                    {{ formatMessage(commonMessages.refreshButton) }}
                </Button>

                <OverflowMenu
                    :dropdown-id="`create-new-${baseId}`"
                    position="bottom"
                    direction="left"
                    :aria-label="formatMessage(messages.createNew)"
                    :disabled="disabled"
                    :tooltip="disabled ? disabledTooltip : undefined"
                    class="!h-10 justify-center gap-2"
                    :options="[
                        { id: 'file', action: () => $emit('create', 'file') },
                        { id: 'directory', action: () => $emit('create', 'directory') },
                        { divider: true, shown: showInstallFromUrl ?? false },
                        { id: 'upload-zip', shown: false, action: () => $emit('uploadZip') },
                        {
                            id: 'install-from-url',
                            shown: showInstallFromUrl ?? false,
                            action: () => $emit('unzipFromUrl', false),
                        },
                        {
                            id: 'install-cf-pack',
                            shown: showInstallFromUrl ?? false,
                            action: () => $emit('unzipFromUrl', true),
                        },
                    ]"
                >
                    <PlusIcon aria-hidden="true" class="h-5 w-5" />
                    <DropdownIcon aria-hidden="true" class="h-5 w-5" />
                    <template #file>
                        <BoxIcon aria-hidden="true" /> {{ formatMessage(messages.newFile) }}
                    </template>
                    <template #directory>
                        <FolderOpenIcon aria-hidden="true" />
                        {{ formatMessage(messages.newFolder) }}
                    </template>
                    <template #upload-zip>
                        <FileArchiveIcon aria-hidden="true" />
                        {{ formatMessage(messages.uploadFromZip) }}
                    </template>
                    <template #install-from-url>
                        <LinkIcon aria-hidden="true" />
                        {{ formatMessage(messages.uploadFromZipUrl) }}
                    </template>
                    <template #install-cf-pack>
                        <CurseForgeIcon aria-hidden="true" />
                        {{ formatMessage(messages.installCurseForgePack) }}
                    </template>
                </OverflowMenu>
            </div>

            <div v-else-if="!isEditingImage" class="flex gap-2">
                <Button
                    v-if="isLogFile"
                    v-tooltip="formatMessage(messages.shareLog)"
                    type="quiet"
                    circular
                    icon-only
                    :aria-label="formatMessage(messages.shareLog)"
                    @click="$emit('share')"
                    ><ShareIcon />
                </Button>
                <IconButton
                    v-tooltip="formatMessage(messages.findInFile)"
                    :label="formatMessage(messages.findInFile)"
                    :type="isEditorFindOpen ? 'chip' : 'quiet'"
                    :color="isEditorFindOpen ? 'brand' : undefined"
                    :aria-pressed="isEditorFindOpen"
                    @click="$emit('find')"
                >
                    <SearchIcon />
                </IconButton>
            </div>
        </div>
    </header>
</template>

<script setup lang="ts">
import CurseForgeIcon from '@modrinth/assets/external/curseforge.svg?component'
import BoxIcon from '@modrinth/assets/icons/box.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import DropdownIcon from '@modrinth/assets/icons/dropdown.svg?component'
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import HomeIcon from '@modrinth/assets/icons/home.svg?component'
import LinkIcon from '@modrinth/assets/icons/link.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import ShareIcon from '@modrinth/assets/icons/share.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import IconButton from '@modrinth/ui/src/components/base/buttons/IconButton.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import OverflowMenu from '../../../overlays/PreviewOverflowMenu.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    fileNavigation: {
        id: 'files.navbar.file-navigation',
        defaultMessage: 'File navigation',
    },
    breadcrumbNavigation: {
        id: 'files.navbar.breadcrumb-navigation',
        defaultMessage: 'Breadcrumb navigation',
    },
    backToHome: {
        id: 'files.navbar.back-to-home',
        defaultMessage: 'Back to home',
    },
    home: {
        id: 'files.navbar.home',
        defaultMessage: 'Home',
    },
    searchFiles: {
        id: 'files.navbar.search-files',
        defaultMessage: 'Search files',
    },
    createNew: {
        id: 'files.navbar.create-new',
        defaultMessage: 'Create new...',
    },
    newFile: {
        id: 'files.navbar.new-file',
        defaultMessage: 'New file',
    },
    newFolder: {
        id: 'files.navbar.new-folder',
        defaultMessage: 'New folder',
    },
    uploadFromZip: {
        id: 'files.navbar.upload-from-zip',
        defaultMessage: 'Upload from .zip file',
    },
    uploadFromZipUrl: {
        id: 'files.navbar.upload-from-zip-url',
        defaultMessage: 'Upload from .zip URL',
    },
    installCurseForgePack: {
        id: 'files.navbar.install-curseforge-pack',
        defaultMessage: 'Install CurseForge pack',
    },
    shareLog: {
        id: 'files.navbar.share-log',
        defaultMessage: 'Share log',
    },
    findInFile: {
        id: 'files.navbar.find-in-file',
        defaultMessage: 'Find in file',
    },
})

const props = defineProps<{
    breadcrumbs: string[]
    isEditing: boolean
    editingFileName?: string
    editingFilePath?: string
    isEditingImage?: boolean
    isEditorFindOpen?: boolean
    searchQuery: string
    showRefreshButton?: boolean
    showInstallFromUrl?: boolean
    baseId: string
    disabled?: boolean
    disabledTooltip?: string
}>()

const emit = defineEmits<{
    navigate: [index: number]
    navigateHome: []
    prefetchHome: []
    'update:searchQuery': [value: string]
    create: [type: 'file' | 'directory']
    upload: []
    uploadZip: []
    unzipFromUrl: [cf: boolean]
    refresh: []
    share: []
    find: []
}>()

const refreshing = ref(false)

function onSearchInput(value: string | number | undefined) {
    emit('update:searchQuery', String(value ?? ''))
}

function handleRefresh() {
    emit('refresh')
    refreshing.value = true
    setTimeout(() => {
        refreshing.value = false
    }, 1000)
}

const breadcrumbOuter = ref<HTMLElement | null>(null)
const breadcrumbInner = ref<{ $el: HTMLElement } | null>(null)
const isBreadcrumbOverflowing = ref(false)
const isBreadcrumbAnimating = ref(false)
const breadcrumbOverflowAmount = ref(0)

let bcHovered = false
let bcStopping = false

function checkBreadcrumbOverflow() {
    const inner = breadcrumbInner.value?.$el
    if (!breadcrumbOuter.value || !inner) return
    const overflow = inner.scrollWidth - breadcrumbOuter.value.clientWidth
    isBreadcrumbOverflowing.value = overflow > 0
    breadcrumbOverflowAmount.value = overflow + 12
}

function onBreadcrumbMouseEnter() {
    bcHovered = true
    bcStopping = false
    if (isBreadcrumbOverflowing.value) {
        isBreadcrumbAnimating.value = true
    }
}

function onBreadcrumbMouseLeave() {
    bcHovered = false
    if (isBreadcrumbAnimating.value) {
        bcStopping = true
    }
}

function onBreadcrumbAnimationIteration() {
    if (bcStopping && !bcHovered) {
        isBreadcrumbAnimating.value = false
        bcStopping = false
    }
}

let bcResizeObserver: ResizeObserver | null = null

onMounted(() => {
    checkBreadcrumbOverflow()
    bcResizeObserver = new ResizeObserver(checkBreadcrumbOverflow)
    if (breadcrumbOuter.value) bcResizeObserver.observe(breadcrumbOuter.value)
    const innerEl = breadcrumbInner.value?.$el
    if (innerEl) bcResizeObserver.observe(innerEl)
})

onBeforeUnmount(() => {
    bcResizeObserver?.disconnect()
})

watch(
    () => props.breadcrumbs,
    () => {
        requestAnimationFrame(checkBreadcrumbOverflow)
    },
)

const isLogFile = computed(() => {
    return (
        props.editingFilePath?.startsWith('logs') ||
        props.editingFilePath?.startsWith('crash-reports') ||
        props.editingFilePath?.endsWith('.log')
    )
})
</script>

<style scoped>
.breadcrumb-move,
.breadcrumb-enter-active,
.breadcrumb-leave-active {
    transition: all 0.2s ease;
}

.breadcrumb-enter-from {
    opacity: 0;
    transform: translateX(-10px) scale(0.9);
}

.breadcrumb-leave-to {
    opacity: 0;
    transform: translateX(-10px) scale(0.8);
    filter: blur(4px);
}

.breadcrumb-leave-active {
    position: relative;
    pointer-events: none;
}

.breadcrumb-move {
    z-index: 1;
}

.breadcrumb-fade-mask {
    mask-image: linear-gradient(
        to right,
        transparent,
        black 12px,
        black calc(100% - 12px),
        transparent
    );
}

.breadcrumbs-scroll {
    animation: breadcrumb-scroll 10s ease-in-out infinite;
}

@keyframes breadcrumb-scroll {
    0% {
        transform: translateX(0);
    }
    35%,
    65% {
        transform: translateX(var(--scroll-distance));
    }
    100% {
        transform: translateX(0);
    }
}
</style>
