import { expect, it } from 'vitest'
import { computed, reactive, shallowRef } from 'vue'

import type { ResourceDependencyNode, ResourceOperationPlan } from '@/helpers/resources'

import { indexResourceOperationPreview } from './resource-operation-preview-index'
import modalSource from './ResourceOperationPreviewModal.vue?raw'

type Plan = Pick<ResourceOperationPlan, 'nodes' | 'edges' | 'actions'>
function node(id: string): ResourceDependencyNode {
    return { id } as ResourceDependencyNode
}
function edge(parent: string, child: string, kind = 'required'): Plan['edges'][number] {
    return { parent, child, kind } as Plan['edges'][number]
}
function action(nodeId: string, entryId: string): Plan['actions'][number] {
    return { nodeId, entryId } as Plan['actions'][number]
}

it('preserves edge order, deduplicates formatted parent names, and separates dependency kinds', () => {
    const plan: Plan = {
        nodes: [node('a'), node('b'), node('c'), node('child'), node('a')],
        edges: [
            edge('c', 'child'),
            edge('a', 'child'),
            edge('b', 'child'),
            edge('missing', 'child'),
            edge('a', 'child'),
            edge('a', 'child', 'optional'),
        ],
        actions: [],
    }
    const index = indexResourceOperationPreview(plan, (parent) =>
        parent.id === 'c' ? 'Third' : 'Same',
    )
    expect(index.requiredBy.get('child')?.get('required')).toBe('Third, Same')
    expect(index.requiredBy.get('child')?.get('optional')).toBe('Same')
    expect(index.requiredBy.has('missing')).toBe(false)
    // Duplicate node IDs retain the first node, matching the previous find lookup.
    const first = plan.nodes[0]
    const duplicate = plan.nodes[4]
    expect(
        indexResourceOperationPreview(plan, (parent) =>
            parent === first ? 'First' : parent === duplicate ? 'Duplicate' : parent.id,
        )
            .requiredBy.get('child')
            ?.get('optional'),
    ).toBe('First')
})

it('groups action identities in original order without copying or mutating the plan', () => {
    const first = action('a', 'first')
    const second = action('a', 'second')
    const other = action('b', 'other')
    const plan: Plan = { nodes: [], edges: [], actions: [first, other, second] }
    const index = indexResourceOperationPreview(plan, (parent) => parent.id)
    expect(index.actions.get('a')).toEqual([first, second])
    expect(index.actions.get('a')?.[0]).toBe(first)
    expect(index.actions.get('b')).toEqual([other])
    expect(index.actions.get('missing')).toBeUndefined()
    expect(plan.actions).toEqual([first, other, second])
})

it('reacts to late names and plan revisions, dropping withdrawn nodes and actions', () => {
    const plan = shallowRef<Plan | null>({
        nodes: [node('a'), node('child')],
        edges: [edge('a', 'child')],
        actions: [action('child', 'install')],
    })
    const names = reactive<Record<string, string>>({})
    const index = computed(() =>
        indexResourceOperationPreview(plan.value, (parent) => names[parent.id] ?? parent.id),
    )
    expect(index.value.requiredBy.get('child')?.get('required')).toBe('a')
    names.a = 'Enriched title'
    expect(index.value.requiredBy.get('child')?.get('required')).toBe('Enriched title')
    plan.value = { nodes: [node('child')], edges: [edge('a', 'child')], actions: [] }
    expect(index.value.requiredBy.has('child')).toBe(false)
    expect(index.value.actions.has('child')).toBe(false)
    plan.value = null
    expect(index.value.requiredBy.size).toBe(0)
    expect(index.value.actions.size).toBe(0)
})

it.each([500, 1000])(
    'indexes %i nodes, edges and actions with only linear property access',
    (size) => {
        let nodeReads = 0
        let edgeReads = 0
        let actionReads = 0
        const plan: Plan = {
            nodes: Array.from(
                { length: size },
                (_, i) =>
                    ({
                        get id() {
                            nodeReads++
                            return `node-${i}`
                        },
                    }) as ResourceDependencyNode,
            ),
            edges: Array.from(
                { length: size },
                (_, i) =>
                    ({
                        get parent() {
                            edgeReads++
                            return `node-${i}`
                        },
                        get child() {
                            edgeReads++
                            return `node-${(i + 1) % size}`
                        },
                        get kind() {
                            edgeReads++
                            return 'required'
                        },
                    }) as Plan['edges'][number],
            ),
            actions: Array.from(
                { length: size },
                (_, i) =>
                    ({
                        get nodeId() {
                            actionReads++
                            return `node-${i}`
                        },
                    }) as Plan['actions'][number],
            ),
        }
        const index = indexResourceOperationPreview(plan, (parent) => parent.id)
        // A per-edge nodes.find or per-node actions.filter crosses these deterministic bounds.
        expect(nodeReads).toBeLessThanOrEqual(size * 4)
        expect(edgeReads).toBeLessThanOrEqual(size * 5)
        expect(actionReads).toBeLessThanOrEqual(size * 2)
        const reads = [nodeReads, edgeReads, actionReads]
        for (let i = 0; i < size; i++) {
            expect(index.requiredBy.get(`node-${(i + 1) % size}`)?.get('required')).toBe(
                `node-${i}`,
            )
            expect(index.actions.get(`node-${i}`)).toHaveLength(1)
        }
        expect([nodeReads, edgeReads, actionReads]).toEqual(reads)
    },
)

it('uses a shared computed preview index for names and per-node actions', () => {
    expect(modalSource).toMatch(
        /const previewIndex = computed\(\(\) => indexResourceOperationPreview\(plan.value, name\)\)/,
    )
    expect(modalSource).toContain('return previewIndex.value.requiredBy.get(id)?.get(kind)')
    expect(modalSource).toContain('v-for="action in previewIndex.actions.get(node.id)"')
    expect(modalSource).not.toContain('plan?.actions.filter(')
})
