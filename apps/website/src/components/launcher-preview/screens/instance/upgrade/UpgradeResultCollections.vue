<template>
    <div class="flex flex-col gap-4">
        <Card v-if="warnings.length" class="!m-0 !p-0">
            <Accordion
                class="block w-full"
                :open-by-default="warningsDefaultOpen"
                button-class="group flex !w-full cursor-pointer border-0 bg-transparent p-4 text-left hover:bg-surface-3 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-inset focus-visible:ring-brand-shadow"
                content-class="border-0 border-t border-solid border-divider p-3"
                @on-open="warningsOpen = true"
                @on-close="warningsOpen = false"
            >
                <template #button="{ open }">
                    <div data-warning-trigger-content class="flex w-full min-w-0 flex-col gap-2">
                        <div class="flex w-full min-w-0 items-center gap-2">
                            <TriangleAlertIcon
                                class="size-5 shrink-0 text-orange"
                                aria-hidden="true"
                            />
                            <strong class="min-w-0">{{
                                formatMessage(messages.warningsTitle)
                            }}</strong>
                            <Badge color="orange" :type="String(warnings.length)" />
                            <DropdownIcon
                                class="ml-auto size-5 shrink-0 text-[var(--color-text-tertiary)] transition-transform duration-300 group-hover:text-[var(--color-text-default)]"
                                :class="{ 'rotate-180': open }"
                                aria-hidden="true"
                            />
                        </div>
                        <div
                            class="flex flex-wrap gap-x-4 gap-y-1 pl-7 text-xs text-[var(--color-text-tertiary)]"
                        >
                            <span v-if="warningSummary.local">{{ summaryLabel('local') }}</span>
                            <span v-if="warningSummary.kept">{{ summaryLabel('kept') }}</span>
                            <span v-if="warningSummary.fallback">{{
                                summaryLabel('fallback')
                            }}</span>
                        </div>
                        <p class="m-0 pl-7 text-xs text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.reassurance) }}
                        </p>
                    </div>
                </template>
                <div v-if="warningsOpen" class="flex flex-col gap-3">
                    <StyledInput
                        v-model="warningSearch"
                        type="search"
                        :icon="SearchIcon"
                        :placeholder="formatMessage(messages.warningSearch)"
                        :aria-label="formatMessage(messages.warningSearch)"
                        clearable
                        wrapper-class="w-full"
                    />
                    <div
                        class="flex flex-wrap gap-2"
                        role="group"
                        :aria-label="formatMessage(messages.warningFilters)"
                    >
                        <Button
                            v-for="option in warningFilters"
                            :key="option.value"
                            size="2xs"
                            :type="warningFilter === option.value ? 'colored' : 'outlined'"
                            :color="warningFilter === option.value ? 'brand' : undefined"
                            :aria-pressed="warningFilter === option.value"
                            @click="warningFilter = option.value"
                        >
                            {{ option.label }}
                        </Button>
                    </div>
                    <span class="text-sm text-[var(--color-text-tertiary)]">{{
                        warningPaginationLabel
                    }}</span>
                    <div v-if="warningPage.items.length">
                        <ul class="m-0 flex list-none flex-col gap-2 p-0">
                            <li
                                v-for="warning in warningPage.items"
                                :key="warning.key"
                                data-upgrade-warning-row
                                class="rounded-md bg-surface-2 p-3"
                            >
                                <strong class="block text-sm text-[var(--color-text-primary)]">{{
                                    warningHeadline(warning)
                                }}</strong>
                                <p class="mb-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                                    {{ warningDescription(warning) }}
                                </p>
                                <div
                                    class="mt-2 text-sm font-medium text-[var(--color-text-primary)]"
                                >
                                    {{ warningIdentity(warning) }}
                                </div>
                                <div class="text-xs text-[var(--color-text-tertiary)]">
                                    {{ warningContext(warning) }}
                                </div>
                                <details
                                    v-if="hasTechnicalDetails(warning)"
                                    class="mt-2 text-xs text-[var(--color-text-tertiary)]"
                                >
                                    <summary class="cursor-pointer">
                                        {{ formatMessage(messages.technicalDetails) }}
                                    </summary>
                                    <dl class="mb-0 mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
                                        <template v-if="warning.relativePath">
                                            <dt>{{ formatMessage(messages.relativePath) }}</dt>
                                            <dd class="m-0 break-all">
                                                <code>{{ warning.relativePath }}</code>
                                            </dd>
                                        </template>
                                        <template v-if="warning.code">
                                            <dt>{{ formatMessage(messages.warningCode) }}</dt>
                                            <dd class="m-0">
                                                <code>{{ warning.code }}</code>
                                            </dd>
                                        </template>
                                        <template v-if="warning.provider || warning.projectId">
                                            <dt>{{ formatMessage(messages.providerIdentity) }}</dt>
                                            <dd class="m-0 break-all">
                                                {{ warning.provider }} · {{ warning.projectId }}
                                            </dd>
                                        </template>
                                    </dl>
                                </details>
                            </li>
                        </ul>
                    </div>
                    <p
                        v-else
                        class="m-0 rounded-md bg-surface-2 p-4 text-center text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.noWarningMatches) }}
                    </p>
                    <div
                        class="flex flex-wrap items-center justify-between gap-3 text-sm text-[var(--color-text-tertiary)]"
                    >
                        <Button
                            type="outlined"
                            size="2xs"
                            :disabled="warningPage.page <= 1"
                            @click="warningPageNumber -= 1"
                            >{{ formatMessage(messages.previous) }}
                        </Button>
                        <span>{{ warningPage.page }} / {{ warningPage.pageCount }}</span>
                        <Button
                            type="outlined"
                            size="2xs"
                            :disabled="warningPage.page >= warningPage.pageCount"
                            @click="warningPageNumber += 1"
                            >{{ formatMessage(messages.next) }}
                        </Button>
                    </div>
                </div>
            </Accordion>
        </Card>

        <Card class="!m-0 !p-0">
            <Accordion
                button-class="flex w-full cursor-pointer items-center border-0 bg-transparent p-4 text-left hover:bg-surface-3"
                content-class="border-0 border-t border-solid border-divider p-4"
                @on-open="detailsOpen = true"
                @on-close="detailsOpen = false"
            >
                <template #title
                    ><strong>{{ formatMessage(messages.detailsTitle) }}</strong></template
                >
                <div v-if="detailsOpen" class="flex flex-col gap-3">
                    <StyledInput
                        v-model="search"
                        type="search"
                        :icon="SearchIcon"
                        :placeholder="formatMessage(messages.search)"
                        :aria-label="formatMessage(messages.search)"
                        clearable
                        wrapper-class="w-full"
                    />
                    <div
                        class="flex flex-wrap gap-2"
                        role="group"
                        :aria-label="formatMessage(messages.filters)"
                    >
                        <Button
                            v-for="option in filters"
                            :key="option.value"
                            size="2xs"
                            :type="filter === option.value ? 'colored' : 'outlined'"
                            :color="filter === option.value ? 'brand' : undefined"
                            :aria-pressed="filter === option.value"
                            @click="filter = option.value"
                        >
                            {{ option.label }}
                        </Button>
                    </div>

                    <div v-if="visibleRows.length" class="divide-y divide-divider">
                        <div
                            v-for="item in visibleRows"
                            :key="item.key"
                            data-upgrade-detail-row
                            class="flex items-center justify-between gap-3 py-2 text-sm"
                        >
                            <div class="min-w-0">
                                <RouterLink
                                    v-if="item.path"
                                    :to="item.path"
                                    class="block truncate font-medium text-[var(--color-text-primary)] hover:text-brand hover:underline"
                                    >{{ item.title }}
                                    <ExternalIcon
                                        class="inline size-3"
                                        aria-hidden="true" /></RouterLink
                                ><span
                                    v-else
                                    class="block truncate font-medium text-[var(--color-text-primary)]"
                                    >{{ item.title }}</span
                                >
                                <div class="truncate text-xs text-[var(--color-text-tertiary)]">
                                    {{ item.context }}
                                </div>
                                <div
                                    class="flex flex-wrap items-center gap-x-2 text-[var(--color-text-tertiary)]"
                                >
                                    <UpgradeVersionChangelogPopout
                                        v-if="item.currentReleaseId"
                                        :label="item.current"
                                        :provider="item.provider"
                                        :project-id="item.projectId"
                                        :release-id="item.currentReleaseId"
                                    /><span v-else>{{ item.current }}</span>
                                    <span v-if="item.target" aria-hidden="true">→</span>
                                    <UpgradeVersionChangelogPopout
                                        v-if="item.targetReleaseId"
                                        :label="item.target"
                                        :provider="item.provider"
                                        :project-id="item.projectId"
                                        :release-id="item.targetReleaseId"
                                    /><span v-else-if="item.target">{{ item.target }}</span>
                                </div>
                            </div>
                            <Badge :color="item.badgeColor" :type="item.actionLabel" />
                        </div>
                    </div>
                    <p
                        v-else
                        class="m-0 rounded-md bg-surface-2 p-4 text-center text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.noMatches) }}
                    </p>

                    <div
                        class="flex flex-wrap items-center justify-between gap-3 text-sm text-[var(--color-text-tertiary)]"
                    >
                        <span>{{ paginationLabel }}</span>
                        <div class="flex items-center gap-2">
                            <Button
                                type="outlined"
                                size="2xs"
                                :disabled="pageData.page <= 1"
                                @click="page -= 1"
                                >{{ formatMessage(messages.previous) }}
                            </Button>
                            <span>{{ pageData.page }} / {{ pageData.pageCount }}</span>
                            <Button
                                type="outlined"
                                size="2xs"
                                :disabled="pageData.page >= pageData.pageCount"
                                @click="page += 1"
                                >{{ formatMessage(messages.next) }}
                            </Button>
                        </div>
                    </div>
                </div>
            </Accordion>
        </Card>
    </div>
</template>

<script setup lang="ts">
import './messages'

import DropdownIcon from '@modrinth/assets/icons/dropdown.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import TriangleAlertIcon from '@modrinth/assets/icons/triangle-alert.svg?component'
import Accordion from '@modrinth/ui/src/components/base/Accordion.vue'
import Badge from '@modrinth/ui/src/components/base/Badge.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Card from '@modrinth/ui/src/components/base/Card.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref, watch } from 'vue'

import type { InstanceUpgradeResult } from './backend'
import { get_content_snapshot } from './content'
import {
    loadUpgradeProjectDisplayMetadata,
    loadUpgradeVersionDisplayMetadata,
    upgradeProjectDisplayCacheKey,
    upgradeVersionDisplayLabel,
} from './metadata'
import { upgradeProjectPath } from './navigation'
import { shouldExpandUpgradeWarningsByDefault } from './notice'
import { useQuery } from './query'
import {
    filterUpgradeDetailItems,
    paginateUpgradeDetailItems,
    type UpgradeDetailFilter,
    type UpgradeDetailItem,
    upgradeDetailItems,
    upgradeDetailProjectIdentities,
    upgradeDetailReleaseIdentities,
} from './upgrade-result-presentation'
import {
    filterUpgradeWarnings,
    paginateUpgradeWarnings,
    summarizeUpgradeWarnings,
    upgradeResultWarningRows,
    type UpgradeWarningCategory,
    upgradeWarningCategory,
    upgradeWarningContentKind,
    upgradeWarningDisplayName,
    type UpgradeWarningFilter,
    type UpgradeWarningRow,
} from './upgrade-warning'
import UpgradeVersionChangelogPopout from './UpgradeVersionChangelogPopout.vue'

const messages = defineMessages({
    warningsTitle: {
        id: 'instance.upgrade.result.warnings-title',
        defaultMessage: 'Compatibility warnings',
    },
    summaryLocal: {
        id: 'instance.upgrade.result.warning-summary-local',
        defaultMessage:
            '{count, plural, one {# local item could not be identified} other {# local items could not be identified}}',
    },
    summaryKept: {
        id: 'instance.upgrade.result.warning-summary-kept',
        defaultMessage:
            '{count, plural, one {# item kept its previous version} other {# items kept their previous version}}',
    },
    summaryFallback: {
        id: 'instance.upgrade.result.warning-summary-fallback',
        defaultMessage:
            '{count, plural, one {# item used another compatibility fallback} other {# items used another compatibility fallback}}',
    },
    reassurance: {
        id: 'instance.upgrade.result.warning-reassurance',
        defaultMessage:
            'These warnings did not prevent the upgrade from completing. If the upgraded instance runs normally, no immediate action is required.',
    },
    warningSearch: {
        id: 'instance.upgrade.result.warning-search',
        defaultMessage: 'Search compatibility warnings',
    },
    warningFilters: {
        id: 'instance.upgrade.result.warning-filters',
        defaultMessage: 'Filter compatibility warnings',
    },
    warningFilterUnidentified: {
        id: 'instance.upgrade.result.warning-filter-unidentified',
        defaultMessage: 'Unidentified',
    },
    warningFilterFallback: {
        id: 'instance.upgrade.result.warning-filter-fallback',
        defaultMessage: 'Compatibility fallback',
    },
    noWarningMatches: {
        id: 'instance.upgrade.result.no-matching-warnings',
        defaultMessage: 'No matching compatibility warnings.',
    },
    unidentifiedHeadline: {
        id: 'instance.upgrade.result.warning-unidentified-headline',
        defaultMessage: 'This content was kept unchanged',
    },
    unidentifiedDescription: {
        id: 'instance.upgrade.result.warning-unidentified-description',
        defaultMessage:
            'The launcher could not confirm whether it supports Minecraft {targetVersion}. If you notice problems, try disabling it temporarily.',
    },
    unsupportedHeadline: {
        id: 'instance.upgrade.result.warning-unsupported-headline',
        defaultMessage: 'This content type was kept unchanged',
    },
    unsupportedDescription: {
        id: 'instance.upgrade.result.warning-unsupported-description',
        defaultMessage:
            'This content type cannot be upgraded automatically. Check for a manual update if problems occur.',
    },
    keptHeadline: {
        id: 'instance.upgrade.result.warning-kept-headline',
        defaultMessage: 'The previous version was kept',
    },
    keptDescription: {
        id: 'instance.upgrade.result.warning-kept-description',
        defaultMessage:
            'No verified compatible replacement was selected. Update it manually or disable it if the game has problems.',
    },
    prereleaseHeadline: {
        id: 'instance.upgrade.result.warning-prerelease-headline',
        defaultMessage: 'A prerelease version was used',
    },
    prereleaseDescription: {
        id: 'instance.upgrade.result.warning-prerelease-description',
        defaultMessage:
            'This item used an alpha, beta, or release-candidate build because no stable target build was available.',
    },
    shaderHeadline: {
        id: 'instance.upgrade.result.warning-shader-headline',
        defaultMessage: 'Shader compatibility could not be confirmed',
    },
    shaderDescription: {
        id: 'instance.upgrade.result.warning-shader-description',
        defaultMessage:
            'The shader was preserved, but compatibility with the target shader runtime is unknown. Disable it if rendering problems occur.',
    },
    dependencyHeadline: {
        id: 'instance.upgrade.result.warning-dependency-headline',
        defaultMessage: 'A dependency needed a compatibility fallback',
    },
    dependencyDescription: {
        id: 'instance.upgrade.result.warning-dependency-description',
        defaultMessage:
            'The upgrade completed, but this dependency could not be verified normally. Review it if the game fails to start.',
    },
    conflictHeadline: {
        id: 'instance.upgrade.result.warning-conflict-headline',
        defaultMessage: 'Some dependency requirements conflicted',
    },
    conflictDescription: {
        id: 'instance.upgrade.result.warning-conflict-description',
        defaultMessage:
            'This content requested dependency versions that could not all be used together. Review its dependencies if the game fails to start.',
    },
    missingDependencyHeadline: {
        id: 'instance.upgrade.result.warning-missing-dependency-headline',
        defaultMessage: 'A required dependency could not be found',
    },
    missingDependencyDescription: {
        id: 'instance.upgrade.result.warning-missing-dependency-description',
        defaultMessage:
            'The provider did not offer a required dependency for the target environment. Install a compatible dependency manually if needed.',
    },
    incompatibleDependencyHeadline: {
        id: 'instance.upgrade.result.warning-incompatible-dependency-headline',
        defaultMessage: 'A dependency may be incompatible',
    },
    incompatibleDependencyDescription: {
        id: 'instance.upgrade.result.warning-incompatible-dependency-description',
        defaultMessage:
            'A dependency could not satisfy the selected versions. Review or disable the affected content if the game has problems.',
    },
    searchLimitHeadline: {
        id: 'instance.upgrade.result.warning-search-limit-headline',
        defaultMessage: 'Compatibility could not be fully verified',
    },
    searchLimitDescription: {
        id: 'instance.upgrade.result.warning-search-limit-description',
        defaultMessage:
            'The bounded compatibility search could not prove a complete result. Review this content if the game has problems.',
    },
    resolvedHeadline: {
        id: 'instance.upgrade.result.warning-resolved-headline',
        defaultMessage: 'A compatibility fallback was applied',
    },
    resolvedDescription: {
        id: 'instance.upgrade.result.warning-resolved-description',
        defaultMessage:
            'This warning occurred while planning, but the content was upgraded successfully. No immediate action is required.',
    },
    disabledHeadline: {
        id: 'instance.upgrade.result.warning-disabled-headline',
        defaultMessage: 'This content was disabled',
    },
    disabledDescription: {
        id: 'instance.upgrade.result.warning-disabled-description',
        defaultMessage:
            'The content was preserved on disk but disabled to avoid affecting the upgraded instance.',
    },
    legacyHeadline: {
        id: 'instance.upgrade.result.warning-legacy-headline',
        defaultMessage: 'Compatibility needs attention',
    },
    technicalDetails: {
        id: 'instance.upgrade.result.technical-details',
        defaultMessage: 'Technical details',
    },
    relativePath: { id: 'instance.upgrade.result.relative-path', defaultMessage: 'Relative path' },
    warningCode: { id: 'instance.upgrade.result.warning-code', defaultMessage: 'Warning' },
    providerIdentity: {
        id: 'instance.upgrade.result.provider-identity',
        defaultMessage: 'Provider identity',
    },
    localContent: { id: 'instance.upgrade.result.local-content', defaultMessage: 'Local content' },
    content: { id: 'instance.upgrade.result.content-kind-content', defaultMessage: 'Content' },
    mod: { id: 'instance.upgrade.result.content-kind-mod', defaultMessage: 'Mod' },
    resourcepack: {
        id: 'instance.upgrade.result.content-kind-resourcepack',
        defaultMessage: 'Resource pack',
    },
    shaderpack: {
        id: 'instance.upgrade.result.content-kind-shaderpack',
        defaultMessage: 'Shader pack',
    },
    datapack: { id: 'instance.upgrade.result.content-kind-datapack', defaultMessage: 'Data pack' },
    detailsTitle: {
        id: 'instance.upgrade.result.details-title',
        defaultMessage: 'Upgrade details',
    },
    search: {
        id: 'instance.upgrade.result.search-details',
        defaultMessage: 'Search upgrade details',
    },
    filters: { id: 'instance.upgrade.result.filters', defaultMessage: 'Filter upgrade details' },
    all: { id: 'instance.upgrade.result.filter-all', defaultMessage: 'All' },
    updated: { id: 'instance.upgrade.result.filter-updated', defaultMessage: 'Updated' },
    kept: { id: 'instance.upgrade.result.filter-kept', defaultMessage: 'Kept' },
    disabled: { id: 'instance.upgrade.result.filter-disabled', defaultMessage: 'Disabled' },
    dependencies: {
        id: 'instance.upgrade.result.filter-dependencies',
        defaultMessage: 'Dependencies',
    },
    showing: {
        id: 'instance.upgrade.result.showing',
        defaultMessage: 'Showing {start}–{end} of {total}',
    },
    previous: { id: 'instance.upgrade.result.previous', defaultMessage: 'Previous' },
    next: { id: 'instance.upgrade.result.next', defaultMessage: 'Next' },
    noMatches: {
        id: 'instance.upgrade.result.no-matching-items',
        defaultMessage: 'No matching upgrade items.',
    },
    unknown: { id: 'instance.upgrade.result.unknown', defaultMessage: 'Unavailable' },
    dependency: { id: 'instance.upgrade.result.dependency', defaultMessage: 'Dependency' },
    upgrade: { id: 'instance.upgrade.result.action-upgrade', defaultMessage: 'Updated' },
    keep: { id: 'instance.upgrade.result.action-keep', defaultMessage: 'Kept' },
    disable: { id: 'instance.upgrade.result.action-disable', defaultMessage: 'Disabled' },
    dependencyAdded: {
        id: 'instance.upgrade.result.dependency-added',
        defaultMessage: 'Dependency added',
    },
    dependencyUpdated: {
        id: 'instance.upgrade.result.dependency-updated-status',
        defaultMessage: 'Dependency updated',
    },
    dependencyKept: {
        id: 'instance.upgrade.result.dependency-kept',
        defaultMessage: 'Dependency kept',
    },
    dependencyRemoved: {
        id: 'instance.upgrade.result.dependency-removed-status',
        defaultMessage: 'Dependency removed',
    },
})

const props = defineProps<{ result: InstanceUpgradeResult; targetVersion: string | null }>()
const { formatMessage } = useVIntl()
const warnings = computed(() => upgradeResultWarningRows(props.result))
const warningSummary = computed(() => summarizeUpgradeWarnings(warnings.value))
const warningsDefaultOpen = computed(() =>
    shouldExpandUpgradeWarningsByDefault(warnings.value.length),
)
const warningsOpen = ref(warningsDefaultOpen.value)
const warningSearch = ref('')
const warningFilter = ref<UpgradeWarningFilter>('all')
const warningPageNumber = ref(1)
const warningFilters = computed(() => [
    { value: 'all' as const, label: formatMessage(messages.all) },
    { value: 'local' as const, label: formatMessage(messages.warningFilterUnidentified) },
    { value: 'kept' as const, label: formatMessage(messages.kept) },
    { value: 'fallback' as const, label: formatMessage(messages.warningFilterFallback) },
])
const filteredWarnings = computed(() =>
    filterUpgradeWarnings(
        warnings.value,
        warningFilter.value,
        warningSearch.value,
        warningSearchFields,
    ),
)
const warningPage = computed(() =>
    paginateUpgradeWarnings(filteredWarnings.value, warningPageNumber.value),
)
const warningPaginationLabel = computed(() =>
    formatMessage(messages.showing, {
        start: warningPage.value.start,
        end: warningPage.value.end,
        total: warningPage.value.total,
    }),
)
watch([warningSearch, warningFilter], () => {
    warningPageNumber.value = 1
})
watch(
    () => warningPage.value.page,
    (value) => {
        warningPageNumber.value = value
    },
)
const detailsOpen = ref(false)

const snapshotsQuery = useQuery({
    queryKey: computed(() => [
        'instance-upgrade',
        'result-content',
        props.result.sourceInstanceId,
        props.result.targetInstanceId,
    ]),
    queryFn: () =>
        Promise.all(
            [...new Set([props.result.sourceInstanceId, props.result.targetInstanceId])].map((id) =>
                get_content_snapshot(id).catch(() => null),
            ),
        ),
    enabled: computed(() => warningsOpen.value || detailsOpen.value),
    staleTime: Number.POSITIVE_INFINITY,
})
const snapshotItems = computed(
    () => snapshotsQuery.data.value?.flatMap((snapshot) => snapshot?.items ?? []) ?? [],
)
const allItems = computed(() => upgradeDetailItems(props.result.solution))
const search = ref('')
const filter = ref<UpgradeDetailFilter>('all')
const page = ref(1)
const filters = computed(() => [
    { value: 'all' as const, label: formatMessage(messages.all) },
    { value: 'updated' as const, label: formatMessage(messages.updated) },
    { value: 'kept' as const, label: formatMessage(messages.kept) },
    { value: 'disabled' as const, label: formatMessage(messages.disabled) },
    { value: 'dependencies' as const, label: formatMessage(messages.dependencies) },
])
const filteredItems = computed(() =>
    filterUpgradeDetailItems(allItems.value, filter.value, search.value, searchFields),
)
const pageData = computed(() => paginateUpgradeDetailItems(filteredItems.value, page.value))
watch([search, filter], () => {
    page.value = 1
})
watch(
    () => pageData.value.page,
    (value) => {
        page.value = value
    },
)

const projectIdentities = computed(() =>
    detailsOpen.value ? upgradeDetailProjectIdentities(pageData.value.items) : [],
)
const releaseIdentities = computed(() =>
    detailsOpen.value ? upgradeDetailReleaseIdentities(pageData.value.items) : [],
)
const projectsQuery = useQuery({
    queryKey: computed(() => [
        'instance-upgrade',
        'result-projects',
        ...projectIdentities.value.map((item) => `${item.provider}:${item.projectId}`),
    ]),
    queryFn: () => loadUpgradeProjectDisplayMetadata(projectIdentities.value),
    enabled: computed(() => detailsOpen.value && projectIdentities.value.length > 0),
    staleTime: Number.POSITIVE_INFINITY,
})
const versionsQuery = useQuery({
    queryKey: computed(() => [
        'instance-upgrade',
        'result-versions',
        ...releaseIdentities.value.map(
            (item) => `${item.provider}:${item.projectId}:${item.releaseId}`,
        ),
    ]),
    queryFn: () => loadUpgradeVersionDisplayMetadata(releaseIdentities.value),
    enabled: computed(() => detailsOpen.value && releaseIdentities.value.length > 0),
    staleTime: Number.POSITIVE_INFINITY,
})
const visibleRows = computed(() =>
    pageData.value.items.map((item) => {
        const snapshot = findSnapshot(item)
        const project =
            item.provider && item.projectId
                ? projectsQuery.data.value?.get(
                      upgradeProjectDisplayCacheKey(item.provider, item.projectId),
                  )
                : null
        return {
            ...item,
            title:
                project?.title ??
                snapshot?.content?.project.title ??
                snapshot?.content?.file_name ??
                filename(snapshot?.expectedRelativePath) ??
                item.projectId ??
                item.contentId ??
                formatMessage(messages.unknown),
            context:
                item.kind === 'dependency'
                    ? formatMessage(messages.dependency)
                    : (snapshot?.expectedRelativePath ??
                      item.provider ??
                      formatMessage(messages.localContent)),
            path: upgradeProjectPath(item.provider, item.projectId),
            current: releaseLabel(item, item.currentReleaseId) ?? formatMessage(messages.unknown),
            target: releaseLabel(item, item.targetReleaseId),
            actionLabel: actionLabel(item),
            badgeColor:
                item.kind === 'selection' && item.action === 'disable'
                    ? ('gray' as const)
                    : item.kind === 'selection' && item.action === 'keep'
                      ? ('blue' as const)
                      : ('green' as const),
        }
    }),
)
const paginationLabel = computed(() =>
    formatMessage(messages.showing, {
        start: pageData.value.start,
        end: pageData.value.end,
        total: pageData.value.total,
    }),
)

function findSnapshot(item: UpgradeDetailItem) {
    return (
        snapshotItems.value.find((snapshot) => snapshot.entryId === item.contentId) ??
        snapshotItems.value.find(
            (snapshot) =>
                snapshot.provider === item.provider &&
                snapshot.providerProjectId === item.projectId,
        )
    )
}
function searchFields(item: UpgradeDetailItem) {
    const snapshot = findSnapshot(item)
    return [
        snapshot?.content?.project.title,
        snapshot?.content?.file_name,
        snapshot?.content?.version?.version_number,
        snapshot?.expectedRelativePath,
        item.contentId,
        item.provider,
        item.projectId,
        item.currentReleaseId,
        item.targetReleaseId,
    ]
}
function releaseLabel(item: UpgradeDetailItem, releaseId: string | null) {
    return releaseId
        ? upgradeVersionDisplayLabel(versionsQuery.data.value, {
              provider: item.provider,
              projectId: item.projectId,
              releaseId,
          })
        : null
}
function actionLabel(item: UpgradeDetailItem) {
    if (item.kind === 'selection')
        return formatMessage(messages[item.action as 'upgrade' | 'keep' | 'disable'])
    return formatMessage(
        item.action === 'add'
            ? messages.dependencyAdded
            : item.action === 'upgrade'
              ? messages.dependencyUpdated
              : item.action === 'remove'
                ? messages.dependencyRemoved
                : messages.dependencyKept,
    )
}
function filename(path: string | null | undefined) {
    return path?.replaceAll('\\', '/').split('/').filter(Boolean).at(-1) ?? null
}
function warningIdentity(warning: UpgradeWarningRow) {
    const snapshot = snapshotItems.value.find(
        (item) =>
            item.entryId === warning.contentId ||
            item.expectedRelativePath === warning.relativePath,
    )
    return (
        snapshot?.content?.project.title ??
        snapshot?.content?.file_name ??
        upgradeWarningDisplayName(warning) ??
        formatMessage(messages.unknown)
    )
}
function warningContext(warning: UpgradeWarningRow) {
    return `${formatMessage(messages[upgradeWarningContentKind(warning)])} · ${warning.provider ?? formatMessage(messages.localContent)}`
}
function warningSearchFields(warning: UpgradeWarningRow) {
    return [
        warningIdentity(warning),
        warning.relativePath,
        warning.code,
        warning.provider,
        warning.projectId,
        formatMessage(
            upgradeWarningCategory(warning) === 'local'
                ? messages.warningFilterUnidentified
                : upgradeWarningCategory(warning) === 'kept'
                  ? messages.kept
                  : messages.warningFilterFallback,
        ),
    ]
}
function summaryLabel(category: UpgradeWarningCategory) {
    return formatMessage(
        category === 'local'
            ? messages.summaryLocal
            : category === 'kept'
              ? messages.summaryKept
              : messages.summaryFallback,
        { count: warningSummary.value[category] },
    )
}
function warningHeadline(warning: UpgradeWarningRow) {
    if (warning.legacyMessage) return formatMessage(messages.legacyHeadline)
    const action = warningAction(warning)
    if (action === 'disable') return formatMessage(messages.disabledHeadline)
    if (action === 'upgrade' && warning.code !== 'prerelease_only') {
        return formatMessage(messages.resolvedHeadline)
    }
    if (warning.code === 'prerelease_only' && action !== 'upgrade') {
        return formatMessage(messages.keptHeadline)
    }
    if (warning.code === 'unidentified') return formatMessage(messages.unidentifiedHeadline)
    if (warning.code === 'unsupported_content_type')
        return formatMessage(messages.unsupportedHeadline)
    if (warning.code === 'keep_incompatible' || warning.code === 'no_compatible_release')
        return formatMessage(messages.keptHeadline)
    if (warning.code === 'prerelease_only') return formatMessage(messages.prereleaseHeadline)
    if (warning.code?.includes('shader')) return formatMessage(messages.shaderHeadline)
    if (warning.code === 'dependency_conflict') return formatMessage(messages.conflictHeadline)
    if (warning.code === 'missing_required_dependency') {
        return formatMessage(messages.missingDependencyHeadline)
    }
    if (warning.code === 'incompatible_dependency') {
        return formatMessage(messages.incompatibleDependencyHeadline)
    }
    if (warning.code === 'search_limit_reached') return formatMessage(messages.searchLimitHeadline)
    return formatMessage(messages.dependencyHeadline)
}
function warningDescription(warning: UpgradeWarningRow) {
    if (warning.legacyMessage) return warning.legacyMessage
    const action = warningAction(warning)
    if (action === 'disable') return formatMessage(messages.disabledDescription)
    if (action === 'upgrade' && warning.code !== 'prerelease_only') {
        return formatMessage(messages.resolvedDescription)
    }
    if (warning.code === 'prerelease_only' && action !== 'upgrade') {
        return formatMessage(messages.keptDescription)
    }
    if (warning.code === 'unidentified')
        return formatMessage(messages.unidentifiedDescription, {
            targetVersion: props.targetVersion ?? formatMessage(messages.unknown),
        })
    if (warning.code === 'unsupported_content_type')
        return formatMessage(messages.unsupportedDescription)
    if (warning.code === 'keep_incompatible' || warning.code === 'no_compatible_release')
        return formatMessage(messages.keptDescription)
    if (warning.code === 'prerelease_only') return formatMessage(messages.prereleaseDescription)
    if (warning.code?.includes('shader')) return formatMessage(messages.shaderDescription)
    if (warning.code === 'dependency_conflict') return formatMessage(messages.conflictDescription)
    if (warning.code === 'missing_required_dependency') {
        return formatMessage(messages.missingDependencyDescription)
    }
    if (warning.code === 'incompatible_dependency') {
        return formatMessage(messages.incompatibleDependencyDescription)
    }
    if (warning.code === 'search_limit_reached')
        return formatMessage(messages.searchLimitDescription)
    return formatMessage(messages.dependencyDescription)
}

function warningAction(warning: UpgradeWarningRow) {
    return props.result.solution.selections.find(
        (selection) => selection.contentId === warning.contentId,
    )?.action
}
function hasTechnicalDetails(warning: UpgradeWarningRow) {
    return Boolean(warning.relativePath || warning.code || warning.provider || warning.projectId)
}
</script>
