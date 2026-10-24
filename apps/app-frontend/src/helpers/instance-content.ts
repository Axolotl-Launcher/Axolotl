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
    type LinkedModpackInfo,
    refresh_content,
} from '@/helpers/instance'
import { resourceCheckUpdates, type ResourceUpdateCheckOptions } from '@/helpers/resources'
import type { CacheBehaviour } from '@/helpers/types'

export type InstanceContentData = {
    path: string
    snapshot: InstanceContentSnapshot
    contentItems: ContentItem[]
    linkedContentItems: ContentItem[]
    modpack: InstanceContentModpackData | null
    updateCheckFailedEntries?: string[]
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

export function preserveContentUpdateState(
    previous: ContentItem | undefined,
    item: ContentItem,
): ContentItem {
    if (
        !previous ||
        previous.file_path !== item.file_path ||
        previous.version?.id !== item.version?.id ||
        previous.project?.id !== item.project?.id ||
        previous.installedSource !== item.installedSource
    )
        return item

    return {
        ...item,
        update: item.update ?? previous.update,
        resourceUpdate: item.resourceUpdate ?? previous.resourceUpdate,
        instanceCapabilities: item.instanceCapabilities
            ? {
                  ...item.instanceCapabilities,
                  canUpdate:
                      item.instanceCapabilities.canUpdate ||
                      (previous.instanceCapabilities?.canUpdate ?? false),
              }
            : item.instanceCapabilities,
    }
}

export async function loadInstanceContentInStages(
    path: string,
    cacheBehaviour: CacheBehaviour | undefined,
    showLocal: (data: InstanceContentData) => boolean,
): Promise<InstanceContentData | null> {
    const local = await loadInstanceContentData(path, undefined, undefined, true)
    if (!local || !showLocal(local)) return local
    return loadInstanceContentData(path, cacheBehaviour)
}

export async function loadInstanceContentData(
    path: string,
    cacheBehaviour?: CacheBehaviour,
    onError?: (error: Error) => unknown,
    deferEnrichment = false,
): Promise<InstanceContentData | null> {
    try {
        const snapshot =
            cacheBehaviour === 'bypass' || cacheBehaviour === 'must_revalidate'
                ? await refresh_content(path)
                : deferEnrichment
                  ? await get_content_snapshot(path, true)
                  : await get_content_snapshot(path)
        const normalizedItems: ContentItem[] = snapshot.items.map((item) => {
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
            return {
                ...(item.content ?? fallbackContent),
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
                    item.artifactRef?.release.project.source ??
                    item.content?.origin_provider ??
                    null,
                compatibility: item.compatibility ?? null,
                instanceCapabilities: item.capabilities,
                instanceMaterializationState: item.materializationState,
                instanceOverrideKind: item.overrideKind,
                pendingManualDownload: requiresManualDownload,
                dependency: item.dependency ?? null,
            }
        }) satisfies ContentItem[]
        for (const item of normalizedItems) {
            if (!item.instanceEntryId) continue
            item.update = null
            item.resourceUpdate = null
            if (item.instanceCapabilities)
                item.instanceCapabilities = { ...item.instanceCapabilities, canUpdate: false }
        }
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
    } catch (error) {
        if (onError) {
            onError(error as Error)
            return null
        }
        throw error
    }
}

/** Discover updates only for the supplied batch, after displaying local content. */
export async function refreshInstanceContentUpdates(
    data: InstanceContentData,
    signal?: AbortSignal,
    options?: ResourceUpdateCheckOptions,
): Promise<InstanceContentData> {
    if (signal?.aborted) return data
    const items = [...data.contentItems, ...data.linkedContentItems].map((item) => ({
        ...item,
        instanceCapabilities: item.instanceCapabilities
            ? { ...item.instanceCapabilities }
            : undefined,
    }))
    const snapshot = {
        ...data.snapshot,
        warnings: data.snapshot.warnings.filter(
            (warning) => warning.code !== 'resource_update_unavailable',
        ),
    }
    const updateCheckFailedEntries = new Set<string>()
    const entryIds =
        options?.entryIds ??
        items
            .filter((item) => item.instanceMaterializationState === 'present')
            .map((item) => item.instanceEntryId)
            .filter((id): id is string => !!id)
    const selected = new Set(entryIds)
    if (entryIds.length) {
        try {
            const updates = await resourceCheckUpdates(data.path, signal, { ...options, entryIds })
            if (signal?.aborted) return data
            for (const item of items) {
                if (!item.instanceEntryId || !selected.has(item.instanceEntryId)) continue
                if (
                    updates.failedEntries.includes(item.instanceEntryId) ||
                    updates.unresolved.includes(item.file_path ?? '')
                ) {
                    updateCheckFailedEntries.add(item.instanceEntryId)
                    continue
                }
                item.resourceUpdate = updates.releases[item.instanceEntryId] ?? null
                if (item.instanceCapabilities)
                    item.instanceCapabilities.canUpdate =
                        !!item.resourceUpdate && item.instanceMaterializationState === 'present'
            }
            if (updates.failures.length)
                snapshot.warnings.push({
                    code: 'resource_update_unavailable',
                    message: updates.failures.join('\n'),
                    provider: null,
                })
        } catch (error) {
            if (signal?.aborted) return data
            for (const item of items) {
                if (item.instanceEntryId && selected.has(item.instanceEntryId))
                    updateCheckFailedEntries.add(item.instanceEntryId)
            }
            snapshot.warnings.push({
                code: 'resource_update_unavailable',
                message: error instanceof Error ? error.message : String(error),
                provider: null,
            })
        }
    }
    return {
        ...data,
        snapshot,
        updateCheckFailedEntries: [...updateCheckFailedEntries],
        contentItems: items.filter((item) => item.instanceOwnershipKind !== 'pack_managed'),
        linkedContentItems: items.filter((item) => item.instanceOwnershipKind === 'pack_managed'),
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
