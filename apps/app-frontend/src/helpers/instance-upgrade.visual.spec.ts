import { expect, it, vi } from 'vitest'

import { plan_instance_upgrade } from './instance-upgrade'

const backend = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ Channel: vi.fn(), invoke: backend.invoke }))

it('stops waiting at once on abort, cancels the planner and discards a plan that arrives late', async () => {
    let finish!: (plan: unknown) => void
    backend.invoke.mockImplementation((command) =>
        command === 'plugin:instance|instance_plan_upgrade'
            ? new Promise((resolve) => {
                  finish = resolve
              })
            : Promise.resolve(undefined),
    )
    const controller = new AbortController()
    const onStop = vi.fn()
    const planned = plan_instance_upgrade(
        'instance',
        {
            gameVersion: '1.21.5',
            modLoader: 'vanilla',
            modLoaderVersion: null,
            shaderRuntime: 'none',
        },
        { signal: controller.signal, onStop },
    )
    controller.abort()
    await expect(planned).rejects.toMatchObject({ name: 'AbortError' })
    const { requestId } = backend.invoke.mock.calls[0][1] as { requestId: string }
    expect(backend.invoke).toHaveBeenCalledWith(
        'plugin:instance|instance_cancel_upgrade_planning',
        {
            requestId,
        },
    )
    expect(onStop).toHaveBeenCalledExactlyOnceWith(expect.any(Promise))
    finish({ id: 'late-plan' })
    await vi.waitFor(() =>
        expect(backend.invoke).toHaveBeenCalledWith(
            'plugin:instance|instance_discard_upgrade_plan',
            {
                planId: 'late-plan',
            },
        ),
    )
})
