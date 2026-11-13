<script setup lang="ts">
import {
    CheckIcon,
    ChevronLeftIcon,
    GameIcon,
    PlusIcon,
    SearchIcon,
    ServerIcon,
} from '@modrinth/assets'
import { Button, defineMessages, NewModal, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, nextTick, reactive, ref } from 'vue'

import type {
    HomeWidgetKind,
    HomeWidgetPlacement,
    HomeWidgetSize,
    HomeWidgetTarget,
} from '@/components/home/home-dashboard'
import { HOME_WIDGET_SIZE_OPTIONS } from '@/components/home/home-dashboard'
import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import {
    countHomeWidgets,
    createHomeWidget,
    filterHomeWidgetCatalog,
    HOME_WIDGET_CATALOG,
    HOME_WIDGET_CATEGORIES,
    type HomeWidgetCategory,
    homeWidgetNeedsTarget,
    homeWidgetPreviewSize,
    resolveHomeWidgetSize,
} from '@/components/home/home-widget-catalog'
import HomeWidgetPreview from '@/components/home/HomeWidgetPreview.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import type { GameInstance } from '@/helpers/types'
import type { World } from '@/helpers/worlds'

/*
 * The widget picker. The catalog on the left shows every widget as a scaled
 * copy of the real thing, grouped and searchable, with what is already on the
 * Home marked. The panel on the right shows the chosen widget larger, its
 * sizes and, for a shortcut, what it should open. Adding keeps the picker
 * open and says what happened, with an undo, so several widgets can be added
 * in one go. Opened to replace a shortcut's target, it shows only the panel.
 */

const props = withDefaults(
    defineProps<{
        instances: GameInstance[]
        widgets?: HomeWidgetPlacement[]
        playerName?: string | null
    }>(),
    { widgets: () => [], playerName: null },
)

const emit = defineEmits<{
    add: [widget: HomeWidgetPlacement]
    remove: [id: string]
}>()

const { formatMessage, locale } = useVIntl()
const runtime = useHomeDashboardRuntime()

const messages = defineMessages({
    title: { id: 'app.home.widgets.add-title', defaultMessage: 'Add widget' },
    search: { id: 'app.home.widgets.search', defaultMessage: 'Search' },
    searchWidgets: { id: 'app.home.widgets.search-widgets', defaultMessage: 'Search widgets' },
    back: { id: 'app.home.widgets.back', defaultMessage: 'Back' },
    noResults: { id: 'app.home.widgets.no-results', defaultMessage: 'No matching items' },
    noWidgets: { id: 'app.home.widgets.no-widgets', defaultMessage: 'No widgets match' },
    loading: { id: 'app.home.widgets.loading', defaultMessage: 'Loading...' },
    categoryAll: { id: 'app.home.widgets.category.all', defaultMessage: 'All' },
    categoryStart: { id: 'app.home.widgets.category.start', defaultMessage: 'Start playing' },
    categoryActivity: { id: 'app.home.widgets.category.activity', defaultMessage: 'Play data' },
    categoryCollections: {
        id: 'app.home.widgets.category.collections',
        defaultMessage: 'Collections',
    },
    categoryShortcuts: { id: 'app.home.widgets.category.shortcuts', defaultMessage: 'Shortcuts' },
    categories: { id: 'app.home.widgets.categories', defaultMessage: 'Widget categories' },
    added: {
        id: 'app.home.widgets.added-count',
        defaultMessage: '{count, plural, =1 {Added} other {Added ×#}}',
    },
    quickAdd: { id: 'app.home.widgets.quick-add', defaultMessage: 'Add {name}' },
    sizeLabel: { id: 'app.home.widgets.size-label', defaultMessage: 'Size' },
    size: { id: 'app.home.widgets.size', defaultMessage: 'Size {size}' },
    addToHome: { id: 'app.home.widgets.add-to-home', defaultMessage: 'Add to Home' },
    addAnother: { id: 'app.home.widgets.add-another', defaultMessage: 'Add another' },
    addedToast: { id: 'app.home.widgets.added-toast', defaultMessage: 'Added {name} to your Home' },
    undo: { id: 'app.home.widgets.undo', defaultMessage: 'Undo' },
    done: { id: 'app.home.widgets.done-adding', defaultMessage: 'Done' },
    replace: { id: 'app.home.widgets.replace', defaultMessage: 'Replace target' },
    chooseTarget: {
        id: 'app.home.widgets.choose-target',
        defaultMessage: 'Pick what it opens',
    },
    chooseInstance: {
        id: 'app.home.widgets.choose-instance',
        defaultMessage: 'Choose an instance',
    },
    chooseWorld: { id: 'app.home.widgets.choose-world', defaultMessage: 'Choose a world' },
    chooseServer: { id: 'app.home.widgets.choose-server', defaultMessage: 'Choose a server' },
    launch: { id: 'app.home.widgets.launch', defaultMessage: 'Launch' },
    launchDescription: {
        id: 'app.home.widgets.launch-description',
        defaultMessage: 'Start your Home instance, or switch to another one.',
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
    pinnedInstances: {
        id: 'app.home.widgets.pinned-instances',
        defaultMessage: 'All pinned instances',
    },
    pinnedInstancesDescription: {
        id: 'app.home.widgets.pinned-instances-description',
        defaultMessage: 'Automatically collects every instance pinned to Home.',
    },
    pinnedWorlds: { id: 'app.home.widgets.pinned-worlds', defaultMessage: 'All favorite worlds' },
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
})

const kindText = {
    launch: [messages.launch, messages.launchDescription],
    greeting: [messages.greeting, messages.greetingDescription],
    recent: [messages.recent, messages.recentDescription],
    calendar: [messages.calendar, messages.calendarDescription],
    'pinned-instances': [messages.pinnedInstances, messages.pinnedInstancesDescription],
    'pinned-worlds': [messages.pinnedWorlds, messages.pinnedWorldsDescription],
    'pinned-servers': [messages.pinnedServers, messages.pinnedServersDescription],
    instance: [messages.instance, messages.instanceDescription],
    world: [messages.world, messages.worldDescription],
    server: [messages.server, messages.serverDescription],
} as const

const categoryText = {
    all: messages.categoryAll,
    start: messages.categoryStart,
    activity: messages.categoryActivity,
    collections: messages.categoryCollections,
    shortcuts: messages.categoryShortcuts,
} as const

const modal = ref<InstanceType<typeof NewModal>>()
const catalogGrid = ref<HTMLElement>()
const targetSection = ref<HTMLElement>()
const mode = ref<'add' | 'replace'>('add')
const query = ref('')
const category = ref<HomeWidgetCategory | 'all'>('all')
const selectedKind = ref<HomeWidgetKind>('launch')
const sizes = reactive<Partial<Record<HomeWidgetKind, HomeWidgetSize>>>({})
const targetQuery = ref('')
const targetInstance = ref<GameInstance | null>(null)
const target = ref<HomeWidgetTarget | null>(null)
const worlds = ref<World[]>([])
const loadingWorlds = ref(false)
const lastAdded = ref<{ id: string; name: string } | null>(null)
/** The kind whose card just took a widget, for a moment of feedback on it. */
const flashKind = ref<HomeWidgetKind | null>(null)
let flashTimer: ReturnType<typeof setTimeout> | undefined

function kindLabel(kind: HomeWidgetKind) {
    return formatMessage(kindText[kind][0])
}

const catalog = computed(() =>
    HOME_WIDGET_CATALOG.map((entry) => ({
        ...entry,
        label: kindLabel(entry.kind),
        description: formatMessage(kindText[entry.kind][1]),
    })),
)
const filtered = computed(() =>
    filterHomeWidgetCatalog(catalog.value, query.value, category.value, locale.value),
)
/** With every category shown the catalog keeps its headings; a filtered one is one grid. */
const sections = computed(() =>
    category.value === 'all' && !query.value.trim()
        ? HOME_WIDGET_CATEGORIES.map((id) => ({
              id,
              label: formatMessage(categoryText[id]),
              items: filtered.value.filter((entry) => entry.category === id),
          }))
        : [{ id: 'results', label: null, items: filtered.value }],
)
const categoryIds = ['all', ...HOME_WIDGET_CATEGORIES] as const
const counts = computed(() => countHomeWidgets(props.widgets))

const selected = computed(
    () => catalog.value.find((entry) => entry.kind === selectedKind.value) ?? catalog.value[0],
)
const selectedSize = computed(() => sizeFor(selectedKind.value))
/** The large preview's box takes the chosen size's proportions, so the widget fills it. */
const stageAspect = computed(() => {
    const natural = homeWidgetPreviewSize(selectedSize.value)
    return natural.width / natural.height
})
const needsTarget = computed(() => homeWidgetNeedsTarget(selectedKind.value))

function sizeFor(kind: HomeWidgetKind): HomeWidgetSize {
    return resolveHomeWidgetSize(kind, sizes[kind])
}

/** What a shortcut card shows before anything is picked: the user's own first match. */
function sampleTarget(kind: HomeWidgetKind): HomeWidgetTarget | null {
    if (kind === 'instance') {
        const instance = props.instances[0]
        return instance ? { instanceId: instance.id, fallbackLabel: instance.name } : null
    }
    const world = runtime.favoriteWorlds.value.find((candidate) =>
        kind === 'server' ? candidate.type === 'server' : candidate.type === 'singleplayer',
    )
    if (!world) return null
    return world.type === 'server'
        ? { instanceId: world.instance_id, address: world.address, fallbackLabel: world.name }
        : { instanceId: world.instance_id, path: world.path, fallbackLabel: world.name }
}

const previewTarget = computed(() =>
    needsTarget.value ? (target.value ?? sampleTarget(selectedKind.value)) : null,
)

const targetHeading = computed(() => {
    if (selectedKind.value === 'instance' || !targetInstance.value) {
        return formatMessage(messages.chooseInstance)
    }
    return formatMessage(
        selectedKind.value === 'server' ? messages.chooseServer : messages.chooseWorld,
    )
})
const showingWorlds = computed(() => selectedKind.value !== 'instance' && !!targetInstance.value)
const targetInstances = computed(() => {
    const needle = targetQuery.value.trim().toLocaleLowerCase(locale.value)
    return props.instances.filter(
        (instance) => !needle || instance.name.toLocaleLowerCase(locale.value).includes(needle),
    )
})
const targetWorlds = computed(() => {
    const type = selectedKind.value === 'server' ? 'server' : 'singleplayer'
    const needle = targetQuery.value.trim().toLocaleLowerCase(locale.value)
    return worlds.value.filter(
        (world) =>
            world.type === type &&
            (!needle || world.name.toLocaleLowerCase(locale.value).includes(needle)),
    )
})

function resetTarget() {
    targetInstance.value = null
    target.value = null
    worlds.value = []
    targetQuery.value = ''
}

function show(kind: HomeWidgetKind | null = null, replace = false) {
    mode.value = replace ? 'replace' : 'add'
    query.value = ''
    category.value = 'all'
    lastAdded.value = null
    flashKind.value = null
    selectedKind.value = kind ?? 'launch'
    resetTarget()
    modal.value?.show()
    void nextTick(() => {
        if (replace) focusTargets()
        else
            catalogGrid.value
                ?.querySelector<HTMLElement>('.home-picker-card-main')
                ?.focus({ preventScroll: true })
    })
}

function hide() {
    modal.value?.hide()
}

function select(kind: HomeWidgetKind) {
    if (kind !== selectedKind.value) resetTarget()
    selectedKind.value = kind
}

function focusTargets() {
    void nextTick(() =>
        targetSection.value?.querySelector<HTMLElement>('.home-picker-target-option')?.focus(),
    )
}

function add(kind: HomeWidgetKind) {
    if (homeWidgetNeedsTarget(kind) && (kind !== selectedKind.value || !target.value)) {
        select(kind)
        focusTargets()
        return
    }
    const widget = createHomeWidget(
        kind,
        sizeFor(kind),
        homeWidgetNeedsTarget(kind) ? target.value : null,
    )
    emit('add', widget)
    if (mode.value === 'replace') {
        hide()
        return
    }
    const name =
        homeWidgetNeedsTarget(kind) && target.value ? target.value.fallbackLabel : kindLabel(kind)
    lastAdded.value = { id: widget.id, name }
    flashKind.value = kind
    clearTimeout(flashTimer)
    flashTimer = setTimeout(() => (flashKind.value = null), 1400)
    if (homeWidgetNeedsTarget(kind)) resetTarget()
}

function undo() {
    if (!lastAdded.value) return
    emit('remove', lastAdded.value.id)
    lastAdded.value = null
    flashKind.value = null
}

async function chooseInstance(instance: GameInstance) {
    if (selectedKind.value === 'instance') {
        target.value = { instanceId: instance.id, fallbackLabel: instance.name }
        return
    }
    targetInstance.value = instance
    target.value = null
    targetQuery.value = ''
    loadingWorlds.value = true
    try {
        worlds.value = await runtime.getInstanceWorlds(instance.id)
    } finally {
        loadingWorlds.value = false
    }
    focusTargets()
}

function chooseWorld(world: World) {
    if (!targetInstance.value) return
    target.value =
        world.type === 'server'
            ? {
                  instanceId: targetInstance.value.id,
                  address: world.address,
                  fallbackLabel: world.name,
              }
            : { instanceId: targetInstance.value.id, path: world.path, fallbackLabel: world.name }
}

function isChosenWorld(world: World) {
    if (!target.value) return false
    return world.type === 'server'
        ? target.value.address === world.address
        : target.value.path === world.path
}

function backToInstances() {
    targetInstance.value = null
    target.value = null
    worlds.value = []
    targetQuery.value = ''
    focusTargets()
}

/** Arrow keys move between the catalog's cards, row by row as they are laid out. */
function moveInCatalog(event: KeyboardEvent) {
    const step = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: 'up', ArrowDown: 'down' }[event.key]
    if (!step || !catalogGrid.value) return
    const cards = [...catalogGrid.value.querySelectorAll<HTMLElement>('.home-picker-card-main')]
    const index = cards.indexOf(document.activeElement as HTMLElement)
    if (index < 0) return
    event.preventDefault()
    let next = index
    if (typeof step === 'number') {
        next = index + step
    } else {
        // The card straight above or below: the nearest one in the next row.
        const here = cards[index].getBoundingClientRect()
        const candidates = cards
            .map((card, position) => ({ position, box: card.getBoundingClientRect() }))
            .filter(({ box }) =>
                step === 'down' ? box.top > here.bottom - 1 : box.bottom < here.top + 1,
            )
        candidates.sort(
            (a, b) =>
                Math.abs(a.box.top - here.top) - Math.abs(b.box.top - here.top) ||
                Math.abs(a.box.left - here.left) - Math.abs(b.box.left - here.left),
        )
        next = candidates[0]?.position ?? index
    }
    cards[Math.max(0, Math.min(cards.length - 1, next))]?.focus()
}

/** The size buttons are one radio group: arrow keys change the size. */
function moveSize(event: KeyboardEvent) {
    const options = HOME_WIDGET_SIZE_OPTIONS[selectedKind.value]
    const delta = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 }[event.key]
    if (!delta || options.length < 2) return
    event.preventDefault()
    const index = options.indexOf(selectedSize.value)
    const next = options[(index + delta + options.length) % options.length]
    sizes[selectedKind.value] = next
    void nextTick(() =>
        (event.currentTarget as HTMLElement | null)
            ?.querySelector<HTMLElement>(`[data-size="${next}"]`)
            ?.focus(),
    )
}

/** A small drawing of a size: its columns and rows as cells. */
function sizeCells(size: HomeWidgetSize) {
    const [columns, rows] = size.split('x').map(Number)
    return { columns, rows }
}

defineExpose({ show, hide })
</script>

<template>
    <NewModal
        ref="modal"
        :header="
            mode === 'replace' ? formatMessage(messages.replace) : formatMessage(messages.title)
        "
        :max-width="mode === 'replace' ? '460px' : '1000px'"
        :width="
            mode === 'replace'
                ? 'min(460px, calc(100vw - 2rem))'
                : 'min(1000px, calc(100vw - 2rem))'
        "
        no-padding
    >
        <div class="home-picker" :data-mode="mode">
            <div v-if="mode === 'add'" class="home-picker-catalog">
                <div class="home-picker-toolbar">
                    <StyledInput
                        v-model="query"
                        type="search"
                        :icon="SearchIcon"
                        :placeholder="formatMessage(messages.searchWidgets)"
                        wrapper-class="home-picker-search"
                        clearable
                    />
                    <div
                        class="home-picker-categories"
                        role="radiogroup"
                        :aria-label="formatMessage(messages.categories)"
                    >
                        <button
                            v-for="id in categoryIds"
                            :key="id"
                            type="button"
                            role="radio"
                            class="home-picker-category"
                            :aria-checked="category === id"
                            @click="category = id"
                        >
                            {{ formatMessage(categoryText[id]) }}
                        </button>
                    </div>
                </div>
                <div ref="catalogGrid" class="home-picker-scroll" @keydown="moveInCatalog">
                    <p v-if="filtered.length === 0" class="home-picker-none">
                        {{ formatMessage(messages.noWidgets) }}
                    </p>
                    <template v-for="section in sections" :key="section.id">
                        <section v-if="section.items.length" class="home-picker-section">
                            <h3 v-if="section.label" class="home-picker-section-title">
                                {{ section.label }}
                            </h3>
                            <ul class="home-picker-grid">
                                <li
                                    v-for="item in section.items"
                                    :key="item.kind"
                                    class="home-picker-card"
                                    :class="{
                                        'is-selected': item.kind === selectedKind,
                                        'is-flashing': item.kind === flashKind,
                                    }"
                                    :data-kind="item.kind"
                                >
                                    <button
                                        type="button"
                                        class="home-picker-card-main"
                                        :aria-pressed="item.kind === selectedKind"
                                        @click="select(item.kind)"
                                        @dblclick="add(item.kind)"
                                    >
                                        <span class="home-picker-card-stage">
                                            <HomeWidgetPreview
                                                :kind="item.kind"
                                                :size="sizeFor(item.kind)"
                                                :instances="instances"
                                                :player-name="playerName"
                                                :target="
                                                    item.kind === selectedKind && target
                                                        ? target
                                                        : sampleTarget(item.kind)
                                                "
                                            />
                                        </span>
                                        <span class="home-picker-card-name">
                                            <span class="truncate">{{ item.label }}</span>
                                            <span
                                                v-if="counts[item.kind]"
                                                class="home-picker-card-added"
                                            >
                                                <CheckIcon aria-hidden="true" />
                                                {{
                                                    formatMessage(messages.added, {
                                                        count: counts[item.kind],
                                                    })
                                                }}
                                            </span>
                                        </span>
                                        <span class="home-picker-card-description">{{
                                            item.description
                                        }}</span>
                                    </button>
                                    <button
                                        v-tooltip="
                                            formatMessage(messages.quickAdd, { name: item.label })
                                        "
                                        type="button"
                                        class="home-picker-card-add"
                                        :aria-label="
                                            formatMessage(messages.quickAdd, { name: item.label })
                                        "
                                        @click="add(item.kind)"
                                    >
                                        <CheckIcon
                                            v-if="item.kind === flashKind"
                                            aria-hidden="true"
                                        />
                                        <PlusIcon v-else aria-hidden="true" />
                                    </button>
                                </li>
                            </ul>
                        </section>
                    </template>
                </div>
            </div>

            <aside class="home-picker-detail">
                <div class="home-picker-detail-scroll">
                    <div class="home-picker-detail-copy">
                        <h3>{{ selected.label }}</h3>
                        <p>{{ selected.description }}</p>
                    </div>

                    <div
                        v-if="mode === 'add' && HOME_WIDGET_SIZE_OPTIONS[selectedKind].length > 1"
                        class="home-picker-field"
                    >
                        <span :id="'home-picker-size-label'" class="home-picker-field-label">{{
                            formatMessage(messages.sizeLabel)
                        }}</span>
                        <div
                            class="home-picker-sizes"
                            role="radiogroup"
                            aria-labelledby="home-picker-size-label"
                            @keydown="moveSize"
                        >
                            <button
                                v-for="size in HOME_WIDGET_SIZE_OPTIONS[selectedKind]"
                                :key="size"
                                type="button"
                                role="radio"
                                class="home-picker-size"
                                :data-size="size"
                                :aria-checked="size === selectedSize"
                                :aria-label="formatMessage(messages.size, { size })"
                                :tabindex="size === selectedSize ? 0 : -1"
                                @click="sizes[selectedKind] = size"
                            >
                                <span
                                    class="home-picker-size-glyph"
                                    :style="{
                                        gridTemplateColumns: `repeat(${sizeCells(size).columns}, 0.5rem)`,
                                        gridTemplateRows: `repeat(${sizeCells(size).rows}, 0.375rem)`,
                                    }"
                                    aria-hidden="true"
                                >
                                    <span
                                        v-for="cell in sizeCells(size).columns *
                                        sizeCells(size).rows"
                                        :key="cell"
                                    />
                                </span>
                                {{ size.replace('x', '×') }}
                            </button>
                        </div>
                    </div>

                    <div
                        class="home-picker-stage"
                        :style="{ '--home-picker-stage-aspect': stageAspect }"
                    >
                        <HomeWidgetPreview
                            :kind="selectedKind"
                            :size="selectedSize"
                            :instances="instances"
                            :player-name="playerName"
                            :target="previewTarget"
                        />
                    </div>

                    <div v-if="needsTarget" ref="targetSection" class="home-picker-field">
                        <span class="home-picker-field-label">{{
                            formatMessage(messages.chooseTarget)
                        }}</span>
                        <div class="home-picker-target-head">
                            <Button
                                v-if="showingWorlds"
                                v-tooltip="formatMessage(messages.back)"
                                type="quiet"
                                size="2xs"
                                circular
                                icon-only
                                :aria-label="formatMessage(messages.back)"
                                @click="backToInstances"
                                ><ChevronLeftIcon />
                            </Button>
                            <InstanceIcon
                                v-if="showingWorlds && targetInstance"
                                :icon-path="targetInstance.icon_path"
                                :instance-id="targetInstance.id"
                                :loader="targetInstance.loader"
                                size="20px"
                            />
                            <span class="truncate">{{
                                showingWorlds && targetInstance
                                    ? targetInstance.name
                                    : targetHeading
                            }}</span>
                        </div>
                        <StyledInput
                            v-if="(showingWorlds ? worlds.length : instances.length) > 5"
                            v-model="targetQuery"
                            type="search"
                            size="small"
                            :icon="SearchIcon"
                            :placeholder="formatMessage(messages.search)"
                            wrapper-class="w-full"
                            clearable
                        />
                        <p v-if="loadingWorlds" class="home-picker-none">
                            {{ formatMessage(messages.loading) }}
                        </p>
                        <ul
                            v-else
                            class="home-picker-targets"
                            role="listbox"
                            :aria-label="targetHeading"
                        >
                            <template v-if="!showingWorlds">
                                <li v-for="instance in targetInstances" :key="instance.id">
                                    <button
                                        type="button"
                                        role="option"
                                        class="home-picker-target-option"
                                        :aria-selected="target?.instanceId === instance.id"
                                        @click="chooseInstance(instance)"
                                    >
                                        <InstanceIcon
                                            :icon-path="instance.icon_path"
                                            :instance-id="instance.id"
                                            :loader="instance.loader"
                                            size="28px"
                                        />
                                        <span class="home-picker-target-copy">
                                            <strong>{{ instance.name }}</strong>
                                            <span class="capitalize"
                                                >{{ instance.loader }}
                                                {{ instance.game_version }}</span
                                            >
                                        </span>
                                        <CheckIcon
                                            v-if="
                                                selectedKind === 'instance' &&
                                                target?.instanceId === instance.id
                                            "
                                            class="home-picker-target-check"
                                            aria-hidden="true"
                                        />
                                    </button>
                                </li>
                            </template>
                            <template v-else>
                                <li
                                    v-for="world in targetWorlds"
                                    :key="world.type === 'server' ? world.address : world.path"
                                >
                                    <button
                                        type="button"
                                        role="option"
                                        class="home-picker-target-option"
                                        :aria-selected="isChosenWorld(world)"
                                        @click="chooseWorld(world)"
                                    >
                                        <span class="home-picker-target-glyph">
                                            <ServerIcon v-if="world.type === 'server'" />
                                            <GameIcon v-else />
                                        </span>
                                        <span class="home-picker-target-copy">
                                            <strong>{{ world.name }}</strong>
                                            <span>{{
                                                world.type === 'server'
                                                    ? world.address
                                                    : targetInstance?.name
                                            }}</span>
                                        </span>
                                        <CheckIcon
                                            v-if="isChosenWorld(world)"
                                            class="home-picker-target-check"
                                            aria-hidden="true"
                                        />
                                    </button>
                                </li>
                            </template>
                        </ul>
                        <p
                            v-if="
                                !loadingWorlds &&
                                (showingWorlds ? targetWorlds.length : targetInstances.length) === 0
                            "
                            class="home-picker-none"
                        >
                            {{ formatMessage(messages.noResults) }}
                        </p>
                    </div>
                </div>

                <footer class="home-picker-footer">
                    <p
                        class="home-picker-feedback"
                        :class="{ 'is-empty': !lastAdded }"
                        role="status"
                        aria-live="polite"
                    >
                        <template v-if="lastAdded">
                            <CheckIcon aria-hidden="true" />
                            <span class="truncate">{{
                                formatMessage(messages.addedToast, { name: lastAdded.name })
                            }}</span>
                            <button type="button" class="home-picker-undo" @click="undo">
                                {{ formatMessage(messages.undo) }}
                            </button>
                        </template>
                    </p>
                    <div class="home-picker-footer-actions">
                        <Button v-if="mode === 'add'" type="quiet" @click="hide">
                            {{ formatMessage(messages.done) }}
                        </Button>
                        <Button
                            type="colored"
                            color="brand"
                            class="home-picker-add"
                            :disabled="needsTarget && !target"
                            @click="add(selectedKind)"
                        >
                            <CheckIcon v-if="mode === 'replace'" aria-hidden="true" />
                            <PlusIcon v-else aria-hidden="true" />
                            {{
                                mode === 'replace'
                                    ? formatMessage(messages.replace)
                                    : counts[selectedKind]
                                      ? formatMessage(messages.addAnother)
                                      : formatMessage(messages.addToHome)
                            }}
                        </Button>
                    </div>
                </footer>
            </aside>
        </div>
    </NewModal>
</template>

<style scoped>
/*
 * The catalog and the panel share one height so neither page jumps while
 * choosing. Each column scrolls on its own; the panel keeps its button in view.
 */
.home-picker {
    @apply grid min-h-0;
    height: min(38rem, calc(100vh - 9rem));
    grid-template-columns: minmax(0, 1fr) 21rem;
    container-type: inline-size;
}

.home-picker[data-mode='replace'] {
    height: min(34rem, calc(100vh - 9rem));
    grid-template-columns: minmax(0, 1fr);
}

.home-picker-catalog {
    @apply flex min-h-0 min-w-0 flex-col;
}

.home-picker-toolbar {
    @apply flex flex-none flex-col gap-3 px-6 pb-3 pt-5;
}

.home-picker-toolbar :deep(.home-picker-search) {
    @apply w-full;
}

.home-picker-categories {
    @apply flex flex-wrap gap-1.5;
}

.home-picker-category {
    @apply cursor-pointer rounded-full border border-solid border-[var(--color-divider)] bg-transparent px-3 py-1 text-[0.8125rem] font-semibold text-[var(--color-text-default)];
    transition:
        background-color 120ms ease,
        color 120ms ease,
        border-color 120ms ease;
}

.home-picker-category:hover {
    @apply bg-surface-4;
}

.home-picker-category[aria-checked='true'] {
    @apply border-brand bg-brand text-[var(--color-accent-contrast)];
}

.home-picker-category:focus-visible,
.home-picker-size:focus-visible,
.home-picker-target-option:focus-visible,
.home-picker-undo:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px var(--color-brand-shadow);
}

.home-picker-scroll {
    @apply min-h-0 flex-1 overflow-y-auto px-6 pb-6;
}

.home-picker-section + .home-picker-section {
    @apply mt-5;
}

.home-picker-section-title {
    @apply m-0 mb-2.5 text-[0.6875rem] font-extrabold uppercase tracking-[0.08em] text-[var(--color-text-tertiary)];
}

.home-picker-grid {
    @apply m-0 grid list-none gap-3 p-0;
    grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
}

/*
 * A card is the widget's real look on a stage of its own, filled edge to edge
 * with a small margin, then its name and one line about it. Nothing is laid
 * over the stage: the "added" mark sits beside the name, and the plus that adds
 * it straight away sits at the right of the text. The card itself opens it in
 * the panel to pick a size or a target first.
 */
.home-picker-card {
    @apply relative min-w-0;
}

.home-picker-card-main {
    @apply flex h-full w-full min-w-0 cursor-pointer flex-col gap-1 border border-solid border-[var(--color-divider)] bg-surface-2 p-2 pb-3 text-left;
    border-radius: var(--radius-lg);
    transition:
        border-color 120ms ease,
        box-shadow 120ms ease,
        transform 160ms ease;
}

.home-picker-card-main:hover {
    border-color: color-mix(in srgb, var(--color-brand) 55%, var(--color-divider));
    transform: translateY(-1px);
}

.home-picker-card-main:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px var(--color-brand-shadow);
}

.home-picker-card.is-selected .home-picker-card-main {
    @apply border-brand;
    box-shadow: 0 0 0 1px var(--color-brand);
}

.home-picker-card.is-selected .home-picker-card-main:focus-visible {
    box-shadow:
        0 0 0 1px var(--color-brand),
        0 0 0 4px var(--color-brand-shadow);
}

.home-picker-card-stage {
    @apply mb-1.5 flex h-36 overflow-hidden p-2.5;
    border-radius: var(--radius-md);
    background: var(--surface-1);
}

.home-picker-card-name {
    @apply flex min-w-0 items-center gap-1.5 pl-1 pr-10 text-sm font-bold text-[var(--color-text-primary)];
}

.home-picker-card-description {
    @apply line-clamp-2 pl-1 pr-10 text-xs leading-snug text-[var(--color-text-tertiary)];
}

.home-picker-card-added {
    @apply inline-flex shrink-0 items-center gap-0.5 rounded-full bg-highlight-green px-1.5 text-[0.6875rem] font-bold leading-[1.125rem] text-brand-green;
}

.home-picker-card-added svg {
    @apply size-3;
}

.home-picker-card-add {
    @apply absolute bottom-3 right-3 flex size-8 cursor-pointer items-center justify-center rounded-full border-0 bg-brand p-0 text-[var(--color-accent-contrast)];
    box-shadow: 0 4px 12px color-mix(in srgb, var(--color-brand) 35%, transparent);
    transition:
        transform 120ms ease,
        background-color 120ms ease;
}

.home-picker-card-add:hover {
    transform: scale(1.08);
}

.home-picker-card-add:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px var(--color-brand-shadow);
}

.home-picker-card-add svg {
    @apply size-4;
}

.home-picker-card.is-flashing .home-picker-card-add {
    @apply bg-brand-green;
    animation: home-picker-pop 360ms ease;
}

@keyframes home-picker-pop {
    40% {
        transform: scale(1.25);
    }
}

.home-picker-none {
    @apply m-0 py-8 text-center text-sm text-[var(--color-text-tertiary)];
}

/* The panel: the chosen widget large, its sizes, its target, then the button. */
.home-picker-detail {
    @apply flex min-h-0 min-w-0 flex-col border-0 border-l border-solid border-[var(--color-divider)] bg-surface-2;
}

.home-picker[data-mode='replace'] .home-picker-detail {
    @apply border-l-0;
}

.home-picker-detail-scroll {
    @apply flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-5;
    container-type: inline-size;
}

/*
 * The large preview: its box takes the chosen size's proportions, so the
 * widget fills it with a small margin whatever its shape.
 */
.home-picker-stage {
    --home-picker-stage-aspect: 1.5;
    @apply flex flex-none p-3;
    height: clamp(
        5.5rem,
        calc((100cqw - 1.5rem) / var(--home-picker-stage-aspect) + 1.5rem),
        17rem
    );
    border-radius: var(--radius-lg);
    background:
        radial-gradient(
                circle,
                color-mix(in srgb, var(--color-text-primary) 9%, transparent) 1px,
                transparent 1.5px
            )
            0 0 / 14px 14px,
        var(--surface-1);
}

.home-picker-detail-copy h3 {
    @apply m-0 text-lg font-extrabold text-[var(--color-text-primary)];
}

.home-picker-detail-copy p {
    @apply m-0 mt-1 text-[0.8125rem] leading-snug text-[var(--color-text-tertiary)];
}

.home-picker-field {
    @apply flex min-w-0 flex-col gap-2;
}

.home-picker-field-label {
    @apply text-[0.6875rem] font-extrabold uppercase tracking-[0.08em] text-[var(--color-text-tertiary)];
}

.home-picker-sizes {
    @apply flex flex-wrap gap-1.5;
}

.home-picker-size {
    @apply flex cursor-pointer items-center gap-2 rounded-[var(--radius-md)] border border-solid border-[var(--color-divider)] bg-transparent px-2.5 py-1.5 text-[0.8125rem] font-bold tabular-nums text-[var(--color-text-default)];
}

.home-picker-size:hover {
    @apply bg-surface-4;
}

.home-picker-size[aria-checked='true'] {
    @apply border-brand text-[var(--color-text-primary)];
    box-shadow: inset 0 0 0 1px var(--color-brand);
}

.home-picker-size-glyph {
    @apply grid gap-[2px];
}

.home-picker-size-glyph > span {
    @apply block rounded-[1px];
    background: var(--color-text-tertiary);
    opacity: 0.45;
}

.home-picker-size[aria-checked='true'] .home-picker-size-glyph > span {
    @apply bg-brand opacity-100;
}

.home-picker-target-head {
    @apply flex min-w-0 items-center gap-2 text-sm font-bold text-[var(--color-text-primary)];
}

.home-picker-targets {
    @apply m-0 flex list-none flex-col gap-1 p-0;
}

.home-picker-target-option {
    @apply flex w-full min-w-0 cursor-pointer items-center gap-2.5 rounded-[var(--radius-md)] border border-solid border-transparent bg-surface-3 px-2.5 py-2 text-left;
}

.home-picker-target-option:hover {
    @apply bg-surface-4;
}

.home-picker-target-option[aria-selected='true'] {
    @apply border-brand;
}

.home-picker-target-glyph {
    @apply flex size-7 shrink-0 items-center justify-center rounded-[var(--radius-sm)] bg-surface-4 text-[var(--color-text-tertiary)];
}

.home-picker-target-glyph svg {
    @apply size-4;
}

.home-picker-target-copy {
    @apply flex min-w-0 flex-1 flex-col;
}

.home-picker-target-copy strong {
    @apply truncate text-[0.8125rem] font-bold text-[var(--color-text-primary)];
}

.home-picker-target-copy span {
    @apply truncate text-xs text-[var(--color-text-tertiary)];
}

.home-picker-target-check {
    @apply size-4 shrink-0 text-brand;
}

.home-picker-footer {
    @apply flex flex-none flex-col gap-2.5 border-0 border-t border-solid border-[var(--color-divider)] px-5 py-4;
}

.home-picker-feedback {
    @apply m-0 flex min-h-5 min-w-0 items-center gap-1.5 text-[0.8125rem] font-semibold text-brand-green;
}

.home-picker-feedback.is-empty {
    @apply -mb-2.5 min-h-0;
}

.home-picker-feedback svg {
    @apply size-4 shrink-0;
}

.home-picker-undo {
    @apply ml-auto shrink-0 cursor-pointer rounded-[var(--radius-sm)] border-0 bg-transparent px-1.5 py-0.5 text-[0.8125rem] font-bold text-[var(--color-text-primary)] underline;
}

.home-picker-footer-actions {
    @apply flex items-center justify-end gap-2;
}

.home-picker-footer-actions :deep(.home-picker-add) {
    @apply flex-1 justify-center;
}

/* A narrow window stacks the panel under the catalog. */
@container (max-width: 40rem) {
    .home-picker {
        grid-template-columns: minmax(0, 1fr);
        grid-template-rows: minmax(0, 1fr) auto;
    }

    .home-picker-detail {
        @apply max-h-[45%] border-l-0 border-t;
    }
}

@media (prefers-reduced-motion: reduce) {
    .home-picker-card-main,
    .home-picker-card-add,
    .home-picker-category {
        transition: none;
    }

    .home-picker-card-main:hover,
    .home-picker-card-add:hover {
        transform: none;
    }

    .home-picker-card.is-flashing .home-picker-card-add {
        animation: none;
    }
}
</style>
