<script setup lang="ts">
import {
    ChevronDownIcon,
    ChevronRightIcon,
    CircleAlertIcon,
    GitGraphIcon,
    ListIcon,
    RotateCounterClockwiseIcon,
    SearchIcon,
    ZoomInIcon,
    ZoomOutIcon,
} from '@modrinth/assets'
import {
    Avatar,
    Button,
    Combobox,
    type ContentItem,
    defineMessages,
    MinecraftFormattedText,
    NewModal,
    StyledInput,
    Toggle,
    useVIntl,
} from '@modrinth/ui'
import { autoCleanToText } from '@sfirew/minecraft-motd-parser'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'

import {
    buildDependencyGraph,
    type DependencyDirection,
    type DependencyGraph,
    type DependencyGraphNode,
    dependencyGraphNodeBounds,
    dependencyGraphNodeGeometry,
    getDependencyTreeRows,
    getRelatedNodeIds,
    layoutDependencyGraph,
} from './dependency-graph'

const { formatMessage } = useVIntl()

const props = withDefaults(
    defineProps<{
        instanceId?: string
        instanceName?: string
        instanceIconUrl?: string
    }>(),
    {
        instanceId: undefined,
        instanceName: undefined,
        instanceIconUrl: undefined,
    },
)

const messages = defineMessages({
    header: { id: 'app.instance.dependencies.header', defaultMessage: 'Dependency relationships' },
    search: { id: 'app.instance.dependencies.search', defaultMessage: 'Search dependencies...' },
    treeView: { id: 'app.instance.dependencies.tree-view', defaultMessage: 'Dependency tree' },
    graphView: { id: 'app.instance.dependencies.graph-view', defaultMessage: 'Relationship graph' },
    requires: { id: 'app.instance.dependencies.requires', defaultMessage: 'Dependencies' },
    requiredBy: { id: 'app.instance.dependencies.required-by', defaultMessage: 'Dependents' },
    onlyRelated: {
        id: 'app.instance.dependencies.only-related',
        defaultMessage: 'Only show related content',
    },
    showIsolated: {
        id: 'app.instance.dependencies.show-isolated',
        defaultMessage: 'Show isolated content',
    },
    isolatedSummary: {
        id: 'app.instance.dependencies.isolated-summary',
        defaultMessage: '{count, number} isolated items are kept out of the relationship graph.',
    },
    allTypes: { id: 'app.instance.dependencies.all-types', defaultMessage: 'All types' },
    allSources: { id: 'app.instance.dependencies.all-sources', defaultMessage: 'All sources' },
    allStatuses: { id: 'app.instance.dependencies.all-statuses', defaultMessage: 'All statuses' },
    packManaged: { id: 'app.instance.dependencies.pack-managed', defaultMessage: 'Pack managed' },
    userAdded: { id: 'app.instance.dependencies.user-added', defaultMessage: 'User added' },
    localDiscovered: { id: 'app.instance.dependencies.local-discovered', defaultMessage: 'Local' },
    disabled: { id: 'app.instance.dependencies.disabled', defaultMessage: 'Disabled' },
    missing: { id: 'app.instance.dependencies.missing', defaultMessage: 'Missing' },
    orphaned: { id: 'app.instance.dependencies.orphaned', defaultMessage: 'Orphaned' },
    cycle: { id: 'app.instance.dependencies.cycle', defaultMessage: 'Cycle' },
    unresolved: { id: 'app.instance.dependencies.unresolved', defaultMessage: 'Unresolved' },
    noContent: {
        id: 'app.instance.dependencies.no-content',
        defaultMessage: 'No content to analyze.',
    },
    noMatches: {
        id: 'app.instance.dependencies.no-matches',
        defaultMessage: 'No dependencies match these filters.',
    },
    noDependencies: {
        id: 'app.instance.dependencies.no-dependencies',
        defaultMessage: 'No dependency edges were found.',
    },
    noGraphRelations: {
        id: 'app.instance.dependencies.no-graph-relations',
        defaultMessage: 'No visible relationships match these filters.',
    },
    graphDirection: {
        id: 'app.instance.dependencies.graph-direction',
        defaultMessage:
            'Stable relationship clusters. Arrows point from dependent content to the content it uses.',
    },
    graphHint: {
        id: 'app.instance.dependencies.graph-hint',
        defaultMessage: 'Drag the canvas to explore. Cards stay where you place them.',
    },
    reference: {
        id: 'app.instance.dependencies.reference',
        defaultMessage: 'Referenced elsewhere',
    },
    cycleReference: {
        id: 'app.instance.dependencies.cycle-reference',
        defaultMessage: 'Cycle continues here',
    },
    unresolvedLabel: {
        id: 'app.instance.dependencies.unresolved-label',
        defaultMessage: 'Dependency details are unavailable.',
    },
    directDependencies: {
        id: 'app.instance.dependencies.direct-dependencies',
        defaultMessage: 'Direct dependencies',
    },
    directDependents: {
        id: 'app.instance.dependencies.direct-dependents',
        defaultMessage: 'Direct dependents',
    },
    version: { id: 'app.instance.dependencies.version', defaultMessage: 'Version {version}' },
    file: { id: 'app.instance.dependencies.file', defaultMessage: 'File {file}' },
    source: { id: 'app.instance.dependencies.source', defaultMessage: 'Source' },
    status: { id: 'app.instance.dependencies.status', defaultMessage: 'Status' },
    stats: {
        id: 'app.instance.dependencies.stats',
        defaultMessage: '{nodes, number} items, {edges, number} relationships',
    },
    resetZoom: { id: 'app.instance.dependencies.reset-zoom', defaultMessage: 'Fit graph to view' },
    zoomIn: { id: 'app.instance.dependencies.zoom-in', defaultMessage: 'Zoom in' },
    zoomOut: { id: 'app.instance.dependencies.zoom-out', defaultMessage: 'Zoom out' },
})

type ViewMode = 'tree' | 'graph'
type Filter =
    'all' | 'pack' | 'user' | 'local' | 'disabled' | 'missing' | 'orphaned' | 'cycle' | 'unresolved'
type Point = { x: number; y: number }

const minZoom = 0.32
const maxZoom = 1.6
const viewportPadding = 36
const modal = ref<InstanceType<typeof NewModal>>()
const items = ref<ContentItem[]>([])
const viewMode = ref<ViewMode>('tree')
const direction = ref<DependencyDirection>('requiredBy')
const searchQuery = ref('')
const sourceFilter = ref<'all' | 'pack' | 'user' | 'local'>('all')
const typeFilter = ref('all')
const statusFilter = ref<Filter>('all')
const onlyRelated = ref(false)
const showIsolated = ref(false)
const expandedIds = ref(new Set<string>())
const selectedNodeId = ref<string>()
const zoom = ref(1)
const pan = ref<Point>({ x: 0, y: 0 })
const draggedNodeId = ref<string>()
const nodeOffsets = ref(new Map<string, Point>())
const graphViewport = ref<HTMLElement | null>(null)
const graphCanvas = ref<HTMLElement | null>(null)
const graphEdgesCanvas = ref<HTMLCanvasElement | null>(null)
const dragState = ref<{ kind: 'pan' | 'node'; id?: string }>()
let activePan: Point | undefined
let lastPointerPosition: Point | undefined
let activePointerId: number | undefined
let pendingPointerMove: { dx: number; dy: number } | undefined
let pointerFrame: number | undefined
let constrainFrame: number | undefined
let viewportObserver: ResizeObserver | undefined
let fitFrame: number | undefined
let edgeFrame: number | undefined

const graph = computed<DependencyGraph>(() => buildDependencyGraph(items.value))
const graphGeometry = computed(() => dependencyGraphNodeGeometry(zoom.value < 0.34))
const typeOptions = computed(() => [
    'all',
    ...new Set(graph.value.nodes.map((node) => node.projectType)),
])
const sourceOptions = ['all', 'pack', 'user', 'local'] as const
const statusOptions = ['all', 'disabled', 'missing', 'orphaned', 'cycle', 'unresolved'] as const

function typeFilterLabel(type: string) {
    return type === 'all' ? formatMessage(messages.allTypes) : type
}

function sourceFilterLabel(source: (typeof sourceOptions)[number]) {
    if (source === 'all') return formatMessage(messages.allSources)
    if (source === 'pack') return formatMessage(messages.packManaged)
    if (source === 'user') return formatMessage(messages.userAdded)
    return formatMessage(messages.localDiscovered)
}

const statusFilterMessages = {
    all: messages.allStatuses,
    cycle: messages.cycle,
    disabled: messages.disabled,
    missing: messages.missing,
    orphaned: messages.orphaned,
    unresolved: messages.unresolved,
} as const

function statusFilterLabel(status: (typeof statusOptions)[number]) {
    return formatMessage(statusFilterMessages[status])
}

function nodeMatchesBaseFilter(node: DependencyGraphNode): boolean {
    const query = searchQuery.value.trim().toLocaleLowerCase()
    if (query && !`${node.title} ${node.fileName ?? ''}`.toLocaleLowerCase().includes(query)) {
        return false
    }
    if (typeFilter.value !== 'all' && node.projectType !== typeFilter.value) return false
    if (sourceFilter.value !== 'all') {
        const ownershipKind =
            sourceFilter.value === 'pack'
                ? 'pack_managed'
                : sourceFilter.value === 'user'
                  ? 'user_added'
                  : 'local_discovered'
        if (node.ownershipKind !== ownershipKind) return false
    }
    if (statusFilter.value === 'disabled' && node.enabled !== false) return false
    if (
        statusFilter.value === 'missing' &&
        !['missing', 'pending_manual', 'removed'].includes(node.materializationState ?? '')
    ) {
        return false
    }
    if (statusFilter.value === 'orphaned' && !node.dependency.orphaned) return false
    if (statusFilter.value === 'cycle' && !node.cycle) return false
    if (statusFilter.value === 'unresolved' && node.resolved) return false
    return true
}

const matchedNodeIds = computed(
    () => new Set(graph.value.nodes.filter(nodeMatchesBaseFilter).map((node) => node.id)),
)

const relationshipNodeIds = computed(
    () => new Set(graph.value.edges.flatMap((edge) => [edge.source, edge.target])),
)

const filteredNodeIds = computed(() => {
    if (!onlyRelated.value) return matchedNodeIds.value
    return new Set([...matchedNodeIds.value].filter((id) => relationshipNodeIds.value.has(id)))
})

const filteredEdges = computed(() =>
    graph.value.edges.filter(
        (edge) => filteredNodeIds.value.has(edge.source) && filteredNodeIds.value.has(edge.target),
    ),
)

const treeRows = computed(() =>
    getDependencyTreeRows(graph.value, expandedIds.value, direction.value).filter((row) => {
        if (!row.nodeId) return true
        return filteredNodeIds.value.has(row.nodeId)
    }),
)

const selectedNode = computed(() =>
    selectedNodeId.value ? graph.value.nodeById.get(selectedNodeId.value) : undefined,
)

const hasActiveGraphFilter = computed(
    () =>
        !!searchQuery.value.trim() ||
        typeFilter.value !== 'all' ||
        sourceFilter.value !== 'all' ||
        statusFilter.value !== 'all',
)

const graphNodeIds = computed(() => {
    if (!hasActiveGraphFilter.value) return relationshipNodeIds.value
    return getRelatedNodeIds(graph.value, matchedNodeIds.value)
})

const graphLayout = computed(() =>
    layoutDependencyGraph(graph.value, graphNodeIds.value, nodeOffsets.value, graphGeometry.value),
)

const focusedGraphNodeIds = computed(() =>
    selectedNodeId.value
        ? getRelatedNodeIds(graph.value, new Set([selectedNodeId.value]))
        : graphNodeIds.value,
)

const visibleGraphNodes = computed(() => {
    const viewport = graphViewport.value
    if (!viewport || zoom.value < 0.58) return graphLayout.value.nodes
    const overscan = 180 / zoom.value
    const left = (-pan.value.x - overscan) / zoom.value
    const top = (-pan.value.y - overscan) / zoom.value
    const right = (viewport.clientWidth - pan.value.x + overscan) / zoom.value
    const bottom = (viewport.clientHeight - pan.value.y + overscan) / zoom.value
    const related = selectedNodeId.value
        ? getRelatedNodeIds(graph.value, new Set([selectedNodeId.value]))
        : new Set<string>()
    return graphLayout.value.nodes.filter(
        (node) =>
            related.has(node.id) ||
            (node.x + graphGeometry.value.nodeWidth >= left &&
                node.x <= right &&
                node.y + graphGeometry.value.nodeHeight >= top &&
                node.y <= bottom),
    )
})

const isolatedNodes = computed(() => {
    const candidates = hasActiveGraphFilter.value
        ? matchedNodeIds.value
        : new Set(graph.value.nodes.map((node) => node.id))
    return graph.value.nodes.filter(
        (node) => candidates.has(node.id) && !relationshipNodeIds.value.has(node.id),
    )
})

const statsText = computed(() =>
    formatMessage(messages.stats, {
        nodes: filteredNodeIds.value.size,
        edges: filteredEdges.value.length,
    }),
)

const graphStatsText = computed(() =>
    formatMessage(messages.stats, {
        nodes: graphLayout.value.nodes.length,
        edges: graphLayout.value.edges.length,
    }),
)

const treeHasRows = computed(() => treeRows.value.length > 0)
const graphStructureKey = computed(
    () =>
        `${[...graphNodeIds.value].sort().join('|')}::${graph.value.edges
            .filter(
                (edge) =>
                    graphNodeIds.value.has(edge.source) && graphNodeIds.value.has(edge.target),
            )
            .map((edge) => edge.id)
            .sort()
            .join('|')}`,
)

function treeRootIdsForDirection(graphData: DependencyGraph, treeDirection: DependencyDirection) {
    if (treeDirection === 'requires') return graphData.rootIds
    return graphData.nodes
        .filter((node) => !(graphData.edgesBySource.get(node.id)?.length ?? 0))
        .map((node) => node.id)
        .sort((left, right) =>
            graphData.nodeById.get(left)!.title.localeCompare(graphData.nodeById.get(right)!.title),
        )
}

function toggleExpanded(nodeId: string) {
    const next = new Set(expandedIds.value)
    if (next.has(nodeId)) next.delete(nodeId)
    else next.add(nodeId)
    expandedIds.value = next
}

function selectNode(nodeId: string) {
    selectedNodeId.value = nodeId
}

function clamp(value: number, lower: number, upper: number): number {
    return Math.min(Math.max(value, lower), upper)
}

function applyCanvasTransform(nextPan = pan.value) {
    if (!graphCanvas.value) return
    graphCanvas.value.style.transform = `translate3d(${nextPan.x}px, ${nextPan.y}px, 0) scale(${zoom.value})`
}

function scheduleEdgeDraw() {
    if (edgeFrame) return
    edgeFrame = requestAnimationFrame(() => {
        edgeFrame = undefined
        drawGraphEdges()
    })
}

function drawGraphEdges() {
    const canvas = graphEdgesCanvas.value
    if (!canvas) return
    const width = graphLayout.value.width
    const height = graphLayout.value.height
    const deviceScale = window.devicePixelRatio || 1
    if (
        canvas.width !== Math.ceil(width * deviceScale) ||
        canvas.height !== Math.ceil(height * deviceScale)
    ) {
        canvas.width = Math.ceil(width * deviceScale)
        canvas.height = Math.ceil(height * deviceScale)
        canvas.style.width = `${width}px`
        canvas.style.height = `${height}px`
    }
    const context = canvas.getContext('2d')
    if (!context) return
    context.setTransform(deviceScale, 0, 0, deviceScale, 0, 0)
    context.clearRect(0, 0, width, height)
    const styles = getComputedStyle(canvas)
    const activeColor = styles.getPropertyValue('--color-brand').trim() || '#8bd450'
    const mutedColor = styles.getPropertyValue('--surface-5').trim() || '#697384'
    const orangeColor = styles.getPropertyValue('--color-orange').trim() || '#f2a65a'
    const positions = new Map(graphLayout.value.nodes.map((node) => [node.id, node]))
    const geometry = graphGeometry.value
    for (const edge of graphLayout.value.edges) {
        const source = positions.get(edge.source)
        const target = positions.get(edge.target)
        if (!source || !target) continue
        const active =
            !selectedNodeId.value ||
            edge.source === selectedNodeId.value ||
            edge.target === selectedNodeId.value
        const color = !edge.resolved ? orangeColor : active ? activeColor : mutedColor
        context.globalAlpha = active ? 0.9 : 0.18
        context.strokeStyle = color
        context.fillStyle = color
        context.lineWidth = active ? 2 : 1
        const sourceBounds = dependencyGraphNodeBounds(source, geometry)
        const targetBounds = dependencyGraphNodeBounds(target, geometry)
        const startX = sourceBounds.right
        const startY = sourceBounds.centerY
        const endX = targetBounds.left
        const endY = targetBounds.centerY
        const curve = Math.max(48, Math.abs(endX - startX) * 0.36)
        context.beginPath()
        context.moveTo(startX, startY)
        context.bezierCurveTo(startX + curve, startY, endX - curve, endY, endX, endY)
        context.stroke()
        const angle = Math.atan2(endY - startY, endX - startX)
        context.save()
        context.translate(endX, endY)
        context.rotate(angle)
        context.beginPath()
        context.moveTo(0, 0)
        context.lineTo(-10, -5)
        context.lineTo(-10, 5)
        context.closePath()
        context.fill()
        context.restore()
    }
    context.globalAlpha = 1
}

function constrainedPan(nextPan: Point, nextZoom = zoom.value): Point {
    const viewport = graphViewport.value
    const bounds = graphContentBounds()
    if (!viewport || !bounds) return nextPan
    const scaledWidth = bounds.width * nextZoom
    const scaledHeight = bounds.height * nextZoom
    const centerX = (viewport.clientWidth - scaledWidth) / 2 - bounds.minX * nextZoom
    const centerY = (viewport.clientHeight - scaledHeight) / 2 - bounds.minY * nextZoom
    return {
        x: clamp(
            nextPan.x,
            scaledWidth <= viewport.clientWidth
                ? centerX
                : viewport.clientWidth - (bounds.minX + bounds.width) * nextZoom - viewportPadding,
            scaledWidth <= viewport.clientWidth
                ? centerX
                : viewportPadding - bounds.minX * nextZoom,
        ),
        y: clamp(
            nextPan.y,
            scaledHeight <= viewport.clientHeight
                ? centerY
                : viewport.clientHeight -
                      (bounds.minY + bounds.height) * nextZoom -
                      viewportPadding,
            scaledHeight <= viewport.clientHeight
                ? centerY
                : viewportPadding - bounds.minY * nextZoom,
        ),
    }
}

function graphContentBounds(visibleIds = focusedGraphNodeIds.value) {
    const nodes = graphLayout.value.nodes.filter((node) => visibleIds.has(node.id))
    if (!nodes.length) return undefined
    const geometry = graphGeometry.value
    const bounds = nodes.map((node) => dependencyGraphNodeBounds(node, geometry))
    const minX = Math.min(...bounds.map((node) => node.left))
    const minY = Math.min(...bounds.map((node) => node.top))
    const maxX = Math.max(...bounds.map((node) => node.right))
    const maxY = Math.max(...bounds.map((node) => node.bottom))
    return { minX, minY, width: maxX - minX, height: maxY - minY }
}

function fitGraph() {
    const viewport = graphViewport.value
    const bounds = graphContentBounds()
    if (!viewport || !bounds) return
    const availableWidth = Math.max(1, viewport.clientWidth - viewportPadding * 2)
    const availableHeight = Math.max(1, viewport.clientHeight - viewportPadding * 2)
    zoom.value = clamp(
        Math.min(1, availableWidth / bounds.width, availableHeight / bounds.height),
        minZoom,
        maxZoom,
    )
    pan.value = {
        x: (viewport.clientWidth - bounds.width * zoom.value) / 2 - bounds.minX * zoom.value,
        y: (viewport.clientHeight - bounds.height * zoom.value) / 2 - bounds.minY * zoom.value,
    }
    applyCanvasTransform()
}

function constrainGraphPan() {
    constrainFrame = undefined
    pan.value = constrainedPan(activePan ?? pan.value)
    activePan = undefined
    applyCanvasTransform()
}

function schedulePanConstraint() {
    if (constrainFrame) cancelAnimationFrame(constrainFrame)
    constrainFrame = requestAnimationFrame(constrainGraphPan)
}

function scheduleGraphFit() {
    if (fitFrame) cancelAnimationFrame(fitFrame)
    fitFrame = requestAnimationFrame(() => {
        fitFrame = undefined
        if (viewMode.value === 'graph') fitGraph()
    })
}

function resetGraphView() {
    nodeOffsets.value = new Map()
    nextTick(() => {
        scheduleGraphFit()
    })
}

function zoomTo(nextZoom: number, anchor?: Point) {
    const viewport = graphViewport.value
    const clampedZoom = clamp(nextZoom, minZoom, maxZoom)
    if (!viewport || clampedZoom === zoom.value) return
    const focalPoint = anchor ?? { x: viewport.clientWidth / 2, y: viewport.clientHeight / 2 }
    const previousBounds =
        zoom.value < 0.34 !== clampedZoom < 0.34 ? graphContentBounds() : undefined
    const previousZoom = zoom.value
    const previousPan = pan.value
    const graphPoint = {
        x: (focalPoint.x - pan.value.x) / zoom.value,
        y: (focalPoint.y - pan.value.y) / zoom.value,
    }
    zoom.value = clampedZoom
    if (previousBounds) {
        const nextBounds = graphContentBounds()
        if (!nextBounds) return
        const previousCenter = {
            x: previousBounds.minX + previousBounds.width / 2,
            y: previousBounds.minY + previousBounds.height / 2,
        }
        const nextCenter = {
            x: nextBounds.minX + nextBounds.width / 2,
            y: nextBounds.minY + nextBounds.height / 2,
        }
        pan.value = constrainedPan(
            {
                x: previousPan.x + previousCenter.x * previousZoom - nextCenter.x * clampedZoom,
                y: previousPan.y + previousCenter.y * previousZoom - nextCenter.y * clampedZoom,
            },
            clampedZoom,
        )
        applyCanvasTransform()
        return
    }
    if (clampedZoom === minZoom) {
        const bounds = graphContentBounds()
        if (!bounds) return
        pan.value = {
            x: (viewport.clientWidth - bounds.width * clampedZoom) / 2 - bounds.minX * clampedZoom,
            y:
                (viewport.clientHeight - bounds.height * clampedZoom) / 2 -
                bounds.minY * clampedZoom,
        }
        applyCanvasTransform()
        return
    }
    pan.value = constrainedPan(
        {
            x: focalPoint.x - graphPoint.x * clampedZoom,
            y: focalPoint.y - graphPoint.y * clampedZoom,
        },
        clampedZoom,
    )
    applyCanvasTransform()
}

function handleWheel(event: WheelEvent) {
    const viewport = graphViewport.value
    if (!viewport) return
    event.preventDefault()
    const rect = viewport.getBoundingClientRect()
    zoomTo(zoom.value * (event.deltaY > 0 ? 0.88 : 1.12), {
        x: event.clientX - rect.left,
        y: event.clientY - rect.top,
    })
}

function startPan(event: PointerEvent) {
    if (event.button !== 0) return
    if ((event.target as Element)?.closest('[data-dependency-node], [data-dependency-control]'))
        return
    event.preventDefault()
    selectedNodeId.value = undefined
    dragState.value = { kind: 'pan' }
    activePointerId = event.pointerId
    lastPointerPosition = { x: event.clientX, y: event.clientY }
    graphViewport.value?.setPointerCapture(event.pointerId)
}

function startNodeDrag(event: PointerEvent, nodeId: string) {
    if (event.button !== 0) return
    event.stopPropagation()
    selectNode(nodeId)
    draggedNodeId.value = nodeId
    dragState.value = { kind: 'node', id: nodeId }
    activePointerId = event.pointerId
    lastPointerPosition = { x: event.clientX, y: event.clientY }
    graphViewport.value?.setPointerCapture(event.pointerId)
}

function applyPointerMove() {
    pointerFrame = undefined
    const state = dragState.value
    const movement = pendingPointerMove
    pendingPointerMove = undefined
    if (!state || !movement) return
    if (state.kind === 'pan') {
        activePan = constrainedPan({
            x: (activePan ?? pan.value).x + movement.dx,
            y: (activePan ?? pan.value).y + movement.dy,
        })
        applyCanvasTransform(activePan)
        return
    }
    if (!state.id) return
    const node = graphLayout.value.nodes.find((candidate) => candidate.id === state.id)
    if (!node) return
    const nextOffsets = new Map(nodeOffsets.value)
    const offset = nextOffsets.get(state.id) ?? { x: 0, y: 0 }
    nextOffsets.set(state.id, {
        x: offset.x + movement.dx / zoom.value,
        y: offset.y + movement.dy / zoom.value,
    })
    nodeOffsets.value = nextOffsets
    scheduleEdgeDraw()
}

function movePointer(event: PointerEvent) {
    if (!dragState.value || !lastPointerPosition) return
    const dx = event.clientX - lastPointerPosition.x
    const dy = event.clientY - lastPointerPosition.y
    lastPointerPosition = { x: event.clientX, y: event.clientY }
    const pending = pendingPointerMove ?? { dx: 0, dy: 0 }
    pendingPointerMove = { dx: pending.dx + dx, dy: pending.dy + dy }
    if (!pointerFrame) pointerFrame = requestAnimationFrame(applyPointerMove)
}

function endPointer() {
    if (pointerFrame) {
        cancelAnimationFrame(pointerFrame)
        pointerFrame = undefined
    }
    applyPointerMove()
    if (activePan) {
        pan.value = activePan
        activePan = undefined
    }
    lastPointerPosition = undefined
    dragState.value = undefined
    draggedNodeId.value = undefined
    if (activePointerId !== undefined && graphViewport.value?.hasPointerCapture(activePointerId)) {
        graphViewport.value.releasePointerCapture(activePointerId)
    }
    activePointerId = undefined
}

function nodeLink(node: DependencyGraphNode) {
    if (!node.resolved || !node.projectId) return undefined
    if (node.provider === 'curseforge')
        return `/project/curseforge/${node.projectId.replace(/^curseforge:/, '')}`
    if (node.provider === 'modrinth') return `/project/${node.projectId}`
    return undefined
}

function nodeLinkQuery() {
    return props.instanceId ? { i: props.instanceId, from: 'instance-content' } : undefined
}

function statusLabel(node: DependencyGraphNode) {
    if (!node.resolved) return formatMessage(messages.unresolved)
    if (node.cycle) return formatMessage(messages.cycle)
    if (node.dependency.orphaned) return formatMessage(messages.orphaned)
    if (node.materializationState === 'missing' || node.materializationState === 'pending_manual') {
        return formatMessage(messages.missing)
    }
    if (node.enabled === false) return formatMessage(messages.disabled)
    return undefined
}

function sourceLabel(node: DependencyGraphNode) {
    if (node.ownershipKind === 'pack_managed') return formatMessage(messages.packManaged)
    if (node.ownershipKind === 'user_added') return formatMessage(messages.userAdded)
    if (node.ownershipKind === 'local_discovered') return formatMessage(messages.localDiscovered)
    return node.provider
}

function nodeStatusClass(node: DependencyGraphNode) {
    if (!node.resolved) return 'border-orange bg-surface-2 node-shadow-orange'
    if (node.cycle) return 'border-red bg-surface-2 node-shadow-red'
    if (node.enabled === false) return 'border-surface-4 bg-surface-2 opacity-70'
    return 'border-surface-4 bg-surface-2 shadow-black/20'
}

function show(contentItems: ContentItem[]) {
    items.value = [...contentItems]
    selectedNodeId.value = undefined
    searchQuery.value = ''
    typeFilter.value = 'all'
    sourceFilter.value = 'all'
    statusFilter.value = 'all'
    onlyRelated.value = false
    showIsolated.value = false
    viewMode.value = 'tree'
    direction.value = 'requiredBy'
    nodeOffsets.value = new Map()
    expandedIds.value = new Set(
        treeRootIdsForDirection(buildDependencyGraph(contentItems), 'requiredBy'),
    )
    zoom.value = 1
    pan.value = { x: 0, y: 0 }
    nextTick(() => modal.value?.show())
}

function setItems(contentItems: ContentItem[]) {
    items.value = [...contentItems]
    const ids = new Set(buildDependencyGraph(contentItems).nodes.map((node) => node.id))
    selectedNodeId.value = ids.has(selectedNodeId.value ?? '') ? selectedNodeId.value : undefined
    expandedIds.value = new Set([...expandedIds.value].filter((id) => ids.has(id)))
    nodeOffsets.value = new Map([...nodeOffsets.value].filter(([id]) => ids.has(id)))
    nextTick(() => {
        scheduleGraphFit()
    })
}

function hide() {
    modal.value?.hide()
}

watch(direction, () => {
    expandedIds.value = new Set()
})

watch(viewMode, (mode) => {
    if (mode === 'graph') {
        nextTick(() => {
            scheduleGraphFit()
        })
    }
})

watch(graphStructureKey, () => {
    nextTick(() => {
        scheduleGraphFit()
    })
})

watch(graphLayout, scheduleEdgeDraw)
watch(selectedNodeId, () => {
    scheduleGraphFit()
    scheduleEdgeDraw()
})

watch(graphViewport, (viewport) => {
    viewportObserver?.disconnect()
    if (!viewport || typeof ResizeObserver === 'undefined') return
    viewportObserver = new ResizeObserver(schedulePanConstraint)
    viewportObserver.observe(viewport)
    nextTick(() => {
        scheduleGraphFit()
        scheduleEdgeDraw()
    })
})

onBeforeUnmount(() => {
    viewportObserver?.disconnect()
    if (fitFrame) cancelAnimationFrame(fitFrame)
    if (pointerFrame) cancelAnimationFrame(pointerFrame)
    if (constrainFrame) cancelAnimationFrame(constrainFrame)
    if (edgeFrame) cancelAnimationFrame(edgeFrame)
})

defineExpose({ show, hide, setItems })
</script>

<template>
    <NewModal
        ref="modal"
        :max-width="'80vw'"
        :width="'80vw'"
        :style="{ height: '80vh', maxHeight: '80vh' }"
        :scrollable="false"
        :no-padding="true"
        :fill-content="true"
    >
        <template #title>
            <Avatar
                v-if="props.instanceIconUrl"
                :src="props.instanceIconUrl"
                size="3rem"
                :tint-by="props.instanceName"
            />
            <div class="flex min-w-0 flex-col">
                <span class="truncate text-lg font-extrabold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.header)
                }}</span>
                <span
                    v-if="props.instanceName"
                    class="truncate text-sm font-medium text-[var(--color-text-tertiary)]"
                    >{{ props.instanceName }}</span
                >
            </div>
        </template>

        <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
            <div
                class="flex flex-wrap items-center gap-3 border-0 border-b border-solid border-surface-4 px-6 py-4"
            >
                <StyledInput
                    v-model="searchQuery"
                    :icon="SearchIcon"
                    :placeholder="formatMessage(messages.search)"
                    clearable
                    wrapper-class="min-w-[220px] flex-1"
                />
                <div class="flex items-center gap-1 rounded-xl bg-surface-2 p-1" role="tablist">
                    <Button
                        :type="viewMode === 'tree' ? 'chip' : 'quiet'"
                        size="2xs"
                        role="tab"
                        :aria-selected="viewMode === 'tree'"
                        @click="viewMode = 'tree'"
                    >
                        <ListIcon class="size-4" /> {{ formatMessage(messages.treeView) }}
                    </Button>
                    <Button
                        :type="viewMode === 'graph' ? 'chip' : 'quiet'"
                        size="2xs"
                        role="tab"
                        :aria-selected="viewMode === 'graph'"
                        @click="viewMode = 'graph'"
                    >
                        <GitGraphIcon class="size-4" /> {{ formatMessage(messages.graphView) }}
                    </Button>
                </div>
            </div>

            <div
                class="flex flex-wrap items-center gap-2 border-0 border-b border-solid border-surface-4 px-6 py-3"
            >
                <Combobox
                    v-model="typeFilter"
                    width="custom"
                    class="w-44"
                    :options="
                        typeOptions.map((value) => ({ value, label: typeFilterLabel(value) }))
                    "
                />
                <Combobox
                    v-model="sourceFilter"
                    width="custom"
                    class="w-44"
                    :options="
                        sourceOptions.map((value) => ({ value, label: sourceFilterLabel(value) }))
                    "
                />
                <Combobox
                    v-model="statusFilter"
                    width="custom"
                    class="w-44"
                    :options="
                        statusOptions.map((value) => ({ value, label: statusFilterLabel(value) }))
                    "
                />
                <div
                    v-if="viewMode === 'tree'"
                    class="flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                >
                    <Toggle id="dependency-related-only" v-model="onlyRelated" small />
                    <span>{{ formatMessage(messages.onlyRelated) }}</span>
                </div>
                <span class="ml-auto text-sm text-[var(--color-text-tertiary)]">{{
                    statsText
                }}</span>
            </div>

            <div class="flex min-h-0 flex-1 flex-col overflow-hidden lg:flex-row">
                <div class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
                    <div
                        v-if="items.length === 0"
                        class="flex h-full items-center justify-center p-8 text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.noContent) }}
                    </div>
                    <div
                        v-else-if="filteredNodeIds.size === 0"
                        class="flex h-full items-center justify-center p-8 text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.noMatches) }}
                    </div>
                    <div v-else-if="viewMode === 'tree'" class="h-full overflow-auto p-4">
                        <div class="mb-3 flex items-center gap-2">
                            <Button
                                :type="direction === 'requires' ? 'chip' : 'quiet'"
                                size="2xs"
                                @click="direction = 'requires'"
                            >
                                {{ formatMessage(messages.requires) }}
                            </Button>
                            <Button
                                :type="direction === 'requiredBy' ? 'chip' : 'quiet'"
                                size="2xs"
                                @click="direction = 'requiredBy'"
                            >
                                {{ formatMessage(messages.requiredBy) }}
                            </Button>
                        </div>
                        <div
                            v-if="!treeHasRows"
                            class="rounded-xl border border-dashed border-surface-4 p-8 text-center text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.noDependencies) }}
                        </div>
                        <div v-else class="flex flex-col gap-1">
                            <div
                                v-for="row in treeRows"
                                :key="row.id"
                                role="treeitem"
                                tabindex="0"
                                class="tree-row-ring group flex min-h-14 w-full items-center gap-2 rounded-xl px-2 text-left transition-colors hover:bg-surface-2 focus-visible:outline-none focus-visible:ring-2"
                                :class="
                                    row.kind === 'cycle'
                                        ? 'text-red'
                                        : row.kind === 'reference'
                                          ? 'text-[var(--color-text-tertiary)]'
                                          : 'text-[var(--color-text-default)]'
                                "
                                :style="{ paddingLeft: `${row.depth * 1.5 + 0.5}rem` }"
                                @click="row.nodeId && selectNode(row.nodeId)"
                                @keydown.enter="row.nodeId && selectNode(row.nodeId)"
                            >
                                <span class="flex size-5 shrink-0 items-center justify-center">
                                    <button
                                        v-if="row.kind === 'node' && row.hasChildren"
                                        class="rounded p-0.5 hover:bg-surface-4"
                                        @click.stop="toggleExpanded(row.nodeId!)"
                                    >
                                        <ChevronDownIcon v-if="row.expanded" class="size-4" />
                                        <ChevronRightIcon v-else class="size-4" />
                                    </button>
                                    <CircleAlertIcon
                                        v-else-if="row.kind === 'cycle'"
                                        class="size-4"
                                    />
                                </span>
                                <template v-if="row.nodeId && graph.nodeById.get(row.nodeId)">
                                    <Avatar
                                        :src="graph.nodeById.get(row.nodeId)?.iconUrl"
                                        :alt="
                                            autoCleanToText(
                                                graph.nodeById.get(row.nodeId)?.title ?? '',
                                            )
                                        "
                                        size="2.5rem"
                                        no-shadow
                                    />
                                    <div class="min-w-0 flex-1">
                                        <MinecraftFormattedText
                                            :text="graph.nodeById.get(row.nodeId)?.title ?? ''"
                                            class="block truncate font-semibold text-[var(--color-text-primary)]"
                                        />
                                        <div
                                            class="mt-0.5 truncate text-xs text-[var(--color-text-tertiary)]"
                                        >
                                            {{
                                                graph.nodeById.get(row.nodeId)?.versionNumber ??
                                                graph.nodeById.get(row.nodeId)?.fileName
                                            }}
                                        </div>
                                    </div>
                                    <span
                                        v-if="row.kind === 'reference'"
                                        class="shrink-0 text-xs text-[var(--color-text-tertiary)]"
                                    >
                                        {{ formatMessage(messages.reference) }}
                                    </span>
                                    <span
                                        v-else-if="row.kind === 'cycle'"
                                        class="shrink-0 text-xs text-red"
                                    >
                                        {{ formatMessage(messages.cycleReference) }}
                                    </span>
                                    <span
                                        v-else-if="statusLabel(graph.nodeById.get(row.nodeId)!)"
                                        class="shrink-0 rounded-md bg-surface-3 px-2 py-1 text-xs font-medium"
                                    >
                                        {{ statusLabel(graph.nodeById.get(row.nodeId)!) }}
                                    </span>
                                </template>
                            </div>
                        </div>
                    </div>

                    <div v-else class="flex min-h-0 flex-1 flex-col overflow-hidden p-4">
                        <div
                            class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl border border-solid border-surface-5 bg-surface-1 shadow-sm"
                        >
                            <div
                                class="flex shrink-0 flex-wrap items-center justify-between gap-3 border-0 border-b border-solid border-surface-4 bg-surface-2 px-4 py-3"
                            >
                                <div class="flex min-w-0 items-center gap-2">
                                    <span
                                        class="graph-icon-box flex size-8 shrink-0 items-center justify-center rounded-lg text-brand"
                                    >
                                        <GitGraphIcon class="size-4" />
                                    </span>
                                    <span
                                        class="truncate text-sm text-[var(--color-text-tertiary)]"
                                        >{{ formatMessage(messages.graphDirection) }}</span
                                    >
                                </div>
                                <div class="flex items-center gap-3">
                                    <span
                                        class="shrink-0 text-xs text-[var(--color-text-tertiary)]"
                                        >{{ graphStatsText }}</span
                                    >
                                    <div
                                        v-if="isolatedNodes.length"
                                        class="flex items-center gap-2 text-xs text-[var(--color-text-tertiary)]"
                                    >
                                        <Toggle
                                            id="dependency-show-isolated"
                                            v-model="showIsolated"
                                            small
                                        />
                                        <span>{{ formatMessage(messages.showIsolated) }}</span>
                                    </div>
                                </div>
                            </div>

                            <div
                                v-if="graphLayout.edges.length === 0"
                                class="flex min-h-0 flex-1 items-center justify-center p-8 text-center text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.noGraphRelations) }}
                            </div>
                            <div
                                v-else
                                ref="graphViewport"
                                class="dependency-graph-viewport relative min-h-0 flex-1 overflow-hidden touch-none"
                                @wheel="handleWheel"
                                @pointerdown.capture="startPan"
                                @pointermove.capture="movePointer"
                                @pointerup.capture="endPointer"
                                @pointercancel.capture="endPointer"
                            >
                                <div
                                    data-dependency-control
                                    class="absolute right-3 top-3 z-20 flex items-center gap-1 rounded-xl border border-solid border-surface-4 bg-surface-2 p-1 shadow-lg"
                                >
                                    <Button
                                        type="quiet"
                                        size="2xs"
                                        circular
                                        icon-only
                                        :aria-label="formatMessage(messages.zoomOut)"
                                        @click="zoomTo(zoom - 0.1)"
                                        ><ZoomOutIcon />
                                    </Button>
                                    <span
                                        class="min-w-10 text-center text-xs tabular-nums text-[var(--color-text-tertiary)]"
                                    >
                                        {{ Math.round(zoom * 100) }}%
                                    </span>
                                    <Button
                                        type="quiet"
                                        size="2xs"
                                        circular
                                        icon-only
                                        :aria-label="formatMessage(messages.zoomIn)"
                                        @click="zoomTo(zoom + 0.1)"
                                        ><ZoomInIcon />
                                    </Button>
                                    <Button
                                        type="quiet"
                                        size="2xs"
                                        circular
                                        icon-only
                                        :aria-label="formatMessage(messages.resetZoom)"
                                        @click="resetGraphView"
                                        ><RotateCounterClockwiseIcon />
                                    </Button>
                                </div>

                                <div
                                    class="pointer-events-none absolute bottom-3 left-3 z-20 max-w-[min(28rem,calc(100%-6rem))] rounded-lg border border-solid border-surface-4 bg-surface-2 px-2.5 py-1.5 text-xs text-[var(--color-text-tertiary)] shadow-sm"
                                >
                                    {{ formatMessage(messages.graphHint) }}
                                </div>

                                <div
                                    ref="graphCanvas"
                                    class="dependency-graph-canvas absolute left-0 top-0"
                                    :style="{
                                        width: `${graphLayout.width}px`,
                                        height: `${graphLayout.height}px`,
                                        transform: `translate3d(${pan.x}px, ${pan.y}px, 0) scale(${zoom})`,
                                        transformOrigin: 'top left',
                                    }"
                                >
                                    <canvas
                                        ref="graphEdgesCanvas"
                                        class="dependency-graph-edges pointer-events-none absolute left-0 top-0"
                                        :width="graphLayout.width"
                                        :height="graphLayout.height"
                                    />
                                    <div
                                        v-for="component in graphLayout.components"
                                        :key="component.id"
                                        class="dependency-graph-component"
                                        :style="{
                                            left: `${component.x - 20}px`,
                                            top: `${component.y - 20}px`,
                                            width: `${component.width + 40}px`,
                                            height: `${component.height + 40}px`,
                                        }"
                                    />
                                    <div
                                        v-for="node in visibleGraphNodes"
                                        :key="node.id"
                                        data-dependency-node
                                        class="dependency-graph-node absolute flex cursor-grab select-none items-center gap-3 rounded-2xl border-2 px-3 shadow-lg transition-[box-shadow,opacity,transform] active:cursor-grabbing"
                                        :class="[
                                            nodeStatusClass(node),
                                            selectedNodeId && selectedNodeId !== node.id
                                                ? 'opacity-35'
                                                : '',
                                            zoom < 0.34 ? 'dependency-graph-node-compact' : '',
                                            draggedNodeId === node.id
                                                ? 'z-10 scale-[1.03] shadow-xl'
                                                : '',
                                        ]"
                                        :style="{
                                            left: `${node.x}px`,
                                            top: `${node.y}px`,
                                            width: `${graphGeometry.nodeWidth}px`,
                                            height: `${graphGeometry.nodeHeight}px`,
                                        }"
                                        @pointerdown="(event) => startNodeDrag(event, node.id)"
                                        @click.stop="selectNode(node.id)"
                                    >
                                        <span
                                            class="dependency-graph-port dependency-graph-port-input"
                                        />
                                        <Avatar
                                            :src="node.iconUrl"
                                            :alt="autoCleanToText(node.title)"
                                            size="2.75rem"
                                            no-shadow
                                        />
                                        <div class="min-w-0 flex-1">
                                            <MinecraftFormattedText
                                                :text="node.title"
                                                class="block truncate text-sm font-bold text-[var(--color-text-primary)]"
                                            />
                                            <div
                                                class="mt-0.5 truncate text-xs text-[var(--color-text-tertiary)]"
                                            >
                                                {{ statusLabel(node) ?? sourceLabel(node) }}
                                            </div>
                                        </div>
                                        <span
                                            class="dependency-graph-port dependency-graph-port-output"
                                        />
                                    </div>
                                </div>
                            </div>

                            <div
                                v-if="isolatedNodes.length"
                                class="shrink-0 border-0 border-t border-solid border-surface-4 bg-surface-2 px-4 py-3"
                            >
                                <p class="m-0 text-xs text-[var(--color-text-tertiary)]">
                                    {{
                                        formatMessage(messages.isolatedSummary, {
                                            count: isolatedNodes.length,
                                        })
                                    }}
                                </p>
                                <div
                                    v-if="showIsolated"
                                    class="mt-3 flex max-h-24 flex-wrap gap-2 overflow-auto pr-1"
                                >
                                    <button
                                        v-for="node in isolatedNodes"
                                        :key="node.id"
                                        class="flex max-w-52 items-center gap-2 rounded-lg border border-solid border-surface-4 bg-surface-1 px-2 py-1.5 text-left text-xs text-[var(--color-text-tertiary)] transition-colors hover:border-brand hover:text-[var(--color-text-primary)]"
                                        @click="selectNode(node.id)"
                                    >
                                        <Avatar
                                            :src="node.iconUrl"
                                            :alt="autoCleanToText(node.title)"
                                            size="1.5rem"
                                            no-shadow
                                        />
                                        <MinecraftFormattedText
                                            :text="node.title"
                                            class="truncate"
                                        />
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                <aside
                    v-if="selectedNode"
                    class="flex min-h-0 w-full shrink-0 flex-col gap-4 overflow-y-auto border-0 border-t border-solid border-surface-4 bg-surface-1 p-5 lg:w-[310px] lg:border-l lg:border-t-0"
                >
                    <div class="flex items-center gap-3">
                        <Avatar
                            :src="selectedNode.iconUrl"
                            :alt="autoCleanToText(selectedNode.title)"
                            size="3rem"
                            no-shadow
                        />
                        <div class="min-w-0">
                            <RouterLink
                                v-if="nodeLink(selectedNode)"
                                :to="{ path: nodeLink(selectedNode), query: nodeLinkQuery() }"
                                class="block truncate font-semibold text-[var(--color-text-primary)] hover:underline"
                            >
                                <MinecraftFormattedText :text="selectedNode.title" />
                            </RouterLink>
                            <MinecraftFormattedText
                                v-else
                                :text="selectedNode.title"
                                class="block truncate font-semibold text-[var(--color-text-primary)]"
                            />
                            <span class="text-sm text-[var(--color-text-tertiary)]">{{
                                sourceLabel(selectedNode)
                            }}</span>
                        </div>
                    </div>
                    <p v-if="!selectedNode.resolved" class="m-0 text-sm text-orange">
                        {{ formatMessage(messages.unresolvedLabel) }}
                    </p>
                    <div class="flex flex-col gap-2 text-sm">
                        <span v-if="selectedNode.versionNumber">{{
                            formatMessage(messages.version, { version: selectedNode.versionNumber })
                        }}</span>
                        <span v-if="selectedNode.fileName">{{
                            formatMessage(messages.file, { file: selectedNode.fileName })
                        }}</span>
                        <span
                            >{{ formatMessage(messages.source) }}:
                            {{ sourceLabel(selectedNode) }}</span
                        >
                        <span v-if="statusLabel(selectedNode)">
                            {{ formatMessage(messages.status) }}: {{ statusLabel(selectedNode) }}
                        </span>
                    </div>
                    <div class="flex flex-col gap-2">
                        <strong class="text-sm text-[var(--color-text-primary)]">{{
                            formatMessage(messages.directDependencies)
                        }}</strong>
                        <button
                            v-for="edge in graph.edgesBySource.get(selectedNode.id)"
                            :key="edge.id"
                            class="truncate text-left text-sm text-brand hover:underline"
                            @click="selectNode(edge.target)"
                        >
                            <MinecraftFormattedText
                                :text="graph.nodeById.get(edge.target)?.title ?? ''"
                            />
                        </button>
                        <span
                            v-if="!graph.edgesBySource.get(selectedNode.id)?.length"
                            class="text-sm text-[var(--color-text-tertiary)]"
                        >
                            -
                        </span>
                    </div>
                    <div class="flex flex-col gap-2">
                        <strong class="text-sm text-[var(--color-text-primary)]">{{
                            formatMessage(messages.directDependents)
                        }}</strong>
                        <button
                            v-for="edge in graph.edgesByTarget.get(selectedNode.id)"
                            :key="edge.id"
                            class="truncate text-left text-sm text-brand hover:underline"
                            @click="selectNode(edge.source)"
                        >
                            <MinecraftFormattedText
                                :text="graph.nodeById.get(edge.source)?.title ?? ''"
                            />
                        </button>
                        <span
                            v-if="!graph.edgesByTarget.get(selectedNode.id)?.length"
                            class="text-sm text-[var(--color-text-tertiary)]"
                        >
                            -
                        </span>
                    </div>
                </aside>
            </div>
        </div>
    </NewModal>
</template>

<style scoped>
.dependency-graph-viewport {
    min-height: 0;
    background-color: var(--surface-1);
    background-image:
        linear-gradient(color-mix(in srgb, var(--surface-4) 76%, transparent) 1px, transparent 1px),
        linear-gradient(
            90deg,
            color-mix(in srgb, var(--surface-4) 76%, transparent) 1px,
            transparent 1px
        );
    background-position: -1px -1px;
    background-size: 24px 24px;
}

.dependency-graph-canvas {
    will-change: transform;
}

.dependency-graph-edges {
    z-index: 1;
    will-change: contents;
}

.dependency-graph-component {
    position: absolute;
    z-index: 0;
    border: 1px solid var(--surface-4);
    border-radius: 22px;
    background: color-mix(in srgb, var(--surface-2) 70%, transparent);
}

.dependency-graph-node {
    z-index: 2;
}

.dependency-graph-node-compact {
    gap: 0;
    justify-content: center;
    padding: 4px;
    border-radius: 999px;
}

.dependency-graph-node-compact > div,
.dependency-graph-node-compact .dependency-graph-port {
    display: none;
}

.dependency-graph-port {
    position: absolute;
    top: 50%;
    display: block;
    width: 9px;
    height: 9px;
    border: 2px solid var(--surface-2);
    border-radius: 999px;
    background: var(--color-brand);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-brand) 50%, transparent);
    transform: translateY(-50%);
}

.dependency-graph-port-input {
    left: -6px;
}

.dependency-graph-port-output {
    right: -6px;
}

.node-shadow-orange {
    --tw-shadow-color: color-mix(
        in srgb,
        var(--color-orange) var(--opacity-ratio-keep-15),
        transparent
    );
    --tw-shadow: var(--tw-shadow-colored);
}

.node-shadow-red {
    --tw-shadow-color: color-mix(
        in srgb,
        var(--color-red) var(--opacity-ratio-keep-15),
        transparent
    );
    --tw-shadow: var(--tw-shadow-colored);
}

.tree-row-ring:focus-visible {
    --tw-ring-color: color-mix(
        in srgb,
        var(--color-brand) var(--opacity-ratio-keep-40),
        transparent
    );
}

.graph-icon-box {
    background-color: color-mix(
        in srgb,
        var(--color-brand) var(--opacity-ratio-keep-10),
        transparent
    );
}
</style>
