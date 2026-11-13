import type { ResourceDependencyNode, ResourceOperationPlan } from '@/helpers/resources'

export function indexResourceOperationPreview(
    plan: Pick<ResourceOperationPlan, 'nodes' | 'edges' | 'actions'> | null,
    name: (node: ResourceDependencyNode) => string,
) {
    const nodes = new Map<string, ResourceDependencyNode>()
    for (const node of plan?.nodes ?? []) {
        if (!nodes.has(node.id)) nodes.set(node.id, node)
    }
    const parentNames = new Map<string, Map<string, Set<string>>>()
    for (const edge of plan?.edges ?? []) {
        const parent = nodes.get(edge.parent)
        if (!parent) continue
        let kinds = parentNames.get(edge.child)
        if (!kinds) parentNames.set(edge.child, (kinds = new Map()))
        let titles = kinds.get(edge.kind)
        if (!titles) kinds.set(edge.kind, (titles = new Set()))
        titles.add(name(parent))
    }
    const requiredBy = new Map<string, Map<string, string>>()
    for (const [child, kinds] of parentNames) {
        requiredBy.set(
            child,
            new Map([...kinds].map(([kind, titles]) => [kind, [...titles].join(', ')])),
        )
    }
    const actions = new Map<string, ResourceOperationPlan['actions']>()
    for (const action of plan?.actions ?? []) {
        let nodeActions = actions.get(action.nodeId)
        if (!nodeActions) actions.set(action.nodeId, (nodeActions = []))
        nodeActions.push(action)
    }
    return { requiredBy, actions }
}
