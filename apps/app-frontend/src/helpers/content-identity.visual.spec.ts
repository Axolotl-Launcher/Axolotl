import { expect, it } from 'vitest'

import { isUnresolvedContentProject, isUnresolvedProjectId } from './content-identity'

it('recognises the synthesised project id and a provider id', () => {
    expect(isUnresolvedProjectId('local:abc')).toBe(true)
    expect(isUnresolvedProjectId('modrinth')).toBe(false)
    expect(isUnresolvedProjectId('curseforge:42')).toBe(false)
    expect(isUnresolvedProjectId('')).toBe(false)
    expect(isUnresolvedProjectId(undefined)).toBe(false)
    expect(isUnresolvedProjectId(null)).toBe(false)
})

it('reads the project id of an item, not any other id it carries', () => {
    expect(isUnresolvedContentProject({ project: { id: 'local:x' } })).toBe(true)
    expect(isUnresolvedContentProject({ project: { id: 'modrinth' } })).toBe(false)
    expect(isUnresolvedContentProject({})).toBe(false)
    expect(isUnresolvedContentProject({ project: null })).toBe(false)
})

it('keeps a self-declared author id from making the project look unresolved', () => {
    // The author id carries the same prefix for a different reason: the author
    // is declared by the local mod file. Only the project id decides this.
    const item = { project: { id: 'modrinth' }, owner: { id: 'local:author' } }
    expect(isUnresolvedContentProject(item)).toBe(false)
    expect(isUnresolvedProjectId(item.owner.id)).toBe(true)
})
