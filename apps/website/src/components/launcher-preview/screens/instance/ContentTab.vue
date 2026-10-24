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
            <div class="border-0 border-t border-solid border-divider bg-bg-orange p-4">
                <p class="m-0">{{ formatMessage(messages.postUpgradeNoticeBody) }}</p>
                <div class="mt-3 flex justify-end">
                    <Button
                        type="colored"
                        color="orange"
                        size="2xs"
                        @click="notices.dismiss(instance.id)"
                        >{{ formatMessage(messages.ignoreAllPostUpgradeWarnings) }}</Button
                    >
                </div>
            </div>
        </CollapsibleAdmonition>
        <CollapsibleAdmonition
            v-if="metadataUnavailable || contentLoadError"
            v-model="contentWarningExpanded"
            type="warning"
            class="mb-4"
        >
            <template #header>{{
                formatMessage(
                    contentLoadError
                        ? messages.contentHistoryWarningTitle
                        : messages.contentRefreshWarningTitle,
                )
            }}</template>
            <div class="border-0 border-t border-solid border-divider bg-bg-orange p-4">
                <p class="m-0">
                    {{
                        formatMessage(
                            contentLoadError
                                ? messages.contentHistoryWarningBody
                                : messages.contentRefreshWarningBody,
                        )
                    }}
                </p>
                <div class="mt-3 flex justify-end">
                    <Button
                        type="outlined"
                        size="2xs"
                        :disabled="refreshingWarnings"
                        @click="retryContentWarnings"
                        >{{ formatMessage(commonMessages.retryButton) }}</Button
                    >
                </div>
            </div>
        </CollapsibleAdmonition>
        <ContentPageLayout>
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
                    :instance-icon-url="instanceIconUrl"
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
                        [linkedModpack?.project.title, pendingModpackUpdateVersion?.version_number]
                            .filter(Boolean)
                            .join(' ')
                    "
                    :symlink-target="props.instance.symlink_target"
                    @confirm="handleModpackUpdateConfirm"
                    @cancel="handleModpackUpdateCancel"
                />
                <ResourceOperationPreviewModal ref="resourceOperationPreview" />
                <ResourceBackupsModal ref="resourceBackupsModal" @cleaned="refreshContentState" />
                <ContentUpdaterModal
                    v-if="updatingProject || updatingModpack"
                    ref="contentUpdaterModal"
                    :versions="updatingProjectVersions"
                    :current-game-version="instance.game_version"
                    :current-loader="instance.loader"
                    :current-version-id="
                        updatingModpack
                            ? (linkVersionId ?? '')
                            : (updatingProject?.version?.id ?? '')
                    "
                    :is-app="true"
                    :project-type="updatingModpack ? 'modpack' : updatingProject?.project_type"
                    :project-icon-url="
                        updatingModpack
                            ? linkedModpack?.project.icon_url
                            : updatingProject?.project?.icon_url
                    "
                    :project-name="
                        updatingModpack
                            ? (linkedModpack?.project.title ??
                              formatMessage(commonMessages.modpackLabel))
                            : (updatingProject?.project?.title ?? updatingProject?.file_name)
                    "
                    :loading="loadingVersions"
                    :action-loading="contentUpdateSubmitting"
                    :hide-on-update="false"
                    @update="handleModalUpdate"
                    @cancel="resetUpdateState"
                />
            </template>
        </ContentPageLayout>
        <input
            ref="uploadInput"
            type="file"
            multiple
            accept=".jar,.zip"
            class="hidden"
            @change="handleUploadInput"
        />
    </ReadyTransition>
</template>

<script setup lang="ts">
import BookOpenIcon from '@modrinth/assets/icons/book-open.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import UndoIcon from '@modrinth/assets/icons/undo.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import CollapsibleAdmonition from '@modrinth/ui/src/components/base/CollapsibleAdmonition.vue'
import LoadingIndicator from '@modrinth/ui/src/components/base/LoadingIndicator.vue'
import type { Option as OverflowMenuOption } from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import ReadyTransition from '@modrinth/ui/src/components/base/ReadyTransition.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { confirmedContentGroupKey } from '@modrinth/ui/src/layouts/shared/content-tab/composables/content-duplicates.ts'
import { provideContentManager } from '@modrinth/ui/src/layouts/shared/content-tab/providers/content-manager.ts'
import type {
    ContentItem,
    ContentModpackCardCategory,
    ContentModpackCardProject,
    ContentModpackCardVersion,
    ContentOwner,
} from '@modrinth/ui/src/layouts/shared/content-tab/types.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { versionChangesGameVersion } from '@modrinth/ui/src/utils/version-compatibility.ts'
import { computed, nextTick, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { bilingualTitle, mcmodUrl, useContentEntries } from '../../lib/content-search'
import { getDisplayInstanceIcon } from '../../lib/instance-icons'
import { instanceContentProject } from '../../lib/instance-project'
import { openUrl, revealInFolder } from '../../lib/platform/opener'
import { instanceUpgradeNotices, matchesPostUpgradeWarning } from '../../lib/post-upgrade-notice'
import { usePreview } from '../../lib/state'
import type { GameInstance } from '../../lib/types'
import PreviewShareModal from '../../overlays/PreviewShareModal.vue'
import ConfirmModpackUpdateModal from '../../shared/content-tab/components/modals/ConfirmModpackUpdateModal.vue'
import ConfirmUnlinkModal from '../../shared/content-tab/components/modals/ConfirmUnlinkModal.vue'
import ContentUpdaterModal, {
    type UpdaterVersion,
} from '../../shared/content-tab/components/modals/ContentUpdaterModal.vue'
import ContentPageLayout from '../../shared/content-tab/layout.vue'
import ResourceOperationPreviewModal from '../discover/components/ResourceOperationPreviewModal.vue'
import { contentTypeOf, createContentInstall, provideContentInstall } from '../discover/install'
import { type ContentSelectionItem, makeContentSelectionKey, useDiscover } from '../discover/state'
import { classifyContentFile } from './content/classify-file'
import ContentToggleDependenciesModal from './content/ContentToggleDependenciesModal.vue'
import DependencyGraphModal from './content/DependencyGraphModal.vue'
import ResourceBackupsModal from './content/ResourceBackupsModal.vue'
import { modsMessages as messages } from './messages'
import {
    applyPackVersion,
    checkModpackUpdate,
    loadModpackProject,
    projectIconUrl,
    runPackInstall,
    toUpdaterVersion,
} from './settings/installation/modpack'
import { projectOwner, useInstanceContentItems } from './state'

/**
 * Port of `apps/app-frontend/src/pages/instance/Mods.vue` on the shared content
 * layout. The instance's files and their project metadata come from the
 * preview's content domain (`./state`); updates and version switches go through
 * the same "Review content changes" modal and download queue as Browse, and a
 * linked modpack's version switch runs the pack install stages of Settings >
 * Installation. Upgrade review and metadata failures are driven by the same
 * in-memory content state; unavailable project details never erase local files.
 */
const ShareModal = PreviewShareModal

const { formatMessage, locale } = useVIntl()
const { handleError, addNotification } = injectNotificationManager()
const router = useRouter()
const preview = usePreview()
const discover = useDiscover()
provideContentInstall(createContentInstall({ preview, discover, router }))

defineOptions({
    inheritAttrs: false,
})

const props = defineProps<{
    instance: GameInstance
    isServerInstance?: boolean
    openSettings?: () => void
}>()

defineEmits<{
    play: []
    stop: []
}>()

const { domain, items: allItems } = useInstanceContentItems(() => props.instance.id)
const notices = instanceUpgradeNotices(preview)
const postUpgradeNotice = computed(() => notices.get(props.instance.id))
const postUpgradeNoticeExpanded = ref(false)
const contentWarningExpanded = ref(false)
const refreshingWarnings = ref(false)
const metadataUnavailable = computed(() =>
    domain
        .sharedItems(props.instance.id)
        .some((item) => domain.metaFor(props.instance.id, item.id)?.metadataUnavailable),
)
const loading = ref(!domain.entryFor(props.instance.id).loaded)
const contentLoadError = computed(() => {
    const error = domain.entryFor(props.instance.id).error
    return error ? new Error(error) : null
})

function simulate(ms = 160) {
    return new Promise<void>((resolve) => preview.clock.after(ms, resolve))
}

function loadContent() {
    const instanceId = props.instance.id
    loading.value = !domain.entryFor(instanceId).loaded
    void domain.ensureLoaded(instanceId).finally(() => {
        if (props.instance.id === instanceId) loading.value = false
    })
}

loadContent()
watch(() => props.instance.id, loadContent)

const contentEntry = useContentEntries()

/** `translateContentItemTitles`: in zh-CN, titles read `中文名 (English)` when the dictionary knows one. */
function localizedTitle<T extends ContentItem>(item: T): T {
    const project = item.project
    if (locale.value !== 'zh-CN' || !project) return item
    const chineseName = contentEntry(project.id, project.slug)?.chineseName
    return chineseName
        ? { ...item, project: { ...project, title: bilingualTitle(chineseName, project.title) } }
        : item
}

const projects = computed(() =>
    allItems.value
        .filter((item) => item.instanceOwnershipKind !== 'pack_managed')
        .map(localizedTitle),
)
const linkedModpackContentItems = computed(() =>
    allItems.value
        .filter((item) => item.instanceOwnershipKind === 'pack_managed')
        .map(localizedTitle),
)

const isPackLocked = computed(
    () =>
        props.instance?.link?.type === 'modrinth_modpack' ||
        props.instance?.link?.type === 'curseforge_modpack' ||
        props.instance?.link?.type === 'server_project_modpack',
)
const hasPackAssociation = computed(
    () => isPackLocked.value || linkedModpackContentItems.value.length > 0,
)

/* -------------------------------------------------------------- jobs and busy */

const isInstanceBusy = computed(() => props.instance?.install_stage !== 'installed')
const activeContentChangeJobs = computed(() =>
    preview.downloads.jobs.value.filter((job) => job.instance_id === props.instance.id),
)
const isPackUnlinkDisabled = computed(
    () => isInstanceBusy.value || activeContentChangeJobs.value.length > 0,
)
const installingProjectIds = computed(
    () => new Set(activeContentChangeJobs.value.map((job) => job.project_id)),
)
const activeContentOperationKeys = ref(new Set<string>())
const isBulkOperating = ref(false)

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

/** The item's id in the content domain (`<instance>:<file>`). */
function domainId(item: ContentItem) {
    return item.id
}

function beginContentOperation(item: ContentItem) {
    const key = getContentItemId(item)
    if (activeContentOperationKeys.value.has(key)) return null
    activeContentOperationKeys.value = new Set([...activeContentOperationKeys.value, key])
    return key
}

function finishContentOperation(key: string) {
    const next = new Set(activeContentOperationKeys.value)
    next.delete(key)
    activeContentOperationKeys.value = next
}

const mergedProjects = computed<ContentItem[]>(() =>
    projects.value.map((item) =>
        (item.project?.id && installingProjectIds.value.has(item.project.id)) ||
        activeContentOperationKeys.value.has(getContentItemId(item))
            ? { ...item, installing: true }
            : item,
    ),
)
const displayedLinkedModpackContentItems = computed(() =>
    linkedModpackContentItems.value.map((item) =>
        activeContentOperationKeys.value.has(getContentItemId(item))
            ? { ...item, installing: true }
            : item,
    ),
)

const duplicateModCounts = computed(() => {
    const counts = new Map<string, number>()
    for (const item of [...mergedProjects.value, ...displayedLinkedModpackContentItems.value]) {
        const key = confirmedContentGroupKey(item)
        if (key) counts.set(key, (counts.get(key) ?? 0) + 1)
    }
    return counts
})

const duplicateModItems = computed(() =>
    [...mergedProjects.value, ...displayedLinkedModpackContentItems.value].filter((item) => {
        const key = confirmedContentGroupKey(item)
        return key != null && (duplicateModCounts.value.get(key) ?? 0) > 1
    }),
)

/** `duplicateGroupUpdateEntries`: every updatable copy of the project is reviewed together. */
async function updateDuplicateGroup(item: ContentItem) {
    const key = confirmedContentGroupKey(item)
    const copies = [...mergedProjects.value, ...displayedLinkedModpackContentItems.value].filter(
        (candidate) => key && confirmedContentGroupKey(candidate) === key,
    )
    if (!key || copies.some((copy) => copy.installing)) return
    try {
        await reviewResourceChanges(
            copies
                .filter(canUpdateProject)
                .map((copy) => ({ item: copy, versionId: updateTarget(copy)! })),
        )
    } catch (error) {
        handleError(error as Error)
    }
}

/* -------------------------------------------------------------- linked modpack */

const linkedModpack = ref<{
    project: ContentModpackCardProject
    version: ContentModpackCardVersion | null
    owner: ContentOwner | null
    categories: ContentModpackCardCategory[]
    hasUpdate: boolean
    updateVersionId: string | null
} | null>(null)
const linkVersionId = computed(() =>
    props.instance.link?.type === 'modrinth_modpack'
        ? props.instance.link.version_id
        : props.instance.link?.type === 'server_project_modpack'
          ? props.instance.link.content_version_id
          : null,
)
const isModpackUpdating = computed(
    () =>
        props.instance.install_stage === 'pack_installing' ||
        props.instance.install_stage === 'minecraft_installing',
)

/** `get_linked_modpack_info` and the pack project's card metadata. */
async function loadLinkedModpack() {
    const link = props.instance.link
    const reference = instanceContentProject(link)
    if (!reference || reference.source !== 'modrinth' || !linkVersionId.value) {
        linkedModpack.value = null
        return
    }
    const instanceId = props.instance.id
    try {
        const [project, tags] = await Promise.all([
            loadModpackProject(reference.projectId),
            discover.ensureTags(),
        ])
        if (props.instance.id !== instanceId) return
        const version = project.versions.find((entry) => entry.id === linkVersionId.value) ?? null
        const update =
            version && !props.isServerInstance
                ? checkModpackUpdate(version, project.versions, props.instance.update_channel)
                : null
        const categoryTags = new Map(
            tags.categories
                .filter((category) => category.project_type === 'modpack')
                .map((category) => [category.name, category]),
        )
        linkedModpack.value = {
            project: {
                id: project.id,
                slug: project.slug ?? project.id,
                title: project.title,
                icon_url: projectIconUrl(project.id),
                description: project.description,
                downloads: project.downloads,
                followers: project.followers,
            },
            version: version
                ? {
                      id: version.id,
                      version_number: version.version_number,
                      date_published: version.date_published,
                  }
                : null,
            owner: projectOwner(project),
            categories: project.categories.flatMap((name) => {
                const tag = categoryTags.get(name)
                return tag
                    ? [
                          {
                              name: tag.name,
                              project_type: tag.project_type,
                              header: tag.header,
                              icon: tag.icon ?? '',
                          } as ContentModpackCardCategory,
                      ]
                    : []
            }),
            hasUpdate: !!update,
            updateVersionId: update?.id ?? null,
        }
    } catch (error) {
        handleError(error as Error)
    }
}

watch(
    () => [props.instance.id, props.instance.link, props.instance.update_channel] as const,
    () => void loadLinkedModpack(),
    { immediate: true, deep: true },
)

const instanceContentProjectQuery = computed(() => ({
    i: props.instance.id,
    from: 'instance-content',
}))

const instanceIconUrl = computed(
    () => getDisplayInstanceIcon(props.instance.icon_path, props.instance.loader).url ?? undefined,
)

/* -------------------------------------------------------------- modals */

const shareModal = ref<InstanceType<typeof ShareModal> | null>()
const resourceOperationPreview = ref<InstanceType<typeof ResourceOperationPreviewModal> | null>(
    null,
)
const resourceBackupsModal = ref<InstanceType<typeof ResourceBackupsModal> | null>(null)
const contentUpdaterModal = ref<InstanceType<typeof ContentUpdaterModal> | null>()
const dependencyGraphModal = ref<InstanceType<typeof DependencyGraphModal> | null>()
const modpackUpdateConfirmModal = ref<InstanceType<typeof ConfirmModpackUpdateModal> | null>()
const packUnlinkModal = ref<InstanceType<typeof ConfirmUnlinkModal>>()
const toggleDependenciesModal = ref<InstanceType<typeof ContentToggleDependenciesModal> | null>(
    null,
)
const uploadInput = ref<HTMLInputElement>()

function allDependencyGraphItems() {
    const itemsById = new Map<string, ContentItem>()
    for (const item of [...mergedProjects.value, ...displayedLinkedModpackContentItems.value]) {
        const id = getContentItemId(item)
        if (id) itemsById.set(id, item)
    }
    return [...itemsById.values()]
}

function handleViewDependencies() {
    dependencyGraphModal.value?.show(allDependencyGraphItems())
}

/* -------------------------------------------------------------- browse and upload */

async function handleBrowseContent() {
    if (!props.instance) return
    await router.push({
        path: `/browse/${props.instance.loader === 'vanilla' ? 'resourcepack' : 'mod'}`,
        query: { i: props.instance.id },
    })
}

/** `open({ multiple: true })`: the browser's file picker stands in for the system dialog. */
function handleUploadFiles() {
    if (!uploadInput.value) return
    uploadInput.value.value = ''
    uploadInput.value.click()
}

/** `add_project_from_path`: each file joins the folder its contents belong in. */
async function handleUploadInput(event: Event) {
    const files = [...((event.target as HTMLInputElement).files ?? [])]
    if (!files.length) return
    const added: string[] = []
    for (const file of files) {
        try {
            const type = await classifyContentFile(file)
            if (!type) throw new Error(formatMessage(messages.parseFailed))
            if (
                domain.sharedItems(props.instance.id).some((item) => item.file_name === file.name)
            ) {
                throw new Error(formatMessage(messages.fileAlreadyExists))
            }
            await simulate(200)
            domain.addLocalFile(props.instance.id, { name: file.name, size: file.size, type })
            added.push(file.name.replace(/\.(jar|zip)$/i, ''))
        } catch (error) {
            handleError(error as Error)
        }
    }
    if (added.length > 0) {
        addNotification({
            type: 'success',
            title: formatMessage(messages.successfullyUploaded),
            text:
                added.length === 1
                    ? formatMessage(messages.projectWasAdded, { name: added[0] })
                    : formatMessage(messages.projectsWereAdded, { count: added.length }),
        })
    }
}

/* -------------------------------------------------------------- enable and disable */

function dependencyRefMatchesContentItem(
    reference: { provider: 'modrinth' | 'curseforge' | 'local'; projectId: string },
    item: ContentItem,
) {
    return (item.project?.id ?? '') === reference.projectId
}

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
            iconUrl: item.project?.icon_url ?? null,
            versionNumber: item.version?.version_number,
        })),
    })
}

/** `toggle_content_entries`: renames the files to and from `.disabled`. */
async function applyToggle(items: ContentItem[], enabled: boolean) {
    const targets = items.filter(
        (item) =>
            item.instanceCapabilities?.canToggle !== false &&
            item.enabled !== enabled &&
            !activeContentOperationKeys.value.has(getContentItemId(item)),
    )
    if (!targets.length) return
    const keys = targets.map((item) => beginContentOperation(item)).filter((key) => key !== null)
    try {
        await simulate(140 + targets.length * 15)
        domain.setEnabled(props.instance.id, targets.map(domainId), enabled)
    } finally {
        for (const key of keys) finishContentOperation(key)
    }
}

async function toggleDisableBatch(items: ContentItem[], enabling: boolean) {
    if (items.length === 0) return false
    const related = collectToggleRelated(items, enabling)
    let targets = items
    if (related.length > 0) {
        const choice = await promptToggleDependencies(items, related, enabling)
        if (choice === 'cancel') return false
        if (choice === 'apply') targets = [...related, ...targets]
    }
    isBulkOperating.value = true
    try {
        await applyToggle(targets, enabling)
    } finally {
        isBulkOperating.value = false
    }
    return true
}

async function toggleDisableMod(mod: ContentItem, desiredEnabled?: boolean) {
    const enabled = desiredEnabled ?? !mod.enabled
    if (mod.dependency) {
        const related = collectToggleRelated([mod], enabled)
        if (related.length > 0) {
            const choice = await promptToggleDependencies([mod], related, enabled)
            if (choice === 'cancel') return
            if (choice === 'apply') await applyToggle(related, enabled)
        }
    }
    await applyToggle([mod], enabled)
}

/* -------------------------------------------------------------- delete */

/** `remove_content_entry`: deletes the file from the instance folder. */
async function removeMod(mod: ContentItem) {
    if (!mod.file_path || mod.instanceCapabilities?.canDelete === false) return
    const key = beginContentOperation(mod)
    if (!key) return
    try {
        await simulate(180)
        domain.remove(props.instance.id, [domainId(mod)])
    } finally {
        finishContentOperation(key)
    }
}

function getDeleteDependencyWarning(items: ContentItem[]) {
    if (props.isServerInstance) return null
    const deletingIds = new Set(items.map(getContentItemId))
    const remainingItems = [...projects.value, ...linkedModpackContentItems.value].filter(
        (item) => !deletingIds.has(getContentItemId(item)),
    )
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
    return dependents.length > 0 ? { items, dependents } : null
}

/* -------------------------------------------------------------- updates */

const updatingProject = ref<ContentItem | null>(null)
const updatingProjectVersions = ref<UpdaterVersion[]>([])
const loadingVersions = ref(false)
const contentUpdateSubmitting = ref(false)
const updatingModpack = ref(false)
const pendingModpackUpdateVersion = ref<UpdaterVersion | null>(null)
const isModpackUpdateDowngrade = ref(false)
let updateRequestId = 0

function beginUpdateRequest() {
    updateRequestId += 1
    return updateRequestId
}

function canUpdateProject(item: ContentItem) {
    return (
        !!item.update &&
        item.instanceCapabilities?.canUpdate !== false &&
        !installingProjectIds.value.has(item.project?.id ?? '')
    )
}

/**
 * `reviewResourceEntryChanges`: the "Review content changes" modal plans the
 * chosen releases with their dependencies and queues the downloads; a finished
 * job replaces the file and keeps the old one for a rollback.
 */
async function reviewResourceChanges(
    entries: { item: ContentItem; versionId: string }[],
): Promise<boolean> {
    const modal = resourceOperationPreview.value
    if (!modal || entries.length === 0) return false
    const selections: ContentSelectionItem[] = entries.map(({ item, versionId }) => ({
        key: makeContentSelectionKey('modrinth', item.project!.id),
        provider: 'modrinth',
        projectId: item.project!.id,
        providerProjectId: item.project!.id,
        versionId,
        contentType: contentTypeOf(item.project_type),
        title: item.project?.title ?? item.file_name,
        iconUrl: item.project?.icon_url ?? null,
        slug: item.project?.slug ?? null,
    }))
    const reviewed = await modal.showInstallSelection({
        instance: props.instance,
        items: selections,
    })
    if (!reviewed) return false
    for (const { key, jobId } of reviewed.jobs) {
        const entry = entries.find(
            ({ item }) => makeContentSelectionKey('modrinth', item.project!.id) === key,
        )
        const previous = entry
            ? domain.sharedItems(props.instance.id).find((shared) => shared.id === entry.item.id)
            : undefined
        if (previous) domain.expectUpdate(jobId, props.instance.id, previous)
    }
    return true
}

function updateTarget(item: ContentItem) {
    return item.update?.provider === 'modrinth' ? item.update.target_version_id : null
}

async function updateProject(mod: ContentItem) {
    const target = updateTarget(mod)
    if (!target) return false
    try {
        return await reviewResourceChanges([{ item: mod, versionId: target }])
    } catch (err) {
        handleError(err as Error)
        return false
    }
}

async function switchProjectVersion(mod: ContentItem, version: UpdaterVersion) {
    try {
        return await reviewResourceChanges([{ item: mod, versionId: version.id }])
    } catch (err) {
        handleError(err as Error)
        return false
    }
}

async function bulkUpdateAllProjects() {
    const updatable = projects.value.filter(canUpdateProject)
    if (!updatable.length) return false
    isBulkOperating.value = true
    try {
        return await reviewResourceChanges(
            updatable.map((item) => ({ item, versionId: updateTarget(item)! })),
        )
    } finally {
        isBulkOperating.value = false
    }
}

async function bulkUpdateProjects(items: ContentItem[]) {
    const updatable = items.filter(canUpdateProject)
    if (!updatable.length) return false
    return await reviewResourceChanges(
        updatable.map((item) => ({ item, versionId: updateTarget(item)! })),
    )
}

/** `rollback_project`: puts the `.old` file the last update kept back in place. */
async function handleRollbackContent(mod: ContentItem) {
    if (!mod.file_path || !mod.rollback) return
    const key = beginContentOperation(mod)
    if (!key) return
    try {
        await simulate(220)
        domain.rollback(props.instance.id, domainId(mod))
    } catch (err) {
        handleError(err as Error)
    } finally {
        finishContentOperation(key)
    }
}

async function handleUpdate(item: ContentItem) {
    if (!item || !canUpdateProject(item)) return
    await updateProject(item)
}

/** The project's releases, newest first, as `get_project_versions` lists them. */
async function getUpdaterProjectVersions(projectId: string) {
    const detail = await domain.project(projectId)
    if (!detail) throw new Error(formatMessage(messages.versionsFailed))
    return [...detail.versions]
        .sort((a, b) => Date.parse(b.date_published) - Date.parse(a.date_published))
        .map((version) => toUpdaterVersion(version, projectId))
}

async function handleSwitchVersion(item: ContentItem) {
    const initialVersionId = item.version?.id
    if (!initialVersionId || !item.project?.id || item.project.id.startsWith('local:')) return

    const requestId = beginUpdateRequest()
    const itemId = getContentItemId(item)

    updatingModpack.value = false
    updatingProject.value = item
    updatingProjectVersions.value = []
    loadingVersions.value = true

    await nextTick()

    contentUpdaterModal.value?.show(initialVersionId, { switchMode: true })

    try {
        const versions = await getUpdaterProjectVersions(item.project.id)
        if (requestId === updateRequestId && getContentItemId(updatingProject.value) === itemId)
            updatingProjectVersions.value = versions
    } catch (error) {
        if (requestId === updateRequestId) handleError(error as Error)
    } finally {
        if (requestId === updateRequestId) loadingVersions.value = false
    }
}

async function refreshContentState() {
    await simulate(320)
    await domain.reload(props.instance.id)
}

async function retryContentWarnings() {
    if (refreshingWarnings.value) return
    refreshingWarnings.value = true
    try {
        await refreshContentState()
    } finally {
        refreshingWarnings.value = false
    }
}

watch(
    () => [props.instance?.update_channel, props.instance?.game_version, props.instance?.loader],
    (newValue, oldValue) => {
        if (newValue !== oldValue) void domain.reload(props.instance.id)
    },
)

async function handleModpackUpdate() {
    const link = props.instance?.link
    if (link?.type !== 'modrinth_modpack') return

    const requestId = beginUpdateRequest()

    updatingModpack.value = true
    updatingProject.value = null
    updatingProjectVersions.value = []
    loadingVersions.value = true

    await nextTick()

    const initialVersionId = linkedModpack.value?.updateVersionId ?? link.version_id
    contentUpdaterModal.value?.show(initialVersionId)

    try {
        const project = await loadModpackProject(link.project_id)
        if (requestId !== updateRequestId || !updatingModpack.value) return
        updatingProjectVersions.value = [...project.versions]
            .sort((a, b) => Date.parse(b.date_published) - Date.parse(a.date_published))
            .map((version) => toUpdaterVersion(version, project.id))
    } catch (error) {
        handleError(error as Error)
    } finally {
        if (requestId === updateRequestId) loadingVersions.value = false
    }
}

function resetUpdateState() {
    updatingModpack.value = false
    updatingProject.value = null
    updatingProjectVersions.value = []
    loadingVersions.value = false
}

async function handleModpackUpdateRequest(selectedVersion: UpdaterVersion) {
    pendingModpackUpdateVersion.value = selectedVersion

    const currentVersion = updatingProjectVersions.value.find(
        (version) => version.id === linkVersionId.value,
    )
    isModpackUpdateDowngrade.value = currentVersion
        ? new Date(selectedVersion.date_published) < new Date(currentVersion.date_published)
        : false
    const shouldShowWarning =
        isModpackUpdateDowngrade.value ||
        versionChangesGameVersion(selectedVersion as never, props.instance.game_version)

    if (preview.settings.feature_flags.skip_non_essential_warnings || !shouldShowWarning) {
        await handleModpackUpdateConfirm()
        return
    }

    modpackUpdateConfirmModal.value?.show()
}

/** `plan_content_updates(..., 'pack', version)` + apply: the pack install stages run. */
async function handleModpackUpdateConfirm() {
    if (!pendingModpackUpdateVersion.value || !props.instance?.id) return

    const version = pendingModpackUpdateVersion.value
    pendingModpackUpdateVersion.value = null

    contentUpdateSubmitting.value = true
    let succeeded = false
    try {
        await simulate(240)
        runPackInstall(preview, props.instance.id, (current) =>
            applyPackVersion(current, version as never),
        )
        succeeded = true
    } catch (error) {
        handleError(error as Error)
    } finally {
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

async function handleModalUpdate(selectedVersion: UpdaterVersion, event?: MouseEvent) {
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
            if (updateTarget(mod) === selectedVersion.id) {
                succeeded = await updateProject(mod)
            } else {
                succeeded = await switchProjectVersion(mod, selectedVersion)
            }
        } finally {
            contentUpdateSubmitting.value = false
        }
    }
    if (succeeded) {
        contentUpdaterModal.value?.hide()
        resetUpdateState()
    }
}

/** `edit(id, { link: null })`: the pack's files stay as content the user manages. */
async function unpairInstance() {
    await simulate(200)
    const target = preview.instances.get(props.instance.id)
    if (target) target.link = null
    domain.unlinkPack(props.instance.id)
    linkedModpack.value = null
}

/* -------------------------------------------------------------- share and menus */

function contentProjectUrl(item: ContentItem): string | null {
    const project = item.project
    if (!project || project.id.startsWith('local:')) return null
    const id = item.provider_refs?.find(
        (reference) => reference.provider === 'modrinth',
    )?.project_id
    return id ? `https://modrinth.com/project/${encodeURIComponent(String(id))}` : null
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

function getOverflowOptions(item: ContentItem): OverflowMenuOption[] {
    const options: OverflowMenuOption[] = []

    if (item.instanceMaterializationState === 'present' && item.file_path) {
        options.push({
            id: formatMessage(commonMessages.showFileButton),
            icon: FolderOpenIcon,
            action: () => revealInFolder(`${props.instance.path}/${item.file_path}`),
        })
    }

    if (item.instanceCapabilities?.canRestorePackDefault && item.instanceMemberId) {
        options.push({
            id: formatMessage(messages.restorePackDefault),
            icon: UndoIcon,
            action: () => restorePackDefault(item),
        })
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

    const mcmod = mcmodUrl(contentEntry(item.project?.id, item.project?.slug)?.wikiId)
    if (mcmod) {
        options.push({
            id: formatMessage(messages.openInMcmod),
            icon: BookOpenIcon,
            action: () => void openUrl(mcmod),
        })
    }

    return options
}

/** `restore_pack_member_default`, then the content list is read again. */
async function restorePackDefault(item: ContentItem) {
    const operation = beginContentOperation(item)
    if (!operation) return
    try {
        await simulate(420)
        domain.restorePackDefault(props.instance.id, item.id)
    } catch (error) {
        handleError(error as Error)
    } finally {
        finishContentOperation(operation)
    }
}

/** The modpack hint the app keeps in its instance cache (`modpackHintDismissed`). */
const hintMemory = domain.hintMemory()
const showContentHint = ref(!hintMemory.has(props.instance.id))
function dismissContentHint() {
    showContentHint.value = false
    hintMemory.add(props.instance.id)
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
        const pack = linkedModpack.value
        if (!pack) return null
        const projectPath = `/project/${pack.project.slug ?? pack.project.id}`
        return {
            project: pack.project,
            projectLink: { path: projectPath, query: instanceContentProjectQuery.value },
            version: pack.version ?? undefined,
            versionLink: pack.version
                ? {
                      path: `${projectPath}/version/${pack.version.id}`,
                      query: instanceContentProjectQuery.value,
                  }
                : undefined,
            owner: pack.owner
                ? {
                      ...pack.owner,
                      link: () => void openOwner(pack.owner!),
                  }
                : undefined,
            categories: pack.categories,
            hasUpdate: pack.hasUpdate,
            disabled: isModpackUpdating.value,
            disabledText: isModpackUpdating.value
                ? formatMessage(commonMessages.updatingLabel)
                : formatMessage(commonMessages.installingLabel),
        }
    }),
    isBusy: isInstanceBusy,
    isBulkOperating,
    skipNonEssentialWarnings: computed(
        () => preview.settings.feature_flags.skip_non_essential_warnings ?? false,
    ),
    contentTypeLabel: ref(formatMessage(messages.contentTypeProject)),
    toggleEnabled: (item) => toggleDisableMod(item),
    bulkEnableItems: async (items: ContentItem[]) =>
        await toggleDisableBatch(
            items.filter(
                (item) => item.enabled === false && item.instanceCapabilities?.canToggle !== false,
            ),
            true,
        ),
    bulkDisableItems: async (items: ContentItem[]) =>
        await toggleDisableBatch(
            items.filter(
                (item) => item.enabled === true && item.instanceCapabilities?.canToggle !== false,
            ),
            false,
        ),
    deleteItem: removeMod,
    bulkDeleteItems: async (items: ContentItem[]) => {
        for (const item of items) {
            await removeMod(item)
        }
    },
    getDeleteDependencyWarning,
    refresh: refreshContentState,
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
        const online = !!item.project?.id && !item.project.id.startsWith('local:')
        const projectLink = online
            ? {
                  path: `/project/${item.project!.id}`,
                  query: instanceContentProjectQuery.value,
              }
            : undefined
        const versionLink =
            online && item.version?.id
                ? {
                      path: `/project/${item.project!.id}/version/${item.version.id}`,
                      query: instanceContentProjectQuery.value,
                  }
                : undefined
        const ownerLink = item.owner
            ? { ...item.owner, link: online ? () => void openOwner(item.owner!) : undefined }
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
                    : formatMessage(commonMessages.unknownLabel),
            duplicateCount: (() => {
                const key = confirmedContentGroupKey(item)
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
            postUpgradeWarningTooltip: postUpgradeNotice.value?.warnings.some((warning) =>
                matchesPostUpgradeWarning(warning, item),
            )
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
        }
    },
})

/** The owner's Modrinth page, which the app opens in the browser. */
async function openOwner(owner: ContentOwner) {
    const { openUrl } = await import('../../lib/platform/opener')
    await openUrl(`https://modrinth.com/${owner.type}/${encodeURIComponent(owner.id)}`)
}
</script>
