<script setup lang="ts">
import RightArrowIcon from '@modrinth/assets/icons/right-arrow.svg?component'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useScrollIndicator } from '@modrinth/ui/src/composables/scroll-indicator.ts'
import { type Component, computed, nextTick, ref } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'

/**
 * Port of `packages/ui/src/components/modal/TabbedModal.vue` on the in-window
 * modal. The content pane's `h-screen max-h-[min(65vh,600px)]` is baked for the
 * 800px-high window (520px).
 */
export interface Tab {
    name: MessageDescriptor
    icon: Component
    content?: Component
    flushContent?: boolean
    href?: string
    badge?: MessageDescriptor
    shown?: boolean
    onboardingId?: string
}

const { formatMessage } = useVIntl()

const props = withDefaults(
    defineProps<{
        tabs: Tab[]
        header?: string
        maxWidth?: string
        width?: string
        closable?: boolean
        beforeTabChange?: (fromIndex: number, toIndex: number) => boolean
        beforeHide?: () => boolean
        onHide?: () => void
        onShow?: () => void
    }>(),
    {
        header: undefined,
        maxWidth: undefined,
        width: undefined,
        closable: true,
        beforeTabChange: undefined,
        beforeHide: undefined,
        onHide: undefined,
        onShow: undefined,
    },
)

const visibleTabs = computed(() => props.tabs.filter((tab) => tab.shown !== false))

const selectedTab = ref(0)

const scrollContainer = ref<HTMLElement | null>(null)
const { showTopFade, showBottomFade, checkScrollState, forceCheck } =
    useScrollIndicator(scrollContainer)

const modal = ref<InstanceType<typeof PreviewModal> | null>(null)

function setTab(index: number) {
    if (index === selectedTab.value || props.beforeTabChange?.(selectedTab.value, index) === false)
        return
    selectedTab.value = index
    nextTick(() => forceCheck())
}

function show(event?: MouseEvent) {
    modal.value?.show(event)
}

function hide() {
    modal.value?.hide()
}

defineExpose({ show, hide, selectedTab, setTab })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="header"
        :max-width="maxWidth"
        :width="width"
        :closable="closable"
        :on-hide="onHide"
        :before-hide="beforeHide"
        :on-show="onShow"
        no-padding
    >
        <template v-if="$slots.title" #title>
            <slot name="title" />
        </template>
        <div
            class="grid min-h-0 grid-cols-[auto_minmax(0,1fr)] grid-rows-[minmax(0,1fr)] p-6 pb-3 pr-0"
        >
            <div
                class="flex min-w-[200px] flex-col gap-1 border-0 border-r-[1px] border-solid border-divider pr-4"
            >
                <component
                    :is="tab.href ? 'a' : 'button'"
                    v-for="(tab, index) in visibleTabs"
                    :key="index"
                    :href="tab.href ?? undefined"
                    :data-onboarding-id="tab.onboardingId"
                    :target="tab.href ? '_blank' : undefined"
                    :rel="tab.href ? 'noopener noreferrer' : undefined"
                    :class="`flex cursor-pointer items-center gap-2 text-nowrap rounded-xl border-none px-4 py-2 text-left font-semibold no-underline transition-all active:scale-[0.97] ${!tab.href && selectedTab === index ? 'bg-button-bgSelected text-button-textSelected' : 'bg-transparent text-button-text hover:bg-surface-4 hover:text-[var(--color-text-primary)]'}`"
                    @click="!tab.href && setTab(index)"
                >
                    <component :is="tab.icon" class="h-4 w-4 flex-shrink-0" />
                    <span>{{ formatMessage(tab.name) }}</span>
                    <span
                        v-if="tab.badge"
                        class="rounded-full bg-brand-highlight px-1.5 py-0.5 text-xs font-bold text-brand"
                    >
                        {{ formatMessage(tab.badge) }}
                    </span>
                    <RightArrowIcon v-if="tab.href" class="ml-auto size-4" />
                </component>

                <slot name="footer" />
            </div>
            <div class="relative min-h-0 min-w-0 overflow-hidden">
                <Transition
                    enter-active-class="transition-all duration-200 ease-out"
                    enter-from-class="opacity-0 max-h-0"
                    enter-to-class="opacity-100 max-h-4"
                    leave-active-class="transition-all duration-200 ease-in"
                    leave-from-class="opacity-100 max-h-4"
                    leave-to-class="opacity-0 max-h-0"
                >
                    <div
                        v-if="showTopFade"
                        class="pointer-events-none absolute left-0 right-0 top-0 z-10 h-4 bg-gradient-to-b from-bg-raised to-transparent"
                    />
                </Transition>

                <div
                    ref="scrollContainer"
                    class="h-[520px] max-h-[520px] min-h-0"
                    :class="
                        visibleTabs[selectedTab]?.flushContent
                            ? 'overflow-hidden'
                            : 'overflow-y-auto px-6 pb-6'
                    "
                    @scroll="checkScrollState"
                >
                    <slot v-if="$slots.content" name="content" />
                    <Suspense v-else>
                        <component
                            :is="visibleTabs[selectedTab]?.content"
                            v-if="visibleTabs[selectedTab]?.content"
                        />
                    </Suspense>
                </div>

                <Transition
                    enter-active-class="transition-all duration-200 ease-out"
                    enter-from-class="opacity-0 max-h-0"
                    enter-to-class="opacity-100 max-h-16"
                    leave-active-class="transition-all duration-200 ease-in"
                    leave-from-class="opacity-100 max-h-16"
                    leave-to-class="opacity-0 max-h-0"
                >
                    <div
                        v-if="showBottomFade"
                        class="pointer-events-none absolute bottom-0 left-0 right-0 z-10 h-16 bg-gradient-to-t from-bg-raised to-transparent"
                    />
                </Transition>
            </div>
        </div>
    </PreviewModal>
</template>
