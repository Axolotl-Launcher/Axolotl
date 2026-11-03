import { expect, it } from 'vitest'

import {
    instanceContentProject,
    instanceServerProjectId,
    isServerProjectLink,
} from './instance-project.ts'
import type { InstanceLink } from './types'

it('instance project routing follows the provider, including numeric Modrinth IDs', () => {
    const curseforge: InstanceLink = {
        type: 'curseforge_modpack',
        project_id: '1266680',
        version_id: '1',
    }
    expect(instanceContentProject(curseforge)).toEqual({
        source: 'curseforge',
        projectId: '1266680',
    })
    expect(instanceServerProjectId(curseforge)).toBe(null)
    expect(isServerProjectLink(curseforge)).toBe(false)
    const modrinth: InstanceLink = {
        type: 'modrinth_modpack',
        project_id: '12666800',
        version_id: '12345678',
    }
    expect(instanceContentProject(modrinth)).toEqual({
        source: 'modrinth',
        projectId: '12666800',
    })
    expect(instanceServerProjectId(modrinth)).toBe('12666800')
})

it('server content and server metadata use their respective recorded project IDs', () => {
    const link: InstanceLink = {
        type: 'server_project_modpack',
        server_project_id: 'server01',
        content_project_id: 'content1',
        content_version_id: 'version1',
    }
    expect(instanceContentProject(link)).toEqual({ source: 'modrinth', projectId: 'content1' })
    expect(instanceServerProjectId(link)).toBe('server01')
    expect(isServerProjectLink(link)).toBe(true)
    expect(instanceServerProjectId({ type: 'server_project', project_id: 'server02' })).toBe(
        'server02',
    )
})

it('local imports and shared instances do not invent a provider for their IDs', () => {
    for (const link of [
        null,
        undefined,
        { type: 'imported_modpack', project_id: '1266680' },
        { type: 'shared_instance', shared_instance_id: 'shared' },
    ] as (InstanceLink | null | undefined)[]) {
        expect(instanceContentProject(link)).toBe(null)
        expect(instanceServerProjectId(link)).toBe(null)
        expect(isServerProjectLink(link)).toBe(false)
    }
})
