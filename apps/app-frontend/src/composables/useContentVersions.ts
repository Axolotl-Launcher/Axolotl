import type { ContentItem } from '@modrinth/ui'
import { ref, shallowRef } from 'vue'

import { isUnresolvedProjectId } from '@/helpers/content-identity'
import type { InstanceContentSnapshotItem } from '@/helpers/instance'
import {
    type ResourceRelease,
    resourceReleaseDocuments,
    resourceReleases,
    type SourceProjectRef,
} from '@/helpers/resources'

/**
 * The resource helpers this composable reads versions through.
 *
 * Injecting them keeps the composable testable without mocking the module graph:
 * a spec that mocks `@/helpers/resources` fails with a confusing
 * `vi.mocked(...) is not a function` whenever the mock does not take effect.
 */
export interface ContentVersionSource {
    readonly resourceReleases: typeof resourceReleases
    readonly resourceReleaseDocuments: typeof resourceReleaseDocuments
}

const defaultContentVersionSource: ContentVersionSource = {
    resourceReleases,
    resourceReleaseDocuments,
}

export function contentVersionContext(item: ContentItem, entry?: InstanceContentSnapshotItem) {
    const exact = entry?.artifactRef
    const pinned = exact?.release.releaseId ?? entry?.providerReleaseId ?? item.version?.id
    const project =
        exact?.release.project ??
        (entry?.provider && entry.providerProjectId
            ? { source: entry.provider, projectId: entry.providerProjectId }
            : item.project?.id && !isUnresolvedProjectId(item.project.id)
              ? {
                    source: item.origin_provider ?? 'modrinth',
                    projectId: item.project.id.replace(/^curseforge:/, ''),
                }
              : null)
    return pinned && project ? { project, pinned } : null
}

const releaseKey = ({ reference }: ResourceRelease) =>
    `${reference.project.source}:${reference.project.projectId}:${reference.releaseId}`

/** Appends releases not listed yet, so a page repeating an earlier release does not duplicate it. */
function mergeReleases(current: ResourceRelease[], page: ResourceRelease[]) {
    const seen = new Set(current.map(releaseKey))
    const added = page.filter((release) => {
        const key = releaseKey(release)
        if (seen.has(key)) return false
        seen.add(key)
        return true
    })
    return added.length ? [...current, ...added] : current
}

/**
 * Loads a project's releases page by page. Each page is shown as soon as it
 * arrives; `loading` stays true until the last page, and a failure on a later
 * page keeps the releases already shown so `retry` can continue from there.
 */
export function useContentVersions(source: ContentVersionSource = defaultContentVersionSource) {
    const versions = shallowRef<ResourceRelease[]>([])
    const loading = ref(false)
    const error = shallowRef<unknown>(null)
    let controller: AbortController | undefined
    let context:
        { project: SourceProjectRef; pinned?: string; nextCursor: string | null } | undefined

    function cancel() {
        controller?.abort()
        controller = undefined
        loading.value = false
    }

    async function load(
        project: SourceProjectRef,
        pinned?: string,
        resume?: { cursor: string; releases: ResourceRelease[] },
    ) {
        cancel()
        const current = { project, pinned, nextCursor: resume?.cursor ?? null }
        context = current
        const request = new AbortController()
        controller = request
        loading.value = true
        error.value = null
        let timer: ReturnType<typeof setTimeout> | undefined
        // Each request gets its own deadline, so a long list that keeps
        // arriving is not cut off.
        const armTimeout = () => {
            clearTimeout(timer)
            timer = setTimeout(() => {
                const failure = new Error('Version request timed out')
                request.abort(failure)
                if (controller === request) {
                    controller = undefined
                    error.value = failure
                    loading.value = false
                }
            }, 30_000)
        }
        try {
            let releases = resume?.releases ?? []
            let cursor: string | null = resume?.cursor ?? null
            const cursors = new Set<string>(cursor ? [cursor] : [])
            do {
                armTimeout()
                const page = await source.resourceReleases(
                    { kind: 'source', project },
                    undefined,
                    cursor,
                    undefined,
                    request.signal,
                )
                request.signal.throwIfAborted()
                releases = mergeReleases(releases, page.entries)
                if (controller === request) versions.value = releases
                cursor = page.cursor
                current.nextCursor = cursor
                if (cursor && cursors.has(cursor)) throw new Error('Repeated version cursor')
                if (cursor) cursors.add(cursor)
            } while (cursor)
            if (pinned && !releases.some((release) => release.reference.releaseId === pinned)) {
                armTimeout()
                const [document] = await source.resourceReleaseDocuments(
                    project.source,
                    [pinned],
                    project.projectId,
                    request.signal,
                )
                request.signal.throwIfAborted()
                if (document) releases = mergeReleases(releases, [document.release])
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
        if (!context) return Promise.resolve()
        const { project, pinned, nextCursor } = context
        return nextCursor && versions.value.length
            ? load(project, pinned, { cursor: nextCursor, releases: versions.value })
            : load(project, pinned)
    }

    function reset() {
        cancel()
        context = undefined
        versions.value = []
        error.value = null
    }

    return { versions, loading, error, open, retry, reset }
}
