import { expect, it, vi } from 'vitest'
import { computed } from 'vue'

import { instanceLaunchStartedAt, trackInstanceLaunch } from './instance-launch-state'

it('shares pending launches by instance and clears state on completion', async () => {
    let finish!: (value: string) => void
    const launch = vi.fn(
        () =>
            new Promise<string>((resolve) => {
                finish = resolve
            }),
    )
    const pending = computed(() => instanceLaunchStartedAt('one') !== undefined)
    const first = trackInstanceLaunch('one', launch)
    const second = trackInstanceLaunch('one', launch)
    expect(pending.value).toBe(true)
    expect(instanceLaunchStartedAt('other')).toBeUndefined()
    await Promise.resolve()
    expect(launch).toHaveBeenCalledTimes(1)
    finish('started')
    expect(await Promise.all([first, second])).toEqual(['started', 'started'])
    expect(pending.value).toBe(false)
})

it('clears a failed launch so it can be retried', async () => {
    await expect(
        trackInstanceLaunch('failed', async () => {
            throw new Error('failed')
        }),
    ).rejects.toThrow('failed')
    expect(instanceLaunchStartedAt('failed')).toBeUndefined()
    await expect(trackInstanceLaunch('failed', async () => 'started')).resolves.toBe('started')
})
