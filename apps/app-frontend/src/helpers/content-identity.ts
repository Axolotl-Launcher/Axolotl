import { invoke } from '@tauri-apps/api/core'

/**
 * Prefix the launcher uses for an id it synthesised because no online identity
 * was resolved. It must stay identical to `LOCAL_ID_PREFIX` in the Rust crate
 * (`packages/app-lib/src/state/instances/content.rs`), which is what writes it.
 */
const LOCAL_ID_PREFIX = 'local:'

export type ContentIdentityProvider = 'modrinth' | 'curseforge'
export type ContentIdentitySource = 'curated_mapping' | 'sha1' | 'heuristic'
export type ContentIdentityConfidence = 'exact' | 'high' | 'possible'

export interface ContentIdentityInput {
    provider: ContentIdentityProvider
    projectId: string
    contentType: string
    slug?: string | null
    title?: string | null
    fileName?: string | null
    sha1?: string | null
}

export interface ContentIdentitySnapshotItem {
    projectType: string
    provider: ContentIdentityProvider | null
    providerProjectId: string | null
    expectedRelativePath: string
    content: {
        id: string
        file_name: string
        project_type: string
        project?: { slug?: string | null; title?: string | null } | null
        provider_refs: Array<{
            provider: ContentIdentityProvider
            project_id: string | number
        }>
    } | null
}

export interface ContentIdentityProjectMetadata {
    slug?: string | null
    title?: string | null
}

export type ContentIdentityProjectMetadataByProvider = Partial<
    Record<ContentIdentityProvider, ReadonlyMap<string, ContentIdentityProjectMetadata>>
>

export interface ContentIdentityCounterpart {
    provider: ContentIdentityProvider
    projectId: string
    slug?: string
}

export interface ContentIdentity {
    key?: string
    source?: ContentIdentitySource
    confidence?: ContentIdentityConfidence
    counterparts?: ContentIdentityCounterpart[]
    provider: ContentIdentityProvider
    projectId: string
    contentType: string
    slug?: string
    title?: string
    fileName?: string
    sha1?: string
    normalizedSlug?: string
    normalizedTitle?: string
    normalizedFileName?: string
    ambiguous?: boolean
}

interface ContentIdentityRecord {
    key: string
    counterparts: ContentIdentityCounterpart[]
}

interface ContentIdentityLookup {
    modrinth: Record<string, ContentIdentityRecord[]>
    curseforge: Record<string, ContentIdentityRecord[]>
}

export interface ContentIdentityMatch {
    source: ContentIdentitySource
    confidence: ContentIdentityConfidence
    identity: ContentIdentity
}

export function contentIdentityInputsFromSnapshot(
    items: ContentIdentitySnapshotItem[],
    projectMetadata: ContentIdentityProjectMetadataByProvider = {},
): ContentIdentityInput[] {
    const inputs: ContentIdentityInput[] = []
    for (const item of items) {
        const references = new Map<
            string,
            { provider: ContentIdentityProvider; projectId: string }
        >()
        if (item.provider && item.providerProjectId) {
            const reference = { provider: item.provider, projectId: item.providerProjectId }
            references.set(`${reference.provider}:${reference.projectId}`, reference)
        }
        for (const providerReference of item.content?.provider_refs ?? []) {
            const reference = {
                provider: providerReference.provider,
                projectId: String(providerReference.project_id),
            }
            references.set(`${reference.provider}:${reference.projectId}`, reference)
        }

        for (const reference of references.values()) {
            const metadata = projectMetadata[reference.provider]?.get(reference.projectId)
            inputs.push({
                ...reference,
                contentType: item.projectType || item.content?.project_type || 'mod',
                slug: metadata?.slug ?? item.content?.project?.slug,
                title: metadata?.title ?? item.content?.project?.title,
                fileName:
                    item.content?.file_name ??
                    item.expectedRelativePath.split(/[\\/]/u).pop() ??
                    item.expectedRelativePath,
                sha1: item.content?.id,
            })
        }
    }
    return inputs
}

function normalizeText(value: string | null | undefined) {
    return (value ?? '')
        .toLocaleLowerCase()
        .replace(/\.(?:jar|zip|mrpack|litemod)(?:\.disabled)?$/u, '')
        .replace(/(?:[-_. ]+)(?:v?\d[\w.-]*)$/u, '')
        .replace(/(?:[-_. ]+)(?:fabric|forge|quilt|neoforge|neo|liteloader)$/u, '')
        .replace(/[^a-z0-9]+/gu, '')
}

export function normalizeContentIdentityText(value: string | null | undefined) {
    return normalizeText(value)
}

function identitySlug(input: ContentIdentityInput) {
    return input.slug ? normalizeText(input.slug) : ''
}

function identityTitle(input: ContentIdentityInput) {
    return input.title ? normalizeText(input.title) : ''
}

function identityFileName(input: ContentIdentityInput) {
    return input.fileName ? normalizeText(input.fileName) : ''
}

export async function resolveContentIdentities(
    inputs: ContentIdentityInput[],
): Promise<ContentIdentity[]> {
    const modrinthSlugs = [
        ...new Set(
            inputs
                .filter((input) => input.provider === 'modrinth' && input.slug)
                .map((input) => input.slug as string),
        ),
    ]
    const curseforgeSlugs = [
        ...new Set(
            inputs
                .filter((input) => input.provider === 'curseforge' && input.slug)
                .map((input) => input.slug as string),
        ),
    ]
    let lookup: ContentIdentityLookup = { modrinth: {}, curseforge: {} }
    if (modrinthSlugs.length || curseforgeSlugs.length) {
        lookup = await invoke<ContentIdentityLookup>(
            'plugin:content-search|lookup_content_identities',
            {
                modrinthSlugs,
                curseforgeSlugs,
            },
        ).catch(() => lookup)
    }

    return inputs.map((input) => {
        const records = input.slug
            ? ((input.provider === 'modrinth'
                  ? (lookup.modrinth[input.slug] ?? lookup.modrinth[input.slug.toLowerCase()])
                  : (lookup.curseforge[input.slug] ??
                    lookup.curseforge[input.slug.toLowerCase()])) ?? [])
            : []
        const uniqueKeys = [...new Set(records.map((record) => record.key))]
        const mapping =
            uniqueKeys.length === 1
                ? records.find((record) => record.key === uniqueKeys[0])
                : undefined
        return {
            ...input,
            slug: input.slug ?? undefined,
            title: input.title ?? undefined,
            fileName: input.fileName ?? undefined,
            sha1: input.sha1?.toLowerCase() || undefined,
            key: mapping?.key,
            source: mapping ? 'curated_mapping' : undefined,
            confidence: mapping ? 'exact' : undefined,
            counterparts: mapping?.counterparts,
            normalizedSlug: identitySlug(input),
            normalizedTitle: identityTitle(input),
            normalizedFileName: identityFileName(input),
            ambiguous: uniqueKeys.length > 1,
        }
    })
}

function sameProvider(left: ContentIdentity, right: ContentIdentity) {
    return left.provider === right.provider
}

function sameContentType(left: ContentIdentity, right: ContentIdentity) {
    return left.contentType === right.contentType
}

export function compareContentIdentities(
    left: ContentIdentity,
    right: ContentIdentity,
): ContentIdentityMatch | null {
    if (sameProvider(left, right) || !sameContentType(left, right)) return null
    if (left.key && right.key && left.key === right.key && !left.ambiguous && !right.ambiguous) {
        return { source: 'curated_mapping', confidence: 'exact', identity: right }
    }
    if (left.sha1 && right.sha1 && left.sha1 === right.sha1) {
        return { source: 'sha1', confidence: 'exact', identity: right }
    }
    if (
        left.normalizedSlug &&
        right.normalizedSlug &&
        left.normalizedSlug === right.normalizedSlug
    ) {
        return { source: 'heuristic', confidence: 'high', identity: right }
    }
    if (
        left.normalizedTitle &&
        right.normalizedTitle &&
        left.normalizedTitle === right.normalizedTitle
    ) {
        return { source: 'heuristic', confidence: 'possible', identity: right }
    }
    if (
        left.normalizedFileName &&
        right.normalizedFileName &&
        left.normalizedFileName === right.normalizedFileName
    ) {
        return { source: 'heuristic', confidence: 'possible', identity: right }
    }
    return null
}

export function contentIdentityFromInput(input: ContentIdentityInput): ContentIdentity {
    return {
        ...input,
        slug: input.slug ?? undefined,
        title: input.title ?? undefined,
        fileName: input.fileName ?? undefined,
        sha1: input.sha1?.toLowerCase() || undefined,
        normalizedSlug: identitySlug(input),
        normalizedTitle: identityTitle(input),
        normalizedFileName: identityFileName(input),
    }
}

/** True when the id is the placeholder synthesised for unresolved metadata. */
export function isUnresolvedProjectId(projectId: string | null | undefined): boolean {
    return !!projectId?.startsWith(LOCAL_ID_PREFIX)
}

export function isUnresolvedContentProject(item: { project?: { id?: string } | null }): boolean {
    return isUnresolvedProjectId(item.project?.id)
}
