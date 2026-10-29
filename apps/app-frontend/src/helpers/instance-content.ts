import type {
    ContentItem,
    ContentModpackCardProject,
    ContentModpackCardVersion,
    ContentOwner,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'

import {
    get_content_snapshot,
    type InstanceContentSnapshot,
    type InstanceContentSnapshotItem,
    type LinkedModpackInfo,
} from '@/helpers/instance'
import type { CacheBehaviour } from '@/helpers/types'

export type InstanceContentData = {
    path: string
    snapshot: InstanceContentSnapshot
    contentItems: ContentItem[]
    linkedContentItems: ContentItem[]
    modpack: InstanceContentModpackData | null
}

export type InstanceContentModpackData = {
    project: ContentModpackCardProject
    version: ContentModpackCardVersion | null
    owner: ContentOwner | null
    hasUpdate: boolean
    updateVersionId: string | null
}

export function isWorldSaveContentItem(item: Pick<ContentItem, 'project_type'>): boolean {
    return ['world', 'worldsave', 'world_save'].includes(item.project_type)
}

export function localContentIconUrl(iconUrl?: string | null): string {
    if (!iconUrl) return ''
    return /^(https?:|data:|blob:|asset:|tauri:)/.test(iconUrl) ? iconUrl : convertFileSrc(iconUrl)
}

export async function loadInstanceContentData(
    path: string,
    _cacheBehaviour?: CacheBehaviour,
    onError?: (error: Error) => unknown,
    deferEnrichment = false,
): Promise<InstanceContentData | null> {
    try {
        const snapshot = await get_content_snapshot(path, deferEnrichment)
        return normalizeInstanceContentSnapshot(path, snapshot)
    } catch (error) {
        if (onError) {
            onError(error as Error)
            return null
        }
        throw error
    }
}

export function normalizeInstanceContentSnapshot(
    path: string,
    snapshot: InstanceContentSnapshot,
    itemCache?: WeakMap<InstanceContentSnapshotItem, ContentItem>,
): InstanceContentData {
    const normalizedItems: ContentItem[] = snapshot.items.map((item) => {
        const cached = itemCache?.get(item)
        if (cached) return cached
        const fileName = item.expectedRelativePath.split('/').pop() ?? item.expectedRelativePath
        const requiresManualDownload =
            item.ownershipKind === 'pack_managed' &&
            item.required &&
            item.provider === 'curseforge' &&
            item.materializationState === 'pending_manual'
        const curseForgeProjectId =
            item.provider === 'curseforge' && /^\d+$/.test(item.providerProjectId ?? '')
                ? Number(item.providerProjectId)
                : null
        const projectId =
            item.provider === 'curseforge' && curseForgeProjectId != null
                ? `curseforge:${curseForgeProjectId}`
                : item.providerProjectId
        const fallbackContent: ContentItem = {
            id: item.memberId ?? item.entryId ?? item.fileId ?? item.expectedRelativePath,
            file_name: fileName,
            file_path: item.expectedRelativePath,
            size: 0,
            enabled: false,
            project_type: item.projectType,
            project: {
                id: projectId ?? `local:${item.expectedRelativePath}`,
                slug: projectId ?? item.expectedRelativePath,
                title: fileName,
                icon_url: undefined,
            },
            version: item.providerReleaseId
                ? {
                      id: item.providerReleaseId,
                      version_number: item.providerReleaseId,
                      file_name: fileName,
                  }
                : undefined,
            update: null,
            origin_provider: item.provider,
            provider_refs:
                curseForgeProjectId != null
                    ? [
                          {
                              provider: 'curseforge' as const,
                              project_id: curseForgeProjectId,
                              file_id:
                                  item.providerReleaseId && /^\d+$/.test(item.providerReleaseId)
                                      ? Number(item.providerReleaseId)
                                      : null,
                          },
                      ]
                    : item.provider === 'modrinth' && item.providerProjectId
                      ? [
                            {
                                provider: 'modrinth' as const,
                                project_id: item.providerProjectId,
                                version_id: item.providerReleaseId,
                            },
                        ]
                      : [],
            pendingManualDownload: requiresManualDownload,
        }
        const providerRefs = [...(item.content?.provider_refs ?? fallbackContent.provider_refs)]
        const artifact = item.artifactRef
        if (artifact) {
            const { source, projectId } = artifact.release.project
            if (source === 'modrinth') {
                const reference = {
                    provider: 'modrinth' as const,
                    project_id: projectId,
                    version_id: artifact.release.releaseId,
                }
                const index = providerRefs.findIndex((ref) => ref.provider === source)
                if (index < 0) providerRefs.push(reference)
                else providerRefs[index] = reference
            } else if (
                source === 'curseforge' &&
                /^\d+$/.test(projectId) &&
                /^\d+$/.test(artifact.release.releaseId)
            ) {
                const reference = {
                    provider: 'curseforge' as const,
                    project_id: Number(projectId),
                    file_id: Number(artifact.release.releaseId),
                }
                const index = providerRefs.findIndex((ref) => ref.provider === source)
                if (index < 0) providerRefs.push(reference)
                else providerRefs[index] = reference
            }
        }
        const normalized: ContentItem = {
            ...(item.content ?? fallbackContent),
            provider_refs: providerRefs,
            version:
                item.content?.version ??
                (item.artifactRef
                    ? {
                          id: item.artifactRef.release.releaseId,
                          version_number: item.artifactRef.release.releaseId,
                          file_name: fileName,
                      }
                    : fallbackContent.version),
            origin_provider:
                item.artifactRef?.release.project.source ??
                item.content?.origin_provider ??
                fallbackContent.origin_provider,
            instanceFileId: item.fileId ?? undefined,
            instanceEntryId: item.entryId ?? undefined,
            instanceResourceId: item.resourceId ?? undefined,
            instanceMemberId: item.memberId ?? undefined,
            instanceOwnershipKind: item.ownershipKind,
            installedSource:
                item.artifactRef?.release.project.source ?? item.content?.origin_provider ?? null,
            compatibility: item.compatibility ?? null,
            resourceUpdate: item.resourceUpdate ?? null,
            instanceCapabilities: {
                ...item.capabilities,
                canUpdate: !!item.resourceUpdate && item.materializationState === 'present',
            },
            instanceMaterializationState: item.materializationState,
            instanceOverrideKind: item.overrideKind,
            pendingManualDownload: requiresManualDownload,
            dependency: item.dependency ?? null,
        }
        itemCache?.set(item, normalized)
        return normalized
    }) satisfies ContentItem[]
    const contentItems = normalizedItems.filter(
        (item) => item.instanceOwnershipKind !== 'pack_managed',
    )
    const linkedContentItems = normalizedItems.filter(
        (item) => item.instanceOwnershipKind === 'pack_managed',
    )

    return {
        path,
        snapshot,
        contentItems,
        linkedContentItems,
        modpack: normalizePack(snapshot),
    }
}

function normalizePack(snapshot: InstanceContentSnapshot): InstanceContentModpackData | null {
    const pack = snapshot.pack
    if (!pack) return null
    const metadata = pack.metadata
    const project = metadata
        ? normalizeProject(metadata, pack.iconPath)
        : {
              id: pack.projectId ?? snapshot.instanceId,
              slug: pack.projectId ?? snapshot.instanceId,
              title: pack.name,
              icon_url: pack.iconPath ?? undefined,
              description: '',
          }
    const version = metadata
        ? ({
              ...metadata.version,
              date_published: metadata.version.date_published.toString(),
          } as ContentModpackCardVersion)
        : null

    return {
        project,
        version,
        owner: metadata?.owner
            ? {
                  ...metadata.owner,
                  avatar_url: metadata.owner.avatar_url ?? undefined,
              }
            : null,
        hasUpdate: pack.canUpdate && metadata?.update != null,
        updateVersionId:
            metadata?.update?.provider === 'modrinth'
                ? metadata.update.target_version_id
                : metadata?.update?.provider === 'curseforge'
                  ? String(metadata.update.target_file_id)
                  : null,
    }
}

function normalizeProject(
    metadata: LinkedModpackInfo,
    fallbackIconPath?: string | null,
): ContentModpackCardProject {
    return {
        ...metadata.project,
        slug: metadata.project.slug ?? metadata.project.id,
        icon_url: metadata.project.icon_url || fallbackIconPath || undefined,
    }
}
