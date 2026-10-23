<script setup lang="ts">
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, onUnmounted, ref, useId } from 'vue'

import { usePreview } from '../../../lib/state'
import PreviewFloatingActionBar from '../../../overlays/PreviewFloatingActionBar.vue'
import type { BrowseInstallContext } from '../browse-context'

/**
 * Port of `SelectedProjectsFloatingBar` (packages/ui browse-tab components): the install
 * cart of the browse and project pages, in the window's floating action bar.
 */
const { formatMessage } = useVIntl()
const preview = usePreview()

const text = defineMessages({
    ariaLabel: {
        id: 'browse.selected-projects-floating-bar.aria-label',
        defaultMessage: 'Selected projects',
    },
    selectedCount: {
        id: 'browse.selected-projects-floating-bar.selected-count',
        defaultMessage: '{count, plural, one {# project selected} other {# projects selected}}',
    },
    installButton: {
        id: 'browse.selected-projects-floating-bar.install',
        defaultMessage: 'Install {count, plural, one {# project} other {# projects}}',
    },
    showProjects: {
        id: 'browse.selected-projects-floating-bar.show-projects',
        defaultMessage:
            'Show {count, plural, one {# selected project} other {# selected projects}}',
    },
    hideProjects: {
        id: 'browse.selected-projects-floating-bar.hide-projects',
        defaultMessage: 'Hide selected projects',
    },
})

const props = defineProps<{ installContext?: BrowseInstallContext | null }>()

const selectedProjects = computed(() => props.installContext?.selectedProjects ?? [])
const selectedCount = computed(() => selectedProjects.value.length)
const iconStackOffset = 24
const isInstallingSelected = computed(() => props.installContext?.isInstallingSelected ?? false)
const shown = computed(() => selectedCount.value > 0 || isInstallingSelected.value)
const projectPreviewId = `selected-project-preview-${useId()}`
const projectPreviewOpen = ref(false)
const visibleProjects = computed(() => selectedProjects.value.slice(0, 3))
const overflowCount = computed(() => Math.max(0, selectedCount.value - 3))
const iconStackWidth = computed(() => {
    if (selectedCount.value === 0) return 0
    return (
        32 +
        (visibleProjects.value.length - 1 + (overflowCount.value > 0 ? 1 : 0)) * iconStackOffset
    )
})
const selectedCountText = computed(() =>
    formatMessage(text.selectedCount, { count: selectedCount.value }),
)
const installButtonText = computed(
    () =>
        props.installContext?.installButtonLabel ??
        formatMessage(text.installButton, { count: selectedCount.value }),
)
const actionButtonText = computed(() =>
    isInstallingSelected.value
        ? (props.installContext?.processingLabel ?? installButtonText.value)
        : installButtonText.value,
)

let cancelClose: (() => void) | null = null

function openProjectPreview() {
    if (selectedCount.value < 2) return
    cancelClose?.()
    cancelClose = null
    projectPreviewOpen.value = true
}

function closeProjectPreview() {
    cancelClose?.()
    cancelClose = null
    projectPreviewOpen.value = false
}

function scheduleProjectPreviewClose() {
    cancelClose?.()
    cancelClose = preview.clock.after(160, closeProjectPreview)
}

function clearSelected() {
    if (isInstallingSelected.value) return
    void props.installContext?.clearSelected?.()
}

function installSelected() {
    if (isInstallingSelected.value) return
    void props.installContext?.installSelected?.()
}

onUnmounted(closeProjectPreview)
</script>

<template>
    <PreviewFloatingActionBar
        :shown="shown"
        :aria-label="formatMessage(text.ariaLabel)"
        allow-overflow
        hide-when-modal-open
    >
        <div class="flex min-w-0 items-center gap-0.5">
            <div
                v-if="selectedCount > 0"
                class="relative h-8 shrink-0"
                @mouseenter="openProjectPreview"
                @mouseleave="scheduleProjectPreviewClose"
            >
                <button
                    type="button"
                    class="project-stack-trigger relative h-8 cursor-pointer rounded-lg border-0 bg-transparent p-0 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                    :style="{ width: `${iconStackWidth}px` }"
                    :aria-controls="projectPreviewId"
                    :aria-expanded="projectPreviewOpen"
                    :aria-label="
                        formatMessage(projectPreviewOpen ? text.hideProjects : text.showProjects, {
                            count: selectedCount,
                        })
                    "
                    @focus="openProjectPreview"
                    @blur="scheduleProjectPreviewClose"
                    @click="openProjectPreview"
                    @keydown.esc.prevent.stop="closeProjectPreview"
                >
                    <span aria-hidden="true">
                        <span
                            v-for="(project, index) in visibleProjects"
                            :key="project.id"
                            v-tooltip="project.name"
                            class="absolute top-0 flex h-8 w-8 items-center justify-center overflow-hidden rounded-lg border-[1.5px] border-solid border-surface-3 bg-surface-4"
                            :style="{
                                left: `${index * iconStackOffset}px`,
                                zIndex: visibleProjects.length - index,
                            }"
                        >
                            <Avatar
                                :src="project.iconUrl"
                                :alt="project.name"
                                :tint-by="project.id"
                                size="100%"
                                no-shadow
                                class="selected-project-avatar"
                            />
                        </span>
                        <span
                            v-if="overflowCount > 0"
                            class="absolute top-0 flex h-8 w-8 items-center justify-center rounded-lg border-[1.5px] border-solid border-surface-3 bg-surface-4 text-xs font-bold text-[var(--color-text-primary)]"
                            :style="{
                                left: `${visibleProjects.length * iconStackOffset}px`,
                                zIndex: 0,
                            }"
                        >
                            +{{ overflowCount }}
                        </span>
                    </span>
                </button>

                <Transition name="selected-project-preview">
                    <div
                        v-if="projectPreviewOpen && selectedCount > 1"
                        :id="projectPreviewId"
                        class="selected-project-preview absolute bottom-[calc(100%+0.75rem)] left-0 z-20 flex w-[min(18rem,1216px)] flex-col gap-1 overflow-y-auto overflow-x-hidden rounded-lg border border-solid border-surface-5 bg-surface-2 p-2 shadow-[0px_6px_10px_0px_rgba(0,0,0,0.15),0px_16px_24px_0px_rgba(0,0,0,0.2)]"
                        role="list"
                        @mouseenter="openProjectPreview"
                        @mouseleave="scheduleProjectPreviewClose"
                    >
                        <div
                            v-for="project in selectedProjects"
                            :key="project.id"
                            class="selected-project-preview-item flex min-w-0 items-center gap-2 rounded-md bg-surface-3 px-2 py-1.5"
                            role="listitem"
                        >
                            <Avatar
                                :src="project.iconUrl"
                                :alt="project.name"
                                :tint-by="project.id"
                                size="2rem"
                                no-shadow
                            />
                            <span
                                class="min-w-0 truncate text-sm font-semibold text-[var(--color-text-primary)]"
                            >
                                {{ project.name }}
                            </span>
                        </div>
                    </div>
                </Transition>
            </div>

            <span
                class="px-3 py-2 text-base font-semibold tabular-nums text-[var(--color-text-primary)]"
            >
                {{ selectedCountText }}
            </span>
            <div class="mx-0.5 h-6 w-px bg-surface-5" />
            <Button
                type="quiet"
                class="!text-[var(--color-text-default)]"
                :disabled="isInstallingSelected"
                @click="clearSelected"
                ><span>{{ formatMessage(commonMessages.clearButton) }}</span>
            </Button>
        </div>

        <div class="ml-auto shrink-0">
            <Button
                type="colored"
                color="brand"
                :disabled="isInstallingSelected"
                @click="installSelected"
                ><SpinnerIcon v-if="isInstallingSelected" class="animate-spin" />
                <PlusIcon v-else />
                {{ actionButtonText }}
            </Button>
        </div>
    </PreviewFloatingActionBar>
</template>

<style scoped>
:deep(.selected-project-avatar) {
    background-color: var(--surface-4);
}

.selected-project-preview {
    max-height: min(18rem, calc(800px - 12rem));
    scrollbar-gutter: stable;
}

.selected-project-preview-enter-active,
.selected-project-preview-leave-active {
    transition:
        opacity 160ms ease,
        transform 180ms ease;
}

.selected-project-preview-enter-from,
.selected-project-preview-leave-to {
    opacity: 0;
    transform: translateY(0.5rem) scale(0.98);
}

@media (prefers-reduced-motion) {
    .selected-project-preview-enter-active,
    .selected-project-preview-leave-active {
        transition: none;
    }
}
</style>
