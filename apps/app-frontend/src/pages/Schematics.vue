<script setup lang="ts">
import {
    BoxIcon,
    ChevronDownIcon,
    GenericListIcon,
    GlobeIcon,
    GridIcon,
    ListIcon,
    PlusIcon,
    SpinnerIcon,
} from '@modrinth/assets'
import {
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
    MAX_RESULTS_OPTIONS,
    Pagination,
    persistMaxResults,
    ProjectCardList,
    readPersistedMaxResults,
    useVIntl,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import { watchDebounced } from '@vueuse/core'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import BrowseInstanceSelector from '@/components/browse/BrowseInstanceSelector.vue'
import { schematicMessages as messages } from '@/components/browse/schematics/messages'
import SchematicCard from '@/components/browse/schematics/SchematicCard.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { createBrowseProjectTabs, getBrowseProjectTabOptions } from '@/helpers/browse-project-tabs'
import { list as listInstances } from '@/helpers/instance'
import { getDisplayInstanceIcon } from '@/helpers/instance-icons'
import { isServerProjectLink } from '@/helpers/instance-project'
import {
    getBrowseDefaultInstanceId,
    getLastBrowseContentDisplayMode,
    setBrowseDefaultInstanceId,
    setLastBrowseContentDisplayMode,
    setLastBrowseContentProjectType,
} from '@/helpers/settings'
import { listShimuSchematics, type ShimuSchematicSort } from '@/helpers/shimu-schematics'
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
const currentSort = computed<ShimuSchematicSort>(() =>
    route.query.s === 'hot' || route.query.s === 'featured' ? route.query.s : 'new',
)
const sortOptions = computed(() => [
    { value: 'hot', label: formatMessage(messages.sortHot) },
    { value: 'new', label: formatMessage(messages.sortNewest) },
    { value: 'featured', label: formatMessage(messages.sortFeatured) },
])
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
const instanceQuery = useQuery({
    queryKey: ['schematic-install-targets'],
    queryFn: listInstances,
    enabled: connected,
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
        'shimu',
        'schematics',
        account.value?.user?.userId,
        account.value?.flowId,
        submittedSearch.value,
        currentPage.value,
        pageSize.value,
        currentSort.value,
    ]),
    enabled: computed(() => connected.value && !authorizing.value),
    queryFn: async () => {
        const request = {
            page: currentPage.value,
            query: submittedSearch.value,
            pageSize: pageSize.value,
            sort: currentSort.value,
            userId: account.value?.user?.userId,
            flowId: account.value?.flowId,
        }
        return {
            ...(await listShimuSchematics(
                request.page,
                request.query,
                request.pageSize,
                request.sort,
            )),
            ...request,
        }
    },
    placeholderData: (previous) =>
        previous?.query === submittedSearch.value &&
        previous.userId === account.value?.user?.userId &&
        previous.flowId === account.value?.flowId &&
        previous.pageSize === pageSize.value &&
        previous.sort === currentSort.value
            ? previous
            : undefined,
    retry: false,
})
const projects = computed(() => pageQuery.data.value?.items ?? [])
const pageCount = computed(() => pageQuery.data.value?.pageCount ?? 1)
const displayedPage = computed(() => pageQuery.data.value?.page ?? currentPage.value)
watch(pageQuery.data, (page) => {
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
const cancel = useMutation({
    mutationFn: async () => {
        if (account.value?.flowId) await cancelThirdPartyAuthorization(account.value.flowId)
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey }),
})
const loginError = computed(
    () =>
        login.isError.value ||
        cancel.isError.value ||
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
    if (failed) void queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey })
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
    if (!['hot', 'new', 'featured'].includes(sort) || sort === currentSort.value) return
    void router.replace({ query: { ...route.query, s: sort, page: undefined } })
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
    <div data-onboarding-id="browse-schematics-content" class="flex flex-col gap-3 p-6">
        <BrowseToolbar
            v-model:query="search"
            :tabs="projectTabs"
            :placeholder="formatMessage(messages.search)"
            :disabled="!connected || authorizing"
        >
            <template v-if="installContext" #install-header>
                <BrowseInstallHeader :install-context="installContext" />
            </template>
            <template #search-bar-actions>
                <Button
                    size="md"
                    class="flex min-w-0 items-center gap-2"
                    :title="selectedInstance?.name"
                    :disabled="
                        !connected ||
                        authorizing ||
                        instanceQuery.isPending.value ||
                        instanceQuery.isError.value
                    "
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
                    selected="shimu"
                    :label="formatMessage(messages.source)"
                    :icon="GlobeIcon"
                    :options="[
                        { id: 'shimu', label: formatMessage(messages.source), icon: GlobeIcon },
                    ]"
                />
            </template>
            <template v-if="connected && !authorizing" #controls>
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
                />
            </template>
        </BrowseToolbar>
        <div
            v-if="connected && instanceQuery.isError.value"
            role="alert"
            class="flex items-center gap-3"
        >
            <span class="text-red">{{ formatMessage(messages.instancesError) }}</span>
            <Button @click="instanceQuery.refetch()">{{ formatMessage(messages.retry) }}</Button>
        </div>
        <div
            v-if="accountQuery.isPending.value"
            class="flex items-center justify-center gap-2 py-12"
            role="status"
        >
            <SpinnerIcon class="animate-spin motion-reduce:animate-none" />{{
                formatMessage(messages.loading)
            }}
        </div>
        <EmptyState
            v-else-if="accountQuery.isError.value"
            :heading="formatMessage(messages.accountError)"
        >
            <template #actions
                ><Button @click="accountQuery.refetch()">{{
                    formatMessage(messages.retry)
                }}</Button></template
            >
        </EmptyState>
        <EmptyState
            v-else-if="!connected || authorizing"
            :icon="BoxIcon"
            :heading="
                formatMessage(
                    authorizing
                        ? messages.waiting
                        : account?.status === 'expired'
                          ? messages.expired
                          : messages.loginTitle,
                )
            "
            :description="
                authorizing
                    ? undefined
                    : formatMessage(
                          !account?.available ? messages.unavailable : messages.loginDescription,
                      )
            "
        >
            <template #actions>
                <Button
                    v-if="authorizing"
                    :disabled="cancel.isPending.value"
                    @click="cancel.mutate()"
                    >{{ formatMessage(messages.cancel) }}</Button
                >
                <Button
                    v-else
                    type="colored"
                    :disabled="login.isPending.value || !account?.available"
                    @click="login.mutate()"
                    >{{ formatMessage(messages.login) }}</Button
                >
            </template>
        </EmptyState>
        <template v-else>
            <div
                v-if="pageQuery.isPending.value"
                class="flex items-center justify-center gap-2 py-12"
                role="status"
            >
                <SpinnerIcon class="animate-spin motion-reduce:animate-none" />{{
                    formatMessage(messages.loading)
                }}
            </div>
            <EmptyState
                v-else-if="!projects.length && !pageQuery.isError.value"
                :icon="BoxIcon"
                :heading="formatMessage(messages.emptyTitle)"
                :description="formatMessage(messages.emptyDescription)"
            />
            <ProjectCardList v-else :layout="displayMode">
                <SchematicCard
                    v-for="project in projects"
                    :key="`shimu:${project.postId}`"
                    :layout="displayMode"
                    :project="project"
                    :instance-id="selectedInstance?.id ?? null"
                    @choose-instance="picker?.show()"
                    @authorization-error="
                        queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey })
                    "
                />
            </ProjectCardList>
            <div
                v-if="pageQuery.isError.value"
                role="alert"
                class="flex flex-wrap items-center gap-3"
            >
                <p class="m-0 text-red">{{ formatMessage(messages.loadError) }}</p>
                <Button @click="pageQuery.refetch()">{{ formatMessage(messages.retry) }}</Button>
                <Button type="quiet" :disabled="login.isPending.value" @click="login.mutate()">{{
                    formatMessage(messages.expired)
                }}</Button>
            </div>
            <div class="flex justify-end">
                <Pagination
                    :page="displayedPage"
                    :count="pageCount"
                    :loading="pageQuery.isFetching.value"
                    @switch-page="setPage"
                />
            </div>
        </template>
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
