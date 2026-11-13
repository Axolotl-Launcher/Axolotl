<script setup lang="ts">
import {
    BoxIcon,
    ChevronDownIcon,
    GenericListIcon,
    GlobeIcon,
    GridIcon,
    ListIcon,
    PlusIcon,
} from '@modrinth/assets'
import {
    Admonition,
    type BrowseDisplayMode,
    type BrowseInstallContext,
    BrowseInstallHeader,
    BrowseResultControls,
    BrowseSourceSelector,
    BrowseToolbar,
    Button,
    commonMessages,
    EmptyState,
    injectNotificationManager,
    IntlFormatted,
    MAX_RESULTS_OPTIONS,
    Pagination,
    persistMaxResults,
    ProjectCardList,
    ProjectCardSkeleton,
    readPersistedMaxResults,
    useVIntl,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import { watchDebounced } from '@vueuse/core'
import { computed, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'

import BrowseInstanceSelector from '@/components/browse/BrowseInstanceSelector.vue'
import {
    schematicMessages as messages,
    schematicSourceMessages,
} from '@/components/browse/schematics/messages'
import SchematicCard from '@/components/browse/schematics/SchematicCard.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { createBrowseProjectTabs, getBrowseProjectTabOptions } from '@/helpers/browse-project-tabs'
import { list as listInstances } from '@/helpers/instance'
import { getDisplayInstanceIcon } from '@/helpers/instance-icons'
import { isServerProjectLink } from '@/helpers/instance-project'
import {
    isSchematicSource,
    listSchematics,
    SCHEMATIC_SOURCES,
    type SchematicBrowseSource,
    type SchematicSort,
} from '@/helpers/schematics'
import {
    getBrowseDefaultInstanceId,
    getLastBrowseContentDisplayMode,
    setBrowseDefaultInstanceId,
    setLastBrowseContentDisplayMode,
    setLastBrowseContentProjectType,
} from '@/helpers/settings'
import {
    beginThirdPartyAuthorization,
    cancelThirdPartyAuthorization,
    claimThirdPartyAuthorizationNotification,
    getThirdPartyAccount,
    thirdPartyAccountKey,
} from '@/helpers/third-party-accounts'
import type { GameInstance } from '@/helpers/types'

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const route = useRoute()
const router = useRouter()
const queryClient = useQueryClient()
const search = ref(typeof route.query.q === 'string' ? route.query.q : '')
const submittedSearch = computed(() =>
    typeof route.query.q === 'string' ? route.query.q.trim() : '',
)
const currentPage = computed(() => {
    const page = Number(route.query.page ?? 1)
    return Number.isInteger(page) && page > 0 && page <= 10000 ? page : 1
})
const rememberedPageSize = readPersistedMaxResults()
const initialPageSize = MAX_RESULTS_OPTIONS.reduce((previous, value) =>
    Math.abs(value - rememberedPageSize) <= Math.abs(previous - rememberedPageSize)
        ? value
        : previous,
)
const pageSize = computed(() => {
    const size = Number(route.query.m)
    return MAX_RESULTS_OPTIONS.includes(size) ? size : initialPageSize
})
const selectedInstanceId = ref(
    typeof route.query.i === 'string' ? route.query.i : getBrowseDefaultInstanceId(),
)
const picker = ref<InstanceType<typeof BrowseInstanceSelector> | null>(null)
const displayMode = ref(getLastBrowseContentDisplayMode())
const displayModeOptions = computed(() => [
    { id: 'list' as const, label: formatMessage(messages.listView), icon: ListIcon },
    { id: 'compact' as const, label: formatMessage(messages.compactView), icon: GenericListIcon },
    { id: 'grid' as const, label: formatMessage(messages.gridView), icon: GridIcon },
])
function setDisplayMode(mode: BrowseDisplayMode) {
    if (mode === 'gallery') return
    displayMode.value = mode
    setLastBrowseContentDisplayMode(mode)
}
const accountQuery = useQuery({
    queryKey: thirdPartyAccountKey,
    queryFn: getThirdPartyAccount,
    retry: false,
    refetchInterval: (query) =>
        ['waiting', 'exchanging'].includes(query.state.data?.authorization ?? '') ? 1000 : 30_000,
})
const account = accountQuery.data
const authorizing = computed(() =>
    ['waiting', 'exchanging'].includes(account.value?.authorization ?? ''),
)
const connected = computed(() => account.value?.status === 'connected')
const source = computed<SchematicBrowseSource>(() => {
    const requested = isSchematicSource(route.query.source) ? route.query.source : 'all'
    return requested === 'shimu' && !connected.value ? 'all' : requested
})
const availableSources = computed(() =>
    SCHEMATIC_SOURCES.filter((entry) => !entry.requiresLogin || connected.value),
)
const requestSources = computed(() => {
    const selected = source.value
    return selected === 'all' ? availableSources.value.map((entry) => entry.id) : [selected]
})
const sourceKey = computed(() => requestSources.value.join(','))
const includesShimu = computed(() => requestSources.value.includes('shimu'))
const sourceConfig = computed(() =>
    source.value === 'all'
        ? { sorts: ['default'] as const }
        : SCHEMATIC_SOURCES.find((entry) => entry.id === source.value)!,
)
const sourceOptions = computed(() => [
    { id: 'all', label: formatMessage(messages.allSources), icon: GlobeIcon },
    ...availableSources.value.map((entry) => ({
        id: entry.id,
        label: formatMessage(schematicSourceMessages[entry.id]),
        icon: GlobeIcon,
    })),
])
const lockedSourceCount = computed(
    () => SCHEMATIC_SOURCES.filter((entry) => entry.requiresLogin && !connected.value).length,
)
const currentSort = computed<SchematicSort>(
    () =>
        sourceConfig.value.sorts.find((sort) => sort === route.query.s) ??
        sourceConfig.value.sorts[0],
)
const sortOptions = computed(() => {
    const labels = {
        hot: messages.sortHot,
        new: messages.sortNewest,
        featured: messages.sortFeatured,
        default: messages.sortDefault,
        name: messages.sortName,
    }
    return sourceConfig.value.sorts.map((value) => ({ value, label: formatMessage(labels[value]) }))
})
const instanceQuery = useQuery({
    queryKey: ['schematic-install-targets'],
    queryFn: listInstances,
})
const instances = computed(() =>
    (instanceQuery.data.value ?? []).filter((instance) => !isServerProjectLink(instance.link)),
)
const selectedInstance = computed(
    () => instances.value.find((instance) => instance.id === selectedInstanceId.value) ?? null,
)
const installContext = computed<BrowseInstallContext | null>(() => {
    const target = selectedInstance.value
    if (!route.query.i || !target) return null
    const icon = getDisplayInstanceIcon(target.icon_path, target.loader)
    return {
        name: target.name,
        loader: target.loader,
        gameVersion: target.game_version,
        iconSrc: icon.url,
        iconFrameless: icon.frameless,
        backUrl: `/instance/${encodeURIComponent(target.id)}`,
        backLabel: formatMessage(messages.backToInstance),
        heading: formatMessage(commonMessages.installingContentLabel),
    }
})
const pageQuery = useQuery({
    queryKey: computed(() => [
        'schematics',
        source.value,
        sourceKey.value,
        includesShimu.value ? account.value?.user?.userId : null,
        includesShimu.value ? account.value?.flowId : null,
        submittedSearch.value,
        currentPage.value,
        pageSize.value,
        currentSort.value,
    ]),
    enabled: computed(() => source.value !== 'shimu' || (connected.value && !authorizing.value)),
    queryFn: async () => {
        const request = {
            page: currentPage.value,
            query: submittedSearch.value,
            source: source.value,
            sources: requestSources.value,
            sourceKey: sourceKey.value,
            pageSize: pageSize.value,
            sort: currentSort.value,
            userId: includesShimu.value ? account.value?.user?.userId : null,
            flowId: includesShimu.value ? account.value?.flowId : null,
        }
        return {
            ...(await listSchematics(
                request.sources,
                request.page,
                request.query,
                request.pageSize,
                request.sort,
            )),
            ...request,
        }
    },
    placeholderData: (previous) =>
        previous?.source === source.value &&
        previous.sourceKey === sourceKey.value &&
        previous.query === submittedSearch.value &&
        previous.userId === (includesShimu.value ? account.value?.user?.userId : null) &&
        previous.flowId === (includesShimu.value ? account.value?.flowId : null) &&
        previous.pageSize === pageSize.value &&
        previous.sort === currentSort.value
            ? previous
            : undefined,
    retry: false,
})
const projects = computed(() => pageQuery.data.value?.items ?? [])
const pageCount = computed(() => pageQuery.data.value?.pageCount ?? 1)
const displayedPage = computed(() => pageQuery.data.value?.page ?? currentPage.value)
const failedSourceNames = computed(() =>
    (pageQuery.data.value?.failedSources ?? []).map((id) =>
        formatMessage(schematicSourceMessages[id]),
    ),
)
const skeletonCount = computed(() =>
    displayMode.value === 'grid' ? 6 : Math.min(pageSize.value, 8),
)
watch(pageQuery.data, (page) => {
    if (page?.failedSources.includes('shimu'))
        void queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey })
    if (page && page.page === currentPage.value && page.page > page.pageCount)
        void router.replace({
            query: { ...route.query, page: page.pageCount > 1 ? page.pageCount : undefined },
        })
})
const login = useMutation({
    mutationFn: async () => {
        const flow = await beginThirdPartyAuthorization(route.fullPath)
        try {
            await openUrl(flow.authorizationUrl)
        } catch (error) {
            await cancelThirdPartyAuthorization(flow.flowId)
            throw error
        }
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey }),
})
const loginError = computed(
    () =>
        login.isError.value ||
        ['failed', 'denied', 'timedOut'].includes(account.value?.authorization ?? ''),
)
watch(
    account,
    (value) => {
        if (value && claimThirdPartyAuthorizationNotification(value))
            addNotification({
                title: formatMessage(messages.connected),
                text: value.user?.nickName || value.user?.userName || '',
                type: 'success',
            })
    },
    { immediate: true },
)
watch(pageQuery.isError, (failed) => {
    if (failed && includesShimu.value)
        void queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey })
})

function chooseInstance(instance: GameInstance) {
    selectedInstanceId.value = instance.id
    setBrowseDefaultInstanceId(instance.id)
    if (route.query.i) void router.replace({ query: { ...route.query, i: instance.id } })
}
function submitSearch() {
    const query = search.value.trim()
    if (query === submittedSearch.value) return
    void router.replace({ query: { ...route.query, q: query || undefined, page: undefined } })
}
async function setPage(page: number) {
    if (pageQuery.isFetching.value || page < 1 || page > pageCount.value) return
    await router.replace({ query: { ...route.query, page: page > 1 ? page : undefined } })
    window.scrollTo({ top: 0, behavior: 'smooth' })
}
function setSort(sort: string) {
    if (!sourceConfig.value.sorts.some((value) => value === sort) || sort === currentSort.value)
        return
    void router.replace({ query: { ...route.query, s: sort, page: undefined } })
}
function selectSource(value: string) {
    if (!sourceOptions.value.some((option) => option.id === value) || value === source.value) return
    void router.replace({ query: { ...route.query, source: value, s: undefined, page: undefined } })
}
function setPageSize(size: number) {
    if (!MAX_RESULTS_OPTIONS.includes(size) || size === pageSize.value) return
    persistMaxResults(size)
    void router.replace({ query: { ...route.query, m: String(size), page: undefined } })
}
watch(
    () => route.query.q,
    () => {
        search.value = submittedSearch.value
    },
)
watchDebounced(search, submitSearch, { debounce: 300 })
if (!route.query.i) setLastBrowseContentProjectType('schematic')
const projectTabs = computed(() =>
    createBrowseProjectTabs(
        {
            modpacks: formatMessage(messages.modpacks),
            mods: formatMessage(messages.mods),
            resourcepacks: formatMessage(messages.resourcepacks),
            datapacks: formatMessage(messages.datapacks),
            maps: formatMessage(messages.maps),
            shaders: formatMessage(messages.shaders),
            schematics: formatMessage(messages.schematics),
            servers: formatMessage(messages.servers),
            favorites: formatMessage(messages.favorites),
        },
        typeof route.query.i === 'string' ? `?i=${encodeURIComponent(route.query.i)}` : '',
        getBrowseProjectTabOptions({
            instance: selectedInstance.value,
            hasInstanceContext: !!route.query.i,
        }),
    ),
)
</script>

<template>
    <div class="flex flex-col gap-3 p-6">
        <BrowseToolbar
            v-model:query="search"
            :tabs="projectTabs"
            :placeholder="formatMessage(messages.search)"
            :disabled="source === 'shimu' && authorizing"
        >
            <template v-if="installContext" #install-header>
                <BrowseInstallHeader :install-context="installContext" />
            </template>
            <template #search-bar-actions>
                <Button
                    size="md"
                    class="flex min-w-0 items-center gap-2"
                    :title="selectedInstance?.name"
                    :disabled="instanceQuery.isPending.value || instanceQuery.isError.value"
                    @click="picker?.show()"
                >
                    <InstanceIcon
                        v-if="selectedInstance"
                        class="shrink-0"
                        size="1.25rem"
                        :icon-path="selectedInstance.icon_path"
                        :instance-id="selectedInstance.id"
                        :loader="selectedInstance.loader"
                    />
                    <PlusIcon v-else class="size-5 shrink-0" />
                    <span class="max-w-40 truncate font-medium">{{
                        selectedInstance?.name || formatMessage(messages.selectInstance)
                    }}</span>
                    <ChevronDownIcon class="size-4 shrink-0 text-[var(--color-text-tertiary)]" />
                </Button>
                <BrowseSourceSelector
                    :selected="source"
                    :label="
                        source === 'all'
                            ? formatMessage(messages.allSources)
                            : formatMessage(schematicSourceMessages[source])
                    "
                    :icon="GlobeIcon"
                    :options="sourceOptions"
                    @select="selectSource"
                />
            </template>
            <template #controls>
                <BrowseResultControls
                    :sort="currentSort"
                    :sort-options="sortOptions"
                    :page-size="pageSize"
                    :display-mode="displayMode"
                    :display-mode-options="displayModeOptions"
                    :display-mode-tooltip="formatMessage(messages.view)"
                    :page="displayedPage"
                    :page-count="pageCount"
                    :loading="pageQuery.isFetching.value"
                    @update:sort="setSort"
                    @update:page-size="setPageSize"
                    @display-mode="setDisplayMode"
                    @switch-page="setPage"
                >
                    <template v-if="lockedSourceCount" #display-mode-after>
                        <span class="whitespace-nowrap text-xs text-[var(--color-text-tertiary)]">
                            <IntlFormatted
                                :message-id="messages.loginRequiredSources"
                                :values="{ count: lockedSourceCount }"
                            >
                                <template #login-link="{ children }">
                                    <RouterLink
                                        to="/settings#third-party-accounts"
                                        class="text-brand hover:underline focus-visible:underline"
                                    >
                                        <component :is="() => children" />
                                    </RouterLink>
                                </template>
                            </IntlFormatted>
                        </span>
                    </template>
                </BrowseResultControls>
            </template>
        </BrowseToolbar>
        <div v-if="instanceQuery.isError.value" role="alert" class="flex items-center gap-3">
            <span class="text-red">{{ formatMessage(messages.instancesError) }}</span>
            <Button @click="instanceQuery.refetch()">{{ formatMessage(messages.retry) }}</Button>
        </div>

        <Admonition v-if="failedSourceNames.length" type="warning">
            <div class="flex flex-wrap items-center gap-3">
                <span>{{
                    formatMessage(messages.failedSources, { sources: failedSourceNames.join(', ') })
                }}</span>
                <Button @click="pageQuery.refetch()">{{ formatMessage(messages.retry) }}</Button>
            </div>
        </Admonition>
        <section
            v-if="pageQuery.isPending.value || pageQuery.isFetching.value"
            class="py-1"
            aria-busy="true"
            aria-live="polite"
            :aria-label="formatMessage(messages.loading)"
        >
            <ProjectCardList :layout="displayMode">
                <ProjectCardSkeleton
                    v-for="index in skeletonCount"
                    :key="`skeleton-${index}`"
                    :layout="displayMode"
                />
            </ProjectCardList>
        </section>
        <EmptyState
            v-else-if="!projects.length && !pageQuery.isError.value"
            :icon="BoxIcon"
            :heading="formatMessage(messages.emptyTitle)"
            :description="formatMessage(messages.emptyDescription)"
        />
        <ProjectCardList v-else :layout="displayMode">
            <SchematicCard
                v-for="project in projects"
                :key="`${project.source}:${project.id}`"
                :layout="displayMode"
                :project="project"
                :instance-id="selectedInstance?.id ?? null"
                @choose-instance="picker?.show()"
                @authorization-error="
                    queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey })
                "
            />
        </ProjectCardList>
        <div v-if="pageQuery.isError.value" role="alert" class="flex flex-wrap items-center gap-3">
            <p class="m-0 text-red">{{ formatMessage(messages.loadError) }}</p>
            <Button @click="pageQuery.refetch()">{{ formatMessage(messages.retry) }}</Button>
            <Button
                v-if="source === 'shimu'"
                type="quiet"
                :disabled="login.isPending.value"
                @click="login.mutate()"
                >{{ formatMessage(messages.expired) }}</Button
            >
        </div>
        <div class="flex justify-end">
            <Pagination
                :page="displayedPage"
                :count="pageCount"
                :loading="pageQuery.isFetching.value"
                @switch-page="setPage"
            />
        </div>
        <p v-if="loginError" role="alert" class="m-0 text-center text-red">
            {{ formatMessage(messages.loginFailed) }}
        </p>
        <BrowseInstanceSelector
            ref="picker"
            :instances="instances"
            :selected-instance="selectedInstance"
            :selected-count="0"
            :install-current="async () => true"
            :clear-current="() => {}"
            @select="chooseInstance"
        />
    </div>
</template>
