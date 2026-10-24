<template>
    <ReadyTransition :pending="loading">
        <template #pending>
            <LoadingIndicator class="pt-4" />
        </template>
        <CollapsibleAdmonition
            v-if="postUpgradeNotice?.warnings.length"
            v-model="postUpgradeNoticeExpanded"
            type="warning"
            class="mb-4"
        >
            <template #header>
                <span class="inline-flex items-center gap-2">
                    {{ formatMessage(messages.postUpgradeNoticeTitle) }}
                    <span class="rounded-full bg-bg-orange px-2 py-0.5 text-sm tabular-nums">{{
                        postUpgradeNotice.warnings.length
                    }}</span>
                </span>
            </template>
            <div class="border-0 border-t border-solid mods-admonition-body bg-bg-orange p-4">
                <p class="m-0">{{ formatMessage(messages.postUpgradeNoticeBody) }}</p>
                <div class="mt-3 flex justify-end">
                    <Button
                        type="colored"
                        color="orange"
                        size="2xs"
                        @click="dismissPostUpgradeNotice"
                        >{{ formatMessage(messages.ignoreAllPostUpgradeWarnings) }}
                    </Button>
                </div>
            </div>
        </CollapsibleAdmonition>
        <CollapsibleAdmonition
            v-if="skippedManualDownloads.length > 0"
            v-model="manualWarningExpanded"
            type="warning"
            class="mb-4"
        >
            <template #header>
                <span class="inline-flex items-center gap-2">
                    {{ formatMessage(messages.skippedFilesWarningTitle) }}
                    <span class="rounded-full bg-bg-orange px-2 py-0.5 text-sm tabular-nums">
                        {{ skippedManualDownloads.length }}
                    </span>
                </span>
            </template>
            <div class="border-0 border-t border-solid mods-admonition-body bg-bg-orange p-4">
                <p class="m-0">{{ skippedFilesWarningBody }}</p>
                <ul class="mb-0 mt-2 flex list-none flex-col gap-1 p-0">
                    <li
                        v-for="item in visibleSkippedManualDownloads"
                        :key="`${item.projectId}:${item.fileId}`"
                        class="min-w-0"
                    >
                        <button
                            class="inline-flex max-w-full cursor-pointer items-center gap-1 text-left font-semibold text-brand hover:underline"
                            @click="openManualCurseForgeDownload(item)"
                        >
                            <span class="truncate">{{ item.fileName }}</span>
                            <ExternalIcon class="size-4 shrink-0" />
                        </button>
                    </li>
                </ul>
                <p
                    v-if="hiddenSkippedManualDownloadCount > 0"
                    class="mb-0 mt-2 text-[var(--color-text-tertiary)]"
                >
                    {{
                        formatMessage(messages.skippedFilesWarningMore, {
                            count: hiddenSkippedManualDownloadCount,
                        })
                    }}
                </p>
                <div class="mt-3 flex justify-end">
                    <Button
                        type="colored"
                        color="orange"
                        size="2xs"
                        @click="openManualCurseForgeResolver"
                        ><FolderSearchIcon />
                        {{ formatMessage(messages.completeSkippedFiles) }}
                    </Button>
                </div>
            </div>
        </CollapsibleAdmonition>
        <CollapsibleAdmonition
            v-if="missingPackMembers.length > 0"
            v-model="missingWarningExpanded"
            type="warning"
            class="mb-4"
        >
            <template #header>
                <span class="inline-flex items-center gap-2">
                    {{ formatMessage(messages.missingFilesWarningTitle) }}
                    <span class="rounded-full bg-bg-orange px-2 py-0.5 text-sm tabular-nums">
                        {{ missingPackMembers.length }}
                    </span>
                </span>
            </template>
            <div class="bg-bg-orange px-4 pb-4 pt-3">
                <p class="m-0 text-sm leading-6 text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.missingFilesWarningBody) }}
                </p>
                <ul class="m-0 mt-2 flex max-h-64 list-none flex-col gap-1 overflow-y-auto p-0">
                    <li
                        v-for="item in missingPackMembers"
                        :key="item.memberId ?? item.expectedRelativePath"
                        class="mods-missing-row flex min-h-14 min-w-0 items-center gap-3 rounded-lg px-2 py-2 transition-colors"
                    >
                        <FileIcon class="size-5 shrink-0 text-brand-orange" aria-hidden="true" />
                        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span
                                class="truncate font-medium text-[var(--color-text-primary)]"
                                :title="item.expectedRelativePath"
                            >
                                {{ fileNameFromPath(item.expectedRelativePath) }}
                            </span>
                            <code
                                class="truncate text-xs text-[var(--color-text-tertiary)]"
                                :title="item.expectedRelativePath"
                            >
                                {{ item.expectedRelativePath }}
                            </code>
                        </span>
                        <Button
                            type="chip-text"
                            color="orange"
                            size="2xs"
                            :disabled="
                                !item.memberId ||
                                isInstanceBusy ||
                                isRestoringMissingPackMember(item)
                            "
                            @click="restoreMissingPackMember(item)"
                            ><SpinnerIcon
                                v-if="isRestoringMissingPackMember(item)"
                                class="animate-spin"
                                aria-hidden="true"
                            />
                            <UndoIcon v-else aria-hidden="true" />
                            {{ formatMessage(messages.restoreMissingFile) }}
                        </Button>
                    </li>
                </ul>
            </div>
        </CollapsibleAdmonition>
        <CollapsibleAdmonition
            v-if="onlineWarnings.length > 0 || localWarnings.length > 0"
            v-model="contentWarningExpanded"
            type="warning"
            class="mb-4"
        >
            <template #header>{{
                formatMessage(
                    localWarnings.length
                        ? messages.contentHistoryWarningTitle
                        : messages.contentRefreshWarningTitle,
                )
            }}</template>
            <div class="border-0 border-t border-solid mods-admonition-body bg-bg-orange p-4">
                <p class="m-0">
                    {{
                        formatMessage(
                            localWarnings.length
                                ? messages.contentHistoryWarningBody
                                : messages.contentRefreshWarningBody,
                        )
                    }}
                </p>
            </div>
        </CollapsibleAdmonition>
        <CollapsibleAdmonition
            v-if="
                contentWarnings.some(
                    (warning) => warning.code === 'pack_membership_reconciliation_failed',
                )
            "
            type="warning"
            class="mb-4"
        >
            <template #header>{{
                formatMessage(messages.packReconciliationWarningTitle)
            }}</template>
            <p class="m-0 p-4">{{ formatMessage(messages.packReconciliationWarningBody) }}</p>
        </CollapsibleAdmonition>
        <ContentPageLayout @visible-items="handleVisibleItems">
            <template #header-status>
                <div
                    v-if="loadingVisibleMetadata && !onlineInformationDismissed"
                    class="flex items-center justify-between gap-2 px-3 pb-2 text-sm text-[var(--color-text-tertiary)]"
                >
                    <span role="status">{{
                        formatMessage(messages.loadingOnlineInformation)
                    }}</span>
                    <IconButton
                        :label="formatMessage(commonMessages.closeButton)"
                        type="quiet"
                        size="2xs"
                        @click="onlineInformationDismissed = true"
                    >
                        <XIcon aria-hidden="true" />
                    </IconButton>
                </div>
            </template>
            <template #toolbar-actions>
                <Button
                    v-if="hasPackAssociation"
                    type="outlined"
                    :disabled="isPackUnlinkDisabled"
                    @click="packUnlinkModal?.show()"
                    >{{ formatMessage(commonMessages.unlinkModpackButton) }}</Button
                >
                <Button
                    v-if="
                        !instance.linked_dot_minecraft &&
                        !instance.symlink_target &&
                        !instance.game_dir_override
                    "
                    type="outlined"
                    class="!h-10"
                    @click="resourceBackupsModal?.show(instance.id)"
                    >{{ formatMessage(messages.manageBackups) }}</Button
                >
            </template>
            <template #modals>
                <ConfirmUnlinkModal
                    ref="packUnlinkModal"
                    :action="unpairInstance"
                    :action-disabled="isPackUnlinkDisabled"
                />
                <ContentToggleDependenciesModal ref="toggleDependenciesModal" />
                <DependencyGraphModal
                    ref="dependencyGraphModal"
                    :instance-id="props.instance.id"
                    :instance-name="props.instance.name"
                    :instance-icon-url="localContentIconUrl(props.instance.icon_path)"
                />

                <ShareModal
                    ref="shareModal"
                    :share-title="formatMessage(messages.shareTitle)"
                    :share-text="formatMessage(messages.shareText)"
                    :open-in-new-tab="false"
                />
                <ConfirmModpackUpdateModal
                    ref="modpackUpdateConfirmModal"
                    :downgrade="isModpackUpdateDowngrade"
                    :backup-tip="
                        [
                            displayedModpackProject?.title,
                            pendingModpackUpdateVersion?.version_number,
                        ]
                            .filter(Boolean)
                            .join(' ')
                    "
                    :symlink-target="props.instance.symlink_target"
                    @confirm="handleModpackUpdateConfirm"
                    @cancel="handleModpackUpdateCancel"
                />
                <ResourceOperationPreviewModal ref="resourceOperationPreview" />
                <ResourceHistoryModal ref="resourceHistoryModal" />
                <ResourceBackupsModal
                    ref="resourceBackupsModal"
                    @cleaned="refreshContentState('bypass')"
                />
                <ExportModal v-if="projects.length > 0" ref="exportModal" :instance="instance" />
                <ContentUpdaterModal
                    v-if="updatingProject || updatingModpack"
                    ref="contentUpdaterModal"
                    :versions="updatingProjectVersions"
                    :current-game-version="instance.game_version"
                    :current-loader="instance.loader"
                    :current-version-id="
                        updatingModpack
                            ? (instance.link?.version_id ?? '')
                            : (updatingProject?.version?.id ?? '')
                    "
                    :is-app="true"
                    :project-type="updatingModpack ? 'modpack' : updatingProject?.project_type"
                    :project-icon-url="
                        updatingModpack
                            ? displayedModpackProject?.icon_url
                            : updatingProject?.project?.icon_url
                    "
                    :project-name="
                        updatingModpack
                            ? (displayedModpackProject?.title ??
                              formatMessage(commonMessages.modpackLabel))
                            : (updatingProject?.project?.title ?? updatingProject?.file_name)
                    "
                    :loading="loadingVersions"
                    :action-loading="contentUpdateSubmitting"
                    :hide-on-update="false"
                    @update="handleModalUpdate"
                    @cancel="resetUpdateState"
                >
                    <template #changelog="{ version }">
                        <ResourceReleaseChangelog
                            v-if="updaterChangelogRef(version)"
                            :release="updaterChangelogRef(version)!"
                        />
                    </template>
                </ContentUpdaterModal>
            </template>
        </ContentPageLayout>
    </ReadyTransition>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
    BookOpenIcon,
    ClipboardCopyIcon,
    ExternalIcon,
    FileIcon,
    FolderOpenIcon,
    FolderSearchIcon,
    PencilIcon,
    SpinnerIcon,
    UndoIcon,
    XIcon,
} from '@modrinth/assets'
import {
    Button,
    CollapsibleAdmonition,
    commonMessages,
    ConfirmModpackUpdateModal,
    ConfirmUnlinkModal,
    ContentCardLayout as ContentPageLayout,
    type ContentItem,
    type ContentModpackCardCategory,
    type ContentModpackCardProject,
    type ContentModpackCardVersion,
    type ContentOwner,
    ContentUpdaterModal,
    defineMessages,
    IconButton,
    injectNotificationManager,
    LoadingIndicator,
    type OverflowMenuOption,
    provideAppBackup,
    provideContentManager,
    ReadyTransition,
    ShareModal,
    useDebugLogger,
    useVIntl,
    versionChangesGameVersion,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import DependencyGraphModal from '@/components/instance/dependencies/DependencyGraphModal.vue'
import ExportModal from '@/components/ui/ExportModal.vue'
import ContentToggleDependenciesModal from '@/components/ui/modal/ContentToggleDependenciesModal.vue'
import ResourceBackupsModal from '@/components/ui/ResourceBackupsModal.vue'
import ResourceHistoryModal from '@/components/ui/ResourceHistoryModal.vue'
import ResourceOperationPreviewModal from '@/components/ui/ResourceOperationPreviewModal.vue'
import ResourceReleaseChangelog from '@/components/ui/ResourceReleaseChangelog.vue'
import { postUpgradeNoticeQueryKey, usePostUpgradeNotice } from '@/composables/usePostUpgradeNotice'
import { useWorldDatapacks } from '@/composables/useWorldDatapacks'
import { trackEvent } from '@/helpers/analytics'
import { get_project_versions, get_version, get_version_many } from '@/helpers/cache.js'
import {
    activeContentChangeJobs as selectActiveContentChangeJobs,
    contentItemStableId,
    hasActiveContentChange,
    pendingContentChangeAffectsItem,
} from '@/helpers/content-change-jobs'
import { mergeContentItemMetadata } from '@/helpers/content-item-metadata'
import { applyContentItemUpdates, matchesContentItem } from '@/helpers/content-item-state'
import { contentProjectUrl } from '@/helpers/content-project-links'
import { lookupContentWikiIds, translateContentItemTitles } from '@/helpers/content-search'
import { type CurseForgeFile, getCurseForgeImageUrl } from '@/helpers/curseforge'
import {
    type CurseForgeManualDownloadItem,
    getCurseForgeManualDownloadUrl,
} from '@/helpers/curseforge-manual'
import { getMissingContentScannerSettings } from '@/helpers/downloads-scanner'
import { instance_listener } from '@/helpers/events.js'
import { install_duplicate_instance, installJobInstanceId } from '@/helpers/install'
import {
    add_project_from_path,
    apply_content_update_plan,
    dismiss_post_upgrade_notice,
    edit,
    get_content_items_by_paths,
    type InstanceContentSnapshotItem,
    list,
    plan_content_updates,
    remove_content_entry,
    restore_pack_member_default,
    rollback_project,
    toggle_content_entries,
    toggle_content_entry,
} from '@/helpers/instance'
import { readInstanceCache, writeInstanceCache } from '@/helpers/instance-cache'
import {
    type InstanceContentData,
    isWorldSaveContentItem,
    loadInstanceContentInStages,
    localContentIconUrl,
    preserveContentUpdateState,
    refreshInstanceContentUpdates,
} from '@/helpers/instance-content'
import { postUpgradeWarningForContent } from '@/helpers/post-upgrade-notice'
import {
    confirmedContentGroupKey,
    duplicateGroupUpdateEntries,
    reviewResourceEntryChanges,
} from '@/helpers/resource-update-selection'
import {
    resourceCheckUpdates,
    resourceListHistory,
    resourceReleaseDocuments,
    resourceReleases,
    type SourceProjectRef,
    type SourceReleaseRef,
} from '@/helpers/resources'
import type { CacheBehaviour, GameInstance } from '@/helpers/types'
import { highlightModInInstance } from '@/helpers/utils.js'
import i18n from '@/i18n.config'
import { injectContentInstall } from '@/providers/content-install'
import { injectDownloadManager } from '@/providers/download-manager'
import { useTheming } from '@/store/state'

const messages = defineMessages({
    loadingOnlineInformation: {
        id: 'app.instance.mods.loading-online-information',
        defaultMessage: 'Loading online information, please wait',
    },
    versionsFailed: {
        id: 'app.instance.content.versions-failed',
        defaultMessage:
            'The release list could not be loaded. Reopen the version selector and try again.',
    },
    manageBackups: {
        id: 'app.instance.mods.manage-backups',
        defaultMessage: 'Old files and backups',
    },
    shareTitle: {
        id: 'app.instance.mods.share-title',
        defaultMessage: 'Sharing modpack content',
    },
    shareText: {
        id: 'app.instance.mods.share-text',
        defaultMessage: "Check out the projects I'm using in my modpack!",
    },
    openInSchematicWorkshop: {
        id: 'instance.files.open-in-schematic-workshop',
        defaultMessage: 'Open in schematic workshop',
    },
    successfullyUploaded: {
        id: 'app.instance.mods.successfully-uploaded',
        defaultMessage: 'Successfully uploaded',
    },
    projectWasAdded: {
        id: 'app.instance.mods.project-was-added',
        defaultMessage: '"{name}" was added',
    },
    projectsWereAdded: {
        id: 'app.instance.mods.projects-were-added',
        defaultMessage: '{count} projects were added',
    },
    orphanedDependenciesTitle: {
        id: 'app.instance.mods.orphaned-dependencies.title',
        defaultMessage: 'Dependencies are no longer required',
    },
    orphanedDependenciesBody: {
        id: 'app.instance.mods.orphaned-dependencies.body',
        defaultMessage:
            'These dependencies are no longer required by other content and will not be removed automatically: {list}',
    },
    contentTypeProject: {
        id: 'app.instance.mods.content-type-project',
        defaultMessage: 'project',
    },
    bulkUpdateResolvingVersions: {
        id: 'app.instance.mods.bulk-update.resolving-versions',
        defaultMessage: 'Resolving versions...',
    },
    bulkUpdateDownloadingProjects: {
        id: 'app.instance.mods.bulk-update.downloading-projects',
        defaultMessage: 'Downloading {current, number}/{total, number} projects...',
    },
    bulkUpdateFinishing: {
        id: 'app.instance.mods.bulk-update.finishing',
        defaultMessage: 'Finishing update...',
    },
    updateAddedContent: {
        id: 'app.instance.mods.update-added-content',
        defaultMessage: 'Update added content',
    },
    updateAddedContentDescription: {
        id: 'app.instance.mods.update-added-content.description',
        defaultMessage:
            'Updates only content added after the modpack was installed. Modpack files are not changed.',
    },
    contentOperationTimedOut: {
        id: 'app.instance.mods.content-operation-timeout',
        defaultMessage: 'Timed out waiting for another content operation to finish. Try again.',
    },
    contentRefreshWarningTitle: {
        id: 'app.instance.mods.content-refresh-warning.title',
        defaultMessage: 'Some online details are unavailable',
    },
    contentRefreshWarningBody: {
        id: 'app.instance.mods.content-refresh-warning.body',
        defaultMessage:
            'Local content is still complete and can be managed. Names, icons, or update information may be temporarily unavailable.',
    },
    contentHistoryWarningTitle: {
        id: 'app.instance.mods.content-history-warning.title',
        defaultMessage: 'Some local history details are unavailable',
    },
    packReconciliationWarningTitle: {
        id: 'app.instance.mods.pack-reconciliation-warning.title',
        defaultMessage: 'Modpack file information could not be refreshed',
    },
    packReconciliationWarningBody: {
        id: 'app.instance.mods.pack-reconciliation-warning.body',
        defaultMessage:
            'Existing files and ownership have been kept. Retry refreshing to verify missing or manually downloaded modpack files.',
    },
    contentHistoryWarningBody: {
        id: 'app.instance.mods.content-history-warning.body',
        defaultMessage:
            'Local content is still complete and can be managed. Rollback or cleanup details may be temporarily unavailable.',
    },
    skippedFilesWarningTitle: {
        id: 'app.instance.mods.skipped-files-warning.title',
        defaultMessage: 'Some modpack files require manual installation',
    },
    skippedFilesWarningBody: {
        id: 'app.instance.mods.skipped-files-warning.body',
        defaultMessage:
            '{count, plural, one {# file was} other {# files were}} skipped during installation. Select a file to download it; the monitored import folder is verified automatically. You can also drag the downloaded file into this instance.',
    },
    skippedFilesWarningBodyDisabled: {
        id: 'app.instance.mods.skipped-files-warning.body-disabled',
        defaultMessage:
            '{count, plural, one {# file was} other {# files were}} skipped during installation. Drag the downloaded file into this instance, or enable automatic import in Resource Management settings.',
    },
    skippedFilesWarningMore: {
        id: 'app.instance.mods.skipped-files-warning.more',
        defaultMessage: 'And {count, number} more.',
    },
    completeSkippedFiles: {
        id: 'app.instance.mods.skipped-files-warning.complete',
        defaultMessage: 'Complete missing files',
    },
    missingFilesWarningTitle: {
        id: 'app.instance.mods.missing-files-warning.title',
        defaultMessage: 'Some modpack files are missing',
    },
    missingFilesWarningBody: {
        id: 'app.instance.mods.missing-files-warning.body',
        defaultMessage:
            'The required files below were not downloaded or are no longer present. Restore them individually; refreshing scans the instance folder again.',
    },
    restoreMissingFile: {
        id: 'app.instance.mods.missing-files-warning.restore',
        defaultMessage: 'Restore',
    },
    parsingFiles: {
        id: 'app.instance.mods.parsing-files',
        defaultMessage: 'Parsing... larger files may take a while',
    },
    dragDropHint: {
        id: 'app.instance.mods.drag-drop-hint',
        defaultMessage: 'Release the file to install it into the current instance',
    },
    parseFailed: {
        id: 'app.instance.mods.parse-failed',
        defaultMessage: 'Failed to parse. This may not be a modpack',
    },
    fileAlreadyExists: {
        id: 'app.instance.mods.file-already-exists',
        defaultMessage: 'Failed to add. The file may already exist',
    },
    dismiss: {
        id: 'app.instance.mods.dismiss',
        defaultMessage: 'Got it',
    },
    restorePackDefault: {
        id: 'app.instance.mods.restore-pack-default',
        defaultMessage: 'Restore modpack default',
    },
    packMemberMissing: {
        id: 'app.instance.mods.pack-member-missing',
        defaultMessage: 'This modpack file is missing locally',
    },
    packMemberRemoved: {
        id: 'app.instance.mods.pack-member-removed',
        defaultMessage: 'This modpack file was removed locally',
    },
    postUpgradeNoticeTitle: {
        id: 'app.instance.mods.post-upgrade-notice.title',
        defaultMessage: 'Post-upgrade compatibility review',
    },
    postUpgradeNoticeBody: {
        id: 'app.instance.mods.post-upgrade-notice.body',
        defaultMessage:
            'Some preserved content may not support the current Minecraft version and should be reviewed manually.',
    },
    ignoreAllPostUpgradeWarnings: {
        id: 'app.instance.mods.post-upgrade-notice.ignore-all',
        defaultMessage: 'Ignore all',
    },
    postUpgradeWarningTooltip: {
        id: 'app.instance.mods.post-upgrade-notice.item-tooltip',
        defaultMessage:
            'This content was preserved during the instance upgrade without installing a new compatible version. It may be incompatible with the current Minecraft version.',
    },
    compatibilityWarning: {
        id: 'app.resources.installed.compatibility',
        defaultMessage:
            'Minecraft: {game, select, compatible {compatible} incompatible {incompatible} other {no supported version declaration}}; loader: {loader, select, compatible {compatible} incompatible {incompatible} other {no loader declaration}}.',
    },
    openInMcmod: {
        id: 'app.project.open-in-mcmod',
        defaultMessage: 'Open in MC Mod',
    },
})

const { formatMessage } = useVIntl()
const debugState = useDebugLogger('Mods:state')
const { handleError, addNotification } = injectNotificationManager()
const downloadManager = injectDownloadManager()
const {
    installingItems,
    installRevisionByInstance,
    installFailureRevisionByInstance,
    showCurseForgeManualDownloads,
} = injectContentInstall()
const router = useRouter()
const debug = useDebugLogger('Mods:ContentUpdate')
const themeStore = useTheming()
const skipNonEssentialWarnings = computed(() =>
    themeStore.getFeatureFlag('skip_non_essential_warnings'),
)

defineOptions({
    inheritAttrs: false,
})

const props = defineProps<{
    instance: GameInstance
    isServerInstance?: boolean
    openSettings?: () => void
    preloadedContent?: InstanceContentData | null
}>()
const queryClient = useQueryClient()
const postUpgradeNoticeQuery = usePostUpgradeNotice(() => props.instance.id)
const postUpgradeNotice = computed(() => postUpgradeNoticeQuery.data.value ?? null)
const postUpgradeNoticeExpanded = ref(false)

async function dismissPostUpgradeNotice() {
    try {
        await dismiss_post_upgrade_notice(props.instance.id)
        queryClient.setQueryData(postUpgradeNoticeQueryKey(props.instance.id), null)
    } catch (error) {
        handleError(error as Error)
    }
}

defineEmits<{
    play: []
    stop: []
}>()

function hasPreloadedContent(contentData: InstanceContentData | null | undefined) {
    return contentData?.path === props.instance.id
}

const loading = ref(!hasPreloadedContent(props.preloadedContent))
const contentLoadError = ref<Error | null>(null)
const contentSnapshot = ref<InstanceContentData['snapshot'] | null>(null)
const projects = ref<ContentItem[]>([])
const linkedModpackContentItems = ref<ContentItem[]>([])
const contentItemsByEntryId = computed(
    () =>
        new Map(
            [...projects.value, ...linkedModpackContentItems.value]
                .filter((item) => item.instanceEntryId)
                .map((item) => [item.instanceEntryId!, item]),
        ),
)
const onlineInformationDismissed = ref(false)
const mcmodUrls = ref(new Map<string, string>())
const visibleMetadataRequestedPaths = new Set<string>()
const visibleMetadataPendingPaths = new Set<string>()
const visibleMetadataPaths = ref(new Set<string>())
const visibleMetadataLoadingPaths = ref(new Set<string>())
const contentUpdateLoadingPaths = ref(new Set<string>())
const loadingVisibleMetadata = computed(() =>
    [...visibleMetadataPaths.value].some(
        (path) =>
            visibleMetadataLoadingPaths.value.has(path) ||
            contentUpdateLoadingPaths.value.has(path),
    ),
)

watch(loadingVisibleMetadata, (loading) => {
    if (loading) onlineInformationDismissed.value = false
})

let visibleMetadataRefreshRunning = false
let mcmodLookupVersion = 0
let isUnmounted = false
const contentWarnings = computed(() => contentSnapshot.value?.warnings ?? [])
const onlineWarnings = computed(() =>
    contentWarnings.value.filter((warning) =>
        [
            'provider_metadata_unavailable',
            'pack_metadata_unavailable',
            'resource_update_unavailable',
        ].includes(warning.code),
    ),
)
const localWarnings = computed(() =>
    contentWarnings.value.filter((warning) => warning.code === 'resource_history_unavailable'),
)
const contentWarningExpanded = ref(true)
const packUnlinkModal = ref<InstanceType<typeof ConfirmUnlinkModal>>()
const hasPackAssociation = computed(
    () =>
        !!contentSnapshot.value?.pack ||
        (contentSnapshot.value?.items.some((item) => item.ownershipKind === 'pack_managed') ??
            false),
)

// 从持久化缓存恢复整合包元数据
const linkedModpackProject = ref<ContentModpackCardProject | null>(null)
const linkedModpackVersion = ref<ContentModpackCardVersion | null>(null)
const linkedModpackOwner = ref<ContentOwner | null>(null)
const linkedModpackCategories = ref<ContentModpackCardCategory[]>([])
const linkedModpackHasUpdate = ref(false)
const linkedModpackUpdateVersionId = ref<string | null>(null)
const localImportedModpackUnlinked = ref(false)

const installingBuffer = ref<ContentItem[]>([])
const handledInstallRevision = ref(0)
const curseForgeReconciliationAttempts = new Set<string>()

watch(
    () => installingItems.value.get(props.instance.id),
    (items) => {
        if (items && items.length > 0) {
            debugState('installingItems → buffer', {
                instanceId: props.instance.id,
                count: items.length,
            })
            installingBuffer.value = [...items]
        } else if (installingBuffer.value.length > 0) {
            debugState('installingItems cleared → reset buffer', { instanceId: props.instance.id })
            installingBuffer.value = []
        }
    },
    { immediate: true, deep: true },
)

watch(projects, (newProjects) => {
    if (installingBuffer.value.length === 0) return
    const realProjectIds = new Set(newProjects.map((p) => p.project?.id).filter(Boolean))
    if (installingBuffer.value.every((item) => realProjectIds.has(item.project?.id))) {
        installingBuffer.value = []
    }
})

function getMcmodLookup(item: ContentItem) {
    const provider = item.origin_provider ?? item.provider_refs[0]?.provider
    const slug = item.project?.slug
    if (!slug || (provider !== 'modrinth' && provider !== 'curseforge')) return null

    return { provider, slug }
}

function getMcmodLookupKey(provider: 'modrinth' | 'curseforge', slug: string) {
    return `${provider}:${slug}`
}

function getMcmodUrl(item: ContentItem) {
    const lookup = getMcmodLookup(item)
    return lookup ? mcmodUrls.value.get(getMcmodLookupKey(lookup.provider, lookup.slug)) : undefined
}

async function refreshMcmodUrls(items: ContentItem[]) {
    const lookupVersion = ++mcmodLookupVersion
    const modrinthSlugs = new Set<string>()
    const curseForgeSlugs = new Set<string>()

    for (const item of items) {
        const lookup = getMcmodLookup(item)
        if (!lookup) continue
        if (lookup.provider === 'modrinth') modrinthSlugs.add(lookup.slug)
        else curseForgeSlugs.add(lookup.slug)
    }

    if (modrinthSlugs.size === 0 && curseForgeSlugs.size === 0) {
        mcmodUrls.value = new Map()
        return
    }

    const wikiIds = await lookupContentWikiIds([...modrinthSlugs], [...curseForgeSlugs]).catch(
        () => null,
    )
    if (isUnmounted || lookupVersion !== mcmodLookupVersion) return

    const urls = new Map<string, string>()
    for (const slug of modrinthSlugs) {
        const wikiId = wikiIds?.modrinth[slug]
        if (wikiId) {
            urls.set(
                getMcmodLookupKey('modrinth', slug),
                `https://www.mcmod.cn/class/${wikiId}.html`,
            )
        }
    }
    for (const slug of curseForgeSlugs) {
        const wikiId = wikiIds?.curseforge[slug]
        if (wikiId) {
            urls.set(
                getMcmodLookupKey('curseforge', slug),
                `https://www.mcmod.cn/class/${wikiId}.html`,
            )
        }
    }
    mcmodUrls.value = urls
}

watch(
    [projects, linkedModpackContentItems],
    ([contentItems, linkedContentItems]) => {
        void refreshMcmodUrls([...contentItems, ...linkedContentItems])
    },
    { immediate: true },
)

const manualDownloadCandidates = computed<CurseForgeManualDownloadItem[]>(() => {
    const stored = (contentSnapshot.value?.pendingManualDownloads ?? [])
        .filter((item) => item.provider === 'curseforge')
        .map((item) => {
            const context = item.context ?? {}
            const contextString = (key: string) =>
                typeof context[key] === 'string' ? context[key] : undefined
            const contextNumber = (key: string) =>
                typeof context[key] === 'number' ? context[key] : undefined
            const contextHashes = Array.isArray(context.hashes)
                ? context.hashes.filter(
                      (hash): hash is { algo: number; value: string } =>
                          typeof hash === 'object' &&
                          hash !== null &&
                          'algo' in hash &&
                          'value' in hash &&
                          typeof hash.algo === 'number' &&
                          typeof hash.value === 'string',
                  )
                : []
            const targetFolder =
                typeof item.targetRelativePath === 'string'
                    ? item.targetRelativePath.replace(/\\/g, '/').split('/').slice(0, -1).join('/')
                    : contextString('targetFolder')
            const snapshotOwnsItem = contentSnapshot.value?.items.some(
                (snapshotItem) => snapshotItem.memberId === item.packMemberId,
            )

            return {
                projectId: Number(item.providerProjectId ?? context.projectId),
                fileId: Number(item.providerReleaseId ?? context.fileId),
                fileName: item.fileName ?? contextString('fileName') ?? '',
                websiteUrl: item.websiteUrl ?? contextString('websiteUrl'),
                projectSlug: contextString('projectSlug'),
                hashes: item.expectedSha1 ? [{ algo: 1, value: item.expectedSha1 }] : contextHashes,
                fileLength: item.expectedSize ?? contextNumber('fileLength') ?? 0,
                fileFingerprint: item.expectedFingerprint ?? contextNumber('fileFingerprint') ?? 0,
                projectType: item.projectType ?? contextString('projectType'),
                targetFolder,
                ownershipKind:
                    snapshotOwnsItem || contextString('ownershipKind') === 'pack_managed'
                        ? ('pack_managed' as const)
                        : ('user_added' as const),
                operationKind:
                    item.operationKind ??
                    (contextString(
                        'operationKind',
                    ) as CurseForgeManualDownloadItem['operationKind']),
            }
        })

    const result = [
        ...new Map(stored.map((item) => [`${item.projectId}:${item.fileId}`, item])).values(),
    ]
    debugState('manualDownloadCandidates', {
        instanceId: props.instance.id,
        storedCount: stored.length,
        resultCount: result.length,
    })
    return result
})

const skippedManualDownloads = computed(() => manualDownloadCandidates.value)
const automaticManualImportEnabled = getMissingContentScannerSettings().enabled
const skippedFilesWarningBody = computed(() =>
    formatMessage(
        automaticManualImportEnabled
            ? messages.skippedFilesWarningBody
            : messages.skippedFilesWarningBodyDisabled,
        { count: skippedManualDownloads.value.length },
    ),
)
const missingPackMembers = computed(
    () =>
        contentSnapshot.value?.items.filter(
            (item) =>
                item.ownershipKind === 'pack_managed' &&
                item.required &&
                item.materializationState === 'missing',
        ) ?? [],
)

const manualPendingItems = computed<ContentItem[]>(() => {
    return skippedManualDownloads.value
        .filter(
            (item) =>
                !contentSnapshot.value?.items.some(
                    (snapshotItem) =>
                        snapshotItem.provider === 'curseforge' &&
                        snapshotItem.providerProjectId === String(item.projectId) &&
                        snapshotItem.providerReleaseId === String(item.fileId),
                ),
        )
        .map((item) => ({
            id: `__manual_${item.projectId}_${item.fileId}`,
            file_name: item.fileName,
            file_path: undefined,
            project_type: 'mod',
            update: null,
            origin_provider: null,
            enabled: false,
            pendingManualDownload: true,
            instanceOwnershipKind: item.ownershipKind,
            instanceMaterializationState: 'pending_manual',
            instanceCapabilities: {
                canToggle: false,
                canDelete: false,
                canUpdate: false,
                canChangeVersion: false,
                canRestorePackDefault: false,
            },
            project: {
                id: `curseforge:${item.projectId}`,
                slug: String(item.projectId),
                title: item.fileName,
                icon_url: undefined,
            },
            version: {
                id: String(item.fileId),
                version_number: String(item.fileId),
                file_name: item.fileName,
            },
            provider_refs: [
                {
                    provider: 'curseforge',
                    project_id: item.projectId,
                    file_id: item.fileId,
                },
            ],
        }))
})

const manualWarningExpanded = ref(true)
const missingWarningExpanded = ref(true)
const restoringMissingPackMemberIds = ref(new Set<string>())
const visibleSkippedManualDownloads = computed(() => skippedManualDownloads.value.slice(0, 5))
const hiddenSkippedManualDownloadCount = computed(() =>
    Math.max(0, skippedManualDownloads.value.length - visibleSkippedManualDownloads.value.length),
)

async function openManualCurseForgeDownload(item: CurseForgeManualDownloadItem) {
    showCurseForgeManualDownloads(props.instance.id, skippedManualDownloads.value)
    await openUrl(getCurseForgeManualDownloadUrl(item))
}

function openManualCurseForgeResolver() {
    showCurseForgeManualDownloads(props.instance.id, skippedManualDownloads.value)
}

function isRestoringMissingPackMember(item: InstanceContentSnapshotItem) {
    return item.memberId != null && restoringMissingPackMemberIds.value.has(item.memberId)
}

async function restoreMissingPackMember(item: InstanceContentSnapshotItem) {
    const memberId = item.memberId
    if (!memberId || restoringMissingPackMemberIds.value.has(memberId)) return

    restoringMissingPackMemberIds.value = new Set([
        ...restoringMissingPackMemberIds.value,
        memberId,
    ])
    try {
        await restore_pack_member_default(props.instance.id, memberId)
        await refreshContentState('bypass')
    } catch (error) {
        handleError(error as Error)
    } finally {
        const next = new Set(restoringMissingPackMemberIds.value)
        next.delete(memberId)
        restoringMissingPackMemberIds.value = next
    }
}

const {
    worldDatapackItems,
    isWorldDatapackItem,
    loadWorldDatapacks,
    deleteWorldDatapackItem,
    toggleWorldDatapackItem,
} = useWorldDatapacks(() => props.instance.id)

function displayContentIconUrl(item: Pick<ContentItem, 'project'>): string | undefined {
    const iconUrl = item.project?.icon_url
    if (!iconUrl) return undefined
    if (item.project?.id.startsWith('curseforge:')) {
        const resolved = getCurseForgeImageUrl(iconUrl) ?? iconUrl
        return resolved
    }
    return localContentIconUrl(iconUrl)
}

const mergedProjects = computed<ContentItem[]>(() => {
    const active = installingItems.value.get(props.instance.id)
    const pending = active ?? installingBuffer.value
    const pendingProjectIds = new Set(pending.map((p) => p.project?.id).filter(Boolean))
    const displayProjects = projects.value
        .filter((project) => !isWorldSaveContentItem(project))
        .map((project) => {
            const resolved = project.project?.icon_url
                ? {
                      ...project,
                      project: {
                          ...project.project,
                          icon_url: displayContentIconUrl(project),
                      },
                  }
                : project
            return (resolved.project?.id && pendingProjectIds.has(resolved.project.id)) ||
                hasGlobalContentChange(resolved)
                ? { ...resolved, installing: true }
                : resolved
        })
    const realProjectIds = new Set(displayProjects.map((p) => p.project?.id).filter(Boolean))
    const placeholders = pending.filter(
        (item) => !isWorldSaveContentItem(item) && !realProjectIds.has(item.project?.id),
    )
    return [...displayProjects, ...placeholders, ...worldDatapackItems.value]
})

const displayedLinkedModpackContentItems = computed(() => [
    ...linkedModpackContentItems.value.map((item) =>
        hasGlobalContentChange(item) ? { ...item, installing: true } : item,
    ),
    ...manualPendingItems.value,
])

function duplicateModKey(item: ContentItem): string | null {
    return confirmedContentGroupKey(item)
}

const duplicateModCounts = computed(() => {
    const counts = new Map<string, number>()
    for (const item of [...mergedProjects.value, ...displayedLinkedModpackContentItems.value]) {
        const key = duplicateModKey(item)
        if (key) counts.set(key, (counts.get(key) ?? 0) + 1)
    }
    return counts
})

const duplicateModItems = computed(() =>
    [...mergedProjects.value, ...displayedLinkedModpackContentItems.value].filter((item) => {
        const key = duplicateModKey(item)
        return key != null && (duplicateModCounts.value.get(key) ?? 0) > 1
    }),
)

let previousManualDownloadKeys = new Set<string>()
watch(
    skippedManualDownloads,
    (items) => {
        const nextKeys = new Set(
            items.map((item) => `${props.instance.id}:${item.projectId}:${item.fileId}`),
        )
        if ([...nextKeys].some((key) => !previousManualDownloadKeys.has(key))) {
            manualWarningExpanded.value = true
        }
        previousManualDownloadKeys = nextKeys
    },
    { immediate: true },
)

watch(
    () => installFailureRevisionByInstance.value.get(props.instance.id) ?? 0,
    (revision, previousRevision) => {
        if (revision === previousRevision) return
        debugState('installFailureRevision changed → clear buffer', {
            instanceId: props.instance.id,
            revision,
        })
        installingBuffer.value = []
    },
)

const localImportedModpackProject = computed<ContentModpackCardProject | null>(() => {
    const link = props.instance.link
    if (localImportedModpackUnlinked.value || link?.type !== 'imported_modpack') return null

    return {
        id: link.filename ?? props.instance.id,
        slug: link.filename ?? props.instance.id,
        title: link.name ?? props.instance.name,
        icon_url: props.instance.icon_path ? convertFileSrc(props.instance.icon_path) : undefined,
        description: '',
        filename: link.filename ?? undefined,
    }
})

const curseForgeModpackFallbackProject = computed<ContentModpackCardProject | null>(() => {
    const link = props.instance.link
    if (link?.type !== 'curseforge_modpack') return null

    return {
        id: `curseforge:${link.project_id}`,
        slug: link.project_id,
        title: props.instance.name,
        icon_url: props.instance.icon_path ? convertFileSrc(props.instance.icon_path) : undefined,
        description: '',
    }
})

const displayedModpackProject = computed(() => {
    const fallbackProject =
        curseForgeModpackFallbackProject.value ?? localImportedModpackProject.value
    const project = linkedModpackProject.value ?? fallbackProject
    if (!project) return undefined
    return {
        ...project,
        icon_url: project.id.startsWith('curseforge:')
            ? (getCurseForgeImageUrl(project.icon_url || fallbackProject?.icon_url) ??
              project.icon_url)
            : localContentIconUrl(project.icon_url || fallbackProject?.icon_url),
    }
})

watch(
    () => props.instance.link,
    (newLink) => {
        localImportedModpackUnlinked.value = false
        if (!newLink) {
            linkedModpackContentItems.value = []
        }
    },
)

const isModpackUpdating = ref(false)
const localBulkOperating = ref(false)
const activeContentChangeJobs = computed(() =>
    selectActiveContentChangeJobs(downloadManager.jobs.value, props.instance.id),
)
const pendingContentChanges = computed(() =>
    downloadManager.pendingContentChanges.value.filter(
        (pending) => pending.instanceId === props.instance.id,
    ),
)
const isBulkOperating = localBulkOperating
const isInstanceBusy = computed(() => props.instance?.install_stage !== 'installed')
const isPackUnlinkDisabled = computed(
    () =>
        isInstanceBusy.value ||
        activeContentChangeJobs.value.length > 0 ||
        pendingContentChanges.value.length > 0,
)
let contentEnrichmentController: AbortController | undefined
let contentRequestGeneration = 0
let activeBypassRefresh: Promise<void> | null = null

function isCurrentContentRequest(instanceId: string, generation: number) {
    return (
        !isUnmounted &&
        props.instance.id === instanceId &&
        props.instance.install_stage === 'installed' &&
        contentRequestGeneration === generation
    )
}

function invalidateContentRequests(clearState = false) {
    contentEnrichmentController?.abort()
    contentRequestGeneration += 1
    visibleMetadataPaths.value.clear()
    visibleMetadataLoadingPaths.value.clear()
    contentUpdateLoadingPaths.value.clear()
    if (!clearState) return

    onlineInformationDismissed.value = false
    contentSnapshot.value = null
    projects.value = []
    linkedModpackContentItems.value = []
    installingBuffer.value = []
    linkedModpackProject.value = null
    linkedModpackVersion.value = null
    linkedModpackOwner.value = null
    linkedModpackCategories.value = []
    linkedModpackHasUpdate.value = false
    linkedModpackUpdateVersionId.value = null
    loading.value = false
}
const isPackLocked = computed(
    () =>
        props.instance?.link?.type === 'modrinth_modpack' ||
        props.instance?.link?.type === 'curseforge_modpack' ||
        props.instance?.link?.type === 'server_project_modpack',
)
const isCurseForgeLinkedModpack = computed(
    () => props.instance?.link?.type === 'curseforge_modpack',
)

const shareModal = ref<InstanceType<typeof ShareModal> | null>()
const exportModal = ref(null)
const resourceOperationPreview = ref<InstanceType<typeof ResourceOperationPreviewModal> | null>(
    null,
)
const resourceHistoryModal = ref<InstanceType<typeof ResourceHistoryModal> | null>(null)
const resourceBackupsModal = ref<InstanceType<typeof ResourceBackupsModal> | null>(null)
const contentUpdaterModal = ref<InstanceType<typeof ContentUpdaterModal> | null>()
const dependencyGraphModal = ref<InstanceType<typeof DependencyGraphModal> | null>()
const modpackUpdateConfirmModal = ref<InstanceType<typeof ConfirmModpackUpdateModal> | null>()

function allDependencyGraphItems() {
    const itemsById = new Map<string, ContentItem>()
    for (const item of [...mergedProjects.value, ...displayedLinkedModpackContentItems.value]) {
        const id = getContentItemId(item)
        if (id) itemsById.set(id, item)
    }
    return [...itemsById.values()].map((item) => {
        if (!item.project?.icon_url) return item
        return {
            ...item,
            project: {
                ...item.project,
                icon_url: displayContentIconUrl(item),
            },
        }
    })
}

function handleViewDependencies() {
    dependencyGraphModal.value?.show(allDependencyGraphItems())
}

// TODO: Extract content operation and updater modal state into composables; this page currently owns file mutations, dependency installs, busy flags, and version selection flow.
const updatingProject = ref<ContentItem | null>(null)
const updatingProjectVersions = ref<Labrinth.Versions.v2.Version[]>([])
const updaterReleaseRefs = new Map<string, SourceReleaseRef>()
const loadingVersions = ref(false)
const contentUpdateSubmitting = ref(false)
const CONTENT_OPERATION_TIMEOUT_MS = 30_000

async function withContentOperationTimeout<T>(operation: Promise<T>): Promise<T> {
    let timeoutId: ReturnType<typeof setTimeout> | undefined
    try {
        return await Promise.race([
            operation,
            new Promise<T>((_, reject) => {
                timeoutId = setTimeout(
                    () => reject(new Error(formatMessage(messages.contentOperationTimedOut))),
                    CONTENT_OPERATION_TIMEOUT_MS,
                )
            }),
        ])
    } finally {
        if (timeoutId) clearTimeout(timeoutId)
    }
}
const updatingModpack = ref(false)
const pendingModpackUpdateVersion = ref<Labrinth.Versions.v2.Version | null>(null)
const isModpackUpdateDowngrade = ref(false)
const activeContentOperationKeys = ref(new Set<string>())

let activeContentOperationCount = 0
let updateRequestId = 0
const activeUpdateRequestId = ref(0)

function fileNameFromPath(path: string) {
    return path.split('/').pop() ?? path
}

function updateLinkedModpackContentCache(
    target: ContentItem,
    originalFileName: string,
    originalFilePath: string | undefined,
    updates: Partial<ContentItem>,
) {
    const items = linkedModpackContentItems.value
    if (items.length === 0) return

    const updated = items.map((item) =>
        matchesContentItem(item, target, originalFileName, originalFilePath)
            ? { ...item, ...updates, installing: false }
            : item,
    )
    linkedModpackContentItems.value = updated
}

function getContentItemId(item: ContentItem | null | undefined) {
    return (
        item?.instanceEntryId ??
        item?.instanceMemberId ??
        item?.instanceFileId ??
        item?.file_path ??
        item?.file_name ??
        item?.id ??
        ''
    )
}

function mergeVisibleMetadataItems(
    refreshedItems: ContentItem[],
    originals: Map<string, ContentItem>,
) {
    const refreshedByPath = new Map(
        refreshedItems.filter((item) => !!item.file_path).map((item) => [item.file_path!, item]),
    )

    const mergeItems = (items: ContentItem[]) =>
        items.map((item) => {
            const refreshed = item.file_path ? refreshedByPath.get(item.file_path) : undefined

            const original = item.file_path ? originals.get(item.file_path) : undefined
            if (
                !refreshed ||
                !original ||
                original.id !== item.id ||
                original.instanceEntryId !== item.instanceEntryId ||
                hasContentOperation(item)
            )
                return item

            return mergeContentItemMetadata(item, refreshed)
        })

    projects.value = mergeItems(projects.value)
    linkedModpackContentItems.value = mergeItems(linkedModpackContentItems.value)

    dependencyGraphModal.value?.setItems(allDependencyGraphItems())
}

function mergeVisibleUpdate(item: ContentItem) {
    if (!item.instanceEntryId) return
    const current = contentItemsByEntryId.value.get(item.instanceEntryId)
    if (!current || current.file_path !== item.file_path || hasContentOperation(current)) return
    current.resourceUpdate = item.resourceUpdate
    if (current.instanceCapabilities) {
        current.instanceCapabilities = {
            ...current.instanceCapabilities,
            canUpdate: !!item.resourceUpdate && current.instanceMaterializationState === 'present',
        }
    }
}

async function flushVisibleMetadataRefresh() {
    if (visibleMetadataRefreshRunning) return
    visibleMetadataRefreshRunning = true

    try {
        while (visibleMetadataPendingPaths.size > 0) {
            const instanceId = props.instance.id
            const generation = contentRequestGeneration
            const paths = [...visibleMetadataPendingPaths].filter((path) => {
                if (visibleMetadataPaths.value.has(path)) return true
                visibleMetadataRequestedPaths.delete(path)
                visibleMetadataLoadingPaths.value.delete(path)
                contentUpdateLoadingPaths.value.delete(path)
                return false
            })
            visibleMetadataPendingPaths.clear()
            const snapshot = contentSnapshot.value
            if (!paths.length) continue
            if (!snapshot) {
                for (const path of paths) {
                    visibleMetadataRequestedPaths.delete(path)
                    visibleMetadataLoadingPaths.value.delete(path)
                    contentUpdateLoadingPaths.value.delete(path)
                }
                continue
            }
            const pathSet = new Set(paths)
            const originals = new Map(
                [...projects.value, ...linkedModpackContentItems.value]
                    .filter((item) => item.file_path && pathSet.has(item.file_path))
                    .map((item) => [item.file_path!, item]),
            )
            const originalsByEntry = new Map(
                [...originals.values()].map((item) => [item.instanceEntryId, item]),
            )
            const controller = (contentEnrichmentController ??= new AbortController())
            const isCurrent = () =>
                !controller.signal.aborted && isCurrentContentRequest(instanceId, generation)

            try {
                const results = await Promise.allSettled([
                    (async () => {
                        let refreshedItems = await get_content_items_by_paths(instanceId, paths)
                        if (!isCurrent()) return
                        refreshedItems = await translateContentItemTitles(
                            refreshedItems,
                            i18n.global.locale.value,
                        ).catch(() => refreshedItems)
                        if (isCurrent()) mergeVisibleMetadataItems(refreshedItems, originals)
                        if (isCurrent()) {
                            for (const path of paths) visibleMetadataLoadingPaths.value.delete(path)
                        }
                    })(),
                    (async () => {
                        const updated = await refreshInstanceContentUpdates(
                            {
                                path: instanceId,
                                snapshot,
                                contentItems: [...originals.values()],
                                linkedContentItems: [],
                                modpack: null,
                            },
                            controller.signal,
                            {
                                onProgress: (progress) => {
                                    if (!isCurrent()) return
                                    const original = originalsByEntry.get(progress.entryId)
                                    if (original?.file_path)
                                        contentUpdateLoadingPaths.value.delete(original.file_path)
                                    if (progress.failed || progress.unresolved) return
                                    if (original)
                                        mergeVisibleUpdate({
                                            ...original,
                                            resourceUpdate: progress.release,
                                        })
                                },
                            },
                        )
                        if (!isCurrent()) return
                        const failedEntries = new Set(updated.updateCheckFailedEntries ?? [])
                        for (const item of [
                            ...updated.contentItems,
                            ...updated.linkedContentItems,
                        ]) {
                            if (item.instanceEntryId && !failedEntries.has(item.instanceEntryId))
                                mergeVisibleUpdate(item)
                        }
                        for (const path of paths) contentUpdateLoadingPaths.value.delete(path)
                        if (contentSnapshot.value) {
                            contentSnapshot.value = {
                                ...contentSnapshot.value,
                                warnings: updated.snapshot.warnings,
                            }
                        }
                    })(),
                ])
                if (!isCurrent()) continue
                for (const result of results) {
                    if (result.status !== 'rejected') continue
                    for (const path of paths) visibleMetadataRequestedPaths.delete(path)
                    debugState('visible metadata refresh failed', {
                        instanceId,
                        paths,
                        error: result.reason,
                    })
                }
            } finally {
                if (isCurrentContentRequest(instanceId, generation)) {
                    for (const path of paths) {
                        visibleMetadataLoadingPaths.value.delete(path)
                        contentUpdateLoadingPaths.value.delete(path)
                    }
                }
            }
        }
    } finally {
        visibleMetadataRefreshRunning = false
    }
}

function handleVisibleItems(items: ContentItem[]) {
    if (isUnmounted || props.instance.install_stage !== 'installed') {
        return
    }

    const knownPaths = new Set(
        [...projects.value, ...linkedModpackContentItems.value]
            .map((item) => item.file_path)
            .filter((path): path is string => !!path),
    )
    visibleMetadataPaths.value = new Set(
        items.map((item) => item.file_path).filter((path): path is string => !!path),
    )

    let queued = false

    for (const item of items) {
        const path = item.file_path

        if (!path || !knownPaths.has(path) || visibleMetadataRequestedPaths.has(path)) {
            continue
        }

        visibleMetadataRequestedPaths.add(path)
        visibleMetadataPendingPaths.add(path)
        visibleMetadataLoadingPaths.value.add(path)
        if (item.instanceEntryId && item.instanceMaterializationState === 'present')
            contentUpdateLoadingPaths.value.add(path)
        queued = true
    }

    if (queued) {
        void flushVisibleMetadataRefresh()
    }
}

function getStableContentId(item: ContentItem) {
    return contentItemStableId(item)
}

function hasGlobalContentChange(item: ContentItem) {
    return (
        pendingContentChanges.value.some((pending) =>
            pendingContentChangeAffectsItem(pending, item),
        ) || hasActiveContentChange(activeContentChangeJobs.value, item)
    )
}

function getContentOperationKeys(item: ContentItem) {
    return [getContentItemId(item), item.file_path, item.file_name].filter(
        (key): key is string => !!key,
    )
}

function hasContentOperation(item: ContentItem) {
    const keys = getContentOperationKeys(item)
    return (
        keys.some((key) => activeContentOperationKeys.value.has(key)) ||
        hasGlobalContentChange(item)
    )
}

function canUpdateProject(item: ContentItem) {
    return (
        (item.resourceUpdate != null || item.update != null) &&
        (item.instanceCapabilities?.canUpdate ?? true)
    )
}

function contentUpdateId(item: ContentItem): string | null {
    if (item.resourceUpdate) return item.resourceUpdate.releaseId
    if (!item.update) return null
    return item.update.provider === 'modrinth'
        ? item.update.target_version_id
        : String(item.update.target_file_id)
}

function setContentItemBusy(item: ContentItem, busy: boolean, _originalFileName = item.file_name) {
    applyContentItemUpdates(
        [...projects.value, ...linkedModpackContentItems.value, ...worldDatapackItems.value],
        item,
        _originalFileName,
        item.file_path,
        { installing: busy },
    )
}

function beginContentOperation(item: ContentItem) {
    if (hasContentOperation(item)) return null

    const keys = getContentOperationKeys(item)
    activeContentOperationKeys.value = new Set([...activeContentOperationKeys.value, ...keys])
    activeContentOperationCount++
    isBulkOperating.value = true
    setContentItemBusy(item, true)

    return { keys, originalFileName: item.file_name }
}

function finishContentOperation(
    item: ContentItem,
    operation: { keys: string[]; originalFileName: string },
) {
    const nextKeys = new Set(activeContentOperationKeys.value)
    for (const key of operation.keys) {
        nextKeys.delete(key)
    }
    for (const key of getContentOperationKeys(item)) {
        nextKeys.delete(key)
    }
    activeContentOperationKeys.value = nextKeys
    activeContentOperationCount = Math.max(0, activeContentOperationCount - 1)
    setContentItemBusy(item, false, operation.originalFileName)
    if (activeContentOperationCount === 0) {
        isBulkOperating.value = false
    }
}

function beginUpdateRequest() {
    updateRequestId++
    activeUpdateRequestId.value = updateRequestId
    return updateRequestId
}

function isActiveUpdateRequest(requestId: number) {
    return activeUpdateRequestId.value === requestId
}

function sortVersionsByPublishedDate(versions: Labrinth.Versions.v2.Version[]) {
    return [...versions].sort(
        (a, b) => new Date(b.date_published).getTime() - new Date(a.date_published).getTime(),
    )
}

function mergeVersionIntoList(
    versions: Labrinth.Versions.v2.Version[],
    version: Labrinth.Versions.v2.Version,
) {
    const existingIndex = versions.findIndex((v) => v.id === version.id)
    if (existingIndex === -1) {
        return sortVersionsByPublishedDate([version, ...versions])
    }

    const mergedVersions = [...versions]
    mergedVersions[existingIndex] = version
    return sortVersionsByPublishedDate(mergedVersions)
}

function parseCurseForgeProjectId(projectId: string): number | null {
    const raw = projectId.startsWith('curseforge:')
        ? projectId.slice('curseforge:'.length)
        : projectId
    const numeric = Number(raw)
    return Number.isFinite(numeric) ? numeric : null
}

function getCurseForgeProjectIdForItem(item: ContentItem | null | undefined): number | null {
    if (!item || item.origin_provider !== 'curseforge') return null

    const reference = item.provider_refs?.find((ref) => ref.provider === 'curseforge')
    if (reference) {
        return reference.project_id
    }

    return item.project?.id ? parseCurseForgeProjectId(item.project.id) : null
}

function mapCurseForgeFileToUpdaterVersion(
    file: {
        id: number
        displayName: string
        fileName: string
        releaseType: number
        fileDate: string
        downloadUrl?: string | null
        fileLength: number
        gameVersions: string[]
    },
    projectId: number,
): Labrinth.Versions.v2.Version {
    const loaders = [
        ...new Set(
            file.gameVersions
                .map((value) => {
                    switch (value.toLowerCase().replaceAll(' ', '')) {
                        case 'forge':
                            return 'forge'
                        case 'fabric':
                        case 'fabricloader':
                            return 'fabric'
                        case 'quilt':
                            return 'quilt'
                        case 'neoforge':
                            return 'neoforge'
                        default:
                            return null
                    }
                })
                .filter(Boolean),
        ),
    ] as string[]
    const gameVersions = file.gameVersions.filter((value) => {
        const normalized = value.toLowerCase().replaceAll(' ', '')
        return !['forge', 'fabric', 'fabricloader', 'quilt', 'neoforge'].includes(normalized)
    })
    return {
        id: file.id.toString(),
        project_id: `curseforge:${projectId}`,
        name: file.displayName,
        version_number: file.displayName,
        game_versions: gameVersions,
        loaders: loaders.length > 0 ? loaders : ['minecraft'],
        date_published: file.fileDate,
        version_type:
            file.releaseType === 1 ? 'release' : file.releaseType === 2 ? 'beta' : 'alpha',
        files: [
            {
                filename: file.fileName,
                url: file.downloadUrl ?? '',
                primary: true,
                size: file.fileLength,
                hashes: {},
            },
        ],
    } as unknown as Labrinth.Versions.v2.Version
}

async function getUpdaterProjectVersions(
    projectId: string,
    pinnedVersionId?: string,
    item?: ContentItem | null,
) {
    if (item?.instanceEntryId) {
        const snapshot = contentSnapshot.value?.items.find(
            (entry) => entry.entryId === item.instanceEntryId,
        )
        const project: SourceProjectRef | null =
            snapshot?.artifactRef?.release.project ??
            (snapshot?.provider && snapshot.providerProjectId
                ? { source: snapshot.provider, projectId: snapshot.providerProjectId }
                : null)
        if (!project) return []
        const requestId = activeUpdateRequestId.value
        const releases = []
        let cursor: string | null = null
        const cursors = new Set<string>()
        do {
            const page = await resourceReleases({ kind: 'source', project }, undefined, cursor)
            if (!isActiveUpdateRequest(requestId)) return []
            releases.push(...page.entries)
            cursor = page.cursor
            if (cursor && cursors.has(cursor))
                throw new Error(formatMessage(messages.versionsFailed))
            if (cursor) cursors.add(cursor)
        } while (cursor)
        if (
            pinnedVersionId &&
            !releases.some((release) => release.reference.releaseId === pinnedVersionId)
        ) {
            const [document] = await resourceReleaseDocuments(
                project.source,
                [pinnedVersionId],
                project.projectId,
            )
            if (!isActiveUpdateRequest(requestId)) return []
            if (document) releases.push(document.release)
        }
        return sortVersionsByPublishedDate(
            releases.map((release) => {
                updaterReleaseRefs.set(release.reference.releaseId, release.reference)
                return {
                    id: release.reference.releaseId,
                    project_id: release.reference.project.projectId,
                    name: release.version,
                    version_number: release.version,
                    date_published: release.publishedAt ?? '',
                    version_type: release.channel,
                    game_versions: release.compatibility.gameVersions,
                    loaders: release.compatibility.loaders,
                    changelog:
                        project.source === 'mcarchive' || project.source === 'planet_minecraft'
                            ? ''
                            : undefined,
                    files: release.artifacts.map((artifact) => ({
                        filename: artifact.fileName ?? '',
                        url:
                            artifact.acquisition.kind === 'publicDownload'
                                ? (artifact.acquisition.urls[0] ?? '')
                                : '',
                        primary: artifact.role === 'primary',
                        size: artifact.integrity.size ?? 0,
                        hashes: Object.fromEntries(
                            artifact.integrity.hashes.map((hash) => [hash.algorithm, hash.value]),
                        ),
                    })),
                } as Labrinth.Versions.v2.Version
            }),
        )
    }
    const itemCurseForgeProjectId = getCurseForgeProjectIdForItem(item)
    const curseForgeProjectId = itemCurseForgeProjectId ?? parseCurseForgeProjectId(projectId)
    const useCurseForge =
        item?.origin_provider === 'curseforge' ||
        (!item && isCurseForgeLinkedModpack.value) ||
        projectId.startsWith('curseforge:')
    if (useCurseForge) {
        if (curseForgeProjectId == null) return []
        const { getCurseForgeFile, getCurseForgeFiles } = await import('@/helpers/curseforge')
        const files: CurseForgeFile[] = []
        let index = 0
        while (true) {
            const response = await getCurseForgeFiles(curseForgeProjectId, {
                index,
                pageSize: 50,
            }).catch((err) => {
                handleError(err as Error)
                return null
            })
            if (!response) break
            files.push(...response.files)
            index += response.files.length
            if (
                response.files.length === 0 ||
                index >= (response.pagination?.totalCount ?? response.files.length)
            ) {
                break
            }
        }
        const pinnedFileId = Number(pinnedVersionId)
        if (Number.isFinite(pinnedFileId) && !files.some((file) => file.id === pinnedFileId)) {
            const pinnedFile = await getCurseForgeFile(curseForgeProjectId, pinnedFileId).catch(
                () => null,
            )
            if (pinnedFile?.isAvailable) {
                files.push(pinnedFile)
            }
        }
        return sortVersionsByPublishedDate(
            files
                .filter((file) => file.isAvailable)
                .map((file) => mapCurseForgeFileToUpdaterVersion(file, curseForgeProjectId)),
        )
    }

    let fetchError: unknown = null
    let versions = (await get_project_versions(projectId, 'bypass').catch((err) => {
        fetchError = err
        return null
    })) as Labrinth.Versions.v2.Version[] | null

    if (!versions) {
        versions = (await get_project_versions(projectId).catch(() => null)) as
            Labrinth.Versions.v2.Version[] | null
    }

    if (!versions && fetchError) {
        handleError(fetchError as Error)
    }

    let mergedVersions = sortVersionsByPublishedDate(versions ?? [])

    if (pinnedVersionId && !mergedVersions.some((version) => version.id === pinnedVersionId)) {
        const pinnedVersion = (await get_version(pinnedVersionId, 'bypass').catch(
            () => null,
        )) as Labrinth.Versions.v2.Version | null

        if (pinnedVersion) {
            mergedVersions = mergeVersionIntoList(mergedVersions, pinnedVersion)
        }
    }

    return mergedVersions
}

async function handleBrowseContent() {
    if (!props.instance) return
    await router.push({
        path: `/browse/${props.instance.loader === 'vanilla' ? 'resourcepack' : 'mod'}`,
        query: { i: props.instance.id },
    })
}

async function handleUploadFiles() {
    if (!props.instance) return
    const files = await open({ multiple: true })
    if (!files) return

    const addedFiles: string[] = []
    for (const file of files) {
        const path = (file as { path?: string }).path ?? file
        const fileName = typeof path === 'string' ? (path.split('/').pop() ?? path) : String(path)
        try {
            await add_project_from_path(props.instance.id, path)
            addedFiles.push(fileName)
        } catch (e) {
            handleError(e as Error)
        }
    }
    await initProjects()

    if (addedFiles.length > 0) {
        const names = addedFiles.map((f) => {
            const item = projects.value.find(
                (p) => p.file_name === f || p.file_name === f.replace('.zip', '.jar'),
            )
            return item?.project?.title ?? f
        })
        addNotification({
            type: 'success',
            title: formatMessage(messages.successfullyUploaded),
            text:
                names.length === 1
                    ? formatMessage(messages.projectWasAdded, { name: names[0] })
                    : formatMessage(messages.projectsWereAdded, { count: names.length }),
        })
    }
}

function dependencyRefMatchesContentItem(
    reference: { provider: 'modrinth' | 'curseforge' | 'local'; projectId: string },
    item: ContentItem,
) {
    const itemProjectId = item.project?.id ?? ''
    if (reference.provider === 'curseforge') {
        // CurseForge content items carry either a bare numeric id or a
        // `curseforge:<id>` prefixed one depending on where they were built.
        const normalized = itemProjectId.startsWith('curseforge:')
            ? itemProjectId.slice('curseforge:'.length)
            : itemProjectId
        return normalized === reference.projectId
    }
    // Modrinth and local edges both reference the project id directly.
    return itemProjectId === reference.projectId
}

const toggleDependenciesModal = ref<InstanceType<typeof ContentToggleDependenciesModal> | null>(
    null,
)

function collectToggleRelated(items: ContentItem[], enabling: boolean): ContentItem[] {
    const targetIds = new Set(items.map(getContentItemId))
    const relatedIds = new Set<string>()
    const related: ContentItem[] = []
    const queue = [...items]
    while (queue.length > 0) {
        const current = queue.shift()
        if (!current) continue
        const references = enabling ? current.dependency?.requires : current.dependency?.requiredBy
        for (const reference of references ?? []) {
            const match = [...projects.value, ...linkedModpackContentItems.value].find(
                (candidate) => dependencyRefMatchesContentItem(reference, candidate),
            )
            if (!match) continue
            if (match.instanceCapabilities?.canToggle === false) continue
            const id = getContentItemId(match)
            if (targetIds.has(id) || relatedIds.has(id)) continue
            const needsChange = enabling ? !match.enabled : match.enabled
            if (!needsChange) continue
            relatedIds.add(id)
            related.push(match)
            queue.push(match)
        }
    }
    return related
}

async function promptToggleDependencies(
    targets: ContentItem[],
    related: ContentItem[],
    enabling: boolean,
): Promise<'apply' | 'selected' | 'cancel'> {
    if (!toggleDependenciesModal.value) return 'selected'
    return await toggleDependenciesModal.value.show({
        enabling,
        bulk: targets.length > 1,
        primaryTitle: targets[0]?.project?.title ?? targets[0]?.file_name ?? '',
        related: related.map((item) => ({
            title: item.project?.title ?? item.file_name,
            iconUrl: displayContentIconUrl(item) ?? null,
            versionNumber: item.version?.version_number,
        })),
    })
}

async function toggleDisableBatch(items: ContentItem[], enabling: boolean) {
    if (items.length === 0) return false
    const related = collectToggleRelated(items, enabling)
    let targets = items
    if (related.length > 0) {
        const choice = await promptToggleDependencies(items, related, enabling)
        if (choice === 'cancel') return false
        if (choice === 'apply') {
            targets = [...related, ...targets]
        }
    }
    const uniqueTargets = [
        ...new Map(targets.map((item) => [getContentItemId(item), item])).values(),
    ]
    const contentTargets = uniqueTargets.filter((item) => !isWorldDatapackItem(item))
    const worldTargets = uniqueTargets.filter(isWorldDatapackItem)

    if (contentTargets.length > 0) {
        await applyToggleDisableBatch(contentTargets, enabling)
    }
    for (const item of worldTargets) {
        await toggleDisableMod(item, enabling, true)
    }
    return true
}

function notifyOrphanedDependencies() {
    const orphaned = projects.value.filter((item) => item.dependency?.orphaned)
    if (orphaned.length === 0) return
    const names = orphaned.map((item) => item.project?.title ?? item.file_name)
    const list = names.slice(0, 5).join(', ') + (names.length > 5 ? ', …' : '')
    addNotification({
        type: 'info',
        title: formatMessage(messages.orphanedDependenciesTitle),
        text: formatMessage(messages.orphanedDependenciesBody, { list }),
    })
}

async function toggleDisableMod(
    mod: ContentItem,
    desiredEnabled?: boolean,
    skipDependencyPrompt = false,
) {
    if (isWorldDatapackItem(mod)) {
        if (mod.instanceCapabilities?.canToggle === false) return
        const enabled = desiredEnabled ?? !mod.enabled
        try {
            await toggleWorldDatapackItem(mod, enabled)
            trackEvent('InstanceProjectDisable', {
                loader: props.instance.loader,
                game_version: props.instance.game_version,
                id: mod.project?.id,
                name: mod.project?.title ?? mod.file_name,
                project_type: mod.project_type,
                disabled: !enabled,
            })
        } catch (err) {
            handleError(err as Error)
            throw err
        }
        return
    }

    const enabled = desiredEnabled ?? !mod.enabled
    if (!skipDependencyPrompt && mod.dependency) {
        const related = collectToggleRelated([mod], enabled)
        if (related.length > 0) {
            const choice = await promptToggleDependencies([mod], related, enabled)
            if (choice === 'cancel') return
            if (choice === 'apply') {
                for (const item of related) {
                    await toggleDisableMod(item, enabled, true)
                }
            }
        }
    }
    await applyToggleDisableMod(mod, enabled)
}

async function applyToggleDisableMod(mod: ContentItem, enabled: boolean) {
    const contentId = getStableContentId(mod)
    if (!mod.file_path || !contentId || mod.instanceCapabilities?.canToggle === false) return
    const operation = beginContentOperation(mod)
    if (!operation) return
    const originalFilePath = mod.file_path
    const originalFileName = mod.file_name
    const originalEnabled = mod.enabled
    let trimmedPath = originalFilePath
    while (trimmedPath.endsWith('.disabled')) {
        trimmedPath = trimmedPath.slice(0, -'.disabled'.length)
    }
    const optimisticPath = enabled ? trimmedPath : `${trimmedPath}.disabled`
    applyContentItemToggleState(mod, operation.originalFileName, originalFilePath, {
        file_path: optimisticPath,
        file_name: fileNameFromPath(optimisticPath),
        enabled,
    })
    const optimisticKeys = getContentOperationKeys(mod)
    activeContentOperationKeys.value = new Set([
        ...activeContentOperationKeys.value,
        ...optimisticKeys,
    ])
    operation.keys = [...operation.keys, ...optimisticKeys]

    try {
        suppressSyncedUntil = Date.now() + CONTENT_EVENT_DEBOUNCE_MS * 4
        const newPath = await toggle_content_entry(props.instance.id, contentId, enabled)
        const newFileName = fileNameFromPath(newPath)
        const actualEnabled = !newPath.endsWith('.disabled')
        applyContentItemToggleState(mod, operation.originalFileName, originalFilePath, {
            file_path: newPath,
            file_name: newFileName,
            enabled: actualEnabled,
        })

        trackEvent('InstanceProjectDisable', {
            loader: props.instance.loader,
            game_version: props.instance.game_version,
            id: mod.project?.id,
            name: mod.project?.title ?? mod.file_name,
            project_type: mod.project_type,
            disabled: !actualEnabled,
        })
    } catch (err) {
        applyContentItemToggleState(mod, operation.originalFileName, originalFilePath, {
            file_path: originalFilePath,
            file_name: originalFileName,
            enabled: originalEnabled,
        })
        handleError(err as Error)
    } finally {
        finishContentOperation(mod, operation)
    }
}

type ContentToggleBatchOperation = {
    item: ContentItem
    keys: string[]
    originalFileName: string
    originalFilePath: string
    originalEnabled: boolean
    optimisticPath: string
}

async function applyToggleDisableBatch(items: ContentItem[], enabled: boolean) {
    const operations: ContentToggleBatchOperation[] = []
    for (const item of items) {
        const contentId = getStableContentId(item)
        if (
            !contentId ||
            !item.file_path ||
            item.instanceCapabilities?.canToggle === false ||
            hasContentOperation(item)
        ) {
            continue
        }
        let trimmedPath = item.file_path
        while (trimmedPath.endsWith('.disabled')) {
            trimmedPath = trimmedPath.slice(0, -'.disabled'.length)
        }
        operations.push({
            item,
            keys: getContentOperationKeys(item),
            originalFileName: item.file_name,
            originalFilePath: item.file_path,
            originalEnabled: item.enabled,
            optimisticPath: enabled ? trimmedPath : `${trimmedPath}.disabled`,
        })
    }
    if (operations.length === 0) return

    const activeKeys = new Set(activeContentOperationKeys.value)
    for (const operation of operations) {
        for (const key of operation.keys) activeKeys.add(key)
    }
    activeContentOperationKeys.value = activeKeys
    activeContentOperationCount += operations.length
    isBulkOperating.value = true
    applyBatchToggleState(operations, enabled, undefined, false, true)

    try {
        suppressSyncedUntil = Date.now() + CONTENT_EVENT_DEBOUNCE_MS * 4
        const results = await toggle_content_entries(
            props.instance.id,
            operations.map((operation) => getStableContentId(operation.item)!),
            enabled,
        )
        const resultById = new Map(results.map((result) => [result.contentId, result]))
        applyBatchToggleState(
            operations.map((operation) => {
                const result = resultById.get(getStableContentId(operation.item)!)
                return {
                    ...operation,
                    optimisticPath: result?.path ?? operation.optimisticPath,
                }
            }),
            enabled,
            resultById,
            false,
        )

        for (const operation of operations) {
            trackEvent('InstanceProjectDisable', {
                loader: props.instance.loader,
                game_version: props.instance.game_version,
                id: operation.item.project?.id,
                name: operation.item.project?.title ?? operation.originalFileName,
                project_type: operation.item.project_type,
                disabled: !enabled,
            })
        }
    } catch (error) {
        for (const operation of operations) {
            operation.optimisticPath = operation.originalFilePath
        }
        applyBatchToggleState(operations, enabled, undefined, true)
        handleError(error as Error)
        throw error
    } finally {
        const activeKeys = new Set(activeContentOperationKeys.value)
        for (const operation of operations) {
            for (const key of operation.keys) activeKeys.delete(key)
        }
        activeContentOperationKeys.value = activeKeys
        activeContentOperationCount = Math.max(0, activeContentOperationCount - operations.length)
        if (activeContentOperationCount === 0) {
            isBulkOperating.value = false
            if (pendingContentRefreshAfterBulk) {
                pendingContentRefreshAfterBulk = false
                void initProjects()
            }
        }
    }
}

function applyBatchToggleState(
    operations: ContentToggleBatchOperation[],
    enabled: boolean,
    results?: Map<string, { path: string; enabled: boolean }>,
    restore = false,
    busy = false,
) {
    const operationsById = new Map(
        operations.map((operation) => [getContentItemId(operation.item), operation]),
    )
    const updateItems = (source: ContentItem[]) =>
        source.map((item) => {
            const operation = operationsById.get(getContentItemId(item))
            if (!operation) return item
            const result = results?.get(getStableContentId(operation.item)!)
            const path = restore
                ? operation.originalFilePath
                : (result?.path ?? operation.optimisticPath)
            return {
                ...item,
                file_path: path,
                file_name: fileNameFromPath(path),
                enabled: restore ? operation.originalEnabled : (result?.enabled ?? enabled),
                installing: busy,
            }
        })

    projects.value = updateItems(projects.value)
    linkedModpackContentItems.value = updateItems(linkedModpackContentItems.value)
}

function applyContentItemToggleState(
    target: ContentItem,
    originalFileName: string,
    originalFilePath: string,
    updates: Partial<ContentItem>,
) {
    applyContentItemUpdates(projects.value, target, originalFileName, originalFilePath, updates)
    updateLinkedModpackContentCache(target, originalFileName, originalFilePath, updates)
}

const toggleDisableDebounced = toggleDisableMod

async function removeMod(mod: ContentItem) {
    const contentId = getStableContentId(mod)
    if (
        !mod.file_path ||
        mod.instanceCapabilities?.canDelete === false ||
        (!isWorldDatapackItem(mod) && !contentId)
    ) {
        return
    }
    const operation = beginContentOperation(mod)
    if (!operation) return

    try {
        if (isWorldDatapackItem(mod)) {
            await deleteWorldDatapackItem(mod)
        } else {
            const removedPath = mod.file_path
            await remove_content_entry(props.instance.id, contentId)
            projects.value = projects.value.filter((x) => removedPath !== x.file_path)
        }

        trackEvent('InstanceProjectRemove', {
            loader: props.instance.loader,
            game_version: props.instance.game_version,
            id: mod.project?.id,
            name: mod.project?.title ?? mod.file_name,
            project_type: mod.project_type,
        })
    } catch (err) {
        handleError(err as Error)
        throw err
    } finally {
        finishContentOperation(mod, operation)
        void refreshContentState().then(notifyOrphanedDependencies).catch(handleError)
    }
}

async function restorePackDefault(item: ContentItem) {
    const memberId = item.instanceMemberId
    if (!memberId || !item.instanceCapabilities?.canRestorePackDefault) return
    const operation = beginContentOperation(item)
    if (!operation) return
    try {
        await restore_pack_member_default(props.instance.id, memberId)
        await refreshContentState('bypass')
    } catch (error) {
        handleError(error as Error)
    } finally {
        finishContentOperation(item, operation)
    }
}

function isBreakingDependency(dependency: Labrinth.Versions.v2.Dependency) {
    return dependency.dependency_type === 'required' || dependency.dependency_type === 'embedded'
}

function dependencyTargetsItem(dependency: Labrinth.Versions.v2.Dependency, item: ContentItem) {
    return (
        (!!dependency.project_id && dependency.project_id === item.project?.id) ||
        ('version_id' in dependency &&
            !!dependency.version_id &&
            dependency.version_id === item.version?.id)
    )
}

async function getDeleteDependencyWarning(items: ContentItem[]) {
    if (props.isServerInstance) return null

    const deletingIds = new Set(items.map(getContentItemId))
    // Pack-managed content participates in the dependency graph like any other
    // content, so dependents can live in either list.
    const remainingItems = [...projects.value, ...linkedModpackContentItems.value].filter(
        (item) => !deletingIds.has(getContentItemId(item)),
    )

    const persistedWarning = (() => {
        const dependents = remainingItems
            .map((candidate) => {
                const dependencies = items.filter(
                    (item) =>
                        !!item.project?.id &&
                        candidate.dependency?.requires?.some((reference) =>
                            dependencyRefMatchesContentItem(reference, item),
                        ),
                )
                return dependencies.length > 0 ? { item: candidate, dependencies } : null
            })
            .filter(
                (dependent): dependent is { item: ContentItem; dependencies: ContentItem[] } =>
                    dependent !== null,
            )
        if (dependents.length > 0) return { items, dependents }
        const dependencyItems = items.filter((item) =>
            item.dependency?.requiredBy?.some((reference) =>
                remainingItems.some((candidate) =>
                    dependencyRefMatchesContentItem(reference, candidate),
                ),
            ),
        )
        if (dependencyItems.length > 0) {
            return {
                items: dependencyItems,
                dependents: remainingItems
                    .map((candidate) => {
                        const dependencies = dependencyItems.filter((item) =>
                            candidate.dependency?.requires?.some((reference) =>
                                dependencyRefMatchesContentItem(reference, item),
                            ),
                        )
                        return dependencies.length > 0 ? { item: candidate, dependencies } : null
                    })
                    .filter(
                        (
                            dependent,
                        ): dependent is { item: ContentItem; dependencies: ContentItem[] } =>
                            dependent !== null,
                    ),
            }
        }
        return null
    })()

    if (persistedWarning?.dependents.length) return persistedWarning

    const versionIds = [
        ...new Set(
            remainingItems.map((item) => item.version?.id).filter((id): id is string => !!id),
        ),
    ]

    if (versionIds.length === 0) return null

    const versions = (await get_version_many(versionIds).catch((err) => {
        handleError(err as Error)
        return null
    })) as Labrinth.Versions.v2.Version[] | null

    if (!versions) return null

    const versionsById = new Map(versions.map((version) => [version.id, version]))

    const dependents = remainingItems
        .map((candidate) => {
            const version = candidate.version?.id ? versionsById.get(candidate.version.id) : null
            if (!version) return null

            const dependencies = items.filter((item) => {
                if (!item.project?.id && !item.version?.id) return false

                return version.dependencies?.some(
                    (dependency) =>
                        isBreakingDependency(dependency) && dependencyTargetsItem(dependency, item),
                )
            })

            return dependencies.length > 0 ? { item: candidate, dependencies } : null
        })
        .filter(
            (dependent): dependent is { item: ContentItem; dependencies: ContentItem[] } =>
                dependent !== null,
        )

    return dependents.length > 0 ? { items, dependents } : null
}

async function reviewResourceChanges(entryIds: string[], release: SourceReleaseRef | null = null) {
    const preview = resourceOperationPreview.value
    if (!preview) return false
    return reviewResourceEntryChanges(
        {
            instanceId: props.instance.id,
            instanceName: props.instance.name,
            entryIds,
            release,
            installDependencies: themeStore.getFeatureFlag('auto_install_dependencies'),
        },
        (input) => preview.show(input),
        (job) => downloadManager.trackJob(job),
    )
}

async function bulkUpdateAllProjects() {
    if (pendingContentChanges.value.length > 0 || activeContentChangeJobs.value.length > 0)
        return false
    localBulkOperating.value = true
    try {
        const result = await resourceCheckUpdates(props.instance.id)
        if (result.failures.length) handleError(new Error(result.failures.join('\n')))
        const ids = result.entries.filter((id) =>
            contentSnapshot.value?.items.some(
                (item) => item.entryId === id && item.ownershipKind === 'user_added',
            ),
        )
        return ids.length ? await reviewResourceChanges(ids) : false
    } catch (err) {
        handleError(err as Error)
        return false
    } finally {
        localBulkOperating.value = false
    }
}

async function bulkUpdateProjects(items: ContentItem[]) {
    const ids = items.map((item) => item.instanceEntryId).filter((id): id is string => !!id)
    if (!ids.length || items.some(hasGlobalContentChange)) return false
    try {
        return await reviewResourceChanges(ids)
    } catch (err) {
        handleError(err as Error)
        return false
    }
}

async function updateProject(mod: ContentItem) {
    if (!mod.instanceEntryId || hasGlobalContentChange(mod)) return false
    const operation = beginContentOperation(mod)
    if (!operation) return false
    try {
        return await reviewResourceChanges([mod.instanceEntryId])
    } catch (err) {
        handleError(err as Error)
        return false
    } finally {
        finishContentOperation(mod, operation)
    }
}

async function switchProjectVersion(mod: ContentItem, version: Labrinth.Versions.v2.Version) {
    if (!mod.instanceEntryId || hasGlobalContentChange(mod)) return false
    const operation = beginContentOperation(mod)
    if (!operation) return false
    const cfId = getCurseForgeProjectIdForItem(mod)
    const source =
        mod.origin_provider === 'curseforge' || version.project_id.startsWith('curseforge:')
            ? 'curseforge'
            : 'modrinth'
    const projectId = source === 'curseforge' ? String(cfId) : version.project_id
    try {
        return await reviewResourceChanges(
            [mod.instanceEntryId],
            updaterReleaseRefs.get(version.id) ?? {
                project: { source, projectId },
                releaseId: version.id,
            },
        )
    } catch (err) {
        handleError(err as Error)
        return false
    } finally {
        finishContentOperation(mod, operation)
    }
}

async function handleRollbackContent(mod: ContentItem) {
    if (!mod.file_path || !mod.rollback) return
    const operation = beginContentOperation(mod)
    if (!operation) return

    let changed = false
    try {
        const history = mod.instanceEntryId
            ? await resourceListHistory(props.instance.id, mod.instanceEntryId)
            : []
        if (history.length && mod.instanceEntryId) {
            const job = await resourceHistoryModal.value?.show(
                props.instance.id,
                mod.instanceEntryId,
            )
            if (!job) return
            downloadManager.trackJob(job)
        } else {
            await rollback_project(props.instance.id, mod.file_path)
            changed = true
        }
        trackEvent('InstanceProjectRollback', {
            loader: props.instance.loader,
            game_version: props.instance.game_version,
            id: mod.project?.id,
            name: mod.project?.title ?? mod.file_name,
            project_type: mod.project_type,
        })
    } catch (err) {
        handleError(err as Error)
    } finally {
        try {
            if (changed) await refreshContentState('bypass')
        } finally {
            finishContentOperation(mod, operation)
        }
    }
}

async function handleUpdate(item: ContentItem) {
    if (!item || !canUpdateProject(item)) return
    await updateProject(item)
}

async function handleSwitchVersion(item: ContentItem) {
    const exact = contentSnapshot.value?.items.find(
        (entry) => entry.entryId === item.instanceEntryId,
    )?.artifactRef
    const initialVersionId = exact?.release.releaseId ?? item.version?.id
    if (!initialVersionId || (!exact && !item.project?.id)) return

    const requestId = beginUpdateRequest()
    const itemId = getContentItemId(item)
    updaterReleaseRefs.clear()

    updatingModpack.value = false
    updatingProject.value = item
    updatingProjectVersions.value = []
    loadingVersions.value = true

    await nextTick()

    contentUpdaterModal.value?.show(initialVersionId, { switchMode: true })

    try {
        const versions = await getUpdaterProjectVersions(
            exact?.release.project.projectId ?? item.project!.id,
            initialVersionId,
            item,
        )
        if (isActiveUpdateRequest(requestId) && getContentItemId(updatingProject.value) === itemId)
            updatingProjectVersions.value = versions
    } catch (error) {
        if (isActiveUpdateRequest(requestId)) handleError(error as Error)
    } finally {
        if (isActiveUpdateRequest(requestId)) loadingVersions.value = false
    }
}

async function refreshContentState(cacheBehaviour?: CacheBehaviour) {
    await initProjects(cacheBehaviour)
}

watch(
    () => installRevisionByInstance.value.get(props.instance.id) ?? 0,
    async (revision) => {
        if (revision <= handledInstallRevision.value) return
        debugState('installRevision changed → refreshContentState', {
            instanceId: props.instance.id,
            revision,
            handledRevision: handledInstallRevision.value,
        })
        handledInstallRevision.value = revision
        await refreshContentState('bypass')
    },
)

async function handleModpackUpdate() {
    if (!props.instance?.link?.project_id) return

    const requestId = beginUpdateRequest()

    updatingModpack.value = true
    updatingProject.value = null
    updatingProjectVersions.value = []
    loadingVersions.value = true

    await nextTick()

    const initialVersionId =
        linkedModpackUpdateVersionId.value ?? props.instance?.link?.version_id ?? undefined
    const projectId =
        props.instance?.link?.type === 'server_project_modpack'
            ? (linkedModpackProject.value?.id ??
              props.instance.link.content_project_id ??
              props.instance.link.project_id ??
              '')
            : (props.instance?.link?.project_id ?? linkedModpackProject.value?.id ?? '')
    debug('handleModpackUpdate: resolved modpack updater project', {
        type: 'modpack',
        projectId,
        linkedModpackProjectId: linkedModpackProject.value?.id ?? null,
        updateProvider: contentSnapshot.value?.pack?.metadata?.update?.provider ?? null,
        link: props.instance.link,
    })
    debug('handleModpackUpdate: opening modpack updater modal', {
        type: 'modpack',
        initialVersionId,
        linkedModpackUpdateVersionId: linkedModpackUpdateVersionId.value,
        linkedModpackProject: linkedModpackProject.value,
        linkedModpackVersion: linkedModpackVersion.value,
        linkedModpackHasUpdate: linkedModpackHasUpdate.value,
        instance: {
            path: props.instance.id,
            name: props.instance.name,
            gameVersion: props.instance.game_version,
            loader: props.instance.loader,
            link: props.instance.link,
        },
        modalStateBeforeFetch: {
            updatingModpack: updatingModpack.value,
            updatingProjectId: updatingProject.value?.id,
            updatingProjectVersions: updatingProjectVersions.value.map((version) => ({
                id: version.id,
                versionNumber: version.version_number,
                gameVersions: version.game_versions,
                loaders: version.loaders,
                datePublished: version.date_published,
            })),
        },
    })
    contentUpdaterModal.value?.show(initialVersionId)

    const versions = await getUpdaterProjectVersions(projectId, initialVersionId)

    if (!isActiveUpdateRequest(requestId) || !updatingModpack.value) return

    loadingVersions.value = false

    if (versions.length === 0) return

    const preselectedVersion =
        versions.find((version) => version.id === initialVersionId) ?? versions[0] ?? null
    debug('handleModpackUpdate: resolved modpack updater preselection', {
        type: 'modpack',
        initialVersionId,
        foundInitialVersion: versions.some((version) => version.id === initialVersionId),
        preselectedVersion: preselectedVersion
            ? {
                  id: preselectedVersion.id,
                  versionNumber: preselectedVersion.version_number,
                  gameVersions: preselectedVersion.game_versions,
                  loaders: preselectedVersion.loaders,
                  datePublished: preselectedVersion.date_published,
              }
            : null,
        versionCount: versions.length,
        linkedModpackUpdateVersionId: linkedModpackUpdateVersionId.value,
        currentLinkedVersionId: props.instance.link.version_id,
    })

    updatingProjectVersions.value = versions
}

function updaterChangelogRef(version: Labrinth.Versions.v2.Version): SourceReleaseRef | null {
    const exact = updaterReleaseRefs.get(version.id)
    if (exact) return exact
    const item = updatingProject.value
    const link = updatingModpack.value ? props.instance.link : null
    const cfProject = item
        ? getCurseForgeProjectIdForItem(item)
        : link?.type === 'curseforge_modpack'
          ? parseCurseForgeProjectId(link.project_id)
          : null
    if (cfProject != null)
        return {
            project: { source: 'curseforge', projectId: String(cfProject) },
            releaseId: version.id,
        }
    const projectId = version.project_id || item?.project?.id || link?.project_id
    return projectId ? { project: { source: 'modrinth', projectId }, releaseId: version.id } : null
}

function resetUpdateState() {
    updaterReleaseRefs.clear()
    activeUpdateRequestId.value = 0
    updatingModpack.value = false
    updatingProject.value = null
    updatingProjectVersions.value = []
    loadingVersions.value = false
}

async function handleModpackUpdateRequest(selectedVersion: Labrinth.Versions.v2.Version) {
    pendingModpackUpdateVersion.value = selectedVersion

    const currentVersionId = props.instance?.link?.version_id
    const currentVersion = updatingProjectVersions.value.find((v) => v.id === currentVersionId)
    isModpackUpdateDowngrade.value = currentVersion
        ? new Date(selectedVersion.date_published) < new Date(currentVersion.date_published)
        : false
    const shouldShowWarning =
        isModpackUpdateDowngrade.value ||
        versionChangesGameVersion(selectedVersion, props.instance.game_version)

    if (skipNonEssentialWarnings.value || !shouldShowWarning) {
        await handleModpackUpdateConfirm()
        return
    }

    modpackUpdateConfirmModal.value?.show()
}

async function handleModpackUpdateConfirm() {
    if (!pendingModpackUpdateVersion.value || !props.instance?.id) return

    const version = pendingModpackUpdateVersion.value
    pendingModpackUpdateVersion.value = null

    isModpackUpdating.value = true
    contentUpdateSubmitting.value = true
    let succeeded = false
    try {
        const plan = await withContentOperationTimeout(
            plan_content_updates(props.instance.id, 'pack', version.id),
        )
        await withContentOperationTimeout(apply_content_update_plan(plan.id))
        await initProjects()
        succeeded = true
    } catch (error) {
        handleError(error as Error)
    } finally {
        isModpackUpdating.value = false
        contentUpdateSubmitting.value = false
        if (succeeded) {
            contentUpdaterModal.value?.hide()
            resetUpdateState()
        }
    }
}

function handleModpackUpdateCancel() {
    pendingModpackUpdateVersion.value = null
}

async function handleModalUpdate(
    selectedVersion: Labrinth.Versions.v2.Version,
    event?: MouseEvent,
) {
    contentUpdateSubmitting.value = true
    let succeeded = false
    if (updatingModpack.value) {
        try {
            if (event?.shiftKey) {
                pendingModpackUpdateVersion.value = selectedVersion
                await handleModpackUpdateConfirm()
            } else {
                await handleModpackUpdateRequest(selectedVersion)
            }
        } finally {
            contentUpdateSubmitting.value = false
        }
    } else if (updatingProject.value) {
        const mod = updatingProject.value

        try {
            if (contentUpdateId(mod) === selectedVersion.id) {
                succeeded = await updateProject(mod)
            } else {
                succeeded = await switchProjectVersion(mod, selectedVersion)
            }
        } catch {
            // The operation function has already reported the error; keep the
            // selected version available for retry.
        } finally {
            contentUpdateSubmitting.value = false
        }
    }
    if (succeeded) {
        contentUpdaterModal.value?.hide()
        resetUpdateState()
    }
}

async function unpairInstance() {
    await edit(props.instance.id, {
        link: null as unknown as undefined,
    })
    linkedModpackProject.value = null
    linkedModpackVersion.value = null
    linkedModpackOwner.value = null
    linkedModpackHasUpdate.value = false
    linkedModpackUpdateVersionId.value = null
    localImportedModpackUnlinked.value = true
    linkedModpackContentItems.value = []
    await initProjects('bypass')
}

async function handleShareItems(
    items: ContentItem[],
    format: 'names' | 'file-names' | 'urls' | 'markdown',
) {
    const source = items.length > 0 ? items : projects.value
    let text: string
    switch (format) {
        case 'names':
            text = source.map((x) => x.project?.title ?? x.file_name).join('\n')
            break
        case 'file-names':
            text = source.map((x) => x.file_name).join('\n')
            break
        case 'urls':
            text = source
                .flatMap((item) => {
                    const url = contentProjectUrl(item)
                    return url ? [url] : []
                })
                .join('\n')
            break
        case 'markdown':
            text = source
                .map((x) => {
                    const name = x.project?.title ?? x.file_name
                    const url = contentProjectUrl(x)
                    if (url) return `[${name}](${url})`
                    return name
                })
                .join('\n')
            break
    }
    await shareModal.value?.show(text)
}

async function updateDuplicateGroup(item: ContentItem) {
    const all = [...mergedProjects.value, ...displayedLinkedModpackContentItems.value]
    const key = confirmedContentGroupKey(item)
    const copies = all.filter((candidate) => confirmedContentGroupKey(candidate) === key)
    if (!key || copies.some(hasGlobalContentChange)) return
    try {
        await reviewResourceChanges(duplicateGroupUpdateEntries(all, item))
    } catch (error) {
        handleError(error as Error)
    }
}

function getOverflowOptions(item: ContentItem): OverflowMenuOption[] {
    const options: OverflowMenuOption[] = []

    if (item.instanceMaterializationState === 'present' && item.file_path) {
        options.push({
            id: formatMessage(commonMessages.showFileButton),
            icon: FolderOpenIcon,
            action: () => highlightModInInstance(props.instance.id, item.file_path),
        })
    }

    if (item.instanceCapabilities?.canRestorePackDefault && item.instanceMemberId) {
        options.push({
            id: formatMessage(messages.restorePackDefault),
            icon: UndoIcon,
            action: () => restorePackDefault(item),
        })
    }

    if (item.pendingManualDownload) {
        const reference = item.provider_refs.find(
            (ref): ref is { provider: 'curseforge'; project_id: number; file_id: number | null } =>
                ref.provider === 'curseforge',
        )
        const manual = reference
            ? skippedManualDownloads.value.find(
                  (candidate) =>
                      candidate.projectId === reference.project_id &&
                      candidate.fileId === reference.file_id,
              )
            : undefined
        if (manual) {
            options.push({
                id: formatMessage(commonMessages.downloadButton),
                icon: ExternalIcon,
                action: () => openManualCurseForgeDownload(manual),
            })
        }
    }

    const projectUrl = contentProjectUrl(item)
    if (projectUrl) {
        options.push({
            id: formatMessage(commonMessages.copyLinkButton),
            icon: ClipboardCopyIcon,
            action: async () => {
                await navigator.clipboard.writeText(projectUrl)
            },
        })
    }

    const mcmodUrl = getMcmodUrl(item)
    if (mcmodUrl) {
        options.push({
            id: formatMessage(messages.openInMcmod),
            icon: BookOpenIcon,
            action: () => void openUrl(mcmodUrl),
        })
    }

    return options
}

function openSchematicInWorkshop(item: ContentItem) {
    void router.push({
        name: 'SchematicWorkshop',
        query: { instance: props.instance.id, path: item.file_path ?? item.file_name },
    })
}

async function initProjects(cacheBehaviour?: CacheBehaviour): Promise<void> {
    if (cacheBehaviour === 'bypass') {
        if (activeBypassRefresh) return activeBypassRefresh
        const refresh = initProjectsInternal(cacheBehaviour)
        const wrappedRefresh = refresh.finally(() => {
            if (activeBypassRefresh === wrappedRefresh) activeBypassRefresh = null
            suppressSyncedUntil = Date.now() + 1000
        })
        activeBypassRefresh = wrappedRefresh
        return wrappedRefresh
    }
    return initProjectsInternal(cacheBehaviour)
}

async function initProjectsInternal(cacheBehaviour?: CacheBehaviour) {
    void loadWorldDatapacks()
    if (!props.instance || props.instance.install_stage !== 'installed') {
        invalidateContentRequests(true)
        return
    }

    const instanceId = props.instance.id
    contentEnrichmentController?.abort()
    contentEnrichmentController = new AbortController()
    const generation = ++contentRequestGeneration
    visibleMetadataLoadingPaths.value.clear()
    contentUpdateLoadingPaths.value.clear()
    visibleMetadataRequestedPaths.clear()
    visibleMetadataPendingPaths.clear()
    debugState('initProjects start', { instanceId, cacheBehaviour, generation })
    let contentData: InstanceContentData | null
    try {
        contentData = await loadInstanceContentInStages(instanceId, cacheBehaviour, (local) => {
            if (!isCurrentContentRequest(instanceId, generation)) return false
            if (projects.value.length || linkedModpackContentItems.value.length) return true
            if (
                !local.contentItems.length &&
                !local.linkedContentItems.length &&
                (props.instance.linked_dot_minecraft || props.instance.symlink_target)
            )
                return true
            return applyContentData(local)
        })
    } catch (error) {
        if (isCurrentContentRequest(instanceId, generation)) {
            handleError(error as Error)
            if (!projects.value.length && !linkedModpackContentItems.value.length)
                contentLoadError.value = error instanceof Error ? error : new Error(String(error))
            loading.value = false
        }
        return
    }
    if (!contentData) {
        if (isCurrentContentRequest(instanceId, generation)) loading.value = false
        return
    }
    if (!isCurrentContentRequest(instanceId, generation)) return
    contentLoadError.value = null
    try {
        if (applyContentData(contentData)) enrichContentData(contentData, generation)
    } catch (error) {
        if (isCurrentContentRequest(instanceId, generation)) handleError(error as Error)
    } finally {
        if (isCurrentContentRequest(instanceId, generation)) loading.value = false
    }
    if (cacheBehaviour !== 'bypass' && beginLegacyCurseForgeReconciliation(contentData)) {
        await initProjects('bypass')
    }
}

function enrichContentData(data: InstanceContentData, generation: number) {
    const controller = (contentEnrichmentController ??= new AbortController())
    const isCurrent = () =>
        !controller.signal.aborted && isCurrentContentRequest(data.path, generation)
    const originals = [...data.contentItems, ...data.linkedContentItems]
    void translateContentItemTitles(originals, i18n.global.locale.value)
        .then((translated) => {
            if (!isCurrent()) return
            const byId = new Map(
                translated.map((item, index) => [item.id, { item, original: originals[index] }]),
            )
            const merge = (items: ContentItem[]) =>
                items.map((item) => {
                    const translated = byId.get(item.id)
                    if (
                        !translated?.item.project ||
                        hasContentOperation(item) ||
                        item.project?.title !== translated.original.project?.title
                    )
                        return item
                    return {
                        ...item,
                        project: { ...item.project, title: translated.item.project.title },
                    }
                })
            projects.value = merge(projects.value)
            linkedModpackContentItems.value = merge(linkedModpackContentItems.value)
        })
        .catch((error) => {
            if (isCurrent()) handleError(error as Error)
        })
}

function beginLegacyCurseForgeReconciliation(contentData: InstanceContentData) {
    if (
        props.instance.link?.type !== 'curseforge_modpack' ||
        curseForgeReconciliationAttempts.has(props.instance.id) ||
        !contentData.snapshot.items.some((item) => item.ownershipKind === 'local_discovered')
    ) {
        return false
    }
    curseForgeReconciliationAttempts.add(props.instance.id)
    return true
}

function applyContentData(contentData: InstanceContentData) {
    if (props.instance.install_stage !== 'installed' || contentData.path !== props.instance.id) {
        debugState('applyContentData path mismatch', {
            expected: props.instance.id,
            got: contentData.path,
        })
        return false
    }
    if (contentSnapshot.value && contentData.snapshot.revision < contentSnapshot.value.revision) {
        debugState('applyContentData stale revision', {
            current: contentSnapshot.value.revision,
            incoming: contentData.snapshot.revision,
        })
        return false
    }

    debugState('applyContentData set projects', {
        instanceId: props.instance.id,
        count: contentData.contentItems.length,
        paths: contentData.contentItems.map((c) => c.file_name).slice(0, 20),
    })
    // A visible batch can finish between the local and cached snapshots.
    const visibleUpdates = [...contentItemsByEntryId.value.values()].filter(
        (item) =>
            item.resourceUpdate &&
            item.file_path &&
            visibleMetadataRequestedPaths.has(item.file_path) &&
            !contentUpdateLoadingPaths.value.has(item.file_path),
    )
    contentSnapshot.value = contentData.snapshot
    const previousItems = new Map(
        [...projects.value, ...linkedModpackContentItems.value]
            .filter((item) => item.instanceEntryId)
            .map((item) => [item.instanceEntryId, item]),
    )
    const preserveUpdateState = (item: ContentItem) => {
        const previous = item.instanceEntryId ? previousItems.get(item.instanceEntryId) : undefined
        if (
            !previous ||
            previous.file_path !== item.file_path ||
            previous.version?.id !== item.version?.id
        )
            return item
        return hasContentOperation(previous) ? item : preserveContentUpdateState(previous, item)
    }
    let contentItems = contentData.contentItems
        .filter((item) => !isWorldSaveContentItem(item))
        .map(preserveUpdateState)
    let linkedContentItems = contentData.linkedContentItems
        .filter((item) => !isWorldSaveContentItem(item))
        .map(preserveUpdateState)
    if (!contentData.modpack && !isPackLocked.value && linkedContentItems.length > 0) {
        contentItems = [...contentItems, ...linkedContentItems]
        linkedContentItems = []
    }
    projects.value = contentItems
    linkedModpackContentItems.value = linkedContentItems
    for (const item of visibleUpdates) mergeVisibleUpdate(item)
    if (updatingProject.value) {
        const entryId = updatingProject.value.instanceEntryId
        const refreshed = [...contentItems, ...linkedContentItems].find((item) =>
            entryId
                ? item.instanceEntryId === entryId
                : item.file_path === updatingProject.value?.file_path,
        )
        if (refreshed) updatingProject.value = refreshed
        else {
            contentUpdaterModal.value?.hide()
            resetUpdateState()
        }
    }
    dependencyGraphModal.value?.setItems(allDependencyGraphItems())

    if (contentData.modpack) {
        linkedModpackProject.value = contentData.modpack.project
        linkedModpackVersion.value = contentData.modpack.version
        linkedModpackOwner.value = contentData.modpack.owner
        linkedModpackCategories.value = []
        linkedModpackHasUpdate.value = contentData.modpack.hasUpdate
        linkedModpackUpdateVersionId.value = contentData.modpack.updateVersionId
    } else {
        linkedModpackProject.value = null
        linkedModpackVersion.value = null
        linkedModpackOwner.value = null
        linkedModpackCategories.value = []
        linkedModpackHasUpdate.value = false
        linkedModpackUpdateVersionId.value = null
    }

    loading.value = false

    const hasContent = contentItems.length > 0 || linkedContentItems.length > 0
    if (hasContent) {
        writeInstanceCache(props.instance.id, {
            contentItems,
            linkedContentItems,
            modpack: contentData.modpack
                ? {
                      project: contentData.modpack.project,
                      version: contentData.modpack.version,
                      owner: contentData.modpack.owner,
                      categories: [],
                      hasUpdate: contentData.modpack.hasUpdate,
                      updateVersionId: contentData.modpack.updateVersionId,
                  }
                : null,
        })
    }

    return true
}

provideAppBackup({
    async createBackup() {
        const allInstances = await list()
        const prefix = `${props.instance.name} - Backup #`
        const existingNums = allInstances
            .filter((p) => p.name.startsWith(prefix))
            .map((p) => parseInt(p.name.slice(prefix.length), 10))
            .filter((n) => !isNaN(n))
        const nextNum = existingNums.length > 0 ? Math.max(...existingNums) + 1 : 1
        const job = await install_duplicate_instance(props.instance.id)
        const newInstanceId = installJobInstanceId(job)
        if (newInstanceId) {
            await edit(newInstanceId, { name: `${prefix}${nextNum}` })
        }
    },
})

const cachedHint = readInstanceCache(props.instance.id)
const showContentHint = ref(cachedHint?.modpackHintDismissed !== true)
const instanceContentProjectQuery = computed(() => ({
    i: props.instance.id,
    from: 'instance-content',
}))
function dismissContentHint() {
    showContentHint.value = false
    writeInstanceCache(props.instance.id, { modpackHintDismissed: true })
}

provideContentManager({
    items: mergedProjects,
    duplicateItems: duplicateModItems,
    updateDuplicateGroup,
    loading,
    error: contentLoadError,
    filterOptionsReady: computed(
        () =>
            !loading.value &&
            mergedProjects.value.length + displayedLinkedModpackContentItems.value.length > 0,
    ),
    modpackItems: displayedLinkedModpackContentItems,
    modpack: computed(() => {
        if (linkedModpackProject.value) {
            const instanceLink = props.instance.link
            const projectPath =
                instanceLink?.type === 'curseforge_modpack'
                    ? `/project/curseforge/${instanceLink.project_id}`
                    : `/project/${linkedModpackProject.value.slug ?? linkedModpackProject.value.id}`

            return {
                project: displayedModpackProject.value ?? linkedModpackProject.value,
                projectLink: {
                    path: projectPath,
                    query: instanceContentProjectQuery.value,
                },
                version: linkedModpackVersion.value ?? undefined,
                versionLink:
                    instanceLink?.type !== 'curseforge_modpack' && linkedModpackVersion.value
                        ? {
                              path: `${projectPath}/version/${linkedModpackVersion.value.id}`,
                              query: instanceContentProjectQuery.value,
                          }
                        : undefined,
                owner: linkedModpackOwner.value
                    ? {
                          ...linkedModpackOwner.value,
                          link: () =>
                              openUrl(
                                  `https://modrinth.com/${linkedModpackOwner.value!.type}/${linkedModpackOwner.value!.id}`,
                              ),
                      }
                    : undefined,
                categories: linkedModpackCategories.value,
                hasUpdate: linkedModpackHasUpdate.value,
                disabled: isModpackUpdating.value,
                disabledText: isModpackUpdating.value
                    ? formatMessage(commonMessages.updatingLabel)
                    : formatMessage(commonMessages.installingLabel),
            }
        }

        if (curseForgeModpackFallbackProject.value) {
            return {
                project: curseForgeModpackFallbackProject.value,
                projectLink: {
                    path: `/project/curseforge/${props.instance.link?.project_id}`,
                    query: instanceContentProjectQuery.value,
                },
                categories: [],
                hasUpdate: false,
                disabled: isModpackUpdating.value,
                disabledText: isModpackUpdating.value
                    ? formatMessage(commonMessages.updatingLabel)
                    : formatMessage(commonMessages.installingLabel),
            }
        }

        if (localImportedModpackProject.value) {
            return {
                project: localImportedModpackProject.value,
                categories: [],
                hasUpdate: false,
                disabled: isModpackUpdating.value,
                disabledText: isModpackUpdating.value
                    ? formatMessage(commonMessages.updatingLabel)
                    : formatMessage(commonMessages.installingLabel),
            }
        }

        return null
    }),
    isBusy: isInstanceBusy,
    isBulkOperating,
    skipNonEssentialWarnings,
    contentTypeLabel: ref(formatMessage(messages.contentTypeProject)),
    toggleEnabled: toggleDisableDebounced,
    bulkEnableItems: async (items: ContentItem[]) => {
        return await toggleDisableBatch(
            items.filter(
                (item) => item.enabled === false && item.instanceCapabilities?.canToggle !== false,
            ),
            true,
        )
    },
    bulkDisableItems: async (items: ContentItem[]) => {
        return await toggleDisableBatch(
            items.filter(
                (item) => item.enabled === true && item.instanceCapabilities?.canToggle !== false,
            ),
            false,
        )
    },
    deleteItem: removeMod,
    bulkDeleteItems: async (items: ContentItem[]) => {
        for (const item of items) {
            await removeMod(item)
        }
    },
    getDeleteDependencyWarning,
    refresh: () => refreshContentState('bypass'),
    browse: handleBrowseContent,
    uploadFiles: handleUploadFiles,
    hasUpdateSupport: true,
    updateItem: handleUpdate,
    rollbackItem: handleRollbackContent,
    bulkUpdateAll: bulkUpdateAllProjects,
    bulkUpdateAllLabel: formatMessage(messages.updateAddedContent),
    bulkUpdateAllDescription: formatMessage(messages.updateAddedContentDescription),
    bulkUpdateIncludesModpack: false,
    bulkUpdateItems: bulkUpdateProjects,
    updateModpack: props.isServerInstance ? undefined : handleModpackUpdate,
    viewDependencies: handleViewDependencies,
    unlinkModpack: unpairInstance,

    openSettings: props.openSettings,
    switchVersion: handleSwitchVersion,
    getOverflowOptions,
    showContentHint,
    dismissContentHint,
    symlinkTarget: computed(() => props.instance.symlink_target),
    shareItems: handleShareItems,
    getItemId: getContentItemId,
    instanceId: props.instance.id,
    mapToTableItem: (item: ContentItem) => {
        const postUpgradeWarning = postUpgradeWarningForContent(
            postUpgradeNotice.value?.warnings ?? [],
            item.instanceEntryId,
            item.file_path,
        )
        const effectiveProvider = item.origin_provider ?? item.provider_refs?.[0]?.provider ?? null

        const curseForgeProjectId =
            effectiveProvider === 'curseforge'
                ? item.provider_refs
                      ?.find(
                          (ref): ref is { provider: 'curseforge'; project_id: number } =>
                              ref.provider === 'curseforge' && typeof ref.project_id === 'number',
                      )
                      ?.project_id.toString()
                : undefined

        const projectLink =
            item.instanceResourceId &&
            ['mcarchive', 'planet_minecraft'].includes(effectiveProvider ?? '')
                ? {
                      path: `/resource/${encodeURIComponent(item.instanceResourceId)}`,
                      query: instanceContentProjectQuery.value,
                  }
                : effectiveProvider && item.project?.id && !item.project.id.startsWith('local:')
                  ? {
                        path:
                            effectiveProvider === 'curseforge' && curseForgeProjectId
                                ? `/project/curseforge/${curseForgeProjectId}`
                                : effectiveProvider === 'curseforge'
                                  ? `/project/curseforge/${item.project.id}`
                                  : `/project/${item.project.id}`,
                        query: instanceContentProjectQuery.value,
                    }
                  : undefined

        const versionLink =
            effectiveProvider === 'modrinth' &&
            item.project?.id &&
            !item.project.id.startsWith('local:') &&
            item.version?.id
                ? {
                      path: `/project/${item.project.id}/version/${item.version.id}`,
                      query: instanceContentProjectQuery.value,
                  }
                : undefined

        const ownerLink = item.owner
            ? {
                  ...item.owner,
                  link:
                      effectiveProvider !== 'modrinth' || item.owner.id.startsWith('local:')
                          ? undefined
                          : () =>
                                openUrl(
                                    `https://modrinth.com/${item.owner!.type}/${item.owner!.id}`,
                                ),
              }
            : undefined

        const dependency = item.dependency
        const dependencyBadge =
            dependency && (dependency.autoDependency || dependency.requiredBy.length > 0)
                ? {
                      autoDependency: dependency.autoDependency,
                      orphaned: dependency.orphaned,
                  }
                : null

        return {
            id: getContentItemId(item),
            localPath: item.file_path ?? item.file_name,
            installationSource:
                item.origin_provider === 'modrinth'
                    ? 'Modrinth'
                    : item.origin_provider === 'curseforge'
                      ? 'CurseForge'
                      : item.origin_provider === 'mcarchive'
                        ? 'MCArchive'
                        : item.origin_provider === 'planet_minecraft'
                          ? 'Planet Minecraft'
                          : formatMessage(commonMessages.unknownLabel),
            duplicateCount: (() => {
                const key = duplicateModKey(item)
                return key ? duplicateModCounts.value.get(key) : undefined
            })(),
            project: item.project ?? {
                id: item.file_name,
                slug: null,
                title: item.file_name.replace('.disabled', ''),
                icon_url: null,
            },
            projectLink,
            version: item.version ?? {
                id: item.file_name,
                version_number: formatMessage(commonMessages.unknownLabel),
                file_name: item.file_name,
            },
            versionLink,
            owner: ownerLink,
            postUpgradeWarningTooltip: postUpgradeWarning
                ? formatMessage(messages.postUpgradeWarningTooltip)
                : null,
            compatibilityWarningTooltip:
                item.compatibility &&
                (item.compatibility.gameVersion !== 'compatible' ||
                    item.compatibility.loader !== 'compatible')
                    ? formatMessage(messages.compatibilityWarning, {
                          game: item.compatibility.gameVersion,
                          loader: item.compatibility.loader,
                      })
                    : null,
            enabled: item.enabled,
            disabledTooltip:
                item.instanceMaterializationState === 'missing'
                    ? formatMessage(messages.packMemberMissing)
                    : item.instanceMaterializationState === 'removed'
                      ? formatMessage(messages.packMemberRemoved)
                      : null,
            toggleDisabled: item.instanceCapabilities?.canToggle === false,
            hideSwitchVersion:
                (isPackLocked.value && item.instanceOwnershipKind === 'pack_managed') ||
                item.instanceCapabilities?.canChangeVersion === false,
            pendingManualDownload: item.pendingManualDownload,
            installing: item.installing,
            dependencyBadge,
            inlineActions:
                item.project_type === 'schematic'
                    ? [
                          {
                              id: 'open-in-schematic-workshop',
                              label: formatMessage(messages.openInSchematicWorkshop),
                              icon: PencilIcon,
                              action: () => openSchematicInWorkshop(item),
                          },
                      ]
                    : undefined,
        }
    },
})

type UnlistenFn = () => void

void loadInitialContent()

function getInstallRevision() {
    return installRevisionByInstance.value.get(props.instance.id) ?? 0
}

async function loadInitialContent(): Promise<void> {
    if (props.instance.install_stage !== 'installed') {
        invalidateContentRequests(true)
        return
    }
    const installRevision = getInstallRevision()
    const cached = readInstanceCache(props.instance.id)

    // 优先使用缓存快速渲染，后台再刷新
    const hasCachedContent =
        (cached?.contentItems?.length ?? 0) > 0 || (cached?.linkedContentItems?.length ?? 0) > 0
    if (hasCachedContent) {
        debugState('loadInitialContent: restoring from localStorage cache', {
            instanceId: props.instance.id,
            contentItems: cached.contentItems.length,
            linkedItems: cached.linkedContentItems.length,
        })
        let cachedContentItems = cached.contentItems.filter((item) => !isWorldSaveContentItem(item))
        let cachedLinkedItems = cached.linkedContentItems.filter(
            (item) => !isWorldSaveContentItem(item),
        )
        if (!cached.modpack && !isPackLocked.value && cachedLinkedItems.length > 0) {
            cachedContentItems = [...cachedContentItems, ...cachedLinkedItems]
            cachedLinkedItems = []
        }
        projects.value = cachedContentItems
        linkedModpackContentItems.value = cachedLinkedItems
        if (cached.modpack) {
            linkedModpackProject.value = cached.modpack.project
            linkedModpackVersion.value = cached.modpack.version
            linkedModpackOwner.value = cached.modpack.owner
            linkedModpackCategories.value = cached.modpack.categories
            linkedModpackHasUpdate.value = cached.modpack.hasUpdate
            linkedModpackUpdateVersionId.value = cached.modpack.updateVersionId
        }
        loading.value = false
    }

    if (installRevision > handledInstallRevision.value) {
        handledInstallRevision.value = installRevision
        await initProjects()
        return
    }

    if (props.preloadedContent && applyContentData(props.preloadedContent)) {
        enrichContentData(props.preloadedContent, ++contentRequestGeneration)
        if (beginLegacyCurseForgeReconciliation(props.preloadedContent)) {
            await initProjects('bypass')
        }
        return
    }

    if (hasCachedContent) {
        initProjects().catch(handleError)
        return
    }

    await initProjects()
}

let unlistenInstances: UnlistenFn | null = null
const CONTENT_EVENT_DEBOUNCE_MS = 180
let contentEventTimer: ReturnType<typeof setTimeout> | null = null
let pendingExternalContentSync = false
let pendingContentChangedRevision: number | null = null
let pendingContentRefreshAfterBulk = false
let suppressSyncedUntil = 0

function scheduleContentEventRefresh(event: { event: string; revision?: number }) {
    if (event.event === 'synced') {
        if (Date.now() < suppressSyncedUntil) return
        pendingExternalContentSync = true
    } else if (event.event === 'content_changed') {
        if (
            event.revision != null &&
            contentSnapshot.value &&
            event.revision <= contentSnapshot.value.revision
        ) {
            return
        }
        pendingContentChangedRevision = Math.max(
            pendingContentChangedRevision ?? 0,
            event.revision ?? 0,
        )
    } else {
        return
    }

    if (contentEventTimer) return
    contentEventTimer = setTimeout(() => {
        contentEventTimer = null
        const refreshFilesystem =
            pendingExternalContentSync && pendingContentChangedRevision == null
        pendingExternalContentSync = false
        pendingContentChangedRevision = null
        if (isBulkOperating.value) {
            pendingContentRefreshAfterBulk = true
            return
        }
        void initProjects(refreshFilesystem ? 'bypass' : undefined)
    }, CONTENT_EVENT_DEBOUNCE_MS)
}

onMounted(() => {
    void instance_listener((event: { event: string; instance_id: string; revision?: number }) => {
        if (event.instance_id === props.instance.id && event.event === 'removed') {
            invalidateContentRequests(true)
            return
        }
        if (
            props.instance &&
            event.instance_id === props.instance.id &&
            (event.event === 'synced' || event.event === 'content_changed') &&
            props.instance.install_stage === 'installed'
        ) {
            scheduleContentEventRefresh(event)
        }
    })
        .then((unlisten) => {
            if (isUnmounted) {
                unlisten()
                return
            }

            unlistenInstances = unlisten
        })
        .catch(handleError)
})

watch(
    () => props.instance?.install_stage,
    async (newStage, oldStage) => {
        if (oldStage !== 'installed' && newStage === 'installed') {
            await refreshContentState('bypass')
        } else if (newStage !== 'installed') {
            invalidateContentRequests(true)
        }
    },
)

watch(
    () => props.instance.id,
    async (newId, oldId) => {
        if (newId === oldId) return
        invalidateContentRequests(true)
        if (props.instance.install_stage === 'installed') {
            await refreshContentState('bypass')
        }
    },
)

watch(
    () => props.instance?.link,
    async (newInstanceLink, oldInstanceLink) => {
        if (oldInstanceLink && !newInstanceLink) {
            await initProjects('bypass')
        }
    },
)

watch(
    () => props.instance?.update_channel,
    async (newValue, oldValue) => {
        if (newValue !== oldValue) {
            await initProjects('bypass')
        }
    },
)

onUnmounted(() => {
    isUnmounted = true
    if (contentEventTimer) clearTimeout(contentEventTimer)
    invalidateContentRequests()
    unlistenInstances?.()
})
</script>

<style>
.fade-enter-active,
.fade-leave-active {
    transition: opacity 0.2s ease-in-out;
}

.fade-enter-from,
.fade-leave-to {
    opacity: 0;
}
</style>

<style scoped>
.mods-admonition-body {
    border-color: color-mix(in srgb, var(--color-orange) var(--opacity-ratio-keep-60), transparent);
}

.mods-missing-row:hover {
    background-color: color-mix(
        in srgb,
        var(--surface-5) var(--opacity-ratio-keep-40),
        transparent
    );
}
</style>
