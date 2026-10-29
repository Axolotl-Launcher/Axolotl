<template>
    <div
        ref="outerRef"
        data-tauri-drag-region
        class="min-w-0 overflow-hidden pl-3"
        :class="{ 'breadcrumb-fade-mask': isOverflowing }"
        :style="isOverflowing ? { '--scroll-distance': `-${overflowAmount}px` } : undefined"
        @mouseenter="onMouseEnter"
        @mouseleave="onMouseLeave"
    >
        <div
            ref="innerRef"
            data-tauri-drag-region
            class="flex w-fit items-center gap-1"
            :class="{ 'breadcrumbs-scroll': isAnimating }"
            @animationiteration="onAnimationIteration"
        >
            <template v-for="(breadcrumb, index) in breadcrumbs" :key="breadcrumb.name">
                <router-link
                    v-if="breadcrumb.link"
                    :to="{
                        path: breadcrumb.link.replace(
                            '{id}',
                            encodeURIComponent($route.params.id as string),
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
                </router-link>
                <span
                    v-else
                    data-tauri-drag-region
                    class="flex shrink-0 items-center gap-1 whitespace-nowrap text-[var(--color-text-primary)] font-semibold cursor-default select-none"
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
                <ChevronRightIcon
                    v-if="index < breadcrumbs.length - 1"
                    data-tauri-drag-region
                    class="w-5 h-5 shrink-0"
                />
            </template>
        </div>
    </div>
</template>

<script setup lang="ts">
import {
    ArrowBigUpDashIcon,
    ChangeSkinIcon,
    ChevronRightIcon,
    CodeIcon,
    CompassIcon,
    DownloadIcon,
    FileTextIcon,
    FlaskConicalIcon,
    FolderIcon,
    GlobeIcon,
    HeartIcon,
    HomeIcon,
    ImagesIcon,
    LibraryIcon,
    MapIcon,
    PackageIcon,
    PencilIcon,
    ServerIcon,
    SettingsIcon,
} from '@modrinth/assets'
import { Avatar, commonMessages, defineMessages, useVIntl } from '@modrinth/ui'
import { type Component, computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import { resolveBreadcrumbLabel } from '@/helpers/breadcrumb-label'
import { useBreadcrumbs } from '@/store/breadcrumbs'

interface Breadcrumb {
    name: string
    link?: string
    query?: Record<string, string>
    iconUrl?: string | null
}

const route = useRoute()
const breadcrumbData = useBreadcrumbs()
const { formatMessage } = useVIntl()

const messages = defineMessages({
    home: { id: 'app.navigation.home', defaultMessage: 'Home' },
    worlds: { id: 'app.navigation.worlds', defaultMessage: 'Worlds' },
    discoverContent: {
        id: 'app.navigation.discover-content',
        defaultMessage: 'Discover content',
    },
    skinSelector: { id: 'app.navigation.skin-selector', defaultMessage: 'Skin selector' },
    multiplayer: { id: 'app.navigation.multiplayer', defaultMessage: 'Multiplayer' },
    library: { id: 'app.navigation.library', defaultMessage: 'Library' },
    downloads: { id: 'app.navigation.downloads', defaultMessage: 'Downloads' },
    lab: { id: 'app.navigation.lab', defaultMessage: 'Lab' },
    gradientText: {
        id: 'app.lab.gradient-text.title',
        defaultMessage: 'Gradient text generator',
    },
    seedMap: { id: 'app.lab.seed-map.title', defaultMessage: 'Seed map' },
    schematicWorkshop: {
        id: 'app.lab.schematic-preview.title',
        defaultMessage: 'Schematic workshop',
    },
    modTranslation: {
        id: 'app.lab.mod-translation.title',
        defaultMessage: 'Mod translation',
    },
    recipeGenerator: {
        id: 'app.lab.recipe-generator.title',
        defaultMessage: 'Recipe generator',
    },
    commandGenerator: {
        id: 'app.lab.command-generator.title',
        defaultMessage: 'Command generator',
    },
    witchHutFinder: {
        id: 'app.lab.low-y-swamp-hut.title',
        defaultMessage: 'Low-Y swamp hut',
    },
    trialChamberFinder: {
        id: 'app.lab.trial-chamber.title',
        defaultMessage: 'Trial chamber',
    },
    skinEditor: { id: 'app.lab.skin-editor.title', defaultMessage: 'Skin editor' },
    content: { id: 'app.instance.tabs.content', defaultMessage: 'Content' },
    files: { id: 'app.instance.tabs.files', defaultMessage: 'Files' },
    studio: { id: 'instance.files.studio.title', defaultMessage: 'Studio' },
    logs: { id: 'app.instance.tabs.logs', defaultMessage: 'Logs' },
    screenshots: { id: 'app.navigation.screenshots', defaultMessage: 'Screenshots' },
    editWorld: { id: 'app.navigation.edit-world', defaultMessage: 'Edit world' },
    upgradeInstance: { id: 'app.instance.upgrade-instance', defaultMessage: 'Upgrade instance' },
})

const staticLabels = {
    Home: messages.home,
    Worlds: messages.worlds,
    'Discover content': messages.discoverContent,
    'Skin selector': messages.skinSelector,
    Multiplayer: messages.multiplayer,
    Library: messages.library,
    Downloads: messages.downloads,
    Settings: commonMessages.settingsLabel,
    Lab: messages.lab,
    'Gradient text generator': messages.gradientText,
    'Seed map': messages.seedMap,
    'Schematic workshop': messages.schematicWorkshop,
    'Mod translation': messages.modTranslation,
    'Recipe generator': messages.recipeGenerator,
    'Command generator': messages.commandGenerator,
    'Witch hut finder': messages.witchHutFinder,
    'Trial chamber finder': messages.trialChamberFinder,
    'Skin editor': messages.skinEditor,
    Content: messages.content,
    Files: messages.files,
    Studio: messages.studio,
    Logs: messages.logs,
    Screenshots: messages.screenshots,
    'Edit world': messages.editWorld,
    Upgrade: messages.upgradeInstance,
}

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

const breadcrumbs = computed<Breadcrumb[]>(() => {
    const additionalContext =
        route.meta.useContext === true
            ? breadcrumbData.context
            : route.meta.useRootContext === true
              ? breadcrumbData.rootContext
              : null
    const crumbs = (route.meta.breadcrumb ?? []) as Breadcrumb[]
    if (
        additionalContext?.name.startsWith('?') &&
        crumbs.some((crumb) => crumb.name === additionalContext.name)
    ) {
        return crumbs
    }
    return additionalContext ? [additionalContext as Breadcrumb, ...crumbs] : crumbs
})

function resolveLabel(name: string): string {
    return resolveBreadcrumbLabel(
        name,
        (key) => breadcrumbData.getName(key),
        staticLabels,
        (message) => formatMessage(message),
    )
}

function resolveIcon(breadcrumb: Breadcrumb): Component | undefined {
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

function resolveIconUrl(breadcrumb: Breadcrumb): string | null {
    return (
        breadcrumb.iconUrl ??
        (breadcrumb.name.startsWith('?') ? breadcrumbData.getIcon(breadcrumb.name.slice(1)) : null)
    )
}

// Overflow detection
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
    if (isOverflowing.value) {
        isAnimating.value = true
    }
}

function onMouseLeave() {
    hovered = false
    if (isAnimating.value) {
        stopping = true
    }
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
