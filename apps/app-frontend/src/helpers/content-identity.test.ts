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
