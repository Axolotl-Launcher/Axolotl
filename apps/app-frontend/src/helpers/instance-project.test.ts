import assert from 'node:assert/strict'
import test from 'node:test'

import {
    instanceContentProject,
    instanceServerProjectId,
    isServerProjectLink,
} from './instance-project.ts'
import type { InstanceLink } from './types'

test('instance project routing follows the provider, including numeric Modrinth IDs', () => {
    const curseforge: InstanceLink = {
        type: 'curseforge_modpack',
        project_id: '1266680',
        version_id: '1',
    }
    assert.deepEqual(instanceContentProject(curseforge), {
        source: 'curseforge',
        projectId: '1266680',
    })
    assert.equal(instanceServerProjectId(curseforge), null)
    assert.equal(isServerProjectLink(curseforge), false)
    const modrinth: InstanceLink = {
        type: 'modrinth_modpack',
        project_id: '12666800',
        version_id: '12345678',
    }
    assert.deepEqual(instanceContentProject(modrinth), {
        source: 'modrinth',
        projectId: '12666800',
    })
    assert.equal(instanceServerProjectId(modrinth), '12666800')
})

test('server content and server metadata use their respective recorded project IDs', () => {
    const link: InstanceLink = {
        type: 'server_project_modpack',
        server_project_id: 'server01',
        content_project_id: 'content1',
        content_version_id: 'version1',
    }
    assert.deepEqual(instanceContentProject(link), { source: 'modrinth', projectId: 'content1' })
    assert.equal(instanceServerProjectId(link), 'server01')
    assert.equal(isServerProjectLink(link), true)
    assert.equal(
        instanceServerProjectId({ type: 'server_project', project_id: 'server02' }),
        'server02',
    )
})

test('local imports and shared instances do not invent a provider for their IDs', () => {
    for (const link of [
        null,
        undefined,
        { type: 'imported_modpack', project_id: '1266680' },
        { type: 'shared_instance', shared_instance_id: 'shared' },
    ] as (InstanceLink | null | undefined)[]) {
        assert.equal(instanceContentProject(link), null)
        assert.equal(instanceServerProjectId(link), null)
        assert.equal(isServerProjectLink(link), false)
    }
})
