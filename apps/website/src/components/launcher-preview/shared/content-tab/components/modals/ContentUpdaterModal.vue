<script setup lang="ts">
import CircleAlertIcon from '@modrinth/assets/icons/circle-alert.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import EyeOffIcon from '@modrinth/assets/icons/eye-off.svg?component'
import FileTextIcon from '@modrinth/assets/icons/file-text.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TriangleAlertIcon from '@modrinth/assets/icons/triangle-alert.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import VersionChannelIndicator from '@modrinth/ui/src/components/version/VersionChannelIndicator.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useContentUpdaterFiltering } from '@modrinth/ui/src/layouts/shared/content-tab/components/modals/content-updater-modal/use-content-updater-filtering.ts'
import { useContentUpdaterSelection } from '@modrinth/ui/src/layouts/shared/content-tab/components/modals/content-updater-modal/use-content-updater-selection.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import {
    versionChangesGameVersion,
    versionMatchesCompatibilityTarget,
} from '@modrinth/ui/src/utils/version-compatibility.ts'
import { computed, ref, toRef } from 'vue'

import { usePreview } from '../../../../lib/state'
import PreviewConfirmModal from '../../../../overlays/PreviewConfirmModal.vue'
import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { renderMarkdown } from '../../../../screens/discover/markdown'

/**
 * Port of `packages/ui/src/layouts/shared/content-tab/components/modals/content-updater-modal/index.vue`
 * on the in-window modal. `min(928px, calc(95vw - 10rem))` and
 * `min(550px, calc(95vh - 10rem))` are baked for the 1280x800 window; the
 * changelog renders with the preview's `renderHighlightedString` port.
 */
export interface UpdaterVersion {
    id: string
    project_id?: string
    name: string
    version_number: string
    version_type: 'release' | 'beta' | 'alpha'
    game_versions: string[]
    loaders: string[]
    date_published: string
    changelog?: string | null
}

const { formatMessage } = useVIntl()
const preview = usePreview()
const debug = () => {}

const messages = defineMessages({
    updateVersionHeader: {
        id: 'instances.updater-modal.header',
        defaultMessage: 'Update version',
    },
    incompatibilityWarningHeader: {
        id: 'instances.updater-modal.incompatibility-warning-header',
        defaultMessage: 'Choose version',
    },
    switchModpackVersionHeader: {
        id: 'instances.updater-modal.header-modpack',
        defaultMessage: 'Switch modpack version',
    },
    searchVersionPlaceholder: {
        id: 'instances.updater-modal.search-placeholder',
        defaultMessage: 'Search version...',
    },
    noVersionsFound: {
        id: 'instances.updater-modal.no-versions',
        defaultMessage: 'No versions found',
    },
    showIncompatible: {
        id: 'instances.updater-modal.show-incompatible',
        defaultMessage: 'Show incompatible',
    },
    hideIncompatible: {
        id: 'instances.updater-modal.hide-incompatible',
        defaultMessage: 'Hide incompatible',
    },
    noChangelog: {
        id: 'instances.updater-modal.no-changelog',
        defaultMessage: 'No changelog provided for this version.',
    },
    selectVersionPrompt: {
        id: 'instances.updater-modal.select-version',
        defaultMessage: 'Select a version to view its changelog',
    },
    updateWarningApp: {
        id: 'instances.updater-modal.warning-app',
        defaultMessage:
            'Updating can break your instance. Review version changelogs and back up first.',
    },
    updateWarningWeb: {
        id: 'instances.updater-modal.warning-web',
        defaultMessage:
            'Updating can break your world. Review version changelogs and back up first.',
    },
    incompatibilityWarning: {
        id: 'instances.updater-modal.incompatibility-warning',
        defaultMessage:
            'This version is not marked as compatible with this instance. Dependencies will not be installed automatically.',
    },
    downgradeToVersion: {
        id: 'instances.updater-modal.downgrade-to',
        defaultMessage: 'Downgrade to {version}',
    },
    updateToVersion: {
        id: 'instances.updater-modal.update-to',
        defaultMessage: 'Update to {version}',
    },
    switchVersionHeader: {
        id: 'instances.updater-modal.header-switch',
        defaultMessage: 'Switch version',
    },
    switchToVersion: {
        id: 'instances.updater-modal.switch-to',
        defaultMessage: 'Switch to {version}',
    },
    currentBadge: {
        id: 'instances.updater-modal.badge.current',
        defaultMessage: 'Current',
    },
    incompatibleBadge: {
        id: 'instances.updater-modal.badge.incompatible',
        defaultMessage: 'Incompatible',
    },
    loadingVersions: {
        id: 'instances.updater-modal.loading-versions',
        defaultMessage: 'Loading versions...',
    },
    loadingChangelog: {
        id: 'instances.updater-modal.loading-changelog',
        defaultMessage: 'Loading changelog...',
    },
    incompatibleUpdateHeader: {
        id: 'instances.updater-modal.incompatible-update.header',
        defaultMessage: 'Update to incompatible version?',
    },
    incompatibleUpdateDescription: {
        id: 'instances.updater-modal.incompatible-update.description',
        defaultMessage:
            '{version} is not marked as compatible with this installation. It may fail to launch or behave unexpectedly.',
    },
    updateAnywayButton: {
        id: 'instances.updater-modal.incompatible-update.proceed',
        defaultMessage: 'Update anyway',
    },
    installAnywayButton: {
        id: 'instances.updater-modal.install-anyway',
        defaultMessage: 'Install anyway',
    },
    searchCompatButton: {
        id: 'instances.updater-modal.search-compat',
        defaultMessage: 'Find compatible version',
    },
})

const props = withDefaults(
    defineProps<{
        versions: UpdaterVersion[]
        currentGameVersion: string
        currentLoader: string
        currentVersionId: string
        isApp: boolean
        /** The project type (e.g. mod, shader, resourcepack, datapack, modpack). */
        projectType?: string
        projectIconUrl?: string
        projectName?: string
        header?: string
        mode?: 'version' | 'incompatibility-warning'
        warning?: string
        actionLoading?: boolean
        /** Keep the modal open until the parent confirms the operation was accepted. */
        hideOnUpdate?: boolean
        /** Whether versions are currently being loaded */
        loading?: boolean
        /** Whether changelog is being loaded for the selected version */
        loadingChangelog?: boolean
        actionDisabled?: boolean
        actionDisabledTooltip?: string
        /** When set in incompatibility-warning mode, adds a "Find compatible version" button */
        searchHref?: string
    }>(),
    {
        projectType: undefined,
        projectIconUrl: undefined,
        projectName: undefined,
        header: undefined,
        mode: 'version',
        warning: undefined,
        actionLoading: false,
        hideOnUpdate: true,
        loading: false,
        loadingChangelog: false,
        actionDisabled: false,
        actionDisabledTooltip: undefined,
        searchHref: undefined,
    },
)

const isModpack = computed(() => props.projectType === 'modpack')
const incompatibilityWarningMode = computed(() => props.mode === 'incompatibility-warning')
/** Simplified warning when in incompatibility-warning mode without version data */
const showSimplifiedWarning = computed(
    () => incompatibilityWarningMode.value && props.versions.length === 0 && props.warning,
)
const defaultHeader = computed(() => {
    if (incompatibilityWarningMode.value) {
        return formatMessage(messages.incompatibilityWarningHeader)
    }

    return formatMessage(
        isModpack.value
            ? messages.switchModpackVersionHeader
            : switchMode.value
              ? messages.switchVersionHeader
              : messages.updateVersionHeader,
    )
})

const emit = defineEmits<{
    update: [version: UpdaterVersion, event: MouseEvent]
    cancel: []
    /** Emitted when user selects a version, so parent can fetch full version data with changelog */
    versionSelect: [version: UpdaterVersion]
    versionHover: [version: UpdaterVersion]
    /** Emitted when user clicks "Find compatible version" in simplified warning mode */
    searchCompat: []
}>()

const modal = ref<InstanceType<typeof PreviewModal>>()
const incompatibleUpdateModal = ref<InstanceType<typeof PreviewConfirmModal>>()
const searchQuery = ref('')
const hideIncompatibleState = ref(true)
const switchMode = ref(false)
const pendingIncompatibleUpdate = ref<{
    version: UpdaterVersion
    event: MouseEvent
} | null>(null)
const suppressCancelOnHide = ref(false)

const { selectedVersion, pinnedInitialVersionId, selectVersion, resetInitialSelection } =
    useContentUpdaterSelection({
        versions: toRef(props, 'versions') as never,
        currentVersionId: toRef(props, 'currentVersionId'),
        onVersionSelect: (version) => emit('versionSelect', version as unknown as UpdaterVersion),
        debug,
    }) as unknown as {
        selectedVersion: import('vue').Ref<UpdaterVersion | null>
        pinnedInitialVersionId: import('vue').Ref<string | undefined>
        selectVersion: (version: UpdaterVersion) => void
        resetInitialSelection: (initialVersionId?: string) => void
    }

function isVersionCompatible(version: UpdaterVersion): boolean {
    return versionMatchesCompatibilityTarget(version, {
        gameVersion: props.currentGameVersion,
        loader: props.currentLoader,
        projectType: props.projectType,
    })
}

const currentVersion = computed(() => props.versions.find((v) => v.id === props.currentVersionId))
const showUpdateWarning = computed(() => !isModpack.value)

const isDowngrade = computed(() => {
    if (!selectedVersion.value || !currentVersion.value) return false
    return (
        new Date(selectedVersion.value.date_published) <
        new Date(currentVersion.value.date_published)
    )
})

const filteredVersions = useContentUpdaterFiltering({
    versions: toRef(props, 'versions') as never,
    searchQuery,
    isModpack,
    incompatibilityWarningMode,
    hideIncompatibleState,
    selectedVersion: selectedVersion as never,
    pinnedInitialVersionId,
    currentVersionId: toRef(props, 'currentVersionId'),
    isVersionCompatible: isVersionCompatible as never,
    debug,
}) as unknown as import('vue').ComputedRef<UpdaterVersion[]>

function shouldShowBadge(version: UpdaterVersion): boolean {
    if (incompatibilityWarningMode.value) return false
    return version.id === props.currentVersionId || shouldShowIncompatibleBadge(version)
}

function shouldShowIncompatibleBadge(version: UpdaterVersion): boolean {
    return (
        version.id !== props.currentVersionId && !isModpack.value && !isVersionCompatible(version)
    )
}

function getBadgeLabel(version: UpdaterVersion): string {
    if (version.id === props.currentVersionId) return formatMessage(messages.currentBadge)
    if (shouldShowIncompatibleBadge(version)) return formatMessage(messages.incompatibleBadge)
    return ''
}

function getBadgeClasses(version: UpdaterVersion): string {
    // Current badge
    if (version.id === props.currentVersionId) {
        return 'bg-surface-4 border-surface-5 text-[var(--color-text-default)]'
    }

    if (shouldShowIncompatibleBadge(version)) {
        return 'bg-highlight-orange border-brand-orange text-brand-orange'
    }

    // Version type badges
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

function getVersionTypeBadgeClasses(version: UpdaterVersion): string {
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

function formatLongDate(dateString: string): string {
    return new Date(dateString).toLocaleDateString('en-US', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
    })
}

/** `capitalizeString` of @modrinth/utils. */
function capitalizeString(value: string) {
    return value ? value.charAt(0).toUpperCase() + value.slice(1) : value
}

function formatLoaderGameVersion(version: UpdaterVersion): string {
    const loader = capitalizeString(version.loaders[0] || '')
    const gameVersions = formatGameVersions(version)
    return [loader, gameVersions].filter(Boolean).join(' ')
}

function formatGameVersions(version: UpdaterVersion): string {
    if (!incompatibilityWarningMode.value) {
        return version.game_versions[0] || ''
    }
    return version.game_versions.join(', ')
}

let cancelPrefetch: (() => void) | null = null
const HOVER_DURATION_TO_PREFETCH_MS = 500
function handleVersionMouseEnter(version: UpdaterVersion) {
    cancelPrefetch?.()
    cancelPrefetch = preview.clock.after(HOVER_DURATION_TO_PREFETCH_MS, () => {
        cancelPrefetch = null
        emit('versionHover', version)
    })
}

function handleVersionMouseLeave() {
    cancelPrefetch?.()
    cancelPrefetch = null
}

function handleVersionSelect(version: UpdaterVersion) {
    handleVersionMouseLeave()
    selectVersion(version)
}

function handleUpdate(event: MouseEvent) {
    if (props.actionLoading || props.actionDisabled) return
    if (showSimplifiedWarning.value) {
        emit('update', undefined as unknown as UpdaterVersion, event)
        if (props.hideOnUpdate) hide()
        return
    }
    if (selectedVersion.value) {
        if (incompatibilityWarningMode.value) {
            emitUpdate(selectedVersion.value, event, { hide: false })
            return
        }

        const changesGameVersion = versionChangesGameVersion(
            selectedVersion.value,
            props.currentGameVersion,
        )
        const shouldShowParentWarning =
            isModpack.value && !event.shiftKey && (changesGameVersion || isDowngrade.value)
        if (
            isModpack.value &&
            !event.shiftKey &&
            !isVersionCompatible(selectedVersion.value) &&
            !changesGameVersion
        ) {
            pendingIncompatibleUpdate.value = {
                version: selectedVersion.value,
                event,
            }
            incompatibleUpdateModal.value?.show()
            return
        }

        emitUpdate(selectedVersion.value, event, {
            hide: !shouldShowParentWarning,
        })
    }
}

function confirmIncompatibleUpdate() {
    const pendingUpdate = pendingIncompatibleUpdate.value
    pendingIncompatibleUpdate.value = null

    if (pendingUpdate) {
        const current = currentVersion.value
        const isPendingDowngrade = current
            ? new Date(pendingUpdate.version.date_published) < new Date(current.date_published)
            : false
        const changesGameVersion = versionChangesGameVersion(
            pendingUpdate.version,
            props.currentGameVersion,
        )
        const shouldShowParentWarning =
            isModpack.value &&
            !pendingUpdate.event.shiftKey &&
            (changesGameVersion || isPendingDowngrade)

        emitUpdate(pendingUpdate.version, pendingUpdate.event, {
            hide: !shouldShowParentWarning,
        })
    }
}

function emitUpdate(version: UpdaterVersion, event: MouseEvent, options: { hide?: boolean } = {}) {
    emit('update', version, event)
    if (options.hide ?? true) {
        if (props.hideOnUpdate) hide()
    }
}

function handleCancel() {
    emit('cancel')
    hide()
}

function handleSearchCompat() {
    emit('searchCompat')
    hide()
}

function handleModalHide() {
    if (suppressCancelOnHide.value) {
        suppressCancelOnHide.value = false
        return
    }

    emit('cancel')
}

function show(initialVersionId?: string, options?: { switchMode?: boolean }) {
    searchQuery.value = ''
    hideIncompatibleState.value = incompatibilityWarningMode.value ? false : !isModpack.value
    pendingIncompatibleUpdate.value = null
    switchMode.value = options?.switchMode ?? false

    resetInitialSelection(initialVersionId)
    modal.value?.show()
}

function hide() {
    suppressCancelOnHide.value = true
    modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
    <PreviewModal
        ref="modal"
        :max-width="showSimplifiedWarning ? '480px' : '928px'"
        :width="showSimplifiedWarning ? '480px' : '928px'"
        :on-hide="handleModalHide"
        :no-padding="!showSimplifiedWarning"
    >
        <template #title>
            <Avatar
                v-if="projectIconUrl"
                :src="projectIconUrl"
                size="3rem"
                :tint-by="projectName"
            />
            <span class="text-lg font-extrabold text-[var(--color-text-primary)]">{{
                header ?? defaultHeader
            }}</span>
        </template>

        <!-- Simplified warning when no version data is available (e.g. drag & drop) -->
        <template v-if="showSimplifiedWarning">
            <div class="p-6">
                <Admonition type="warning" :body="warning ?? ''" />
            </div>
        </template>

        <!-- Full version picker layout -->
        <template v-else>
            <div
                class="flex h-[550px] border-[1px] border-solid border-transparent border-b-surface-4"
            >
                <div class="relative flex w-[300px] flex-col bg-surface-3">
                    <div class="p-4 pb-2">
                        <StyledInput
                            v-model="searchQuery"
                            :icon="SearchIcon"
                            type="text"
                            :placeholder="formatMessage(messages.searchVersionPlaceholder)"
                            wrapper-class="w-full"
                        />
                    </div>

                    <div class="flex-1 overflow-y-auto px-4" :class="isModpack ? 'pb-4' : 'pb-16'">
                        <div
                            v-if="loading"
                            class="flex h-full flex-col items-center justify-center gap-2"
                        >
                            <SpinnerIcon
                                class="h-8 w-8 animate-spin text-[var(--color-text-tertiary)]"
                            />
                            <span class="text-sm text-[var(--color-text-tertiary)]">{{
                                formatMessage(messages.loadingVersions)
                            }}</span>
                        </div>
                        <div v-else class="min-h-full">
                            <div class="flex flex-col gap-1.5" role="listbox">
                                <button
                                    v-for="version in filteredVersions"
                                    :key="version.id"
                                    role="option"
                                    :aria-selected="selectedVersion?.id === version.id"
                                    class="flex h-10 cursor-pointer items-center rounded-xl border-none px-4 py-2.5 transition-colors"
                                    :class="[
                                        selectedVersion?.id === version.id
                                            ? 'bg-brand-highlight'
                                            : 'bg-transparent hover:bg-surface-4',
                                    ]"
                                    @mouseenter="handleVersionMouseEnter(version)"
                                    @mouseleave="handleVersionMouseLeave"
                                    @focus="emit('versionHover', version)"
                                    @click="handleVersionSelect(version)"
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
                                        <span
                                            v-if="shouldShowBadge(version)"
                                            class="flex flex-shrink-0 items-center rounded-full border border-solid text-sm font-medium"
                                            :class="[
                                                getBadgeClasses(version),
                                                shouldShowIncompatibleBadge(version)
                                                    ? 'p-1'
                                                    : 'px-2.5 py-0.5',
                                            ]"
                                        >
                                            <CircleAlertIcon
                                                v-if="shouldShowIncompatibleBadge(version)"
                                                v-tooltip="
                                                    formatMessage(messages.incompatibleBadge)
                                                "
                                                class="size-4"
                                            />
                                            <template v-else>{{ getBadgeLabel(version) }}</template>
                                        </span>
                                    </div>
                                </button>
                            </div>
                            <div
                                v-if="filteredVersions.length === 0"
                                class="p-4 text-center text-sm text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.noVersionsFound) }}
                            </div>
                        </div>
                    </div>

                    <div
                        v-if="!isModpack && !incompatibilityWarningMode"
                        class="pointer-events-none absolute bottom-0 left-0 right-0 flex h-24 flex-col items-center justify-end bg-gradient-to-b from-transparent to-bg-raised to-70% pb-3"
                    >
                        <div class="pointer-events-auto">
                            <Button
                                type="quiet"
                                class="flex items-center gap-1.5"
                                :aria-pressed="!hideIncompatibleState"
                                @click="hideIncompatibleState = !hideIncompatibleState"
                            >
                                <EyeIcon v-if="hideIncompatibleState" class="h-6 w-6" />
                                <EyeOffIcon v-else class="h-6 w-6" />
                                <span class="font-medium">{{
                                    hideIncompatibleState
                                        ? formatMessage(messages.showIncompatible)
                                        : formatMessage(messages.hideIncompatible)
                                }}</span>
                            </Button>
                        </div>
                    </div>
                </div>

                <div class="w-px bg-divider" />

                <div
                    class="relative flex min-h-0 min-w-0 flex-1 flex-col bg-surface-1"
                    aria-live="polite"
                >
                    <div
                        v-if="selectedVersion"
                        class="relative flex min-h-0 min-w-0 flex-1 flex-col"
                    >
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
                                            :class="getVersionTypeBadgeClasses(selectedVersion)"
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
                                        <span
                                            class="font-medium text-[var(--color-text-default)]"
                                            >{{
                                                formatMessage(commonMessages.changelogLabel)
                                            }}</span
                                        >
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
                            <slot name="changelog" :version="selectedVersion">
                                <div
                                    v-if="loadingChangelog"
                                    class="flex h-full flex-col items-center justify-center gap-2"
                                >
                                    <SpinnerIcon
                                        class="h-6 w-6 animate-spin text-[var(--color-text-tertiary)]"
                                    />
                                    <span class="text-sm text-[var(--color-text-tertiary)]">{{
                                        formatMessage(messages.loadingChangelog)
                                    }}</span>
                                </div>
                                <div
                                    v-else-if="selectedVersion.changelog"
                                    class="markdown [&_img]:h-auto [&_img]:max-w-full"
                                    v-html="renderMarkdown(selectedVersion.changelog)"
                                />
                                <div v-else class="italic text-[var(--color-text-tertiary)]">
                                    {{ formatMessage(messages.noChangelog) }}
                                </div>
                            </slot>
                        </div>

                        <div
                            class="pointer-events-none absolute bottom-0 left-0 right-0 h-14 bg-gradient-to-t from-bg to-transparent"
                        />
                    </div>
                    <div
                        v-else-if="loading || loadingChangelog || props.versions.length > 0"
                        class="flex h-full flex-1 flex-col items-center justify-center gap-2 bg-bg text-[var(--color-text-tertiary)]"
                    >
                        <SpinnerIcon class="h-6 w-6 animate-spin" />
                        <span class="text-sm">{{ formatMessage(messages.loadingChangelog) }}</span>
                    </div>
                    <div
                        v-else
                        class="flex flex-1 items-center justify-center bg-bg text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.selectVersionPrompt) }}
                    </div>
                </div>
            </div>

            <div
                class="flex w-full flex-row items-center gap-4 border-x-0 border-b-0 border-t border-solid border-surface-4 p-4"
            >
                <div
                    v-if="showUpdateWarning"
                    class="mr-auto flex max-w-[55%] flex-1 flex-row items-center gap-2 text-orange"
                >
                    <TriangleAlertIcon class="size-6 shrink-0" />
                    <span>{{
                        warning ??
                        formatMessage(
                            incompatibilityWarningMode
                                ? messages.incompatibilityWarning
                                : isApp
                                  ? messages.updateWarningApp
                                  : messages.updateWarningWeb,
                        )
                    }}</span>
                </div>
                <div class="ml-auto flex shrink-0 flex-row gap-2">
                    <Button type="outlined" @click="handleCancel"
                        ><XIcon />
                        {{ formatMessage(commonMessages.cancelButton) }}
                    </Button>
                    <Button
                        v-tooltip="props.actionDisabled ? props.actionDisabledTooltip : undefined"
                        type="colored"
                        :color="incompatibilityWarningMode ? 'orange' : 'brand'"
                        :disabled="
                            actionLoading ||
                            props.actionDisabled ||
                            !selectedVersion ||
                            (!incompatibilityWarningMode && selectedVersion.id === currentVersionId)
                        "
                        :loading="actionLoading"
                        @click="handleUpdate"
                    >
                        <SpinnerIcon v-if="actionLoading" class="size-5 animate-spin" />
                        <DownloadIcon v-else />
                        {{
                            actionLoading
                                ? formatMessage(commonMessages.installingLabel)
                                : incompatibilityWarningMode
                                  ? formatMessage(messages.installAnywayButton)
                                  : formatMessage(
                                        isDowngrade
                                            ? messages.downgradeToVersion
                                            : switchMode
                                              ? messages.switchToVersion
                                              : messages.updateToVersion,
                                        {
                                            version: selectedVersion?.version_number ?? '...',
                                        },
                                    )
                        }}
                    </Button>
                </div>
            </div>
        </template>

        <!-- Simplified warning mode actions -->
        <template v-if="showSimplifiedWarning" #actions>
            <div class="flex w-full gap-3">
                <Button @click="handleCancel"
                    >{{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="brand" @click="handleSearchCompat"
                    ><SearchIcon class="size-4" />
                    {{ formatMessage(messages.searchCompatButton) }}
                </Button>
                <Button
                    type="colored"
                    color="orange"
                    :disabled="actionLoading"
                    @click="handleUpdate"
                    ><SpinnerIcon v-if="actionLoading" class="size-5 animate-spin" />
                    <DownloadIcon v-else />
                    {{ formatMessage(messages.installAnywayButton) }}
                </Button>
            </div>
        </template>
    </PreviewModal>

    <PreviewConfirmModal
        ref="incompatibleUpdateModal"
        :title="formatMessage(messages.incompatibleUpdateHeader)"
        :description="
            formatMessage(messages.incompatibleUpdateDescription, {
                version: pendingIncompatibleUpdate?.version.version_number ?? '...',
            })
        "
        :proceed-icon="DownloadIcon"
        :proceed-label="formatMessage(messages.updateAnywayButton)"
        :danger="false"
        @proceed="confirmIncompatibleUpdate"
    />
</template>
