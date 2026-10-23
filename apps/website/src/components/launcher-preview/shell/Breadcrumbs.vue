<template>
    <div
        ref="outerRef"
        class="min-w-0 overflow-hidden pl-3"
        :class="{ 'breadcrumb-fade-mask': isOverflowing }"
        :style="isOverflowing ? { '--scroll-distance': `-${overflowAmount}px` } : undefined"
        @mouseenter="onMouseEnter"
        @mouseleave="onMouseLeave"
    >
        <div
            ref="innerRef"
            class="flex w-fit items-center gap-1"
            :class="{ 'breadcrumbs-scroll': isAnimating }"
            @animationiteration="onAnimationIteration"
        >
            <template v-for="(breadcrumb, index) in breadcrumbs" :key="breadcrumb.name">
                <RouterLink
                    v-if="breadcrumb.link"
                    :to="{
                        path: breadcrumb.link.replace(
                            '{id}',
                            encodeURIComponent(String(route.params.id ?? '')),
                        ),
                        query: breadcrumb.query,
                    }"
                    class="flex shrink-0 items-center gap-1 whitespace-nowrap text-[var(--color-text-default)]"
                >
                    <Avatar
                        v-if="resolveIconUrl(breadcrumb)"
                        :src="resolveIconUrl(breadcrumb)"
                        :alt="resolveLabel(breadcrumb.name)"
                        size="20px"
                        no-shadow
                        raised
                        class="shrink-0 !rounded-md"
                    />
                    <component
                        :is="resolveIcon(breadcrumb)"
                        v-else-if="resolveIcon(breadcrumb)"
                        class="size-5 shrink-0 text-[var(--color-text-default)]"
                        aria-hidden="true"
                    />
                    {{ resolveLabel(breadcrumb.name) }}
                </RouterLink>
                <span
                    v-else
                    class="flex shrink-0 cursor-default select-none items-center gap-1 whitespace-nowrap font-semibold text-[var(--color-text-primary)]"
                >
                    <Avatar
                        v-if="resolveIconUrl(breadcrumb)"
                        :src="resolveIconUrl(breadcrumb)"
                        :alt="resolveLabel(breadcrumb.name)"
                        size="20px"
                        no-shadow
                        raised
                        class="shrink-0 !rounded-md"
                    />
                    <component
                        :is="resolveIcon(breadcrumb)"
                        v-else-if="resolveIcon(breadcrumb)"
                        class="size-5 shrink-0 text-[var(--color-text-default)]"
                        aria-hidden="true"
                    />
                    {{ resolveLabel(breadcrumb.name) }}
                </span>
                <ChevronRightIcon v-if="index < breadcrumbs.length - 1" class="size-5 shrink-0" />
            </template>
        </div>
    </div>
</template>

<script setup lang="ts">
import ArrowBigUpDashIcon from '@modrinth/assets/icons/arrow-big-up-dash.svg?component'
import ChangeSkinIcon from '@modrinth/assets/icons/change-skin.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import CodeIcon from '@modrinth/assets/icons/code.svg?component'
import CompassIcon from '@modrinth/assets/icons/compass.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import FileTextIcon from '@modrinth/assets/icons/file-text.svg?component'
import FlaskConicalIcon from '@modrinth/assets/icons/flask-conical.svg?component'
import FolderIcon from '@modrinth/assets/icons/folder.svg?component'
import GlobeIcon from '@modrinth/assets/icons/globe.svg?component'
import HeartIcon from '@modrinth/assets/icons/heart.svg?component'
import HomeIcon from '@modrinth/assets/icons/home.svg?component'
import ImagesIcon from '@modrinth/assets/icons/images.svg?component'
import LibraryIcon from '@modrinth/assets/icons/library.svg?component'
import MapIcon from '@modrinth/assets/icons/map.svg?component'
import PackageIcon from '@modrinth/assets/icons/package.svg?component'
import PencilIcon from '@modrinth/assets/icons/pencil.svg?component'
import ServerIcon from '@modrinth/assets/icons/server.svg?component'
import SettingsIcon from '@modrinth/assets/icons/settings.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { type Component, computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'

import { breadcrumbMessages } from '../lib/messages'
import { usePreview } from '../lib/state'
import type { BreadcrumbContext } from '../lib/state/breadcrumbs'

/** Port of the app's titlebar Breadcrumbs, including the hover marquee for long trails. */
const route = useRoute()
const { breadcrumbs: breadcrumbData } = usePreview()
const { formatMessage } = useVIntl()

const staticLabels = {
    Home: breadcrumbMessages.home,
    Worlds: breadcrumbMessages.worlds,
    'Discover content': breadcrumbMessages.discoverContent,
    'Skin selector': breadcrumbMessages.skinSelector,
    Multiplayer: breadcrumbMessages.multiplayer,
    Library: breadcrumbMessages.library,
    Downloads: breadcrumbMessages.downloads,
    Settings: commonMessages.settingsLabel,
    Lab: breadcrumbMessages.lab,
    'Gradient text generator': breadcrumbMessages.gradientText,
    'Seed map': breadcrumbMessages.seedMap,
    'Schematic workshop': breadcrumbMessages.schematicWorkshop,
    'Mod translation': breadcrumbMessages.modTranslation,
    'Skin editor': breadcrumbMessages.skinEditor,
    Content: breadcrumbMessages.content,
    Files: breadcrumbMessages.files,
    Studio: breadcrumbMessages.studio,
    Logs: breadcrumbMessages.logs,
    Screenshots: breadcrumbMessages.screenshots,
    'Edit world': breadcrumbMessages.editWorld,
    Upgrade: breadcrumbMessages.upgradeInstance,
} as const

const staticIcons: Record<string, Component> = {
    Home: HomeIcon,
    Worlds: GlobeIcon,
    'Discover content': CompassIcon,
    'Skin selector': ChangeSkinIcon,
    Multiplayer: ServerIcon,
    Library: LibraryIcon,
    Downloads: DownloadIcon,
    Settings: SettingsIcon,
    Lab: FlaskConicalIcon,
    'Gradient text generator': FlaskConicalIcon,
    'Seed map': MapIcon,
    'Schematic workshop': CodeIcon,
    'Mod translation': CodeIcon,
    'Skin editor': PencilIcon,
    Content: PackageIcon,
    Files: FolderIcon,
    Studio: CodeIcon,
    Logs: FileTextIcon,
    'Edit world': PencilIcon,
    Upgrade: ArrowBigUpDashIcon,
    Favorites: HeartIcon,
    Versions: PackageIcon,
    Gallery: ImagesIcon,
    Screenshots: ImagesIcon,
    'Drop help': FileTextIcon,
    'Recipe generator': FlaskConicalIcon,
    Downloaded: DownloadIcon,
    Modpacks: PackageIcon,
    LibraryServers: ServerIcon,
    Custom: PackageIcon,
    Shared: PackageIcon,
    Saved: HeartIcon,
}

const breadcrumbs = computed<BreadcrumbContext[]>(() => {
    const additionalContext =
        route.meta.useContext === true
            ? breadcrumbData.state.context
            : route.meta.useRootContext === true
              ? breadcrumbData.state.rootContext
              : null
    const crumbs = (route.meta.breadcrumb ?? []) as BreadcrumbContext[]
    if (
        additionalContext?.name.startsWith('?') &&
        crumbs.some((crumb) => crumb.name === additionalContext.name)
    ) {
        return crumbs
    }
    return additionalContext ? [additionalContext, ...crumbs] : crumbs
})

function resolveLabel(name: string): string {
    if (name.startsWith('?')) return breadcrumbData.getName(name.slice(1))
    const label = staticLabels[name as keyof typeof staticLabels]
    return label === undefined ? name : formatMessage(label)
}

function resolveIcon(breadcrumb: BreadcrumbContext): Component | undefined {
    if (breadcrumb.iconUrl || breadcrumbData.getIcon(breadcrumb.name.slice(1))) return undefined
    const dynamicIcons: Record<string, Component> = {
        '?Project': PackageIcon,
        '?Version': PackageIcon,
        '?BrowseTitle': CompassIcon,
        '?FavoritesTitle': HeartIcon,
    }
    if (dynamicIcons[breadcrumb.name]) return dynamicIcons[breadcrumb.name]
    const key = breadcrumb.name.startsWith('?') ? resolveLabel(breadcrumb.name) : breadcrumb.name
    return staticIcons[key]
}

function resolveIconUrl(breadcrumb: BreadcrumbContext): string | null {
    return (
        breadcrumb.iconUrl ??
        (breadcrumb.name.startsWith('?') ? breadcrumbData.getIcon(breadcrumb.name.slice(1)) : null)
    )
}

const outerRef = ref<HTMLDivElement | null>(null)
const innerRef = ref<HTMLDivElement | null>(null)
const isOverflowing = ref(false)
const isAnimating = ref(false)
const overflowAmount = ref(0)

let hovered = false
let stopping = false

function checkOverflow() {
    if (!outerRef.value || !innerRef.value) return
    const overflow = innerRef.value.scrollWidth - outerRef.value.clientWidth
    isOverflowing.value = overflow > 0
    overflowAmount.value = overflow + 12
}

function onMouseEnter() {
    hovered = true
    stopping = false
    if (isOverflowing.value) isAnimating.value = true
}

function onMouseLeave() {
    hovered = false
    if (isAnimating.value) stopping = true
}

function onAnimationIteration() {
    if (stopping && !hovered) {
        isAnimating.value = false
        stopping = false
    }
}

let resizeObserver: ResizeObserver | null = null

onMounted(() => {
    checkOverflow()
    resizeObserver = new ResizeObserver(checkOverflow)
    if (outerRef.value) resizeObserver.observe(outerRef.value)
    if (innerRef.value) resizeObserver.observe(innerRef.value)
})

onBeforeUnmount(() => {
    resizeObserver?.disconnect()
})

watch(
    breadcrumbs,
    () => {
        breadcrumbData.resetToNames(breadcrumbs.value)
        requestAnimationFrame(checkOverflow)
    },
    { immediate: true },
)
</script>

<style scoped>
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
