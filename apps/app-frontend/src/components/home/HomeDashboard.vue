<script setup lang="ts">
import {
    ChevronDownIcon,
    ChevronUpIcon,
    ExpandIcon,
    GripVerticalIcon,
    ListIcon,
    MoreVerticalIcon,
    PencilIcon,
    PlusIcon,
    RefreshCwIcon,
    TrashIcon,
} from '@modrinth/assets'
import {
    Button,
    defineMessages,
    injectNotificationManager,
    OverflowMenu,
    useVIntl,
} from '@modrinth/ui'
import { until, useResizeObserver } from '@vueuse/core'
import { computed, defineAsyncComponent, ref, shallowRef, watch } from 'vue'
import Draggable from 'vuedraggable'

import {
    addHomeWidget,
    enableFreeHomeDashboard,
    findNearestFreeHomeWidgetPosition,
    getHomeGridColumnCount,
    getHomeWidgetDimensions,
    getHomeWidgetSpan,
    HOME_RECENT_DEFAULT_LIMIT,
    HOME_RECENT_LIMIT_OPTIONS,
    HOME_WIDGET_GRID_GAP,
    HOME_WIDGET_GRID_ROW_HEIGHT,
    HOME_WIDGET_SIZE_OPTIONS,
    type HomeDashboardConfig,
    type HomeGreetingFont,
    type HomeGreetingMode,
    type HomeWidgetLayout,
    type HomeWidgetPlacement,
    type HomeWidgetPosition,
    type HomeWidgetSize,
    moveHomeWidget,
    packHomeWidgets,
    removeHomeWidget,
    replaceHomeDashboardWidgets,
    resizeHomeWidget,
    setHomeDashboardLayout,
    setHomeGreetingOptions,
    setHomeRecentLimit,
    setHomeWidgetPosition,
} from '@/components/home/home-dashboard'
import { provideHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import HomeCalendar from '@/components/home/HomeCalendar.vue'
import HomeGreeting from '@/components/home/HomeGreeting.vue'
import type HomeGreetingSettingsModalInstance from '@/components/home/HomeGreetingSettingsModal.vue'
import HomePinnedInstances from '@/components/home/HomePinnedInstances.vue'
import HomePinnedServers from '@/components/home/HomePinnedServers.vue'
import HomePinnedWorlds from '@/components/home/HomePinnedWorlds.vue'
import HomeRecentWorlds from '@/components/home/HomeRecentWorlds.vue'
import HomeShortcutWidget from '@/components/home/HomeShortcutWidget.vue'
import type HomeWidgetPickerModalInstance from '@/components/home/HomeWidgetPickerModal.vue'
import type { GameInstance } from '@/helpers/types'

// Opened from the edit controls only: loaded and mounted the first time they open.
const HomeGreetingSettingsModal = defineAsyncComponent(
    () => import('@/components/home/HomeGreetingSettingsModal.vue'),
)
const HomeWidgetPickerModal = defineAsyncComponent(
    () => import('@/components/home/HomeWidgetPickerModal.vue'),
)

const props = defineProps<{
    config: HomeDashboardConfig
    instances: GameInstance[]
    playerName: string | null
    editing: boolean
}>()

const emit = defineEmits<{
    change: [config: HomeDashboardConfig]
    reset: []
}>()

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
provideHomeDashboardRuntime(handleError)
const editing = computed(() => props.editing)
const isFreeLayout = computed(() => props.config.layout === 'free')
const gridContainer = ref<HTMLElement>()
const widgetPicker = ref<InstanceType<typeof HomeWidgetPickerModalInstance>>()
const greetingSettings = ref<InstanceType<typeof HomeGreetingSettingsModalInstance>>()
const widgetPickerUsed = ref(false)
const greetingSettingsUsed = ref(false)
const replacingWidgetId = ref<string | null>(null)
const dragging = ref(false)
const draggableWidgets = ref<HomeWidgetPlacement[]>([])
const previewPositions = ref<Record<string, HomeWidgetPosition>>({})
// The grid's content width as the observer reports it after layout; measuring while
// mounting (as useElementSize does) forces the window's first layout in that task.
const width = ref(0)
useResizeObserver(gridContainer, ([entry]) => {
    width.value = entry.contentRect.width
})
const columnCount = computed(() => getHomeGridColumnCount(width.value))
const widgetsForPacking = computed(() =>
    editing.value ? draggableWidgets.value : props.config.widgets,
)
const packedWidgets = computed(() => packHomeWidgets(widgetsForPacking.value, columnCount.value))
const packedById = computed(() => new Map(packedWidgets.value.map((widget) => [widget.id, widget])))
const freeDrag = shallowRef<{
    id: string
    pointerId: number
    startClientX: number
    startClientY: number
    startPosition: HomeWidgetPosition
    target: HTMLElement
    article: HTMLElement
    deltaX: number
    deltaY: number
    frame: number | null
} | null>(null)

const freeGridColumnPitch = computed(
    () =>
        getHomeWidgetDimensions('1x1', columnCount.value, width.value).width + HOME_WIDGET_GRID_GAP,
)
const freeGridRowPitch = HOME_WIDGET_GRID_ROW_HEIGHT + HOME_WIDGET_GRID_GAP
const resolvedFreePositions = computed(() => {
    const positions: Record<string, HomeWidgetPosition> = {}
    const positioned: HomeWidgetPlacement[] = []
    const activeId = freeDrag.value?.id
    const orderedWidgets = activeId
        ? [
              ...props.config.widgets.filter((widget) => widget.id !== activeId),
              ...props.config.widgets.filter((widget) => widget.id === activeId),
          ]
        : props.config.widgets

    for (const widget of orderedWidgets) {
        const position = findNearestFreeHomeWidgetPosition(
            positioned,
            widget,
            rawFreeWidgetPosition(widget),
            columnCount.value,
        )
        positions[widget.id] = position
        positioned.push({ ...widget, position })
    }
    return positions
})
const freeContentRows = computed(() =>
    props.config.widgets.reduce((lastRow, widget) => {
        const position = freeWidgetPosition(widget)
        return Math.max(
            lastRow,
            position.row + getHomeWidgetSpan(widget.size, columnCount.value).rows,
        )
    }, 0),
)
const freeCanvasHeight = computed(() => {
    if (!props.config.widgets.length) return 0
    return Math.max(480, freeContentRows.value * freeGridRowPitch)
})
const dashboardGridStyle = computed(() =>
    isFreeLayout.value
        ? {
              height: `${freeCanvasHeight.value}px`,
              '--home-free-grid-column-pitch': `${freeGridColumnPitch.value}px`,
              '--home-free-grid-row-pitch': `${freeGridRowPitch}px`,
          }
        : { gridTemplateColumns: `repeat(${columnCount.value}, minmax(0, 1fr))` },
)

const messages = defineMessages({
    add: { id: 'app.home.widgets.add', defaultMessage: 'Add widget' },
    options: { id: 'app.home.widgets.options', defaultMessage: 'Widget options' },
    moveEarlier: { id: 'app.home.widgets.move-earlier', defaultMessage: 'Move earlier' },
    moveLater: { id: 'app.home.widgets.move-later', defaultMessage: 'Move later' },
    remove: { id: 'app.home.widgets.remove', defaultMessage: 'Remove widget' },
    drag: { id: 'app.home.widgets.drag', defaultMessage: 'Drag to move widget' },
    replace: { id: 'app.home.widgets.replace', defaultMessage: 'Replace target' },
    empty: { id: 'app.home.widgets.empty', defaultMessage: 'Add a widget to build your Home.' },
    size: { id: 'app.home.widgets.size', defaultMessage: 'Size {size}' },
    recentItems: {
        id: 'app.home.widgets.recent-items',
        defaultMessage: 'Show {count} recent items',
    },
    greetingSettings: {
        id: 'app.home.greeting.settings.title',
        defaultMessage: 'Customize greeting',
    },
})

watch(
    () => props.config.widgets,
    (widgets) => {
        if (!dragging.value) {
            draggableWidgets.value = [...widgets]
            previewPositions.value = Object.fromEntries(
                widgets.flatMap((widget) =>
                    widget.position ? [[widget.id, widget.position] as const] : [],
                ),
            )
        }
    },
    { immediate: true, deep: true },
)

function widgetStyle(widget: HomeWidgetPlacement) {
    if (isFreeLayout.value) {
        const position = freeWidgetPosition(widget)
        const dimensions = getWidgetDimensions(widget)
        return {
            left: `${position.column * freeGridColumnPitch.value}px`,
            top: `${position.row * freeGridRowPitch}px`,
            width: `${dimensions.width}px`,
            height: `${dimensions.height}px`,
        }
    }

    const packed = packedById.value.get(widget.id)
    if (!packed) return undefined
    if (editing.value && dragging.value) {
        return {
            gridColumn: `span ${packed.effectiveColumns}`,
            gridRow: `span ${packed.effectiveRows}`,
        }
    }
    return {
        gridColumn: `${packed.column} / span ${packed.effectiveColumns}`,
        gridRow: `${packed.row} / span ${packed.effectiveRows}`,
    }
}

function getWidgetDimensions(widget: HomeWidgetPlacement) {
    return getHomeWidgetDimensions(widget.size, columnCount.value, width.value)
}

function defaultFreeWidgetPosition(widget: HomeWidgetPlacement): HomeWidgetPosition {
    const packed = packedById.value.get(widget.id)
    if (!packed) return { column: 0, row: 0 }
    return {
        column: packed.column - 1,
        row: packed.row - 1,
    }
}

function rawFreeWidgetPosition(widget: HomeWidgetPlacement): HomeWidgetPosition {
    const position =
        previewPositions.value[widget.id] ?? widget.position ?? defaultFreeWidgetPosition(widget)
    const span = getHomeWidgetSpan(widget.size, columnCount.value)
    return {
        column: Math.min(
            Math.max(0, Math.round(position.column)),
            Math.max(0, columnCount.value - span.columns),
        ),
        row: Math.max(0, Math.round(position.row)),
    }
}

function freeWidgetPosition(widget: HomeWidgetPlacement): HomeWidgetPosition {
    return resolvedFreePositions.value[widget.id] ?? rawFreeWidgetPosition(widget)
}

function startFreeWidgetDrag(event: PointerEvent, widget: HomeWidgetPlacement) {
    if (!editing.value || !isFreeLayout.value || event.button !== 0) return
    event.preventDefault()
    const target = event.currentTarget as HTMLElement
    const article = target.closest<HTMLElement>('.home-widget')
    if (!article) return
    const position = freeWidgetPosition(widget)
    target.setPointerCapture(event.pointerId)
    freeDrag.value = {
        id: widget.id,
        pointerId: event.pointerId,
        startClientX: event.clientX,
        startClientY: event.clientY,
        startPosition: position,
        target,
        article,
        deltaX: 0,
        deltaY: 0,
        frame: null,
    }
    dragging.value = true
}

function updateFreeWidgetDrag(event: PointerEvent, widget: HomeWidgetPlacement) {
    const current = freeDrag.value
    if (!current || current.id !== widget.id || current.pointerId !== event.pointerId) return
    const dimensions = getWidgetDimensions(widget)
    const startLeft = current.startPosition.column * freeGridColumnPitch.value
    const startTop = current.startPosition.row * freeGridRowPitch
    current.deltaX = Math.min(
        Math.max(event.clientX - current.startClientX, -startLeft),
        Math.max(-startLeft, width.value - dimensions.width - startLeft),
    )
    current.deltaY = Math.max(event.clientY - current.startClientY, -startTop)
    if (current.frame !== null) return

    current.frame = window.requestAnimationFrame(() => {
        current.frame = null
        if (freeDrag.value !== current) return
        current.article.style.transform = `translate3d(${current.deltaX}px, ${current.deltaY}px, 0)`
    })
}

function finishFreeWidgetDrag(event: PointerEvent, widget: HomeWidgetPlacement) {
    const current = freeDrag.value
    if (!current || current.id !== widget.id || current.pointerId !== event.pointerId) return
    if (current.target.hasPointerCapture(event.pointerId)) {
        current.target.releasePointerCapture(event.pointerId)
    }
    if (current.frame !== null) window.cancelAnimationFrame(current.frame)
    current.article.style.transform = ''
    const position = findNearestFreeHomeWidgetPosition(
        props.config.widgets.map((candidate) => ({
            ...candidate,
            position: freeWidgetPosition(candidate),
        })),
        widget,
        {
            column:
                current.startPosition.column +
                Math.round(current.deltaX / freeGridColumnPitch.value),
            row: current.startPosition.row + Math.round(current.deltaY / freeGridRowPitch),
        },
        columnCount.value,
    )
    previewPositions.value = { ...previewPositions.value, [widget.id]: position }
    freeDrag.value = null
    dragging.value = false
    emit('change', setHomeWidgetPosition(props.config, widget.id, position))
}

function moveFreeWidgetWithKeyboard(event: KeyboardEvent, widget: HomeWidgetPlacement) {
    if (!editing.value || !isFreeLayout.value) return
    const movement = {
        ArrowLeft: [-1, 0],
        ArrowRight: [1, 0],
        ArrowUp: [0, -1],
        ArrowDown: [0, 1],
    }[event.key]
    if (!movement) return

    event.preventDefault()
    const current = freeWidgetPosition(widget)
    const position = findNearestFreeHomeWidgetPosition(
        props.config.widgets.map((candidate) => ({
            ...candidate,
            position: freeWidgetPosition(candidate),
        })),
        widget,
        {
            column: current.column + movement[0],
            row: current.row + movement[1],
        },
        columnCount.value,
    )
    previewPositions.value = { ...previewPositions.value, [widget.id]: position }
    emit('change', setHomeWidgetPosition(props.config, widget.id, position))
}

function startWidgetDrag() {
    dragging.value = true
}

function finishWidgetDrag() {
    dragging.value = false
    const reordered = [...draggableWidgets.value]
    const unchanged = reordered.every(
        (widget, index) => widget.id === props.config.widgets[index]?.id,
    )
    if (!unchanged) emit('change', replaceHomeDashboardWidgets(props.config, reordered))
}

function effectiveSize(widget: HomeWidgetPlacement): HomeWidgetSize {
    const packed = packedById.value.get(widget.id)
    return packed
        ? (`${packed.effectiveColumns}x${packed.effectiveRows}` as HomeWidgetSize)
        : widget.size
}

async function showWidgetPicker(kind?: HomeWidgetPlacement['kind']) {
    widgetPickerUsed.value = true
    const picker = await until(widgetPicker).toBeTruthy()
    picker.show(kind)
}

function openWidgetPicker() {
    replacingWidgetId.value = null
    void showWidgetPicker()
}

function addWidget(widget: HomeWidgetPlacement) {
    const replacingId = replacingWidgetId.value
    replacingWidgetId.value = null
    if (!replacingId) {
        const placement = isFreeLayout.value
            ? { ...widget, position: { column: 0, row: freeContentRows.value } }
            : widget
        emit('change', addHomeWidget(props.config, placement))
        return
    }

    emit(
        'change',
        replaceHomeDashboardWidgets(
            props.config,
            props.config.widgets.map((current) =>
                current.id === replacingId
                    ? {
                          ...widget,
                          id: current.id,
                          size: current.size,
                          ...(current.position ? { position: current.position } : {}),
                      }
                    : current,
            ),
        ),
    )
}

function replaceWidgetTarget(widget: HomeWidgetPlacement) {
    replacingWidgetId.value = widget.id
    void showWidgetPicker(widget.kind)
}

function removeWidget(id: string) {
    emit('change', removeHomeWidget(props.config, id))
}

function resizeWidget(id: string, size: HomeWidgetSize) {
    let config = resizeHomeWidget(props.config, id, size)
    if (isFreeLayout.value) {
        const widget = config.widgets.find((candidate) => candidate.id === id)
        if (widget) {
            const position = findNearestFreeHomeWidgetPosition(
                config.widgets.map((candidate) => ({
                    ...candidate,
                    position: freeWidgetPosition(candidate),
                })),
                widget,
                freeWidgetPosition(widget),
                columnCount.value,
            )
            config = setHomeWidgetPosition(config, id, position)
        }
    }
    emit('change', config)
}

function setRecentLimit(id: string, limit: (typeof HOME_RECENT_LIMIT_OPTIONS)[number]) {
    emit('change', setHomeRecentLimit(props.config, id, limit))
}

async function openGreetingSettings(widget: HomeWidgetPlacement) {
    greetingSettingsUsed.value = true
    const settings = await until(greetingSettings).toBeTruthy()
    settings.show(widget)
}

function saveGreetingSettings(
    id: string,
    mode: HomeGreetingMode,
    text: string,
    font: HomeGreetingFont,
    fontSize: number,
) {
    emit('change', setHomeGreetingOptions(props.config, id, mode, text, font, fontSize))
}

function moveWidget(index: number, direction: -1 | 1) {
    emit('change', moveHomeWidget(props.config, index, direction))
}

function widgetOptions(widget: HomeWidgetPlacement, index: number) {
    const sizeOptions = HOME_WIDGET_SIZE_OPTIONS[widget.kind]
    return [
        ...(widget.kind === 'greeting'
            ? [
                  {
                      id: 'greeting-settings',
                      icon: PencilIcon,
                      action: () => openGreetingSettings(widget),
                  },
                  { divider: true },
              ]
            : []),
        ...(widget.kind === 'recent'
            ? [
                  ...HOME_RECENT_LIMIT_OPTIONS.map((limit) => ({
                      id: `recent-limit-${limit}`,
                      icon: ListIcon,
                      disabled:
                          (widget.options?.recentLimit ?? HOME_RECENT_DEFAULT_LIMIT) === limit,
                      action: () => setRecentLimit(widget.id, limit),
                  })),
                  { divider: true },
              ]
            : []),
        ...(sizeOptions.length > 1
            ? [
                  ...sizeOptions.map((size) => ({
                      id: `size-${size}`,
                      icon: ExpandIcon,
                      disabled: widget.size === size,
                      action: () => resizeWidget(widget.id, size),
                  })),
                  { divider: true },
              ]
            : []),
        ...(widget.target
            ? [
                  {
                      id: 'replace',
                      icon: RefreshCwIcon,
                      action: () => replaceWidgetTarget(widget),
                  },
                  { divider: true },
              ]
            : []),
        ...(!isFreeLayout.value
            ? [
                  {
                      id: 'move-earlier',
                      icon: ChevronUpIcon,
                      disabled: index === 0,
                      action: () => moveWidget(index, -1),
                  },
                  {
                      id: 'move-later',
                      icon: ChevronDownIcon,
                      disabled: index === props.config.widgets.length - 1,
                      action: () => moveWidget(index, 1),
                  },
                  { divider: true },
              ]
            : []),
        {
            id: 'remove',
            icon: TrashIcon,
            color: 'red' as const,
            action: () => removeWidget(widget.id),
        },
    ]
}

function setLayout(layout: HomeWidgetLayout) {
    if (layout === props.config.layout) return
    emit(
        'change',
        layout === 'free'
            ? enableFreeHomeDashboard(props.config, columnCount.value)
            : setHomeDashboardLayout(props.config, 'grid'),
    )
}

defineExpose({ openWidgetPicker, setLayout })
</script>

<template>
    <HomeWidgetPickerModal
        v-if="widgetPickerUsed"
        ref="widgetPicker"
        :instances="instances"
        @add="addWidget"
    />
    <HomeGreetingSettingsModal
        v-if="greetingSettingsUsed"
        ref="greetingSettings"
        :player-name="playerName"
        @save="saveGreetingSettings"
    />
    <section class="home-dashboard p-6 pb-20" :class="{ 'is-dragging': dragging }">
        <div ref="gridContainer" class="mx-auto w-full max-w-[96rem]">
            <Draggable
                :list="draggableWidgets"
                item-key="id"
                tag="div"
                class="home-dashboard-grid"
                :class="{
                    'is-editing': editing,
                    'is-dragging': dragging,
                    'is-free': isFreeLayout,
                    'has-widgets': config.widgets.length > 0,
                }"
                :style="dashboardGridStyle"
                handle=".home-widget-drag-handle"
                :disabled="!editing || isFreeLayout"
                :animation="80"
                :swap-threshold="0.2"
                :invert-swap="true"
                :inverted-swap-threshold="0.65"
                :empty-insert-threshold="12"
                :force-fallback="true"
                :fallback-on-body="false"
                :fallback-tolerance="0"
                :scroll="true"
                :scroll-sensitivity="96"
                :scroll-speed="24"
                :bubble-scroll="true"
                ghost-class="home-widget-ghost !border-2 !border-dashed !border-brand !bg-brand-highlight !shadow-none opacity-[0.45]"
                chosen-class="home-widget-chosen"
                drag-class="home-widget-drag"
                fallback-class="home-widget-fallback"
                data-onboarding-id="home-widget-grid"
                @start="startWidgetDrag"
                @end="finishWidgetDrag"
            >
                <template #item="{ element: widget, index }">
                    <article
                        class="home-widget"
                        :class="{ 'is-free-dragging': freeDrag?.id === widget.id }"
                        :data-widget-kind="widget.kind"
                        :style="widgetStyle(widget)"
                    >
                        <div v-if="editing" class="home-widget-edit-bar">
                            <button
                                v-tooltip="formatMessage(messages.drag)"
                                type="button"
                                class="home-widget-drag-handle"
                                @pointerdown="startFreeWidgetDrag($event, widget)"
                                @pointermove="updateFreeWidgetDrag($event, widget)"
                                @pointerup="finishFreeWidgetDrag($event, widget)"
                                @pointercancel="finishFreeWidgetDrag($event, widget)"
                                @keydown="moveFreeWidgetWithKeyboard($event, widget)"
                            >
                                <GripVerticalIcon />
                            </button>
                            <span class="home-widget-size-label">{{ widget.size }}</span>
                            <div class="home-widget-options">
                                <OverflowMenu
                                    class="relative inline-flex size-6 min-w-6 shrink-0 cursor-pointer items-center justify-center rounded-full border border-solid border-transparent bg-transparent p-0 text-[var(--color-text-default)] transition-[background-color,color,filter,transform] duration-150 hover:bg-surface-4 hover:brightness-[--hover-brightness] active:scale-[0.97] [&>svg]:size-4"
                                    :options="widgetOptions(widget, index)"
                                    :tooltip="formatMessage(messages.options)"
                                >
                                    <MoreVerticalIcon />
                                    <template #greeting-settings>
                                        <PencilIcon />
                                        {{ formatMessage(messages.greetingSettings) }}
                                    </template>
                                    <template
                                        v-for="limit in HOME_RECENT_LIMIT_OPTIONS"
                                        #[`recent-limit-${limit}`]
                                        :key="`recent-limit-${limit}`"
                                    >
                                        <ListIcon />
                                        {{ formatMessage(messages.recentItems, { count: limit }) }}
                                    </template>
                                    <template
                                        v-for="size in HOME_WIDGET_SIZE_OPTIONS[widget.kind]"
                                        #[`size-${size}`]
                                        :key="size"
                                    >
                                        <ExpandIcon /> {{ formatMessage(messages.size, { size }) }}
                                    </template>
                                    <template #move-earlier>
                                        <ChevronUpIcon /> {{ formatMessage(messages.moveEarlier) }}
                                    </template>
                                    <template #move-later>
                                        <ChevronDownIcon /> {{ formatMessage(messages.moveLater) }}
                                    </template>
                                    <template #replace>
                                        <RefreshCwIcon /> {{ formatMessage(messages.replace) }}
                                    </template>
                                    <template #remove>
                                        <TrashIcon /> {{ formatMessage(messages.remove) }}
                                    </template>
                                </OverflowMenu>
                            </div>
                        </div>
                        <div class="home-widget-content min-w-0 min-h-0 flex-1 overflow-hidden p-4">
                            <HomeGreeting
                                v-if="widget.kind === 'greeting'"
                                :player-name="playerName"
                                :dashboard-size="effectiveSize(widget)"
                                :greeting-mode="widget.options?.greetingMode"
                                :greeting-text="widget.options?.greetingText"
                                :greeting-font="widget.options?.greetingFont"
                                :greeting-font-size="widget.options?.greetingFontSize"
                            />
                            <HomeRecentWorlds
                                v-else-if="widget.kind === 'recent'"
                                :instances="instances"
                                :dashboard-size="effectiveSize(widget)"
                                :limit="widget.options?.recentLimit"
                                dashboard
                            />
                            <HomeCalendar
                                v-else-if="widget.kind === 'calendar'"
                                :instances="instances"
                                :dashboard-size="effectiveSize(widget)"
                            />
                            <HomePinnedInstances
                                v-else-if="widget.kind === 'pinned-instances'"
                                :instances="instances"
                                :dashboard-size="effectiveSize(widget)"
                                dashboard
                            />
                            <HomePinnedWorlds
                                v-else-if="widget.kind === 'pinned-worlds'"
                                :instances="instances"
                                :dashboard-size="effectiveSize(widget)"
                                dashboard
                            />
                            <HomePinnedServers
                                v-else-if="widget.kind === 'pinned-servers'"
                                :instances="instances"
                                :dashboard-size="effectiveSize(widget)"
                                dashboard
                            />
                            <HomeShortcutWidget
                                v-else
                                :placement="widget"
                                :instances="instances"
                                :dashboard-size="effectiveSize(widget)"
                            />
                        </div>
                    </article>
                </template>
            </Draggable>
            <div
                v-if="config.widgets.length === 0"
                class="flex min-h-64 flex-col items-center justify-center gap-4 rounded-lg border border-dashed border-divider text-center"
            >
                <p class="m-0 text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.empty) }}
                </p>
                <Button @click="openWidgetPicker"
                    ><PlusIcon /> {{ formatMessage(messages.add) }}</Button
                >
            </div>
        </div>
    </section>
</template>

<style scoped>
.home-dashboard {
    min-width: 0;
    container-type: inline-size;
}

.home-dashboard-grid {
    display: grid;
    grid-auto-rows: 10rem;
    align-items: stretch;
    gap: 1rem;
}

.home-dashboard-grid.is-editing.is-dragging {
    grid-auto-flow: dense;
}

.home-dashboard-grid.is-free {
    position: relative;
    display: block;
}

.home-dashboard-grid.is-free.is-editing::before {
    position: absolute;
    inset: 0;
    content: '';
    border: 1px solid
        color-mix(in srgb, var(--color-divider) var(--opacity-ratio-keep-55), transparent);
    border-radius: var(--radius-lg);
    background-image:
        linear-gradient(
            to right,
            color-mix(in srgb, var(--color-divider) var(--opacity-ratio-keep-45), transparent) 1px,
            transparent 1px
        ),
        linear-gradient(
            to bottom,
            color-mix(in srgb, var(--color-divider) var(--opacity-ratio-keep-45), transparent) 1px,
            transparent 1px
        );
    background-size:
        var(--home-free-grid-column-pitch) var(--home-free-grid-row-pitch),
        var(--home-free-grid-column-pitch) var(--home-free-grid-row-pitch);
    pointer-events: none;
}

.home-dashboard-grid.is-free.has-widgets {
    min-height: 30rem;
}

.home-dashboard-grid.is-free .home-widget {
    position: absolute;
}

.home-widget {
    position: relative;
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
    overflow: hidden;
    box-sizing: border-box;
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    background: color-mix(
        in srgb,
        var(--surface-3) var(--home-widget-bg-opacity, 100%),
        transparent
    );
    box-shadow: var(--shadow-card);
    transition:
        border-color 120ms ease,
        box-shadow 120ms ease,
        filter 120ms ease;
}

.home-widget[data-widget-kind='greeting'] {
    border-color: transparent;
    background: transparent;
    box-shadow: none;
}

.home-dashboard-grid:not(.is-editing)
    .home-widget:is(
        [data-widget-kind='instance'],
        [data-widget-kind='world'],
        [data-widget-kind='server']
    ):hover {
    filter: brightness(var(--hover-brightness));
}

.home-widget-edit-bar {
    position: absolute;
    top: 0.5rem;
    right: 0.5rem;
    z-index: 12;
    display: flex;
    /* The bar is sized by its content and anchored to the right edge, so
       revealing the size label and options menu widens it leftwards past the
       card border. Reserving the full width up front keeps the hit area stable:
       otherwise the growing bar moves out from under the pointer, the hover
       ends, the bar collapses, and the widget flickers. */
    width: calc(2rem + 2.25rem + 0.25rem + 2rem + 0.25rem);
    max-width: calc(100% - 1rem);
    height: 2.25rem;
    align-items: center;
    justify-content: flex-end;
    gap: 0;
    padding: 0.125rem;
    border: 1px solid var(--color-divider);
    border-radius: var(--radius-lg);
    background: var(--surface-3);
    box-shadow: var(--shadow-button);
    overflow: hidden;
    opacity: 0.9;
    transition:
        box-shadow 120ms ease,
        opacity 120ms ease;
}

.home-widget-drag-handle {
    display: inline-flex;
    width: 2rem;
    height: 2rem;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-tertiary);
    cursor: grab;
    touch-action: none;
    transition:
        background-color 100ms ease,
        color 100ms ease;
}

.home-widget-size-label {
    width: 2.25rem;
    margin-left: 0.25rem;
    overflow: hidden;
    color: var(--color-text-tertiary);
    font-size: 0.75rem;
    font-weight: 600;
    line-height: 1;
    text-align: center;
    opacity: 0;
    white-space: nowrap;
    transition: opacity 120ms ease;
}

.home-widget-options {
    display: flex;
    min-width: 0;
    width: 2rem;
    flex: 0 0 auto;
    overflow: hidden;
    opacity: 0;
    pointer-events: none;
    transition: opacity 120ms ease;
}

.home-widget:hover .home-widget-edit-bar,
.home-widget:focus-within .home-widget-edit-bar {
    box-shadow: var(--shadow-card);
    opacity: 1;
}

.home-widget:hover .home-widget-size-label,
.home-widget:focus-within .home-widget-size-label {
    opacity: 1;
}

.home-widget:hover .home-widget-options,
.home-widget:focus-within .home-widget-options {
    opacity: 1;
    pointer-events: auto;
}

.home-widget-drag-handle:hover,
.home-widget-drag-handle:focus-visible {
    background: var(--surface-4);
    color: var(--color-text-primary);
    outline: none;
}

.home-widget-drag-handle:active {
    cursor: grabbing;
}

.home-widget[data-widget-kind='instance'] .home-widget-content,
.home-widget[data-widget-kind='world'] .home-widget-content,
.home-widget[data-widget-kind='server'] .home-widget-content {
    padding: 0;
}

.home-widget-content > :deep(*) {
    height: 100%;
    min-height: 0;
}

.home-widget-ghost > * {
    opacity: 0;
}

.home-widget-chosen {
    border-color: var(--color-brand);
    box-shadow: 0 0 0 4px var(--color-brand-shadow);
}

.home-widget-drag,
.home-widget-fallback,
.home-widget.is-free-dragging {
    z-index: 1000 !important;
    border-color: var(--color-brand);
    box-shadow: var(--shadow-card);
    cursor: grabbing;
    opacity: 0.98;
    will-change: transform;
}

.home-dashboard-grid.is-editing .home-widget {
    border-color: transparent;
}

.home-dashboard-grid.is-editing .home-widget[data-widget-kind='greeting'] {
    border-color: var(--color-divider);
    border-style: dashed;
}

.home-dashboard-grid.is-editing .home-widget-content {
    pointer-events: none;
}

.home-dashboard-grid.is-editing
    .home-widget:is(
        [data-widget-kind='instance'],
        [data-widget-kind='world'],
        [data-widget-kind='server']
    ):hover,
.home-dashboard-grid.is-editing
    .home-widget:is(
        [data-widget-kind='instance'],
        [data-widget-kind='world'],
        [data-widget-kind='server']
    ):focus-within {
    border-color: var(--color-divider);
    box-shadow: var(--shadow-card);
}

.home-dashboard.is-dragging,
.home-dashboard.is-dragging * {
    user-select: none;
}

@media (prefers-reduced-motion: reduce) {
    .home-dashboard-grid {
        scroll-behavior: auto;
    }
}
</style>
