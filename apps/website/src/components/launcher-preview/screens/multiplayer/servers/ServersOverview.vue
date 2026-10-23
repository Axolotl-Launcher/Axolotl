<script setup lang="ts">
import CollectionIcon from '@modrinth/assets/icons/collection.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import PopoutMenu from '@modrinth/ui/src/components/base/PopoutMenu.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, onMounted, ref, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'
import { useRouter } from 'vue-router'

import { getLastLibraryDisplayMode, setLastLibraryDisplayMode } from '../app-helpers.ts'
import { useServerLifecycle } from '../composables/useServerLifecycle.ts'
import { type ServerView, useServers } from '../composables/useServers.ts'
import CreateServerModal from './CreateServerModal.vue'
import EulaModal from './EulaModal.vue'
import ServerCard from './ServerCard.vue'

const router = useRouter()
const { formatMessage } = useVIntl()
const { servers, isRefreshing, hasLoaded, loadError, refresh, stopServer } = useServers()
const { eulaModal, eulaText, tryStartServer, acceptEula, declineEula, resumeInstall } =
    useServerLifecycle()
const createModal = useTemplateRef<ComponentExposed<typeof CreateServerModal>>('createModal')

const messages = defineMessages({
    heading: { id: 'app.servers.management.heading', defaultMessage: 'Your servers' },
    search: {
        id: 'app.servers.management.search',
        defaultMessage: 'Search by name, loader or version',
    },
    noMatches: { id: 'app.servers.management.no-matches', defaultMessage: 'No matching servers' },
    clearSearch: { id: 'app.servers.management.clear-search', defaultMessage: 'Clear search' },
    loadFailed: { id: 'app.servers.load-failed', defaultMessage: 'Failed to load servers' },
    create: { id: 'app.servers.create.title', defaultMessage: 'Create server' },
    refresh: { id: 'app.servers.refresh', defaultMessage: 'Refresh' },
    emptyHeading: {
        id: 'app.servers.empty.heading',
        defaultMessage: 'No servers yet',
    },
    emptyDescription: {
        id: 'app.servers.empty.description',
        defaultMessage: 'Create a server to play with friends, right from the launcher.',
    },
    count: {
        id: 'app.servers.count',
        defaultMessage: '{count, plural, =0 {No servers yet} one {# server} other {# servers}}',
    },
    loading: { id: 'app.servers.loading', defaultMessage: 'Loading servers...' },
    view: { id: 'app.library.view', defaultMessage: 'View' },
    standardView: { id: 'app.library.view.standard', defaultMessage: 'Standard grid' },
    cardsView: { id: 'app.library.view.cards', defaultMessage: 'Library cards' },
})

const searchQuery = ref('')
const filteredServers = computed(() => {
    const query = searchQuery.value.trim().toLocaleLowerCase()
    return servers.value.filter((server) =>
        `${server.name} ${server.serverType} ${server.gameVersion}`
            .toLocaleLowerCase()
            .includes(query),
    )
})

const displayMode = ref(getLastLibraryDisplayMode())
const displayModeOptions = computed(() => [
    { id: 'standard' as const, label: formatMessage(messages.standardView), icon: GridIcon },
    { id: 'cards' as const, label: formatMessage(messages.cardsView), icon: CollectionIcon },
])
const currentDisplayMode = computed(() =>
    displayModeOptions.value.find((option) => option.id === displayMode.value),
)

onMounted(() => {
    void refresh()
})

async function openServer(id: string) {
    // Refresh first so the freshly created server is present in the shared store
    // before ServerDetail mounts; otherwise it briefly shows "server not found".
    await refresh().catch(() => {})
    void router.push('/multiplayer/servers/' + encodeURIComponent(id))
}

function setDisplayMode(mode: 'standard' | 'cards') {
    displayMode.value = mode
    setLastLibraryDisplayMode(mode)
}

async function toggleRunning(server: ServerView) {
    if (server.running || server.starting) {
        await stopServer(server.id)
    } else {
        await tryStartServer(server)
    }
}
</script>

<template>
    <div data-onboarding-id="servers-overview" class="flex min-h-0 w-full flex-1 flex-col gap-4">
        <div class="flex flex-wrap items-start justify-between gap-4">
            <div class="flex min-w-0 flex-col gap-1">
                <h2 class="m-0 text-xl font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.heading) }}
                </h2>
                <span class="text-sm text-[var(--color-text-tertiary)]" role="status">{{
                    loadError && !hasLoaded
                        ? formatMessage(messages.loadFailed)
                        : hasLoaded
                          ? formatMessage(messages.count, { count: servers.length })
                          : formatMessage(messages.loading)
                }}</span>
            </div>
            <div class="flex gap-2">
                <PopoutMenu
                    v-if="servers.length"
                    :tooltip="formatMessage(messages.view)"
                    placement="bottom-end"
                >
                    <Button type="quiet" circular icon-only :label="formatMessage(messages.view)"
                        ><component :is="currentDisplayMode?.icon" />
                    </Button>
                    <template #menu>
                        <div class="flex w-44 flex-col gap-1 p-1">
                            <Button
                                v-for="option in displayModeOptions"
                                :key="option.id"
                                :type="displayMode === option.id ? 'base' : 'quiet'"
                                class="flex w-full items-center !justify-start gap-2 text-left"
                                :aria-pressed="displayMode === option.id"
                                @click="setDisplayMode(option.id)"
                            >
                                <component :is="option.icon" class="size-4" />
                                {{ option.label }}
                            </Button>
                        </div>
                    </template>
                </PopoutMenu>
                <Button type="quiet" :disabled="isRefreshing" @click="refresh()"
                    ><RefreshCwIcon :class="{ 'animate-spin': isRefreshing }" />
                    {{ formatMessage(messages.refresh) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    data-onboarding-id="create-server-button"
                    @click="createModal?.show()"
                    ><PlusIcon />
                    {{ formatMessage(messages.create) }}
                </Button>
            </div>
        </div>

        <StyledInput
            v-if="servers.length"
            v-model="searchQuery"
            :icon="SearchIcon"
            :placeholder="formatMessage(messages.search)"
            :aria-label="formatMessage(messages.search)"
            wrapper-class="w-full max-w-lg"
            clearable
        />

        <Admonition
            v-if="loadError && hasLoaded"
            type="warning"
            :header="formatMessage(messages.loadFailed)"
        >
            <Button type="outlined" :loading="isRefreshing" @click="refresh()">{{
                formatMessage(commonMessages.retryButton)
            }}</Button>
        </Admonition>
        <div
            v-if="!hasLoaded && !loadError"
            aria-hidden="true"
            class="grid w-full grid-cols-[repeat(auto-fill,minmax(min(100%,19rem),1fr))] gap-4"
        >
            <div
                v-for="index in 3"
                :key="index"
                class="rounded-xl border border-solid border-surface-4 bg-surface-2 p-4"
            >
                <div class="flex items-start gap-3 motion-safe:animate-pulse">
                    <div class="size-10 shrink-0 rounded-lg bg-surface-4" />
                    <div class="flex flex-1 flex-col gap-3 py-1">
                        <div class="h-4 w-3/4 rounded bg-surface-4" />
                        <div class="h-3 w-1/2 rounded bg-surface-4" />
                    </div>
                </div>
                <div class="mt-6 h-7 w-1/3 rounded bg-surface-4 motion-safe:animate-pulse" />
            </div>
        </div>
        <EmptyState
            v-else-if="loadError && !hasLoaded"
            type="error"
            :heading="formatMessage(messages.loadFailed)"
        >
            <template #actions>
                <Button type="outlined" :loading="isRefreshing" @click="refresh()">{{
                    formatMessage(commonMessages.retryButton)
                }}</Button>
            </template>
        </EmptyState>
        <EmptyState
            v-else-if="hasLoaded && servers.length === 0 && !isRefreshing"
            type="empty"
            :heading="formatMessage(messages.emptyHeading)"
            :description="formatMessage(messages.emptyDescription)"
        >
        </EmptyState>

        <EmptyState
            v-else-if="servers.length && !filteredServers.length"
            type="empty"
            :heading="formatMessage(messages.noMatches)"
        >
            <template #actions
                ><Button type="outlined" @click="searchQuery = ''">{{
                    formatMessage(messages.clearSearch)
                }}</Button></template
            >
        </EmptyState>
        <div
            v-else
            class="grid w-full auto-rows-fr gap-4"
            :class="
                displayMode === 'cards'
                    ? 'grid-cols-[repeat(auto-fill,minmax(min(100%,14rem),1fr))]'
                    : 'grid-cols-[repeat(auto-fill,minmax(min(100%,19rem),1fr))]'
            "
        >
            <ServerCard
                v-for="entry in filteredServers"
                :key="entry.id"
                :server="entry"
                :variant="displayMode === 'cards' ? 'library' : 'standard'"
                @open="openServer(entry.id)"
                @start-stop="toggleRunning(entry)"
                @resume="resumeInstall(entry)"
            />
        </div>

        <CreateServerModal ref="createModal" @created="openServer" />
        <EulaModal ref="eulaModal" :text="eulaText" @continue="acceptEula" @decline="declineEula" />
    </div>
</template>
