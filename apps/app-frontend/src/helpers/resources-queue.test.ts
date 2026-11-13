import { invoke } from '@tauri-apps/api/core'
import { expect, it, vi } from 'vitest'

import { resourceQueuePlan } from './resources'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), Channel: vi.fn() }))

it('passes optional node-name metadata using the existing native queue command', async () => {
    const resourceNames = { root: 'Sodium', dependency: 'Fabric API' }
    vi.mocked(invoke).mockResolvedValueOnce({ job_id: 'queued' })
    await expect(resourceQueuePlan('plan', 3, resourceNames)).resolves.toEqual({ job_id: 'queued' })
    expect(invoke).toHaveBeenLastCalledWith('plugin:axolotl-resources|resource_queue_plan', {
        planId: 'plan',
        revision: 3,
        resourceNames,
    })
    await resourceQueuePlan('old-caller', 1)
    expect(invoke).toHaveBeenLastCalledWith('plugin:axolotl-resources|resource_queue_plan', {
        planId: 'old-caller',
        revision: 1,
        resourceNames: undefined,
    })
})
