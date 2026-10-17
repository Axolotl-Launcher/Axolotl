<script setup lang="ts">
import {
    CalendarIcon,
    ChevronLeftIcon,
    ChevronRightIcon,
    CollectionIcon,
    GameIcon,
    GridIcon,
    HistoryIcon,
    LayoutTemplateIcon,
    LinkIcon,
    SearchIcon,
    ServerIcon,
    UserIcon,
} from '@modrinth/assets'
import { Button, defineMessages, NewModal, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, nextTick, ref } from 'vue'

import type { HomeWidgetKind, HomeWidgetPlacement } from '@/components/home/home-dashboard'
import {
    HOME_ALMANAC_DEFAULT_SHOW_ACTIVITIES,
    HOME_ALMANAC_DEFAULT_SHOW_HOURS,
    HOME_ALMANAC_DEFAULT_TAG_LIMIT,
    HOME_ALMANAC_DEFAULT_TERM_STYLE,
    HOME_GREETING_DEFAULT_MODE,
    HOME_RECENT_DEFAULT_LIMIT,
    HOME_WIDGET_DEFAULT_SIZE,
} from '@/components/home/home-dashboard'
import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import type { GameInstance } from '@/helpers/types'
import type { World } from '@/helpers/worlds'

const props = defineProps<{
    instances: GameInstance[]
}>()

const emit = defineEmits<{
    add: [widget: HomeWidgetPlacement]
}>()

const { formatMessage, locale } = useVIntl()
const runtime = useHomeDashboardRuntime()
const modal = ref<InstanceType<typeof NewModal>>()
const searchInput = ref<InstanceType<typeof StyledInput>>()
const searchQuery = ref('')
const selectedKind = ref<HomeWidgetKind | null>(null)
const selectedInstance = ref<GameInstance | null>(null)
const worlds = ref<World[]>([])
const loadingWorlds = ref(false)

const messages = defineMessages({
    title: { id: 'app.home.widgets.add-title', defaultMessage: 'Add widget' },
    search: { id: 'app.home.widgets.search', defaultMessage: 'Search' },
    back: { id: 'app.home.widgets.back', defaultMessage: 'Back' },
    noResults: { id: 'app.home.widgets.no-results', defaultMessage: 'No matching items' },
    loading: { id: 'app.home.widgets.loading', defaultMessage: 'Loading...' },
    overviewGroup: { id: 'app.home.widgets.group.overview', defaultMessage: 'Overview' },
    collectionsGroup: {
        id: 'app.home.widgets.group.collections',
        defaultMessage: 'Pinned collections',
    },
    shortcutsGroup: {
        id: 'app.home.widgets.group.shortcuts',
        defaultMessage: 'Single-item shortcuts',
    },
    greeting: { id: 'app.home.widgets.greeting', defaultMessage: 'Greeting' },
    greetingDescription: {
        id: 'app.home.widgets.greeting-description',
        defaultMessage: 'A personal welcome that changes throughout the day.',
    },
    recent: { id: 'app.home.widgets.recent', defaultMessage: 'Recently played' },
    recentDescription: {
        id: 'app.home.widgets.recent-description',
        defaultMessage: 'Resume the worlds and instances you played most recently.',
    },
    calendar: { id: 'app.home.widgets.calendar', defaultMessage: 'Calendar' },
    calendarDescription: {
        id: 'app.home.widgets.calendar-description',
        defaultMessage: 'See the month and your play activity at a glance.',
    },
    almanac: { id: 'app.home.widgets.almanac', defaultMessage: 'Almanac' },
    almanacDescription: {
        id: 'app.home.widgets.almanac-description',
        defaultMessage:
            'Traditional Chinese calendar: auspicious and inauspicious activities for the day.',
    },
    pinnedInstances: {
        id: 'app.home.widgets.pinned-instances',
        defaultMessage: 'All pinned instances',
    },
    pinnedInstancesDescription: {
        id: 'app.home.widgets.pinned-instances-description',
        defaultMessage: 'Automatically collects every instance pinned to Home.',
    },
    pinnedWorlds: {
        id: 'app.home.widgets.pinned-worlds',
        defaultMessage: 'All favorite worlds',
    },
    pinnedWorldsDescription: {
        id: 'app.home.widgets.pinned-worlds-description',
        defaultMessage: 'Automatically collects favorite singleplayer worlds.',
    },
    pinnedServers: {
        id: 'app.home.widgets.pinned-servers',
        defaultMessage: 'All favorite servers',
    },
    pinnedServersDescription: {
        id: 'app.home.widgets.pinned-servers-description',
        defaultMessage: 'Automatically collects favorite multiplayer servers.',
    },
    instance: { id: 'app.home.widgets.instance', defaultMessage: 'Single instance' },
    instanceDescription: {
        id: 'app.home.widgets.instance-description',
        defaultMessage: 'Choose one instance for a dedicated launch shortcut.',
    },
    world: { id: 'app.home.widgets.world', defaultMessage: 'Single world' },
    worldDescription: {
        id: 'app.home.widgets.world-description',
        defaultMessage: 'Choose one world for a dedicated play shortcut.',
    },
    server: { id: 'app.home.widgets.server', defaultMessage: 'Single server' },
    serverDescription: {
        id: 'app.home.widgets.server-description',
        defaultMessage: 'Choose one server for a dedicated join shortcut.',
    },
    chooseInstance: {
        id: 'app.home.widgets.choose-instance',
        defaultMessage: 'Choose an instance',
    },
    chooseWorld: { id: 'app.home.widgets.choose-world', defaultMessage: 'Choose a world' },
    chooseServer: { id: 'app.home.widgets.choose-server', defaultMessage: 'Choose a server' },
})

const catalogSections = computed(() => [
    {
        id: 'overview',
        label: formatMessage(messages.overviewGroup),
        icon: LayoutTemplateIcon,
        items: [
            {
                kind: 'greeting' as const,
                label: formatMessage(messages.greeting),
                description: formatMessage(messages.greetingDescription),
                icon: UserIcon,
            },
            {
                kind: 'recent' as const,
                label: formatMessage(messages.recent),
                description: formatMessage(messages.recentDescription),
                icon: HistoryIcon,
            },
            {
                kind: 'calendar' as const,
                label: formatMessage(messages.calendar),
                description: formatMessage(messages.calendarDescription),
                icon: CalendarIcon,
            },
            {
                kind: 'almanac' as const,
                label: formatMessage(messages.almanac),
                description: formatMessage(messages.almanacDescription),
                icon: CalendarIcon,
            },
        ],
    },
    {
        id: 'collections',
        label: formatMessage(messages.collectionsGroup),
        icon: CollectionIcon,
        items: [
            {
                kind: 'pinned-instances' as const,
                label: formatMessage(messages.pinnedInstances),
                description: formatMessage(messages.pinnedInstancesDescription),
                icon: GridIcon,
            },
            {
                kind: 'pinned-worlds' as const,
                label: formatMessage(messages.pinnedWorlds),
                description: formatMessage(messages.pinnedWorldsDescription),
                icon: GameIcon,
            },
            {
                kind: 'pinned-servers' as const,
                label: formatMessage(messages.pinnedServers),
                description: formatMessage(messages.pinnedServersDescription),
                icon: ServerIcon,
            },
        ],
    },
    {
        id: 'shortcuts',
        label: formatMessage(messages.shortcutsGroup),
        icon: LinkIcon,
        items: [
            {
                kind: 'instance' as const,
                label: formatMessage(messages.instance),
                description: formatMessage(messages.instanceDescription),
                icon: GridIcon,
            },
            {
                kind: 'world' as const,
                label: formatMessage(messages.world),
                description: formatMessage(messages.worldDescription),
                icon: GameIcon,
            },
            {
                kind: 'server' as const,
                label: formatMessage(messages.server),
                description: formatMessage(messages.serverDescription),
                icon: ServerIcon,
            },
        ],
    },
])

const filteredInstances = computed(() => {
    const query = searchQuery.value.trim().toLocaleLowerCase(locale.value)
    return props.instances.filter((instance) =>
        query ? instance.name.toLocaleLowerCase(locale.value).includes(query) : true,
    )
})

const filteredWorlds = computed(() => {
    const type = selectedKind.value === 'server' ? 'server' : 'singleplayer'
    const query = searchQuery.value.trim().toLocaleLowerCase(locale.value)
    return worlds.value.filter(
        (world) =>
            world.type === type &&
            (!query || world.name.toLocaleLowerCase(locale.value).includes(query)),
    )
})

const pickerTitle = computed(() => {
    if (!selectedKind.value) return formatMessage(messages.title)
    if (!selectedInstance.value) return formatMessage(messages.chooseInstance)
    return formatMessage(
        selectedKind.value === 'server' ? messages.chooseServer : messages.chooseWorld,
    )
})

function show(kind: HomeWidgetKind | null = null) {
    selectedKind.value = kind
    selectedInstance.value = null
    worlds.value = []
    searchQuery.value = ''
    modal.value?.show()
    if (kind) void nextTick(() => searchInput.value?.focus())
}

function addWidget(widget: HomeWidgetPlacement) {
    emit('add', widget)
    modal.value?.hide()
}

function chooseKind(kind: HomeWidgetKind) {
    if (kind !== 'instance' && kind !== 'world' && kind !== 'server') {
        addWidget({
            id: crypto.randomUUID(),
            kind,
            size: HOME_WIDGET_DEFAULT_SIZE[kind],
            ...(kind === 'recent' ? { options: { recentLimit: HOME_RECENT_DEFAULT_LIMIT } } : {}),
            ...(kind === 'almanac'
                ? {
                      options: {
                          almanacShowHours: HOME_ALMANAC_DEFAULT_SHOW_HOURS,
                          almanacShowActivities: HOME_ALMANAC_DEFAULT_SHOW_ACTIVITIES,
                          almanacTagLimit: HOME_ALMANAC_DEFAULT_TAG_LIMIT,
                          almanacTermStyle: HOME_ALMANAC_DEFAULT_TERM_STYLE,
                      },
                  }
                : {}),
            ...(kind === 'greeting'
                ? { options: { greetingMode: HOME_GREETING_DEFAULT_MODE } }
                : {}),
        })
        return
    }
    selectedKind.value = kind
    searchQuery.value = ''
    void nextTick(() => searchInput.value?.focus())
}

async function chooseInstance(instance: GameInstance) {
    if (selectedKind.value === 'instance') {
        addWidget({
            id: crypto.randomUUID(),
            kind: 'instance',
            size: HOME_WIDGET_DEFAULT_SIZE.instance,
            target: { instanceId: instance.id, fallbackLabel: instance.name },
        })
        return
    }

    selectedInstance.value = instance
    searchQuery.value = ''
    loadingWorlds.value = true
    worlds.value = await runtime.getInstanceWorlds(instance.id)
    loadingWorlds.value = false
}

function chooseWorld(world: World) {
    if (!selectedInstance.value || (world.type !== 'server' && world.type !== 'singleplayer'))
        return
    const kind = world.type === 'server' ? 'server' : 'world'
    addWidget({
        id: crypto.randomUUID(),
        kind,
        size: HOME_WIDGET_DEFAULT_SIZE[kind],
        target: {
            instanceId: selectedInstance.value.id,
            ...(world.type === 'server' ? { address: world.address } : { path: world.path }),
            fallbackLabel: world.name,
        },
    })
}

function goBack() {
    searchQuery.value = ''
    if (selectedInstance.value) {
        selectedInstance.value = null
        worlds.value = []
    } else {
        selectedKind.value = null
    }
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="pickerTitle"
        max-width="640px"
        width="min(640px, calc(100vw - 2rem))"
        scrollable
        max-content-height="min(38rem, 72vh)"
    >
        <div class="flex min-w-0 flex-col gap-4">
            <div v-if="selectedKind" class="flex min-w-0 items-center gap-3">
                <Button
                    v-tooltip="formatMessage(messages.back)"
                    type="quiet"
                    size="2xs"
                    circular
                    icon-only
                    :aria-label="formatMessage(messages.back)"
                    @click="goBack"
                    ><ChevronLeftIcon />
                </Button>
                <div v-if="selectedInstance" class="flex min-w-0 items-center gap-2">
                    <InstanceIcon
                        class="size-8 shrink-0"
                        :icon-path="selectedInstance.icon_path"
                        :instance-id="selectedInstance.id"
                        :loader="selectedInstance.loader"
                    />
                    <span class="truncate text-sm font-semibold text-[var(--color-text-primary)]">{{
                        selectedInstance.name
                    }}</span>
                </div>
            </div>

            <div v-if="!selectedKind" class="flex min-w-0 flex-col gap-5">
                <section v-for="section in catalogSections" :key="section.id" class="min-w-0">
                    <h3
                        class="mb-2 mt-0 flex items-center gap-2 px-1 text-sm font-semibold text-[var(--color-text-tertiary)]"
                    >
                        <component :is="section.icon" class="size-4" aria-hidden="true" />
                        {{ section.label }}
                    </h3>
                    <div
                        class="overflow-hidden rounded-lg border border-solid border-divider bg-surface-2"
                    >
                        <button
                            v-for="item in section.items"
                            :key="item.kind"
                            type="button"
                            class="group flex min-h-16 w-full cursor-pointer items-center gap-3 border-0 border-b border-solid border-divider bg-transparent px-3 py-2 text-left text-[var(--color-text-default)] transition-colors last:border-b-0 hover:bg-surface-4 focus-visible:z-[1] focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                            @click="chooseKind(item.kind)"
                        >
                            <span
                                class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-surface-4 text-[var(--color-text-tertiary)] transition-colors group-hover:text-brand"
                            >
                                <component :is="item.icon" class="size-5" aria-hidden="true" />
                            </span>
                            <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                                <strong class="truncate text-sm text-[var(--color-text-primary)]">{{
                                    item.label
                                }}</strong>
                                <span
                                    class="line-clamp-2 text-xs leading-5 text-[var(--color-text-tertiary)]"
                                    >{{ item.description }}</span
                                >
                            </span>
                            <ChevronRightIcon
                                class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                                aria-hidden="true"
                            />
                        </button>
                    </div>
                </section>
            </div>

            <template v-else>
                <StyledInput
                    ref="searchInput"
                    v-model="searchQuery"
                    type="search"
                    :icon="SearchIcon"
                    :placeholder="formatMessage(messages.search)"
                    wrapper-class="w-full"
                    clearable
                />
                <p
                    v-if="loadingWorlds"
                    class="m-0 py-8 text-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.loading) }}
                </p>
                <ul
                    v-else
                    class="m-0 flex list-none flex-col overflow-hidden rounded-lg border border-solid border-divider bg-surface-2 p-0"
                >
                    <li
                        v-for="item in selectedInstance ? filteredWorlds : filteredInstances"
                        :key="
                            'id' in item
                                ? item.id
                                : item.type === 'server'
                                  ? item.address
                                  : item.path
                        "
                        class="min-w-0 border-0 border-b border-solid border-divider last:border-b-0"
                    >
                        <button
                            type="button"
                            class="flex min-h-16 w-full cursor-pointer items-center gap-3 border-0 bg-transparent px-3 py-2 text-left transition-colors hover:bg-surface-4 focus-visible:z-[1] focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                            @click="
                                selectedInstance
                                    ? chooseWorld(item as World)
                                    : chooseInstance(item as GameInstance)
                            "
                        >
                            <InstanceIcon
                                v-if="'id' in item"
                                class="size-9 shrink-0"
                                :icon-path="item.icon_path"
                                :instance-id="item.id"
                                :loader="item.loader"
                            />
                            <ServerIcon
                                v-else-if="item.type === 'server'"
                                class="size-5 shrink-0"
                            />
                            <GameIcon v-else class="size-5 shrink-0" />
                            <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                                <strong class="truncate text-sm text-[var(--color-text-primary)]">{{
                                    item.name
                                }}</strong>
                                <span
                                    v-if="'id' in item"
                                    class="truncate text-xs capitalize text-[var(--color-text-tertiary)]"
                                >
                                    {{ item.loader }} · {{ item.game_version }}
                                </span>
                                <span
                                    v-else-if="item.type === 'server'"
                                    class="truncate text-xs text-[var(--color-text-tertiary)]"
                                >
                                    {{ item.address }}
                                </span>
                                <span
                                    v-else
                                    class="truncate text-xs text-[var(--color-text-tertiary)]"
                                >
                                    {{ selectedInstance?.name }}
                                </span>
                            </span>
                            <ChevronRightIcon
                                class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                                aria-hidden="true"
                            />
                        </button>
                    </li>
                </ul>
                <p
                    v-if="
                        !loadingWorlds &&
                        (selectedInstance
                            ? filteredWorlds.length === 0
                            : filteredInstances.length === 0)
                    "
                    class="m-0 py-8 text-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noResults) }}
                </p>
            </template>
        </div>
    </NewModal>
</template>
