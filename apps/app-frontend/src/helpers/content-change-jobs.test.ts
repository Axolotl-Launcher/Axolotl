import type { ContentItem } from '@modrinth/ui'
import { expect, it } from 'vitest'

import type { PendingContentChange } from '@/providers/download-manager.ts'

import {
    activeContentChangeJobs,
    contentChangeAffectsItem,
    hasActiveContentChange,
    pendingContentChangeAffectsItem,
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
    expect(contentChangeAffectsItem(update, userItem)).toBe(true)
    expect(contentChangeAffectsItem(update, packItem)).toBe(false)
    expect(hasActiveContentChange([update], userItem)).toBe(true)
})

it('unresolved update-all marks user-added content without blocking pack content', () => {
    const updateAll = job('queued', { type: 'update_all_user_added' })
    expect(contentChangeAffectsItem(updateAll, userItem)).toBe(true)
    expect(contentChangeAffectsItem(updateAll, packItem)).toBe(false)
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

    expect(pendingContentChangeAffectsItem(selected, userItem)).toBe(true)
    expect(pendingContentChangeAffectsItem(selected, packItem)).toBe(false)
    expect(pendingContentChangeAffectsItem(updateAll, userItem)).toBe(true)
    expect(pendingContentChangeAffectsItem(updateAll, packItem)).toBe(false)
})
