<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import FileTextIcon from '@modrinth/assets/icons/file-text.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TriangleAlertIcon from '@modrinth/assets/icons/triangle-alert.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import VersionChannelIndicator from '@modrinth/ui/src/components/version/VersionChannelIndicator.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref } from 'vue'

import type { ProjectVersion } from '../../../data'
import PreviewModal from '../../../overlays/PreviewModal.vue'
import { injectContentInstall } from '../install'
import { renderMarkdown } from '../markdown'
import { useDiscover } from '../state'
import { formatVersionsForDisplay } from '../versions'

/**
 * The incompatibility warning of the content install flow: the shared `ContentUpdaterModal`
 * (packages/ui content-tab modals) in its `incompatibility-warning` mode, as App.vue mounts it.
 */
const { formatMessage } = useVIntl()
const install = injectContentInstall()
const discover = useDiscover()

const text = defineMessages({
    incompatibilityWarningHeader: {
        id: 'instances.updater-modal.incompatibility-warning-header',
        defaultMessage: 'Choose version',
    },
    searchVersionPlaceholder: {
        id: 'instances.updater-modal.search-placeholder',
        defaultMessage: 'Search version...',
    },
    noVersionsFound: {
        id: 'instances.updater-modal.no-versions',
        defaultMessage: 'No versions found',
    },
    noChangelog: {
        id: 'instances.updater-modal.no-changelog',
        defaultMessage: 'No changelog provided for this version.',
    },
    selectVersionPrompt: {
        id: 'instances.updater-modal.select-version',
        defaultMessage: 'Select a version to view its changelog',
    },
    incompatibilityWarning: {
        id: 'instances.updater-modal.incompatibility-warning',
        defaultMessage:
            'This version is not marked as compatible with this instance. Dependencies will not be installed automatically.',
    },
    installAnywayButton: {
        id: 'instances.updater-modal.install-anyway',
        defaultMessage: 'Install anyway',
    },
})

const modal = ref<InstanceType<typeof PreviewModal>>()
const searchQuery = ref('')
const selectedVersion = ref<ProjectVersion | null>(null)
let suppressCancelOnHide = false

const filteredVersions = computed(() => {
    const versions = install.incompatibilityVersions.value
    if (!searchQuery.value) return versions
    const query = searchQuery.value.toLowerCase()
    return versions.filter(
        (version) =>
            version.name.toLowerCase().includes(query) ||
            version.version_number.toLowerCase().includes(query) ||
            [...version.loaders, ...version.game_versions].some((value) =>
                value.toLowerCase().includes(query),
            ),
    )
})

function versionTypeBadgeClasses(version: ProjectVersion) {
    switch (version.version_type) {
        case 'release':
            return 'bg-highlight-green border-brand text-brand'
        case 'beta':
            return 'bg-highlight-blue border-brand-blue text-brand-blue'
        case 'alpha':
            return 'bg-highlight-purple border-brand-purple text-brand-purple'
        default:
            return 'bg-surface-4 border-surface-5 text-[var(--color-text-default)]'
    }
}

function formatLongDate(dateString: string) {
    return new Date(dateString).toLocaleDateString('en-US', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
    })
}

function formatLoaderGameVersion(version: ProjectVersion) {
    const loader = version.loaders[0]
        ? version.loaders[0].charAt(0).toUpperCase() + version.loaders[0].slice(1)
        : ''
    const tags = discover.tags.value?.gameVersions ?? []
    const gameVersions = tags.length
        ? formatVersionsForDisplay(version.game_versions, tags)
        : version.game_versions
    return [loader, gameVersions.join(', ')].filter(Boolean).join(' ')
}

function handleUpdate() {
    if (install.incompatibilityInstalling.value || !selectedVersion.value) return
    void install.handleIncompatibilityWarningInstall(selectedVersion.value)
}

function handleCancel() {
    install.handleIncompatibilityWarningCancel()
    hide()
}

function handleModalHide() {
    if (suppressCancelOnHide) {
        suppressCancelOnHide = false
        return
    }
    install.handleIncompatibilityWarningCancel()
}

function show(initialVersionId?: string) {
    searchQuery.value = ''
    const versions = install.incompatibilityVersions.value
    selectedVersion.value = initialVersionId
        ? (versions.find((version) => version.id === initialVersionId) ?? versions[0] ?? null)
        : (versions[0] ?? null)
    modal.value?.show()
}

function hide() {
    suppressCancelOnHide = true
    modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
    <PreviewModal
        ref="modal"
        max-width="min(928px, calc(1216px - 10rem))"
        width="min(928px, calc(1216px - 10rem))"
        :on-hide="handleModalHide"
        no-padding
    >
        <template #title>
            <Avatar
                v-if="install.incompatibilityProjectIcon.value"
                :src="install.incompatibilityProjectIcon.value"
                size="3rem"
                :tint-by="install.incompatibilityProjectName.value"
            />
            <span class="text-lg font-extrabold text-[var(--color-text-primary)]">{{
                formatMessage(text.incompatibilityWarningHeader)
            }}</span>
        </template>

        <div
            class="flex h-[min(550px,calc(760px-10rem))] border-[1px] border-solid border-transparent border-b-surface-4"
        >
            <div class="relative flex w-[300px] flex-col bg-surface-3">
                <div class="p-4 pb-2">
                    <StyledInput
                        v-model="searchQuery"
                        :icon="SearchIcon"
                        type="text"
                        :placeholder="formatMessage(text.searchVersionPlaceholder)"
                        wrapper-class="w-full"
                    />
                </div>

                <div class="flex-1 overflow-y-auto px-4 pb-16">
                    <div class="min-h-full">
                        <div class="flex flex-col gap-1.5" role="listbox">
                            <button
                                v-for="version in filteredVersions"
                                :key="version.id"
                                role="option"
                                :aria-selected="selectedVersion?.id === version.id"
                                class="flex h-10 cursor-pointer items-center rounded-xl border-none px-4 py-2.5 transition-colors"
                                :class="
                                    selectedVersion?.id === version.id
                                        ? 'bg-brand-highlight'
                                        : 'bg-transparent hover:bg-surface-4'
                                "
                                @click="selectedVersion = version"
                            >
                                <div class="flex w-full items-center justify-between gap-2">
                                    <div class="flex min-w-0 items-center gap-2">
                                        <VersionChannelIndicator
                                            :channel="version.version_type"
                                            size="sm"
                                            class="shrink-0"
                                        />
                                        <span
                                            v-tooltip="version.version_number"
                                            class="truncate font-semibold text-[var(--color-text-primary)]"
                                        >
                                            {{ version.version_number }}
                                        </span>
                                    </div>
                                </div>
                            </button>
                        </div>
                        <div
                            v-if="filteredVersions.length === 0"
                            class="p-4 text-center text-sm text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(text.noVersionsFound) }}
                        </div>
                    </div>
                </div>
            </div>

            <div class="w-px bg-divider" />

            <div
                class="relative flex min-h-0 min-w-0 flex-1 flex-col bg-surface-1"
                aria-live="polite"
            >
                <div v-if="selectedVersion" class="relative flex min-h-0 min-w-0 flex-1 flex-col">
                    <div class="bg-bg p-4">
                        <div class="flex flex-col gap-1.5">
                            <div class="flex items-center justify-between">
                                <div class="flex items-center gap-2">
                                    <span
                                        class="text-xl font-semibold text-[var(--color-text-primary)]"
                                    >
                                        {{ selectedVersion.version_number }}
                                    </span>
                                    <span
                                        class="flex flex-shrink-0 items-center rounded-full border border-solid px-2.5 py-0.5 text-sm font-medium"
                                        :class="versionTypeBadgeClasses(selectedVersion)"
                                    >
                                        {{
                                            formatMessage(
                                                selectedVersion.version_type === 'release'
                                                    ? commonMessages.release
                                                    : selectedVersion.version_type === 'beta'
                                                      ? commonMessages.beta
                                                      : commonMessages.alpha,
                                            )
                                        }}
                                    </span>
                                </div>
                                <span class="font-medium text-[var(--color-text-default)]">
                                    {{ formatLongDate(selectedVersion.date_published) }}
                                </span>
                            </div>
                            <div class="flex items-center gap-2">
                                <div class="flex items-center gap-2 rounded-xl">
                                    <FileTextIcon
                                        class="h-6 w-6 text-[var(--color-text-default)]"
                                    />
                                    <span class="font-medium text-[var(--color-text-default)]">{{
                                        formatMessage(commonMessages.changelogLabel)
                                    }}</span>
                                </div>
                                <span class="h-1.5 w-1.5 rounded-full bg-divider" />
                                <span class="font-medium text-[var(--color-text-default)]">
                                    {{ formatLoaderGameVersion(selectedVersion) }}
                                </span>
                            </div>
                        </div>
                    </div>

                    <div class="h-px bg-divider" />

                    <div class="min-h-0 flex-1 overflow-y-auto bg-bg p-4">
                        <!-- eslint-disable vue/no-v-html -->
                        <div
                            v-if="selectedVersion.changelog"
                            class="markdown [&_img]:h-auto [&_img]:max-w-full"
                            v-html="renderMarkdown(selectedVersion.changelog)"
                        />
                        <!-- eslint-enable vue/no-v-html -->
                        <div v-else class="italic text-[var(--color-text-tertiary)]">
                            {{ formatMessage(text.noChangelog) }}
                        </div>
                    </div>

                    <div
                        class="pointer-events-none absolute bottom-0 left-0 right-0 h-14 bg-gradient-to-t from-bg to-transparent"
                    />
                </div>
                <div
                    v-else
                    class="flex h-full flex-1 items-center justify-center bg-bg text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(text.selectVersionPrompt) }}
                </div>
            </div>
        </div>

        <div
            class="flex w-full flex-row items-center gap-4 border-x-0 border-b-0 border-t border-solid border-surface-4 p-4"
        >
            <div class="mr-auto flex max-w-[55%] flex-1 flex-row items-center gap-2 text-orange">
                <TriangleAlertIcon class="size-6 shrink-0" />
                <span>{{
                    install.incompatibilityMessage.value ??
                    formatMessage(text.incompatibilityWarning)
                }}</span>
            </div>
            <div class="ml-auto flex shrink-0 flex-row gap-2">
                <Button type="outlined" @click="handleCancel"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    type="colored"
                    color="orange"
                    :disabled="install.incompatibilityInstalling.value || !selectedVersion"
                    @click="handleUpdate"
                >
                    <SpinnerIcon
                        v-if="install.incompatibilityInstalling.value"
                        class="size-5 animate-spin"
                    />
                    <DownloadIcon v-else />
                    {{
                        install.incompatibilityInstalling.value
                            ? formatMessage(commonMessages.installingLabel)
                            : formatMessage(text.installAnywayButton)
                    }}
                </Button>
            </div>
        </div>
    </PreviewModal>
</template>
