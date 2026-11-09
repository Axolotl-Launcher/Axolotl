import { expect, it } from 'vitest'

import {
    compareContentIdentities,
    contentIdentityFromInput,
    contentIdentityInputsFromSnapshot,
    normalizeContentIdentityText,
} from './content-identity.ts'

function identity(
    provider: 'modrinth' | 'curseforge',
    projectId: string,
    contentType = 'mod',
    values: Record<string, string> = {},
) {
    return contentIdentityFromInput({ provider, projectId, contentType, ...values })
}

it('normalizes platform suffixes and versions', () => {
    expect(normalizeContentIdentityText('Sodium-Fabric-0.5.8.jar')).toBe('sodium')
    expect(normalizeContentIdentityText('sodium_forge_1.20.1')).toBe('sodium')
})

it('same provider never creates a cross-platform conflict', () => {
    expect(
        compareContentIdentities(
            identity('modrinth', 'a', 'mod', { title: 'Sodium' }),
            identity('modrinth', 'b', 'mod', { title: 'Sodium' }),
        ),
    ).toBe(null)
})

it('different content types never conflict', () => {
    expect(
        compareContentIdentities(
            identity('modrinth', 'a', 'mod', { title: 'Sodium' }),
            identity('curseforge', 'b', 'resourcepack', { title: 'Sodium' }),
        ),
    ).toBe(null)
})

it('matching sha1 is an exact cross-platform conflict', () => {
    const result = compareContentIdentities(
        identity('modrinth', 'a', 'mod', { sha1: 'ABC123', title: 'First' }),
        identity('curseforge', 'b', 'mod', { sha1: 'abc123', title: 'Second' }),
    )
    expect(result?.source).toBe('sha1')
    expect(result?.confidence).toBe('exact')
})

it('matching names are heuristic conflicts', () => {
    const result = compareContentIdentities(
        identity('modrinth', 'a', 'mod', { title: 'Example Mod' }),
        identity('curseforge', 'b', 'mod', { title: 'example-mod' }),
    )
    expect(result?.source).toBe('heuristic')
})

it('curated mapping conflicts are exact', () => {
    const left = { ...identity('modrinth', 'a'), key: 'mapping:1', ambiguous: false }
    const right = { ...identity('curseforge', 'b'), key: 'mapping:1', ambiguous: false }
    const result = compareContentIdentities(left, right)
    expect(result?.source).toBe('curated_mapping')
    expect(result?.confidence).toBe('exact')
})

it('pack-managed snapshot members participate in cross-platform conflicts', () => {
    const [installedInput] = contentIdentityInputsFromSnapshot(
        [
            {
                projectType: 'mod',
                provider: 'modrinth',
                providerProjectId: 'AANobbMI',
                expectedRelativePath: 'mods/sodium-fabric-0.6.13.jar',
                content: null,
            },
        ],
        {
            modrinth: new Map([['AANobbMI', { slug: 'sodium', title: 'Sodium' }]]),
        },
    )
    const installed = { ...contentIdentityFromInput(installedInput), key: 'mapping:sodium' }
    const candidate = {
        ...identity('curseforge', '394468', 'mod', { slug: 'sodium' }),
        key: 'mapping:sodium',
    }

    const result = compareContentIdentities(candidate, installed)
    expect(result?.source).toBe('curated_mapping')
    expect(result?.confidence).toBe('exact')
})

it('recognizes the verified artifact for a manually imported file', () => {
    const item = {
        projectType: 'mod',
        provider: null,
        providerProjectId: null,
        materializationState: 'present',
        expectedRelativePath: 'mods/example.jar',
        artifactRef: { release: { project: { source: 'curseforge', projectId: '123' } } },
        content: { id: 'sha1', file_name: 'example.jar', project_type: 'mod', provider_refs: [] },
    }
    expect(contentIdentityInputsFromSnapshot([item])).toMatchObject([
        { provider: 'curseforge', projectId: '123', sha1: 'sha1' },
    ])
    expect(contentIdentityInputsFromSnapshot([{ ...item, artifactRef: null }])).toEqual([])
    expect(
        contentIdentityInputsFromSnapshot([{ ...item, materializationState: 'missing' }]),
    ).toEqual([])
})

it('deduplicates an artifact and existing provider reference', () => {
    expect(
        contentIdentityInputsFromSnapshot([
            {
                projectType: 'mod',
                provider: 'modrinth',
                providerProjectId: 'abc',
                expectedRelativePath: 'mods/example.jar',
                artifactRef: { release: { project: { source: 'modrinth', projectId: 'abc' } } },
                content: {
                    id: 'sha1',
                    file_name: 'example.jar',
                    project_type: 'mod',
                    provider_refs: [{ provider: 'modrinth', project_id: 'abc' }],
                },
            },
        ]),
    ).toHaveLength(1)
})

it('does not treat missing, removed or pending pack members as installed identities', () => {
    for (const materializationState of ['missing', 'removed', 'pending_manual']) {
        expect(
            contentIdentityInputsFromSnapshot([
                {
                    projectType: 'mod',
                    provider: 'modrinth',
                    providerProjectId: 'abc',
                    expectedRelativePath: 'mods/example.jar',
                    materializationState,
                    artifactRef: { release: { project: { source: 'modrinth', projectId: 'abc' } } },
                    content: null,
                },
            ]),
        ).toEqual([])
        expect(
            contentIdentityInputsFromSnapshot([
                {
                    projectType: 'mod',
                    provider: 'modrinth',
                    providerProjectId: 'abc',
                    expectedRelativePath: 'mods/example.jar',
                    materializationState,
                    content: {
                        id: 'sha1',
                        file_name: 'example.jar',
                        project_type: 'mod',
                        provider_refs: [{ provider: 'curseforge', project_id: '123' }],
                    },
                },
            ]),
        ).toEqual([])
    }
})

it('does not treat a local mod id as an online project slug', () => {
    const [input] = contentIdentityInputsFromSnapshot([
        {
            projectType: 'mod',
            provider: null,
            providerProjectId: null,
            expectedRelativePath: 'mods/example.jar',
            artifactRef: { release: { project: { source: 'modrinth', projectId: 'abc' } } },
            content: {
                id: 'sha1',
                file_name: 'example.jar',
                project_type: 'mod',
                provider_refs: [],
                project: { id: 'local:example', slug: 'example', title: 'Example' },
            },
        },
    ])
    expect(input.slug).toBeUndefined()
})
