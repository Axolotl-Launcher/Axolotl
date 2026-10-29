import type { ContentItem } from '@modrinth/ui'
import { expect, it } from 'vitest'

import {
    buildDependencyGraph,
    dependencyGraphMetrics,
    dependencyGraphNodeBounds,
    dependencyGraphNodeGeometry,
    getConnectedComponents,
    getDependencyNodeDepths,
    getDependencyTreeRows,
    getRelatedNodeIds,
    layoutDependencyGraph,
} from './dependency-graph.ts'

type Ref = { provider: 'modrinth'; projectId: string; releaseId: string }

type ItemOptions = {
    title?: string
    requires?: Ref[]
    requiredBy?: Ref[]
    autoDependency?: boolean
    entryId?: string
    filePath?: string
}

function item(projectId: string, versionId: string, options: ItemOptions = {}): ContentItem {
    return {
        id: projectId,
        file_name: `${projectId}.jar`,
        file_path: options.filePath ?? `mods/${projectId}.jar`,
        size: 1,
        enabled: true,
        project_type: 'mod',
        project: {
            id: projectId,
            slug: projectId,
            title: options.title ?? projectId,
            icon_url: null,
        },
        version: { id: versionId, version_number: versionId, file_name: `${projectId}.jar` },
        update: null,
        origin_provider: 'modrinth',
        provider_refs: [{ provider: 'modrinth', project_id: projectId, version_id: versionId }],
        instanceEntryId: options.entryId,
        dependency: {
            autoDependency: options.autoDependency ?? false,
            requires: options.requires ?? [],
            requiredBy: options.requiredBy ?? [],
            orphaned: false,
        },
    } as ContentItem
}

const ref = (projectId: string, releaseId = '1') => ({
    provider: 'modrinth' as const,
    projectId,
    releaseId,
})

function nodeId(projectId: string) {
    return `item:modrinth:${projectId}:1`
}

it('builds dependency edges, roots, shared nodes, and relationship layout', () => {
    const graph = buildDependencyGraph([
        item('a', '1', { requires: [ref('b')] }),
        item('c', '1', { requires: [ref('b')] }),
        item('b', '1'),
    ])

    expect(graph.edges.length).toBe(2)
    expect(graph.rootIds.length).toBe(2)
    expect(graph.nodeById.get(nodeId('b'))?.shared).toBe(true)
    expect(layoutDependencyGraph(graph).edges.length).toBe(2)
})

it('uses one geometry model for regular and compact graph nodes', () => {
    expect(dependencyGraphNodeGeometry(false)).toEqual({ nodeWidth: 228, nodeHeight: 76 })
    expect(dependencyGraphNodeGeometry(true)).toEqual({ nodeWidth: 48, nodeHeight: 48 })
})

it('keeps node centers stable while compacting endpoints and bounds', () => {
    const graph = buildDependencyGraph([item('a', '1', { requires: [ref('b')] }), item('b', '1')])
    const regular = layoutDependencyGraph(graph)
    const compact = layoutDependencyGraph(
        graph,
        undefined,
        new Map(),
        dependencyGraphNodeGeometry(true),
    )
    const regularSource = regular.nodes.find((node) => node.id === nodeId('a'))!
    const compactSource = compact.nodes.find((node) => node.id === nodeId('a'))!
    const regularTarget = regular.nodes.find((node) => node.id === nodeId('b'))!
    const compactTarget = compact.nodes.find((node) => node.id === nodeId('b'))!
    const regularGeometry = dependencyGraphNodeGeometry(false)
    const compactGeometry = dependencyGraphNodeGeometry(true)
    const regularSourceBounds = dependencyGraphNodeBounds(regularSource, regularGeometry)
    const compactSourceBounds = dependencyGraphNodeBounds(compactSource, compactGeometry)
    const regularTargetBounds = dependencyGraphNodeBounds(regularTarget, regularGeometry)
    const compactTargetBounds = dependencyGraphNodeBounds(compactTarget, compactGeometry)

    expect(compactSourceBounds.centerX).toBe(regularSourceBounds.centerX)
    expect(compactSourceBounds.centerY).toBe(regularSourceBounds.centerY)
    expect(compactTargetBounds.centerX).toBe(regularTargetBounds.centerX)
    expect(compactTargetBounds.centerY).toBe(regularTargetBounds.centerY)
    expect(compact.edges[0]?.connector.x).toBe(
        compactSourceBounds.right + dependencyGraphMetrics.edgeClearance,
    )
    expect(compact.edges[0]?.connector.y).toBe(compactSourceBounds.centerY)
    expect(compact.edges[0]?.connector.length).toBe(
        Math.hypot(
            compactTargetBounds.left -
                dependencyGraphMetrics.edgeClearance -
                compact.edges[0]!.connector.x,
            compactTargetBounds.centerY - compact.edges[0]!.connector.y,
        ),
    )
})

it('keeps unresolved dependency targets visible', () => {
    const graph = buildDependencyGraph([item('a', '1', { requires: [ref('missing', '9')] })])
    expect(graph.unresolvedIds.size).toBe(1)
    expect(graph.edges[0]?.resolved).toBe(false)
    expect(graph.nodeById.get('missing:modrinth:missing:9')?.title).toBe('missing')
})

it('deduplicates edges and stops tree traversal at cycles and shared references', () => {
    const graph = buildDependencyGraph([
        item('a', '1', { requires: [ref('b'), ref('b')] }),
        item('b', '1', { requires: [ref('a')] }),
    ])
    expect(graph.edges.length).toBe(2)
    expect(graph.cycleIds.size).toBe(2)

    const rows = getDependencyTreeRows(graph, new Set([nodeId('a'), nodeId('b')]))
    expect(rows.some((row) => row.kind === 'cycle')).toBeTruthy()
    expect(rows.length < 6).toBeTruthy()
})

it('preserves isolated content as a root without dependency edges', () => {
    const graph = buildDependencyGraph([item('standalone', '1')])
    expect(graph.rootIds).toEqual([nodeId('standalone')])
    expect(getDependencyTreeRows(graph, new Set())).toEqual([
        {
            id: `node:${nodeId('standalone')}:0`,
            nodeId: nodeId('standalone'),
            depth: 0,
            kind: 'node',
            hasChildren: false,
            expanded: false,
        },
    ])
    expect(layoutDependencyGraph(graph).nodes.length).toBe(0)
})

it('keeps duplicate installed copies as distinct nodes and connects each matching copy', () => {
    const graph = buildDependencyGraph([
        item('parent', '1', { requires: [ref('library')] }),
        item('library', '1', { entryId: 'library-a', filePath: 'mods/library-a.jar' }),
        item('library', '1', { entryId: 'library-b', filePath: 'mods/library-b.jar' }),
    ])

    expect(graph.nodes.filter((node) => node.projectId === 'library').length).toBe(2)
    expect(graph.edges.length).toBe(2)
    expect(graph.nodeById.has('item:entry:library-a')).toBeTruthy()
    expect(graph.nodeById.has('item:entry:library-b')).toBeTruthy()
})

it('partitions unrelated relationships into compact graph components', () => {
    const graph = buildDependencyGraph([
        item('a', '1', { requires: [ref('b')] }),
        item('b', '1'),
        item('c', '1', { requires: [ref('d')] }),
        item('d', '1'),
        item('isolated', '1'),
    ])
    const relationshipIds = new Set(graph.edges.flatMap((edge) => [edge.source, edge.target]))
    const components = getConnectedComponents(graph, relationshipIds)
    const layout = layoutDependencyGraph(graph)

    expect(components.length).toBe(2)
    expect(components.map((component) => component.nodeIds.length)).toEqual([2, 2])
    expect(layout.components.length).toBe(2)
    expect(layout.nodes.length).toBe(4)
    expect(layout.nodes.some((node) => node.id === nodeId('isolated'))).toBe(false)
})

it('returns the complete relationship context for a filtered node', () => {
    const graph = buildDependencyGraph([
        item('a', '1', { requires: [ref('b')] }),
        item('b', '1', { requires: [ref('c')] }),
        item('c', '1'),
        item('isolated', '1'),
    ])
    const related = getRelatedNodeIds(graph, new Set([nodeId('b')]))

    expect(related).toEqual(new Set([nodeId('a'), nodeId('b'), nodeId('c')]))
})

it('focus context includes all incoming and outgoing relationship closure', () => {
    const graph = buildDependencyGraph([
        item('root', '1', { requires: [ref('focus')] }),
        item('focus', '1', { requires: [ref('child')] }),
        item('child', '1', { requires: [ref('leaf')] }),
        item('leaf', '1'),
        item('unrelated', '1'),
    ])
    const focused = getRelatedNodeIds(graph, new Set([nodeId('focus')]))
    const layout = layoutDependencyGraph(graph, focused)

    expect(focused).toEqual(
        new Set([nodeId('root'), nodeId('focus'), nodeId('child'), nodeId('leaf')]),
    )
    expect(layout.nodes.length).toBe(4)
    expect(layout.nodes.every((node) => focused.has(node.id))).toBeTruthy()
    expect(layout.width >= dependencyGraphMetrics.nodeWidth).toBeTruthy()
    expect(layout.height >= dependencyGraphMetrics.nodeHeight).toBeTruthy()
})

it('uses deterministic directional columns with stable node ordering', () => {
    const graph = buildDependencyGraph([
        item('hub', '1', { requires: [ref('a'), ref('b'), ref('c')] }),
        item('a', '1'),
        item('b', '1'),
        item('c', '1'),
    ])
    const first = layoutDependencyGraph(graph)
    const second = layoutDependencyGraph(graph)
    expect(first.nodes.map(({ id, x, y }) => ({ id, x, y }))).toEqual(
        second.nodes.map(({ id, x, y }) => ({ id, x, y })),
    )
    const hub = first.nodes.find((node) => node.id === nodeId('hub'))!
    const leaves = first.nodes.filter((node) => node.id !== nodeId('hub'))
    expect(leaves.every((node) => node.x > hub.x)).toBeTruthy()
    expect(new Set(leaves.map((node) => `${node.x}:${node.y}`)).size).toBe(leaves.length)
})

it('connects each edge from a source output port to a target input port with an HTML connector', () => {
    const graph = buildDependencyGraph([item('a', '1', { requires: [ref('b')] }), item('b', '1')])
    const layout = layoutDependencyGraph(graph)
    const source = layout.nodes.find((node) => node.id === nodeId('a'))!
    const target = layout.nodes.find((node) => node.id === nodeId('b'))!
    const edge = layout.edges[0]!
    const expectedStartX =
        source.x + dependencyGraphMetrics.nodeWidth + dependencyGraphMetrics.edgeClearance
    const expectedStartY = source.y + dependencyGraphMetrics.nodeHeight / 2
    const expectedEndX = target.x - dependencyGraphMetrics.edgeClearance
    const expectedEndY = target.y + dependencyGraphMetrics.nodeHeight / 2

    expect(edge.connector.x).toBe(expectedStartX)
    expect(edge.connector.y).toBe(expectedStartY)
    expect(edge.connector.length).toBe(
        Math.hypot(expectedEndX - expectedStartX, expectedEndY - expectedStartY),
    )
    expect(edge.connector.rotation).toBe(
        (Math.atan2(expectedEndY - expectedStartY, expectedEndX - expectedStartX) * 180) / Math.PI,
    )
})

it('keeps output and input ports stable for reverse cycle connectors', () => {
    const graph = buildDependencyGraph([
        item('a', '1', { requires: [ref('b')] }),
        item('b', '1', { requires: [ref('a')] }),
    ])
    const layout = layoutDependencyGraph(graph)
    const reverseEdge = layout.edges.find((edge) => edge.source === nodeId('b'))!
    const source = layout.nodes.find((node) => node.id === reverseEdge.source)!
    const target = layout.nodes.find((node) => node.id === reverseEdge.target)!

    expect(reverseEdge.connector.x).toBe(
        source.x + dependencyGraphMetrics.nodeWidth + dependencyGraphMetrics.edgeClearance,
    )
    expect(reverseEdge.connector.y).toBe(source.y + dependencyGraphMetrics.nodeHeight / 2)
    expect(reverseEdge.connector.rotation).toBe(
        (Math.atan2(
            target.y + dependencyGraphMetrics.nodeHeight / 2 - reverseEdge.connector.y,
            target.x - dependencyGraphMetrics.edgeClearance - reverseEdge.connector.x,
        ) *
            180) /
            Math.PI,
    )
    expect(reverseEdge.connector.length > 0).toBeTruthy()
    expect(reverseEdge.connector.length).toBe(
        Math.hypot(
            target.x - dependencyGraphMetrics.edgeClearance - reverseEdge.connector.x,
            target.y + dependencyGraphMetrics.nodeHeight / 2 - reverseEdge.connector.y,
        ),
    )
})

it('uses dragged final coordinates for connectors and canvas bounds', () => {
    const graph = buildDependencyGraph([item('a', '1', { requires: [ref('b')] }), item('b', '1')])
    const offsets = new Map([[nodeId('b'), { x: 420, y: 180 }]])
    const layout = layoutDependencyGraph(graph, undefined, offsets)
    const source = layout.nodes.find((node) => node.id === nodeId('a'))!
    const target = layout.nodes.find((node) => node.id === nodeId('b'))!
    const edge = layout.edges[0]!

    expect(target.x >= dependencyGraphMetrics.canvasPadding + 420).toBeTruthy()
    expect(target.y >= dependencyGraphMetrics.canvasPadding + 180).toBeTruthy()
    expect(edge.connector.length).toBe(
        Math.hypot(
            target.x - dependencyGraphMetrics.edgeClearance - edge.connector.x,
            target.y + dependencyGraphMetrics.nodeHeight / 2 - edge.connector.y,
        ),
    )
    expect(edge.connector.x).toBe(
        source.x + dependencyGraphMetrics.nodeWidth + dependencyGraphMetrics.edgeClearance,
    )
    expect(
        layout.width >=
            target.x + dependencyGraphMetrics.nodeWidth + dependencyGraphMetrics.canvasPadding,
    ).toBeTruthy()
    expect(
        layout.height >=
            target.y + dependencyGraphMetrics.nodeHeight + dependencyGraphMetrics.canvasPadding,
    ).toBeTruthy()
})

it('keeps directional depths and lays out a 500-node graph within the performance target', () => {
    const items = Array.from({ length: 500 }, (_, index) => {
        const requires = [1, 2, 3]
            .map((offset) => index - offset)
            .filter((dependencyIndex) => dependencyIndex >= 0)
            .map((dependencyIndex) => ref(`mod-${dependencyIndex}`))
        return item(`mod-${index}`, '1', { requires })
    })
    const graph = buildDependencyGraph(items)
    const visibleIds = new Set(graph.edges.flatMap((edge) => [edge.source, edge.target]))
    const started = performance.now()
    const depths = getDependencyNodeDepths(graph, visibleIds)
    const layout = layoutDependencyGraph(graph, visibleIds)
    const elapsed = performance.now() - started

    expect(graph.nodes.length).toBe(500)
    expect(graph.edges.length).toBe(1494)
    expect(depths.get(nodeId('mod-499'))).toBe(0)
    expect(depths.get(nodeId('mod-0'))).toBe(499)
    expect(layout.nodes.length).toBe(500)
    expect(layout.edges.length).toBe(1494)
    expect(elapsed < 1000, `static graph preparation took ${elapsed.toFixed(1)}ms`).toBeTruthy()
    expect(layout.edges.every((edge) => edge.source && edge.target)).toBeTruthy()
})
