import type { ContentItem } from '@modrinth/ui'
import { expect, it } from 'vitest'
import { computed, shallowRef } from 'vue'

import modsSource from '@/pages/instance/Mods.vue?raw'
import type { PendingContentChange } from '@/providers/download-manager.ts'

import {
    activeContentChangeJobs,
    contentChangeAffectsItem,
    indexContentChanges,
} from './content-change-jobs.ts'
import type { InstallJobSnapshot } from './install.ts'

function job(
    status: InstallJobSnapshot['status'],
    intent: NonNullable<InstallJobSnapshot['content_change']>['intent'],
    contentIds: string[] = [],
): InstallJobSnapshot {
    return {
        job_id: `${status}-${intent.type}`,
        instance_id: 'instance-a',
        kind: 'change_content',
        status,
        target: { type: 'existing_instance', instance_id: 'instance-a' },
        content_change: { intent, content_ids: contentIds },
    } as InstallJobSnapshot
}

const userItem = {
    instanceEntryId: 'entry-a',
    instanceOwnershipKind: 'user_added',
} as ContentItem
const packItem = {
    instanceEntryId: 'entry-b',
    instanceOwnershipKind: 'pack_managed',
} as ContentItem

it('active content jobs are restored by kind, instance, and active status', () => {
    const active = job('running', { type: 'update_one', content_id: 'entry-a' }, ['entry-a'])
    const paused = job('waiting_for_user', { type: 'update_one', content_id: 'entry-a' }, [
        'entry-a',
    ])
    const finished = job('succeeded', { type: 'update_one', content_id: 'entry-a' }, ['entry-a'])
    const otherInstance = {
        ...active,
        job_id: 'other-instance',
        instance_id: 'instance-b',
        target: { type: 'existing_instance', instance_id: 'instance-b' },
    } as InstallJobSnapshot
    const install = { ...active, job_id: 'install', kind: 'install_content' } as InstallJobSnapshot

    expect(
        activeContentChangeJobs([active, paused, finished, otherInstance, install], 'instance-a'),
    ).toEqual([active, paused])
})

it('resolved jobs affect only their persisted stable content IDs', () => {
    const update = job('queued', { type: 'update_one', content_id: 'entry-a' }, ['entry-a'])
    const index = indexContentChanges([update], [])
    expect(contentChangeAffectsItem(index, userItem)).toBe(true)
    expect(contentChangeAffectsItem(index, packItem)).toBe(false)
})

it('unresolved update-all marks user-added content without blocking pack content', () => {
    const updateAll = job('queued', { type: 'update_all_user_added' })
    expect(contentChangeAffectsItem(indexContentChanges([updateAll], []), userItem)).toBe(true)
    expect(contentChangeAffectsItem(indexContentChanges([updateAll], []), packItem)).toBe(false)
    updateAll.content_change!.content_ids = ['entry-b']
    const resolved = indexContentChanges([updateAll], [])
    expect(contentChangeAffectsItem(resolved, userItem)).toBe(false)
    expect(contentChangeAffectsItem(resolved, packItem)).toBe(true)
})

it('submission reservations synchronously cover their exact content scope', () => {
    const selected: PendingContentChange = {
        id: 'pending-selected',
        instanceId: 'instance-a',
        intent: {
            type: 'update_selected',
            targets: [{ content_id: 'entry-a', target_release_id: 'release-a' }],
        },
        contentIds: ['entry-a'],
        updateAll: false,
    }
    const updateAll: PendingContentChange = {
        id: 'pending-all',
        instanceId: 'instance-a',
        intent: { type: 'update_all_user_added' },
        contentIds: [],
        updateAll: true,
    }

    expect(contentChangeAffectsItem(indexContentChanges([], [selected]), userItem)).toBe(true)
    expect(contentChangeAffectsItem(indexContentChanges([], [selected]), packItem)).toBe(false)
    // updateAll is scoped to user-added entries even if a stale reservation has IDs.
    updateAll.contentIds = ['entry-b']
    expect(contentChangeAffectsItem(indexContentChanges([], [updateAll]), userItem)).toBe(true)
    expect(contentChangeAffectsItem(indexContentChanges([], [updateAll]), packItem)).toBe(false)
})

it('uses entry, member, then file identity without falling through an unmatched higher-priority ID', () => {
    const index = indexContentChanges(
        [job('running', { type: 'update_all_user_added' }, ['member', 'file'])],
        [],
    )
    expect(
        contentChangeAffectsItem(index, {
            instanceEntryId: 'other',
            instanceMemberId: 'member',
            instanceFileId: 'file',
        } as ContentItem),
    ).toBe(false)
    expect(
        contentChangeAffectsItem(index, {
            instanceMemberId: 'member',
            instanceFileId: 'other',
        } as ContentItem),
    ).toBe(true)
    expect(contentChangeAffectsItem(index, { instanceFileId: 'file' } as ContentItem)).toBe(true)
    expect(contentChangeAffectsItem(index, {} as ContentItem)).toBe(false)
})

it('rebuilds the computed index when the instance or pending/job snapshots change', () => {
    const instanceId = shallowRef('instance-a')
    const jobs = shallowRef([
        job('running', { type: 'update_one', content_id: 'entry-a' }, ['entry-a']),
    ])
    const pending = shallowRef<PendingContentChange[]>([])
    let builds = 0
    const index = computed(() => {
        builds++
        return indexContentChanges(
            activeContentChangeJobs(jobs.value, instanceId.value),
            pending.value.filter((change) => change.instanceId === instanceId.value),
        )
    })
    expect(contentChangeAffectsItem(index.value, userItem)).toBe(true)
    expect(contentChangeAffectsItem(index.value, packItem)).toBe(false)
    expect(builds).toBe(1)
    jobs.value = []
    expect(contentChangeAffectsItem(index.value, userItem)).toBe(false)
    pending.value = [
        {
            instanceId: 'instance-b',
            contentIds: ['entry-b'],
            updateAll: false,
        } as PendingContentChange,
    ]
    expect(contentChangeAffectsItem(index.value, packItem)).toBe(false)
    instanceId.value = 'instance-b'
    expect(contentChangeAffectsItem(index.value, packItem)).toBe(true)
    pending.value = []
    expect(contentChangeAffectsItem(index.value, packItem)).toBe(false)
})

it.each([500, 1000])('reads %i IDs once per snapshot, not again for every content row', (size) => {
    let reads = 0
    const ids = Array.from({ length: size }, (_, i) => `entry-${i}`)
    const counted = new Proxy(ids, {
        get(target, property, receiver) {
            if (typeof property === 'string' && /^\d+$/.test(property)) reads++
            return Reflect.get(target, property, receiver)
        },
    })
    const index = indexContentChanges(
        [job('running', { type: 'update_all_user_added' }, counted)],
        [{ contentIds: counted, updateAll: false } as PendingContentChange],
    )
    expect(reads).toBe(size * 2)
    let membershipScans = 0
    const iterateIds = index.contentIds[Symbol.iterator].bind(index.contentIds)
    index.contentIds[Symbol.iterator] = function* () {
        for (const id of iterateIds()) {
            membershipScans++
            yield id
        }
    }
    for (let i = 0; i < size; i++) {
        expect(
            contentChangeAffectsItem(index, { instanceEntryId: `entry-${i}` } as ContentItem),
        ).toBe(true)
        expect(
            contentChangeAffectsItem(index, { instanceEntryId: `absent-${i}` } as ContentItem),
        ).toBe(false)
    }
    expect(reads).toBe(size * 2)
    expect(membershipScans).toBe(0)
})

it('shares the computed membership index across rendered mod rows', () => {
    expect(modsSource).toMatch(
        /const contentChangeIndex = computed\(\(\) =>\s*indexContentChanges\(activeContentChangeJobs.value, pendingContentChanges.value\)/,
    )
    expect(modsSource).toMatch(
        /function hasGlobalContentChange\(item: ContentItem\) \{\s*return contentChangeAffectsItem\(contentChangeIndex.value, item\)/,
    )
})
