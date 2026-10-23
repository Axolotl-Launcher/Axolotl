<script setup lang="ts">
import CalendarIcon from '@modrinth/assets/icons/calendar.svg?component'
import ChevronLeftIcon from '@modrinth/assets/icons/chevron-left.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import CollectionIcon from '@modrinth/assets/icons/collection.svg?component'
import GameIcon from '@modrinth/assets/icons/game.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import HistoryIcon from '@modrinth/assets/icons/history.svg?component'
import LayoutTemplateIcon from '@modrinth/assets/icons/layout-template.svg?component'
import LinkIcon from '@modrinth/assets/icons/link.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import ServerIcon from '@modrinth/assets/icons/server.svg?component'
import UserIcon from '@modrinth/assets/icons/user.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, ref } from 'vue'

import type { GameInstance } from '../../../lib/types'
import PreviewModal from '../../../overlays/PreviewModal.vue'
import {
    createHomeWidgetId,
    HOME_GREETING_DEFAULT_MODE,
    HOME_RECENT_DEFAULT_LIMIT,
    HOME_WIDGET_DEFAULT_SIZE,
    type HomeWidgetKind,
    type HomeWidgetPlacement,
} from '../home-dashboard'
import type { HomeWorld } from '../home-utils'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import InstanceIcon from './InstanceIcon.vue'

const props = defineProps<{
    instances: GameInstance[]
}>()

const emit = defineEmits<{
    add: [widget: HomeWidgetPlacement]
}>()

const { formatMessage, locale } = useVIntl()
const runtime = useHomeRuntime()
const modal = ref<InstanceType<typeof PreviewModal>>()
const searchInput = ref<InstanceType<typeof StyledInput>>()
const searchQuery = ref('')
const selectedKind = ref<HomeWidgetKind | null>(null)
const selectedInstance = ref<GameInstance | null>(null)
const worlds = ref<HomeWorld[]>([])
const loadingWorlds = ref(false)
const messages = homeMessages

const catalogSections = computed(() => [
    {
        id: 'overview',
        label: formatMessage(messages.widgetsGroupOverview),
        icon: LayoutTemplateIcon,
        items: [
            {
                kind: 'greeting' as const,
                label: formatMessage(messages.widgetsGreeting),
                description: formatMessage(messages.widgetsGreetingDescription),
                icon: UserIcon,
            },
            {
                kind: 'recent' as const,
                label: formatMessage(messages.widgetsRecent),
                description: formatMessage(messages.widgetsRecentDescription),
                icon: HistoryIcon,
            },
            {
                kind: 'calendar' as const,
                label: formatMessage(messages.widgetsCalendar),
                description: formatMessage(messages.widgetsCalendarDescription),
                icon: CalendarIcon,
            },
        ],
    },
    {
        id: 'collections',
        label: formatMessage(messages.widgetsGroupCollections),
        icon: CollectionIcon,
        items: [
            {
                kind: 'pinned-instances' as const,
                label: formatMessage(messages.widgetsPinnedInstances),
                description: formatMessage(messages.widgetsPinnedInstancesDescription),
                icon: GridIcon,
            },
            {
                kind: 'pinned-worlds' as const,
                label: formatMessage(messages.widgetsPinnedWorlds),
                description: formatMessage(messages.widgetsPinnedWorldsDescription),
                icon: GameIcon,
            },
            {
                kind: 'pinned-servers' as const,
                label: formatMessage(messages.widgetsPinnedServers),
                description: formatMessage(messages.widgetsPinnedServersDescription),
                icon: ServerIcon,
            },
        ],
    },
    {
        id: 'shortcuts',
        label: formatMessage(messages.widgetsGroupShortcuts),
        icon: LinkIcon,
        items: [
            {
                kind: 'instance' as const,
                label: formatMessage(messages.widgetsInstance),
                description: formatMessage(messages.widgetsInstanceDescription),
                icon: GridIcon,
            },
            {
                kind: 'world' as const,
                label: formatMessage(messages.widgetsWorld),
                description: formatMessage(messages.widgetsWorldDescription),
                icon: GameIcon,
            },
            {
                kind: 'server' as const,
                label: formatMessage(messages.widgetsServer),
                description: formatMessage(messages.widgetsServerDescription),
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
    if (!selectedKind.value) return formatMessage(messages.widgetsAddTitle)
    if (!selectedInstance.value) return formatMessage(messages.widgetsChooseInstance)
    return formatMessage(
        selectedKind.value === 'server'
            ? messages.widgetsChooseServer
            : messages.widgetsChooseWorld,
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
            id: createHomeWidgetId(),
            kind,
            size: HOME_WIDGET_DEFAULT_SIZE[kind],
            ...(kind === 'recent' ? { options: { recentLimit: HOME_RECENT_DEFAULT_LIMIT } } : {}),
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
            id: createHomeWidgetId(),
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

function chooseWorld(world: HomeWorld) {
    if (!selectedInstance.value || (world.type !== 'server' && world.type !== 'singleplayer'))
        return
    const kind = world.type === 'server' ? 'server' : 'world'
    addWidget({
        id: createHomeWidgetId(),
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
    <PreviewModal
        ref="modal"
        :header="pickerTitle"
        max-width="640px"
        width="min(640px, calc(100% - 2rem))"
        scrollable
        max-content-height="min(38rem, 576px)"
    >
        <div class="flex min-w-0 flex-col gap-4">
            <div v-if="selectedKind" class="flex min-w-0 items-center gap-3">
                <Button
                    v-tooltip="formatMessage(messages.widgetsBack)"
                    type="quiet"
                    size="2xs"
                    circular
                    icon-only
                    :aria-label="formatMessage(messages.widgetsBack)"
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
                    :placeholder="formatMessage(messages.widgetsSearch)"
                    wrapper-class="w-full"
                    clearable
                />
                <p
                    v-if="loadingWorlds"
                    class="m-0 py-8 text-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.widgetsLoading) }}
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
                                    ? chooseWorld(item as HomeWorld)
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
                    {{ formatMessage(messages.widgetsNoResults) }}
                </p>
            </template>
        </div>
    </PreviewModal>
</template>
