<template>
    <nav
        v-if="filteredLinks.length > 1"
        ref="scrollContainer"
        :role="mode === 'local' ? 'tablist' : undefined"
        class="relative flex w-fit select-none overflow-x-auto rounded-full bg-surface-3 p-1 text-sm font-bold"
        :class="{ 'shadow-xl border border-solid border-surface-4': mode === 'navigation' }"
    >
        <template v-if="mode === 'navigation'">
            <RouterLink
                v-for="(link, index) in filteredLinks"
                v-show="link.shown ?? true"
                :key="link.href"
                ref="tabLinkElements"
                :replace="replace"
                :to="query ? (link.href ? `?${query}=${link.href}` : '?') : link.href"
                :data-onboarding-id="link.onboardingId"
                class="button-animation z-[1] flex flex-row items-center gap-2 px-4 py-2 focus:rounded-full"
                :class="getSSRFallbackClasses(index)"
                @click="saveSliderSnapshot(true)"
                @mouseenter="link.onHover?.()"
                @focus="link.onHover?.()"
            >
                <component
                    :is="link.icon"
                    v-if="link.icon"
                    class="size-5"
                    :class="getIconClasses(index)"
                />
                <span class="text-nowrap" :class="getLabelClasses(index)">
                    {{ link.label }}
                </span>
            </RouterLink>
        </template>

        <template v-else>
            <!--
				A local tab carries an id derived from its href so a consumer can point
				its panel's aria-labelledby at `nav-tab-<href>`. The href is also the
				v-for key, so the id is unique without a prefix prop to thread through.
			-->
            <button
                v-for="(link, index) in filteredLinks"
                v-show="link.shown ?? true"
                :id="link.href ? `nav-tab-${link.href}` : undefined"
                :key="link.href"
                ref="tabLinkElements"
                type="button"
                role="tab"
                :aria-selected="index === currentActiveIndex"
                class="button-animation z-[1] flex flex-row items-center gap-2 border-0 bg-transparent px-4 py-2 text-inherit hover:cursor-pointer focus:rounded-full"
                :class="getSSRFallbackClasses(index)"
                @click="emit('tabClick', index, link)"
            >
                <component
                    :is="link.icon"
                    v-if="link.icon"
                    class="size-5"
                    :class="getIconClasses(index)"
                />
                <span class="text-nowrap" :class="getLabelClasses(index)">
                    {{ link.label }}
                </span>
            </button>
        </template>

        <!-- Animated slider background -->
        <div
            v-if="sliderReady && currentActiveIndex !== -1"
            class="pointer-events-none absolute h-[calc(100%-0.5rem)] overflow-hidden rounded-full p-1"
            :class="[
                subpageSelected ? 'bg-surface-4' : 'bg-button-bgSelected',
                { 'navtabs-transition': transitionsEnabled },
            ]"
            :style="sliderStyle"
            aria-hidden="true"
        />
    </nav>
</template>

<script setup lang="ts">
import type { Component } from 'vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'

const route = useRoute()

interface Tab {
    label: string
    href: string
    shown?: boolean
    icon?: Component
    subpages?: string[]
    onHover?: () => void
    onboardingId?: string
}

interface SliderSnapshot {
    left: number
    top: number
    right: number
    bottom: number
    savedAt: number
    preserveOnUnmount: boolean
}

const sliderSnapshotMaxAge = 1000
const navigationSliderSnapshots = new Map<string, SliderSnapshot>()

const props = withDefaults(
    defineProps<{
        replace?: boolean
        links: Tab[]
        query?: string
        mode?: 'navigation' | 'local'
        activeIndex?: number
    }>(),
    {
        mode: 'navigation',
        query: undefined,
        activeIndex: undefined,
    },
)

const emit = defineEmits<{
    tabClick: [index: number, tab: Tab]
}>()

// DOM refs
const scrollContainer = ref<HTMLElement | null>(null)
const tabLinkElements = ref<HTMLElement[]>()
let containerResizeObserver: ResizeObserver | null = null

// Slider pos state
const sliderLeft = ref(4)
const sliderTop = ref(4)
const sliderRight = ref(4)
const sliderBottom = ref(4)

// active tab state
const currentActiveIndex = ref(-1)
const subpageSelected = ref(false)

// SSR state
const sliderReady = ref(false)
const transitionsEnabled = ref(false)

// Stagger delays for the trailing edges of the slider animation
const sliderDelays = ref({ left: '0ms', top: '0ms', right: '0ms', bottom: '0ms' })

const filteredLinks = computed(() => props.links.filter((link) => link.shown ?? true))
const navigationGroupKey = computed(() =>
    props.mode === 'navigation'
        ? filteredLinks.value.map((link) => link.href.split('?')[0]).join('|')
        : null,
)

const sliderStyle = computed(() => ({
    left: `${sliderLeft.value}px`,
    top: `${sliderTop.value}px`,
    right: `${sliderRight.value}px`,
    bottom: `${sliderBottom.value}px`,
    opacity: sliderReady.value && currentActiveIndex.value !== -1 ? 1 : 0,
}))

const leftDelay = computed(() => sliderDelays.value.left)
const rightDelay = computed(() => sliderDelays.value.right)
const topDelay = computed(() => sliderDelays.value.top)
const bottomDelay = computed(() => sliderDelays.value.bottom)

const isActiveAndNotSubpage = computed(
    () => (index: number) => currentActiveIndex.value === index && !subpageSelected.value,
)

function getSSRFallbackClasses(index: number) {
    if (sliderReady.value) return {}
    if (currentActiveIndex.value !== index) return {}

    return {
        'rounded-full': true,
        'bg-button-bgSelected': !subpageSelected.value,
        'bg-surface-4': subpageSelected.value,
    }
}

function getIconClasses(index: number) {
    return {
        'text-button-textSelected': isActiveAndNotSubpage.value(index),
        'text-[var(--color-text-tertiary)]': !isActiveAndNotSubpage.value(index),
    }
}

function getLabelClasses(index: number) {
    return {
        'text-button-textSelected': isActiveAndNotSubpage.value(index),
        'text-[var(--color-text-primary)]': !isActiveAndNotSubpage.value(index),
    }
}

function computeActiveIndex(): { index: number; isSubpage: boolean } {
    if (props.mode === 'local' && props.activeIndex !== undefined) {
        return {
            index: Math.min(props.activeIndex, filteredLinks.value.length - 1),
            isSubpage: false,
        }
    }

    for (let i = filteredLinks.value.length - 1; i >= 0; i--) {
        const link = filteredLinks.value[i]
        const decodedPath = decodeURIComponent(route.path)
        const decodedHref = decodeURIComponent(link.href.split('?')[0])

        if (props.query) {
            const queryValue = route.query[props.query]
            if (queryValue === link.href || (!queryValue && !link.href)) {
                return { index: i, isSubpage: false }
            }
            continue
        }

        if (decodedPath === decodedHref) {
            return { index: i, isSubpage: false }
        }

        const isSubpageMatch =
            (decodedPath.startsWith(decodedHref) &&
                (decodedPath.length === decodedHref.length ||
                    decodedPath[decodedHref.length] === '/')) ||
            link.subpages?.some((subpage) => decodedPath.includes(subpage))

        if (isSubpageMatch) {
            return { index: i, isSubpage: true }
        }
    }

    return { index: -1, isSubpage: false }
}

function getTabElement(index: number): HTMLElement | null {
    if (index === -1) return null

    const container = scrollContainer.value as HTMLElement | undefined
    if (!container) return null

    const tabs = container.querySelectorAll('.button-animation')
    const element = tabs[index] as HTMLElement | undefined

    if (!element) return null

    return element
}

function measureActiveTab() {
    const el = getTabElement(currentActiveIndex.value)
    if (!el?.offsetParent) return null
    const parent = el.offsetParent as HTMLElement
    return {
        left: el.offsetLeft,
        top: el.offsetTop,
        right: parent.offsetWidth - el.offsetLeft - el.offsetWidth,
        bottom: parent.offsetHeight - el.offsetTop - el.offsetHeight,
    }
}

function animateSliderTo(newPosition: {
    left: number
    top: number
    right: number
    bottom: number
}) {
    const STAGGER_DELAY = '200ms'

    sliderDelays.value = {
        left: newPosition.left < sliderLeft.value ? '0ms' : STAGGER_DELAY,
        right: newPosition.left < sliderLeft.value ? STAGGER_DELAY : '0ms',
        top: newPosition.top < sliderTop.value ? '0ms' : STAGGER_DELAY,
        bottom: newPosition.top < sliderTop.value ? STAGGER_DELAY : '0ms',
    }

    sliderLeft.value = newPosition.left
    sliderRight.value = newPosition.right
    sliderTop.value = newPosition.top
    sliderBottom.value = newPosition.bottom
}

function applySliderPosition(
    newPosition: { left: number; top: number; right: number; bottom: number },
    animate: boolean,
) {
    if (!animate) {
        const restore = transitionsEnabled.value
        transitionsEnabled.value = false
        sliderLeft.value = newPosition.left
        sliderTop.value = newPosition.top
        sliderRight.value = newPosition.right
        sliderBottom.value = newPosition.bottom
        sliderReady.value = true
        requestAnimationFrame(() => {
            transitionsEnabled.value = restore
        })
        return
    }

    animateSliderTo(newPosition)
    sliderReady.value = true
}

function positionSlider() {
    const newPosition = measureActiveTab()
    if (!newPosition) return

    // First paint: snap so the pill never slides in from the default 4/4 inset
    // (that read as a left jump after browse finished loading).
    if (!sliderReady.value) {
        transitionsEnabled.value = false
        applySliderPosition(newPosition, false)
        requestAnimationFrame(() => {
            transitionsEnabled.value = true
        })
        return
    }

    applySliderPosition(newPosition, true)
}

function snapSliderToActiveTab() {
    // ResizeObserver can fire mid-layout while browse swaps skeletons for cards.
    // Measure on the next frame, and only commit when the rect actually moved.
    requestAnimationFrame(() => {
        const newPosition = measureActiveTab()
        if (!newPosition) return
        if (
            newPosition.left === sliderLeft.value &&
            newPosition.top === sliderTop.value &&
            newPosition.right === sliderRight.value &&
            newPosition.bottom === sliderBottom.value
        ) {
            return
        }
        applySliderPosition(newPosition, false)
    })
}

function saveSliderSnapshot(preserveOnUnmount = false) {
    const key = navigationGroupKey.value
    if (!key || !sliderReady.value || currentActiveIndex.value === -1) return

    navigationSliderSnapshots.set(key, {
        left: sliderLeft.value,
        top: sliderTop.value,
        right: sliderRight.value,
        bottom: sliderBottom.value,
        savedAt: Date.now(),
        preserveOnUnmount,
    })
}

async function updateActiveTab() {
    await nextTick()
    const { index, isSubpage } = computeActiveIndex()
    currentActiveIndex.value = index
    subpageSelected.value = isSubpage

    if (index !== -1) {
        positionSlider()
    } else {
        sliderLeft.value = 0
        sliderRight.value = 0
    }
}

const initialActive = computeActiveIndex()
currentActiveIndex.value = initialActive.index
subpageSelected.value = initialActive.isSubpage

const restoredNavigationGroupKey = navigationGroupKey.value
const navigationGroupSnapshot = restoredNavigationGroupKey
    ? navigationSliderSnapshots.get(restoredNavigationGroupKey)
    : undefined
const restoredSliderSnapshot =
    !!navigationGroupSnapshot &&
    Date.now() - navigationGroupSnapshot.savedAt <= sliderSnapshotMaxAge
if (restoredSliderSnapshot) {
    sliderLeft.value = navigationGroupSnapshot.left
    sliderTop.value = navigationGroupSnapshot.top
    sliderRight.value = navigationGroupSnapshot.right
    sliderBottom.value = navigationGroupSnapshot.bottom
    sliderReady.value = true
    transitionsEnabled.value = true
    navigationSliderSnapshots.delete(restoredNavigationGroupKey)
} else if (restoredNavigationGroupKey) {
    navigationSliderSnapshots.delete(restoredNavigationGroupKey)
}

onMounted(() => {
    if (!restoredSliderSnapshot) {
        void updateActiveTab()
    } else {
        requestAnimationFrame(() => {
            requestAnimationFrame(() => void updateActiveTab())
        })
    }
})

watch(
    scrollContainer,
    (el) => {
        containerResizeObserver?.disconnect()
        containerResizeObserver = null
        if (!el) return
        containerResizeObserver = new ResizeObserver(() => snapSliderToActiveTab())
        containerResizeObserver.observe(el)
    },
    { immediate: true },
)

onBeforeUnmount(() => {
    containerResizeObserver?.disconnect()
    containerResizeObserver = null
    const key = navigationGroupKey.value
    const existingSnapshot = key ? navigationSliderSnapshots.get(key) : undefined
    if (
        existingSnapshot?.preserveOnUnmount &&
        Date.now() - existingSnapshot.savedAt <= sliderSnapshotMaxAge
    ) {
        return
    }
    saveSliderSnapshot()
})

watch(
    () => [route.path, route.query],
    () => {
        if (props.mode === 'navigation') {
            updateActiveTab()
        }
    },
)

watch(
    () => props.activeIndex,
    () => {
        if (props.mode === 'local') {
            updateActiveTab()
        }
    },
)

watch(
    () => props.links,
    async () => {
        await nextTick()
        updateActiveTab()
    },
    { deep: true },
)
</script>

<style scoped>
.navtabs-transition {
    transition:
        left 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(leftDelay),
        right 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(rightDelay),
        top 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(topDelay),
        bottom 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(bottomDelay),
        opacity 250ms cubic-bezier(0.5, 0, 0.2, 1) 50ms;
}
</style>
