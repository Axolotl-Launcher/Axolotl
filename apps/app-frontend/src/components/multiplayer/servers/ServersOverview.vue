<script setup lang="ts">
import {
    CollectionIcon,
    GridIcon,
    PlusIcon,
    RefreshCwIcon,
    ServerIcon,
    SpinnerIcon,
} from '@modrinth/assets'
import {
    Admonition,
    Button,
    commonMessages,
    defineMessages,
    EmptyState,
    PopoutMenu,
    useVIntl,
} from '@modrinth/ui'
import { computed, onMounted, ref, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'
import { useRouter } from 'vue-router'

import CreateServerModal from '@/components/multiplayer/servers/CreateServerModal.vue'
import EulaModal from '@/components/multiplayer/servers/EulaModal.vue'
import ServerCard from '@/components/multiplayer/servers/ServerCard.vue'
import { useServerLifecycle } from '@/composables/useServerLifecycle'
import { type ServerView, useServers } from '@/composables/useServers'
import {
    getLastLibraryDisplayMode,
    setLastLibraryDisplayMode,
} from '@/helpers/library-display-mode'

const router = useRouter()
const { formatMessage } = useVIntl()
const { servers, isRefreshing, hasLoaded, loadError, refresh, stopServer } = useServers()
const { eulaModal, eulaText, tryStartServer, acceptEula, declineEula, resumeInstall } =
    useServerLifecycle()
const createModal = useTemplateRef<ComponentExposed<typeof CreateServerModal>>('createModal')

const messages = defineMessages({
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
    if (server.status === 'running') {
        await stopServer(server.id)
    } else {
        await tryStartServer(server)
    }
}
</script>

<template>
    <div data-onboarding-id="servers-overview" class="flex min-h-0 w-full flex-1 flex-col gap-4">
        <div class="flex items-center justify-between gap-3">
            <span class="flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]">
                <SpinnerIcon v-if="isRefreshing" class="size-4 animate-spin" />
                <ServerIcon v-else class="size-4" />
                {{
                    isRefreshing
                        ? formatMessage(messages.loading)
                        : loadError && !hasLoaded
                          ? formatMessage(messages.loadFailed)
                          : hasLoaded
                            ? formatMessage(messages.count, { count: servers.length })
                            : formatMessage(messages.loading)
                }}
            </span>
            <div class="flex gap-2">
                <PopoutMenu :tooltip="formatMessage(messages.view)" placement="bottom-end">
                    <Button circular icon-only :aria-label="formatMessage(messages.view)"
                        ><component :is="currentDisplayMode?.icon" />
                    </Button>
                    <template #menu>
                        <div class="flex w-44 flex-col gap-1 p-1">
                            <Button
                                v-for="option in displayModeOptions"
                                :key="option.id"
                                :type="displayMode === option.id ? 'base' : 'quiet'"
                                class="flex w-full items-center gap-2 !justify-start text-left"
                                :aria-pressed="displayMode === option.id"
                                @click="setDisplayMode(option.id)"
                            >
                                <component :is="option.icon" class="size-4" />
                                {{ option.label }}
                            </Button>
                        </div>
                    </template>
                </PopoutMenu>
                <Button type="outlined" :disabled="isRefreshing" @click="refresh()"
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

        <Admonition
            v-if="loadError && hasLoaded"
            type="warning"
            :header="formatMessage(messages.loadFailed)"
        >
            <Button type="outlined" :loading="isRefreshing" @click="refresh()">{{
                formatMessage(commonMessages.retryButton)
            }}</Button>
        </Admonition>
        <EmptyState
            v-if="loadError && !hasLoaded"
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
            <Button type="colored" color="brand" size="xl" @click="createModal?.show()"
                ><ServerIcon />
                {{ formatMessage(messages.create) }}
            </Button>
        </EmptyState>

        <div
            v-else
            class="grid grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] w-full max-w-[72rem] gap-3"
            :class="{
                'grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-4': displayMode === 'cards',
            }"
        >
            <ServerCard
                v-for="entry in servers"
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

<style lang="scss" scoped></style>
