import type { ContentItem } from '@modrinth/ui'

export type DependencyDirection = 'requires' | 'requiredBy'

export const dependencyGraphMetrics = {
    canvasPadding: 56,
    componentGap: 96,
    edgeClearance: 2,
    layerGap: 112,
    minHeight: 360,
    minWidth: 640,
    nodeHeight: 76,
    nodeWidth: 228,
    rowGap: 30,
} as const

export type DependencyGraphNodeGeometry = Pick<
    typeof dependencyGraphMetrics,
    'nodeWidth' | 'nodeHeight'
>

export type DependencyGraphNodeBounds = DependencyGraphNodeGeometry & {
    left: number
    top: number
    right: number
    bottom: number
    centerX: number
    centerY: number
}

export function dependencyGraphNodeGeometry(compact: boolean): DependencyGraphNodeGeometry {
    return compact
        ? { nodeWidth: 48, nodeHeight: 48 }
        : {
              nodeWidth: dependencyGraphMetrics.nodeWidth,
              nodeHeight: dependencyGraphMetrics.nodeHeight,
          }
}

export function dependencyGraphNodeBounds(
    node: Pick<DependencyGraphNode, 'x' | 'y'>,
    geometry: DependencyGraphNodeGeometry,
): DependencyGraphNodeBounds {
    const centerX = node.x + geometry.nodeWidth / 2
    const centerY = node.y + geometry.nodeHeight / 2
    return {
        ...geometry,
        left: centerX - geometry.nodeWidth / 2,
        top: centerY - geometry.nodeHeight / 2,
        right: centerX + geometry.nodeWidth / 2,
        bottom: centerY + geometry.nodeHeight / 2,
        centerX,
        centerY,
    }
}

export type DependencyGraphNode = {
    id: string
    title: string
    iconUrl?: string
    projectId?: string
    versionId?: string
    versionNumber?: string
    fileName?: string
    projectType: string
    provider: string
    ownershipKind?: ContentItem['instanceOwnershipKind']
    enabled?: boolean
    materializationState?: ContentItem['instanceMaterializationState']
    dependency: NonNullable<ContentItem['dependency']>
    resolved: boolean
    item?: ContentItem
    cycle: boolean
    shared: boolean
}

export type DependencyGraphEdge = {
    id: string
    source: string
    target: string
    resolved: boolean
}

export type DependencyGraph = {
    nodes: DependencyGraphNode[]
    edges: DependencyGraphEdge[]
    nodeById: Map<string, DependencyGraphNode>
    edgesBySource: Map<string, DependencyGraphEdge[]>
    edgesByTarget: Map<string, DependencyGraphEdge[]>
    rootIds: string[]
    cycleIds: Set<string>
    unresolvedIds: Set<string>
}

export type DependencyTreeRow = {
    id: string
    nodeId?: string
    depth: number
    kind: 'node' | 'reference' | 'cycle'
    hasChildren: boolean
    expanded: boolean
}

export type DependencyGraphConnector = {
    length: number
    rotation: number
    x: number
    y: number
}

export type DependencyGraphLayoutEdge = DependencyGraphEdge & {
    connector: DependencyGraphConnector
}

export type DependencyGraphComponent = {
    edgeCount: number
    height: number
    id: string
    nodeIds: string[]
    width: number
    x: number
    y: number
}

export type DependencyGraphLayout = {
    components: DependencyGraphComponent[]
    edges: DependencyGraphLayoutEdge[]
    height: number
    nodes: Array<DependencyGraphNode & { x: number; y: number }>
    width: number
}

type DependencyReference = {
    provider: string
    projectId: string
    releaseId: string
}

type NodePosition = {
    x: number
    y: number
}

type ComponentLayout = {
    edges: DependencyGraphLayoutEdge[]
    height: number
    nodeIds: string[]
    nodes: Array<DependencyGraphNode & NodePosition>
    width: number
}

const emptyDependency = (): NonNullable<ContentItem['dependency']> => ({
    autoDependency: false,
    requiredBy: [],
    requires: [],
    orphaned: false,
})

function normalizeProjectId(provider: string, projectId: string): string {
    return provider === 'curseforge' ? projectId.replace(/^curseforge:/, '') : projectId
}

function referenceKey(reference: DependencyReference): string {
    return `${reference.provider}:${normalizeProjectId(reference.provider, reference.projectId)}:${reference.releaseId}`
}

function itemReferenceKeys(item: ContentItem): Set<string> {
    const keys = new Set<string>()
    const projectId = item.project?.id
    const versionId = item.version?.id
    const provider = item.origin_provider ?? item.provider_refs[0]?.provider

    if (projectId) keys.add(`project:${projectId}`)
    if (provider && projectId) {
        const normalizedProjectId = normalizeProjectId(provider, projectId)
        keys.add(`${provider}:${normalizedProjectId}:`)
        if (versionId) keys.add(`${provider}:${normalizedProjectId}:${versionId}`)
    }

    for (const providerRef of item.provider_refs) {
        if (providerRef.provider === 'modrinth') {
            keys.add(`modrinth:${providerRef.project_id}:${providerRef.version_id ?? ''}`)
            keys.add(`modrinth:${providerRef.project_id}:`)
        } else {
            keys.add(`curseforge:${providerRef.project_id}:${providerRef.file_id ?? ''}`)
            keys.add(`curseforge:${providerRef.project_id}:`)
        }
    }

    return keys
}

function nodeIdForItem(item: ContentItem): string {
    if (item.instanceEntryId) return `item:entry:${item.instanceEntryId}`
    if (item.instanceMemberId) return `item:member:${item.instanceMemberId}`
    if (item.instanceFileId) return `item:file:${item.instanceFileId}`

    const provider = item.origin_provider ?? item.provider_refs[0]?.provider
    const projectId = item.project?.id
    if (provider && projectId) {
        return `item:${provider}:${normalizeProjectId(provider, projectId)}:${item.version?.id ?? ''}`
    }
    if (item.file_path) return `item:path:${item.file_path}`
    if (item.file_name) return `item:name:${item.file_name}`
    return `item:id:${item.id}`
}

function nodeFromItem(item: ContentItem, id: string): DependencyGraphNode {
    const dependency = item.dependency ?? emptyDependency()
    const provider = item.origin_provider ?? item.provider_refs[0]?.provider ?? 'local'
    return {
        id,
        title: item.project?.title ?? item.file_name,
        iconUrl: item.project?.icon_url,
        projectId: item.project?.id,
        versionId: item.version?.id,
        versionNumber: item.version?.version_number,
        fileName: item.file_name,
        projectType: item.project_type,
        provider,
        ownershipKind: item.instanceOwnershipKind,
        enabled: item.enabled,
        materializationState: item.instanceMaterializationState,
        dependency,
        resolved: true,
        item,
        cycle: false,
        shared: false,
    }
}

function unresolvedNode(reference: DependencyReference): DependencyGraphNode {
    return {
        id: `missing:${referenceKey(reference)}`,
        title: reference.projectId || reference.releaseId || 'Unresolved dependency',
        projectId: reference.projectId,
        versionId: reference.releaseId,
        projectType: 'unknown',
        provider: reference.provider,
        dependency: emptyDependency(),
        resolved: false,
        cycle: false,
        shared: false,
    }
}

function addNodeForReference(
    nodesByItemKey: Map<string, DependencyGraphNode[]>,
    key: string,
    node: DependencyGraphNode,
) {
    const matches = nodesByItemKey.get(key) ?? []
    if (!matches.some((candidate) => candidate.id === node.id)) matches.push(node)
    nodesByItemKey.set(key, matches)
}

function findNodesForReference(
    reference: DependencyReference,
    nodesByItemKey: Map<string, DependencyGraphNode[]>,
): DependencyGraphNode[] {
    const exact = nodesByItemKey.get(referenceKey(reference))
    if (exact?.length) return exact

    const byProviderProject = nodesByItemKey.get(
        `${reference.provider}:${normalizeProjectId(reference.provider, reference.projectId)}:`,
    )
    if (byProviderProject?.length) return byProviderProject

    return nodesByItemKey.get(`project:${reference.projectId}`) ?? []
}

function markCycles(
    nodes: DependencyGraphNode[],
    edgesBySource: Map<string, DependencyGraphEdge[]>,
): Set<string> {
    const state = new Map<string, 0 | 1 | 2>()
    const cycleIds = new Set<string>()

    function visit(id: string, stack: string[]) {
        const currentState = state.get(id) ?? 0
        if (currentState === 2) return
        if (currentState === 1) {
            const cycleStart = stack.indexOf(id)
            for (const cycleId of stack.slice(cycleStart)) cycleIds.add(cycleId)
            return
        }

        state.set(id, 1)
        for (const edge of edgesBySource.get(id) ?? []) visit(edge.target, [...stack, id])
        state.set(id, 2)
    }

    for (const node of nodes) visit(node.id, [])
    return cycleIds
}

export function buildDependencyGraph(items: ContentItem[]): DependencyGraph {
    const nodesByItemKey = new Map<string, DependencyGraphNode[]>()
    const nodes = new Map<string, DependencyGraphNode>()

    for (const item of items) {
        const id = nodeIdForItem(item)
        const node = nodeFromItem(item, id)
        nodes.set(id, node)
        for (const key of itemReferenceKeys(item)) addNodeForReference(nodesByItemKey, key, node)
    }

    const edges = new Map<string, DependencyGraphEdge>()
    const addEdge = (source: DependencyGraphNode, target: DependencyGraphNode) => {
        if (!nodes.has(source.id)) nodes.set(source.id, source)
        if (!nodes.has(target.id)) nodes.set(target.id, target)
        const id = `${source.id}->${target.id}`
        edges.set(id, { id, source: source.id, target: target.id, resolved: target.resolved })
    }
    for (const source of nodes.values()) {
        for (const reference of source.dependency.requires as DependencyReference[]) {
            const targets = findNodesForReference(reference, nodesByItemKey)
            if (targets.length) {
                for (const target of targets) addEdge(source, target)
            } else {
                addEdge(source, unresolvedNode(reference))
            }
        }
    }
    for (const target of nodes.values()) {
        for (const reference of target.dependency.requiredBy as DependencyReference[]) {
            const sources = findNodesForReference(reference, nodesByItemKey)
            if (sources.length) {
                for (const source of sources) addEdge(source, target)
            } else {
                addEdge(unresolvedNode(reference), target)
            }
        }
    }

    const allNodes = [...nodes.values()]
    const allEdges = [...edges.values()]
    const edgesBySource = new Map<string, DependencyGraphEdge[]>()
    const edgesByTarget = new Map<string, DependencyGraphEdge[]>()
    for (const edge of allEdges) {
        const sourceEdges = edgesBySource.get(edge.source) ?? []
        sourceEdges.push(edge)
        edgesBySource.set(edge.source, sourceEdges)
        const targetEdges = edgesByTarget.get(edge.target) ?? []
        targetEdges.push(edge)
        edgesByTarget.set(edge.target, targetEdges)
    }

    const cycleIds = markCycles(allNodes, edgesBySource)
    const unresolvedIds = new Set(allNodes.filter((node) => !node.resolved).map((node) => node.id))
    for (const node of allNodes) {
        node.cycle = cycleIds.has(node.id)
        node.shared = (edgesByTarget.get(node.id)?.length ?? 0) > 1
    }

    const rootIds = allNodes
        .filter((node) => !(edgesByTarget.get(node.id)?.length ?? 0))
        .map((node) => node.id)
        .sort((a, b) => (nodes.get(a)!.title ?? '').localeCompare(nodes.get(b)!.title ?? ''))
    const covered = new Set<string>()
    const visitFromRoot = (id: string) => {
        if (covered.has(id)) return
        covered.add(id)
        for (const edge of edgesBySource.get(id) ?? []) visitFromRoot(edge.target)
    }
    for (const rootId of rootIds) visitFromRoot(rootId)
    for (const node of allNodes
        .filter((candidate) => !covered.has(candidate.id))
        .sort((a, b) => a.title.localeCompare(b.title))) {
        rootIds.push(node.id)
    }

    return {
        nodes: allNodes,
        edges: allEdges,
        nodeById: nodes,
        edgesBySource,
        edgesByTarget,
        rootIds,
        cycleIds,
        unresolvedIds,
    }
}

export function getDependencyTreeRows(
    graph: DependencyGraph,
    expandedIds: Set<string>,
    direction: DependencyDirection = 'requires',
): DependencyTreeRow[] {
    const rows: DependencyTreeRow[] = []
    const seen = new Set<string>()
    const roots =
        direction === 'requires'
            ? graph.rootIds
            : graph.nodes
                  .filter((node) => !(graph.edgesBySource.get(node.id)?.length ?? 0))
                  .map((node) => node.id)
                  .sort((a, b) =>
                      graph.nodeById.get(a)!.title.localeCompare(graph.nodeById.get(b)!.title),
                  )

    function visit(nodeId: string, depth: number, stack: Set<string>) {
        const node = graph.nodeById.get(nodeId)
        if (!node) return
        const edges =
            direction === 'requires'
                ? graph.edgesBySource.get(nodeId)
                : graph.edgesByTarget.get(nodeId)
        const children = (edges ?? []).map((edge) =>
            direction === 'requires' ? edge.target : edge.source,
        )
        const expanded = expandedIds.has(nodeId)
        const firstVisit = !seen.has(nodeId)
        if (firstVisit) {
            seen.add(nodeId)
            rows.push({
                id: `node:${nodeId}:${depth}`,
                nodeId,
                depth,
                kind: 'node',
                hasChildren: children.length > 0,
                expanded,
            })
        } else {
            rows.push({
                id: `reference:${nodeId}:${depth}`,
                nodeId,
                depth,
                kind: 'reference',
                hasChildren: false,
                expanded: false,
            })
            return
        }

        if (!expanded) return
        for (const childId of children) {
            if (stack.has(childId)) {
                rows.push({
                    id: `cycle:${childId}:${depth + 1}`,
                    nodeId: childId,
                    depth: depth + 1,
                    kind: 'cycle',
                    hasChildren: false,
                    expanded: false,
                })
                continue
            }
            visit(childId, depth + 1, new Set([...stack, nodeId]))
        }
    }

    for (const rootId of roots) visit(rootId, 0, new Set())
    return rows
}

export function getRelatedNodeIds(
    graph: DependencyGraph,
    nodeIds: ReadonlySet<string>,
): Set<string> {
    const related = new Set(nodeIds)
    const queue = [...nodeIds]
    while (queue.length) {
        const id = queue.shift()!
        for (const edge of [
            ...(graph.edgesBySource.get(id) ?? []),
            ...(graph.edgesByTarget.get(id) ?? []),
        ]) {
            const next = edge.source === id ? edge.target : edge.source
            if (!related.has(next)) {
                related.add(next)
                queue.push(next)
            }
        }
    }
    return related
}

export function getConnectedComponents(
    graph: DependencyGraph,
    visibleIds: ReadonlySet<string>,
): Array<{ edgeCount: number; nodeIds: string[] }> {
    const components: Array<{ edgeCount: number; nodeIds: string[] }> = []
    const remaining = new Set(visibleIds)

    while (remaining.size) {
        const start = remaining.values().next().value as string
        const nodeIds = getRelatedNodeIds(graph, new Set([start]))
        const componentIds = [...nodeIds].filter((id) => visibleIds.has(id)).sort()
        for (const id of componentIds) remaining.delete(id)
        const edgeCount = graph.edges.filter(
            (edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target),
        ).length
        components.push({ edgeCount, nodeIds: componentIds })
    }

    return components.sort(
        (left, right) =>
            right.edgeCount - left.edgeCount ||
            right.nodeIds.length - left.nodeIds.length ||
            left.nodeIds[0]!.localeCompare(right.nodeIds[0]!),
    )
}

function nodeDepths(graph: DependencyGraph, visibleIds: ReadonlySet<string>): Map<string, number> {
    const depths = new Map<string, number>()
    const visiting = new Set<string>()

    function depth(id: string): number {
        const cached = depths.get(id)
        if (cached !== undefined) return cached
        if (visiting.has(id)) return 0
        visiting.add(id)
        const parents = (graph.edgesByTarget.get(id) ?? []).filter(
            (edge) => visibleIds.has(edge.source) && visibleIds.has(edge.target),
        )
        const value =
            parents.length === 0 ? 0 : Math.max(...parents.map((edge) => depth(edge.source) + 1))
        visiting.delete(id)
        depths.set(id, value)
        return value
    }

    for (const id of visibleIds) depth(id)
    return depths
}

export function getDependencyNodeDepths(
    graph: DependencyGraph,
    visibleIds: ReadonlySet<string>,
): Map<string, number> {
    const depths = new Map<string, number>()
    const queue = graph.nodes
        .filter(
            (node) =>
                visibleIds.has(node.id) &&
                !(graph.edgesByTarget.get(node.id) ?? []).some((edge) =>
                    visibleIds.has(edge.source),
                ),
        )
        .map((node) => node.id)
    for (const id of queue) depths.set(id, 0)
    while (queue.length) {
        const id = queue.shift()!
        const depth = depths.get(id) ?? 0
        for (const edge of graph.edgesBySource.get(id) ?? []) {
            if (!visibleIds.has(edge.target)) continue
            const nextDepth = Math.max(depths.get(edge.target) ?? 0, depth + 1)
            if (nextDepth !== depths.get(edge.target)) {
                depths.set(edge.target, nextDepth)
                queue.push(edge.target)
            }
        }
    }
    for (const id of visibleIds) {
        if (!depths.has(id)) depths.set(id, 0)
    }
    return depths
}

function edgeGeometry(
    source: NodePosition,
    target: NodePosition,
    geometry: DependencyGraphNodeGeometry,
): Pick<DependencyGraphLayoutEdge, 'connector'> {
    const x = source.x + geometry.nodeWidth + dependencyGraphMetrics.edgeClearance
    const y = source.y + geometry.nodeHeight / 2
    const endX = target.x - dependencyGraphMetrics.edgeClearance
    const endY = target.y + geometry.nodeHeight / 2
    const dx = endX - x
    const dy = endY - y

    return {
        connector: {
            length: Math.hypot(dx, dy),
            rotation: (Math.atan2(dy, dx) * 180) / Math.PI,
            x,
            y,
        },
    }
}

function layoutComponent(
    graph: DependencyGraph,
    nodeIds: string[],
    nodeOffsets: ReadonlyMap<string, NodePosition>,
    geometry: DependencyGraphNodeGeometry,
): ComponentLayout {
    const visibleIds = new Set(nodeIds)
    const nodesToLayout = graph.nodes.filter((node) => visibleIds.has(node.id))
    const depths = nodeDepths(graph, visibleIds)
    const columns = new Map<number, DependencyGraphNode[]>()
    for (const node of nodesToLayout) {
        const column = columns.get(depths.get(node.id) ?? 0) ?? []
        column.push(node)
        columns.set(depths.get(node.id) ?? 0, column)
    }
    const positions = new Map<string, NodePosition>()
    const maxRowsPerColumn = 8
    let columnX = 0
    for (const [, column] of [...columns.entries()].sort(([left], [right]) => left - right)) {
        column.sort(
            (left, right) =>
                left.title.localeCompare(right.title) || left.id.localeCompare(right.id),
        )
        const columnCount = Math.max(1, Math.ceil(column.length / maxRowsPerColumn))
        column.forEach((node, index) => {
            positions.set(node.id, {
                x:
                    columnX +
                    Math.floor(index / maxRowsPerColumn) *
                        (dependencyGraphMetrics.nodeWidth + dependencyGraphMetrics.rowGap),
                y:
                    (index % maxRowsPerColumn) *
                    (dependencyGraphMetrics.nodeHeight + dependencyGraphMetrics.rowGap),
            })
        })
        columnX +=
            columnCount * (dependencyGraphMetrics.nodeWidth + dependencyGraphMetrics.rowGap) +
            dependencyGraphMetrics.layerGap
    }

    const rawPositions = [...positions.values()]
    const minX = Math.min(...rawPositions.map((position) => position.x))
    const minY = Math.min(...rawPositions.map((position) => position.y))
    const normalizedPositions = new Map<string, NodePosition>()
    for (const [id, position] of positions) {
        const offset = nodeOffsets.get(id) ?? { x: 0, y: 0 }
        const centerX = position.x + dependencyGraphMetrics.nodeWidth / 2 + offset.x
        const centerY = position.y + dependencyGraphMetrics.nodeHeight / 2 + offset.y
        normalizedPositions.set(id, {
            x: centerX - geometry.nodeWidth / 2 - minX,
            y: centerY - geometry.nodeHeight / 2 - minY,
        })
    }

    const nodes = nodesToLayout.map((node) => ({ ...node, ...normalizedPositions.get(node.id)! }))
    const edges = graph.edges
        .filter((edge) => visibleIds.has(edge.source) && visibleIds.has(edge.target))
        .map((edge) => ({
            ...edge,
            ...edgeGeometry(
                normalizedPositions.get(edge.source)!,
                normalizedPositions.get(edge.target)!,
                geometry,
            ),
        }))

    return {
        edges,
        height: Math.max(
            dependencyGraphMetrics.nodeHeight,
            ...nodes.map(
                (node) => node.y + geometry.nodeHeight / 2 + dependencyGraphMetrics.nodeHeight / 2,
            ),
        ),
        nodeIds,
        nodes,
        width: Math.max(
            dependencyGraphMetrics.nodeWidth,
            ...nodes.map(
                (node) => node.x + geometry.nodeWidth / 2 + dependencyGraphMetrics.nodeWidth / 2,
            ),
        ),
    }
}

export function layoutDependencyGraph(
    graph: DependencyGraph,
    visibleIds: ReadonlySet<string> = new Set(graph.nodes.map((node) => node.id)),
    nodeOffsets: ReadonlyMap<string, NodePosition> = new Map(),
    geometry: DependencyGraphNodeGeometry = dependencyGraphNodeGeometry(false),
): DependencyGraphLayout {
    const relationshipIds = new Set(
        graph.edges
            .filter((edge) => visibleIds.has(edge.source) && visibleIds.has(edge.target))
            .flatMap((edge) => [edge.source, edge.target]),
    )
    const components = getConnectedComponents(graph, relationshipIds)
    if (!components.length) {
        return {
            components: [],
            edges: [],
            height: dependencyGraphMetrics.minHeight,
            nodes: [],
            width: dependencyGraphMetrics.minWidth,
        }
    }

    const layouts = components.map((component) => ({
        component,
        layout: layoutComponent(graph, component.nodeIds, nodeOffsets, geometry),
    }))
    const rowWidth = Math.max(
        dependencyGraphMetrics.minWidth - dependencyGraphMetrics.canvasPadding * 2,
        Math.max(...layouts.map(({ layout }) => layout.width)),
    )
    let cursorX = dependencyGraphMetrics.canvasPadding
    let cursorY = dependencyGraphMetrics.canvasPadding
    let rowHeight = 0
    const layoutComponents: DependencyGraphComponent[] = []
    const nodes: DependencyGraphLayout['nodes'] = []
    const edges: DependencyGraphLayout['edges'] = []

    for (const { component, layout } of layouts) {
        if (cursorX > dependencyGraphMetrics.canvasPadding && cursorX + layout.width > rowWidth) {
            cursorX = dependencyGraphMetrics.canvasPadding
            cursorY += rowHeight + dependencyGraphMetrics.componentGap
            rowHeight = 0
        }
        const id = component.nodeIds.join('|')
        layoutComponents.push({
            edgeCount: component.edgeCount,
            height: layout.height,
            id,
            nodeIds: component.nodeIds,
            width: layout.width,
            x: cursorX,
            y: cursorY,
        })
        nodes.push(
            ...layout.nodes.map((node) => ({ ...node, x: node.x + cursorX, y: node.y + cursorY })),
        )
        edges.push(
            ...layout.edges.map((edge) => {
                const source = layout.nodes.find((node) => node.id === edge.source)!
                const target = layout.nodes.find((node) => node.id === edge.target)!
                return {
                    ...edge,
                    ...edgeGeometry(
                        { x: source.x + cursorX, y: source.y + cursorY },
                        { x: target.x + cursorX, y: target.y + cursorY },
                        geometry,
                    ),
                }
            }),
        )
        cursorX += layout.width + dependencyGraphMetrics.componentGap
        rowHeight = Math.max(rowHeight, layout.height)
    }

    return {
        components: layoutComponents,
        edges,
        height: Math.max(
            dependencyGraphMetrics.minHeight,
            cursorY + rowHeight + dependencyGraphMetrics.canvasPadding,
        ),
        nodes,
        width: Math.max(
            dependencyGraphMetrics.minWidth,
            Math.max(
                ...nodes.map(
                    (node) =>
                        node.x +
                        dependencyGraphMetrics.nodeWidth +
                        dependencyGraphMetrics.canvasPadding,
                ),
            ),
        ),
    }
}
