import { expect, it } from 'vitest'

import type { InstallJobSnapshot, InstallJobStatus } from '@/helpers/install'
import type { InstanceUpgradeResult } from '@/helpers/instance-upgrade'

import {
    isInstanceUpgradeJobWith,
    isRecoverableUpgradeStatus,
    selectRecoverableUpgradeJobWith,
    submitInstanceUpgradeWith,
} from './install-job-core.ts'

function job(
    jobId: string,
    status: InstallJobStatus,
    options: {
        instanceId?: string
        sourceInstanceId?: string | null
        kind?: InstallJobSnapshot['kind']
        modified?: string
        executionMode?: InstallJobSnapshot['execution_mode']
        result?: InstanceUpgradeResult | null
    } = {},
): InstallJobSnapshot {
    return {
        job_id: jobId,
        instance_id: options.instanceId ?? 'instance-a',
        source_instance_id: options.sourceInstanceId,
        kind: options.kind ?? 'upgrade_unmanaged_instance',
        status,
        execution_mode: options.executionMode ?? 'normal',
        target: { type: 'existing_instance', instance_id: options.instanceId ?? 'instance-a' },
        modified: options.modified ?? '2026-08-22T10:00:00Z',
        created: '2026-08-22T09:00:00Z',
        upgrade_result: options.result,
    } as InstallJobSnapshot
}

const result = { planId: 'plan-a' } as InstanceUpgradeResult
const instanceIdOf = (candidate: InstallJobSnapshot) =>
    candidate.instance_id ?? candidate.target.instance_id ?? null

it('upgrade job ownership requires matching kind and instance identity', () => {
    expect(isInstanceUpgradeJobWith(job('correct', 'running'), 'instance-a', instanceIdOf)).toBe(
        true,
    )
    expect(
        isInstanceUpgradeJobWith(
            job('wrong-kind', 'running', { kind: 'install_content' }),
            'instance-a',
            instanceIdOf,
        ),
    ).toBe(false)
    expect(
        isInstanceUpgradeJobWith(
            job('wrong-instance', 'running', { instanceId: 'instance-b' }),
            'instance-a',
            instanceIdOf,
        ),
    ).toBe(false)
})

it('copy upgrade ownership follows source instance, not target instance', () => {
    const copy = job('copy', 'queued', {
        instanceId: 'copy-target',
        sourceInstanceId: 'instance-a',
    })
    expect(isInstanceUpgradeJobWith(copy, 'instance-a', instanceIdOf)).toBe(true)
    expect(isInstanceUpgradeJobWith(copy, 'copy-target', instanceIdOf)).toBe(false)
    expect(
        isInstanceUpgradeJobWith(
            job('unrelated', 'queued', { sourceInstanceId: 'instance-c' }),
            'instance-a',
            instanceIdOf,
        ),
    ).toBe(false)
})

it('active recovery includes waiting and recovery validation and chooses freshest job', () => {
    expect(isRecoverableUpgradeStatus('waiting_for_user')).toBe(true)
    const selected = selectRecoverableUpgradeJobWith(
        [
            job('older', 'running', { modified: '2026-08-22T10:00:00Z' }),
            job('newer', 'waiting_for_user', {
                modified: '2026-08-22T11:00:00Z',
                executionMode: 'recovery_validation',
            }),
        ],
        'instance-a',
        {},
        instanceIdOf,
    )
    expect(selected?.job_id).toBe('newer')
})

it('ordinary entry ignores old success while continuation recovers backend result', () => {
    const succeeded = job('succeeded', 'succeeded', { result })
    expect(selectRecoverableUpgradeJobWith([succeeded], 'instance-a', {}, instanceIdOf)).toBe(null)
    expect(
        selectRecoverableUpgradeJobWith(
            [succeeded],
            'instance-a',
            { continuation: true },
            instanceIdOf,
        )?.upgrade_result,
    ).toBe(result)
})

it('known terminal job preserves flow ownership', () => {
    const failed = job('known', 'failed')
    expect(
        selectRecoverableUpgradeJobWith(
            [failed],
            'instance-a',
            { knownJobId: 'known' },
            instanceIdOf,
        )?.job_id,
    ).toBe('known')
})

function submissionDependencies(calls: unknown[][], jobs: InstallJobSnapshot[] = []) {
    return {
        instanceIdOf,
        listJobs: async () => jobs,
        execute: async (
            planId: string,
            backup: boolean,
            mode: 'direct' | 'copy_and_upgrade',
            names: typeof displayNames,
        ) => {
            calls.push([planId, backup, mode, names])
            return job(`job-${calls.length}`, 'queued')
        },
    }
}

const displayNames = {
    backup: 'Backup',
    copy: 'Copy',
    upgradedTarget: 'Target',
    shouldAutoRename: false,
}

it('normal, shared direct, and copy submissions pass exact execution parameters', async () => {
    const calls: unknown[][] = []
    const dependencies = submissionDependencies(calls)
    for (const request of [
        {
            instanceId: 'instance-a',
            planId: 'normal',
            createFullBackup: true,
            sharedUpgradeMode: 'direct' as const,
            displayNames,
        },
        {
            instanceId: 'instance-a',
            planId: 'shared-direct',
            createFullBackup: false,
            sharedUpgradeMode: 'direct' as const,
            displayNames,
        },
        {
            instanceId: 'instance-a',
            planId: 'copy',
            createFullBackup: false,
            sharedUpgradeMode: 'copy_and_upgrade' as const,
            displayNames,
        },
    ]) {
        await submitInstanceUpgradeWith(request, { value: false }, dependencies)
    }
    expect(calls).toEqual([
        ['normal', true, 'direct', displayNames],
        ['shared-direct', false, 'direct', displayNames],
        ['copy', false, 'copy_and_upgrade', displayNames],
    ])
})

it('synchronous lock prevents double submission', async () => {
    let releaseList: (() => void) | undefined
    let executeCalls = 0
    const lock = { value: false }
    const dependencies = {
        instanceIdOf,
        listJobs: () =>
            new Promise<InstallJobSnapshot[]>((resolve) => {
                releaseList = () => resolve([])
            }),
        execute: async () => {
            executeCalls += 1
            return job('started', 'queued')
        },
    }
    const request = {
        instanceId: 'instance-a',
        planId: 'plan-a',
        createFullBackup: true,
        sharedUpgradeMode: 'direct' as const,
        displayNames,
    }
    const first = submitInstanceUpgradeWith(request, lock, dependencies)
    const second = submitInstanceUpgradeWith(request, lock, dependencies)
    expect(await second).toBe(null)
    releaseList?.()
    await first
    expect(executeCalls).toBe(1)
})

it('active preflight attaches without a second execution', async () => {
    const calls: unknown[][] = []
    const submitted = await submitInstanceUpgradeWith(
        {
            instanceId: 'instance-a',
            planId: 'plan-a',
            createFullBackup: true,
            sharedUpgradeMode: 'direct',
            displayNames,
        },
        { value: false },
        submissionDependencies(calls, [job('existing', 'running')]),
    )
    expect(submitted?.attached).toBe(true)
    expect(submitted?.job.job_id).toBe('existing')
    expect(calls.length).toBe(0)
})

it('copy execution result attaches by source identity despite different target', async () => {
    const submitted = await submitInstanceUpgradeWith(
        {
            instanceId: 'instance-a',
            planId: 'copy-plan',
            createFullBackup: false,
            sharedUpgradeMode: 'copy_and_upgrade',
            displayNames,
        },
        { value: false },
        {
            instanceIdOf,
            listJobs: async () => [],
            execute: async () =>
                job('copy-job', 'queued', {
                    instanceId: 'copy-target',
                    sourceInstanceId: 'instance-a',
                }),
        },
    )
    expect(submitted?.job.job_id).toBe('copy-job')
    expect(submitted?.attached).toBe(false)
})

it('submission failure releases lock', async () => {
    const lock = { value: false }
    await expect(
        submitInstanceUpgradeWith(
            {
                instanceId: 'instance-a',
                planId: 'plan-a',
                createFullBackup: true,
                sharedUpgradeMode: 'direct',
                displayNames,
            },
            lock,
            {
                instanceIdOf,
                listJobs: async () => [],
                execute: async () => {
                    throw new Error('stale plan')
                },
            },
        ),
    ).rejects.toThrow(/stale plan/)
    expect(lock.value).toBe(false)
})
