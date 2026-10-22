export interface AggregatedDependency {
    id: string
    title: string
    requiredBy: string[]
    requiredByKeys?: string[]
    required?: boolean
    requiredForKeys?: string[]
}

function dependencyIdentity(id: string) {
    const separator = id.lastIndexOf(':')
    return {
        identity: separator === -1 ? id : id.slice(0, separator),
        versionId: separator === -1 ? '' : id.slice(separator + 1),
    }
}

export function getActiveDependencyConflictIdentities(
    dependencies: Pick<AggregatedDependency, 'id' | 'requiredByKeys'>[],
    visibleOwnerKeys: Set<string>,
) {
    const versions = new Map<string, Set<string>>()
    for (const dependency of dependencies) {
        if (
            dependency.requiredByKeys?.length &&
            !dependency.requiredByKeys.some((key) => visibleOwnerKeys.has(key))
        ) {
            continue
        }
        const { identity, versionId } = dependencyIdentity(dependency.id)
        const identityVersions = versions.get(identity) ?? new Set<string>()
        identityVersions.add(versionId)
        versions.set(identity, identityVersions)
    }
    return new Set(
        [...versions.entries()]
            .filter(([, versionIds]) => versionIds.size > 1)
            .map(([identity]) => identity),
    )
}
