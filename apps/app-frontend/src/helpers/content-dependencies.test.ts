import { expect, it } from 'vitest'

import { dependencyRefMatchesContentItem } from './content-dependencies.ts'

it('dependency references match either provider identity of a recognized mod', () => {
    const item = {
        project: { id: 'modrinth-id' },
        provider_refs: [
            { provider: 'modrinth' as const, project_id: 'modrinth-id', version_id: null },
            { provider: 'curseforge' as const, project_id: 123, file_id: null },
        ],
    }
    expect(
        dependencyRefMatchesContentItem({ provider: 'curseforge', projectId: '123' }, item),
    ).toBe(true)
    expect(
        dependencyRefMatchesContentItem(
            { provider: 'curseforge', projectId: 'curseforge:123' },
            item,
        ),
    ).toBe(true)
    expect(
        dependencyRefMatchesContentItem({ provider: 'modrinth', projectId: 'other' }, item),
    ).toBe(false)
})

it('provider references prevent matching the same project ID from another provider', () => {
    const item = {
        project: { id: '123' },
        provider_refs: [{ provider: 'modrinth' as const, project_id: '123', version_id: null }],
    }
    expect(
        dependencyRefMatchesContentItem({ provider: 'curseforge', projectId: '123' }, item),
    ).toBe(false)
    expect(dependencyRefMatchesContentItem({ provider: 'local', projectId: '123' }, item)).toBe(
        false,
    )
})

it('dependency references retain local and legacy CurseForge project matching', () => {
    for (const projectId of ['123', 'curseforge:123']) {
        expect(
            dependencyRefMatchesContentItem(
                { provider: 'curseforge', projectId: '123' },
                { project: { id: projectId }, provider_refs: [] },
            ),
        ).toBe(true)
    }
    expect(
        dependencyRefMatchesContentItem(
            { provider: 'local', projectId: 'local:library' },
            { project: { id: 'local:library' }, provider_refs: [] },
        ),
    ).toBe(true)
})

it('local endpoints match explicit local identities even with incomplete online references', () => {
    const item = {
        project: { id: 'local:library' },
        provider_refs: [
            { provider: 'modrinth' as const, project_id: 'online-id', version_id: null },
        ],
    }
    expect(
        dependencyRefMatchesContentItem({ provider: 'local', projectId: 'local:library' }, item),
    ).toBe(true)
    expect(
        dependencyRefMatchesContentItem({ provider: 'local', projectId: 'local:other' }, item),
    ).toBe(false)
})
