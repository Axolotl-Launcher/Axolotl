export interface UpgradeReleaseIdentity {
    provider: string
    projectId: string
    releaseId: string
}

export interface UpgradeVersionDisplayMetadata {
    version: string
    channel?: string | number
}

export function upgradeVersionCacheKey(provider: string, projectId: string, releaseId: string) {
    return `${provider}:${projectId}:${releaseId}`
}

export function upgradeVersionDisplayLabel(
    metadata: Map<string, UpgradeVersionDisplayMetadata> | undefined,
    identity: UpgradeReleaseIdentity,
): string {
    return (
        metadata?.get(
            upgradeVersionCacheKey(identity.provider, identity.projectId, identity.releaseId),
        )?.version ?? identity.releaseId
    )
}
