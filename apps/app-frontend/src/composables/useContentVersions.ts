import type { ContentItem } from '@modrinth/ui'
import { ref, shallowRef } from 'vue'

import type { InstanceContentSnapshotItem } from '@/helpers/instance'
import {
    type ResourceRelease,
    resourceReleaseDocuments,
    resourceReleases,
    type SourceProjectRef,
} from '@/helpers/resources'

export function contentVersionContext(item: ContentItem, entry?: InstanceContentSnapshotItem) {
    const exact = entry?.artifactRef
    const pinned = exact?.release.releaseId ?? entry?.providerReleaseId ?? item.version?.id
    const project =
        exact?.release.project ??
        (entry?.provider && entry.providerProjectId
            ? { source: entry.provider, projectId: entry.providerProjectId }
            : item.project?.id && !item.project.id.startsWith('local:')
              ? {
                    source: item.origin_provider ?? 'modrinth',
                    projectId: item.project.id.replace(/^curseforge:/, ''),
                }
              : null)
    return pinned && project ? { project, pinned } : null
}

export function useContentVersions() {
    const versions = shallowRef<ResourceRelease[]>([])
    const loading = ref(false)
    const error = shallowRef<unknown>(null)
    let controller: AbortController | undefined
    let context: { project: SourceProjectRef; pinned?: string } | undefined

    function cancel() {
        controller?.abort()
        controller = undefined
        loading.value = false
    }

    async function load(project: SourceProjectRef, pinned?: string) {
        cancel()
        context = { project, pinned }
        const request = new AbortController()
        controller = request
        loading.value = true
        error.value = null
        const timer = setTimeout(() => {
            const failure = new Error('Version request timed out')
            request.abort(failure)
            if (controller === request) {
                controller = undefined
                error.value = failure
                loading.value = false
            }
        }, 30_000)
        try {
            const releases: ResourceRelease[] = []
            let cursor: string | null = null
            const cursors = new Set<string>()
            do {
                const page = await resourceReleases(
                    { kind: 'source', project },
                    undefined,
                    cursor,
                    undefined,
                    request.signal,
                )
                request.signal.throwIfAborted()
                releases.push(...page.entries)
                cursor = page.cursor
                if (cursor && cursors.has(cursor)) throw new Error('Repeated version cursor')
                if (cursor) cursors.add(cursor)
            } while (cursor)
            if (pinned && !releases.some((release) => release.reference.releaseId === pinned)) {
                const [document] = await resourceReleaseDocuments(
                    project.source,
                    [pinned],
                    project.projectId,
                    request.signal,
                )
                request.signal.throwIfAborted()
                if (document) releases.push(document.release)
            }
            if (controller === request) versions.value = releases
        } catch (failure) {
            if (controller === request) error.value = failure
        } finally {
            clearTimeout(timer)
            if (controller === request) loading.value = false
        }
    }

    function open(project: SourceProjectRef, pinned?: string) {
        versions.value = []
        return load(project, pinned)
    }

    function retry() {
        return context ? load(context.project, context.pinned) : Promise.resolve()
    }

    function reset() {
        cancel()
        context = undefined
        versions.value = []
        error.value = null
    }

    return { versions, loading, error, open, retry, reset }
}
