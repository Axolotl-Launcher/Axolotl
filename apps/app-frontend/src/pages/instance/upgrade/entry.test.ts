import { expect, it } from 'vitest'

import type { InstallJobSnapshot } from '@/helpers/install'
import type { GameInstance } from '@/helpers/types'

import { isActiveUpgradeJobForInstance, isUnmanagedUpgradeEligible } from './entry.ts'

const instance = (link: GameInstance['link'] = null): GameInstance => ({
    id: 'instance',
    path: 'path',
    install_stage: 'installed',
    launcher_feature_version: '1',
    name: 'Instance',
    game_version: '1.21.8',
    loader: 'fabric',
    loader_components: [],
    groups: [],
    link,
    update_channel: 'release',
    created: new Date(),
    modified: new Date(),
    submitted_time_played: 0,
    recent_time_played: 0,
    hooks: {},
})

it('eligibility allows local/shared and excludes managed packs', () => {
    expect(isUnmanagedUpgradeEligible(instance())).toBe(true)
    expect(
        isUnmanagedUpgradeEligible(
            instance({ type: 'shared_instance', shared_instance_id: 'shared' }),
        ),
    ).toBe(true)
    expect(
        isUnmanagedUpgradeEligible({
            ...instance(),
            linked_dot_minecraft: String.raw`C:\Games\.minecraft`,
        }),
    ).toBe(true)
    expect(
        isUnmanagedUpgradeEligible(
            instance({ type: 'modrinth_modpack', project_id: 'p', version_id: 'v' }),
        ),
    ).toBe(false)
    expect(isUnmanagedUpgradeEligible({ ...instance(), install_stage: 'not_installed' })).toBe(
        false,
    )
})

it('active upgrade job ownership is exact', () => {
    const job = {
        kind: 'upgrade_unmanaged_instance',
        status: 'running',
        instance_id: 'instance',
    } as InstallJobSnapshot
    expect(isActiveUpgradeJobForInstance(job, 'instance')).toBe(true)
    expect(isActiveUpgradeJobForInstance(job, 'other')).toBe(false)
    expect(isActiveUpgradeJobForInstance({ ...job, status: 'succeeded' }, 'instance')).toBe(false)
})

it('active copy upgrade belongs to original source instance', () => {
    const job = {
        kind: 'upgrade_unmanaged_instance',
        status: 'running',
        instance_id: 'copy',
        source_instance_id: 'source',
    } as InstallJobSnapshot
    expect(isActiveUpgradeJobForInstance(job, 'source')).toBe(true)
    expect(isActiveUpgradeJobForInstance(job, 'copy')).toBe(false)
})
