import { beforeEach, expect, it, vi } from 'vitest'

import { confirmJavaSelection } from './java-selection'

const mocks = vi.hoisted(() => ({ inspect: vi.fn(), approve: vi.fn(), confirm: vi.fn() }))
vi.mock('./jre', () => ({
    get_java_selection_info: mocks.inspect,
    approve_java_selection: mocks.approve,
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: mocks.confirm }))
vi.mock('@modrinth/ui', () => ({ defineMessages: (messages: unknown) => messages }))
const runtime = { path: 'E:/jdk/bin/javaw.exe', parsed_version: 17 }
const format = (message: { defaultMessage: string }, values?: Record<string, string>) =>
    message.defaultMessage.replace('{path}', values?.path ?? '')

beforeEach(() => {
    vi.resetAllMocks()
    mocks.inspect.mockResolvedValue({
        runtime,
        storage: { kind: 'external', volume_id: 'volume-a' },
        requires_confirmation: true,
    })
    mocks.approve.mockResolvedValue(runtime)
})

it('does not approve or return a declined external selection', async () => {
    mocks.confirm.mockResolvedValue(false)
    expect(await confirmJavaSelection(runtime, format)).toBeNull()
    expect(mocks.approve).not.toHaveBeenCalled()
    expect(mocks.confirm.mock.calls[0][0]).toContain(runtime.path)
})

it('approves only the reviewed canonical runtime and volume after confirmation', async () => {
    mocks.confirm.mockResolvedValue(true)
    expect(await confirmJavaSelection({ path: 'alias/java.exe' }, format)).toEqual(runtime)
    expect(mocks.approve).toHaveBeenCalledWith(runtime.path, 'volume-a')
})

it('uses approved or local selections without repeatedly asking', async () => {
    mocks.inspect.mockResolvedValue({ runtime, requires_confirmation: false })
    expect(await confirmJavaSelection(runtime, format)).toEqual(runtime)
    expect(mocks.confirm).not.toHaveBeenCalled()
    expect(mocks.approve).not.toHaveBeenCalled()
})

it('does not commit when storage identity changed during confirmation', async () => {
    mocks.confirm.mockResolvedValue(true)
    mocks.approve.mockRejectedValue(new Error('storage changed'))
    await expect(confirmJavaSelection(runtime, format)).rejects.toThrow('storage changed')
})

it('can clear a selection without querying or granting storage access', async () => {
    expect(await confirmJavaSelection({ path: '' }, format)).toEqual({ path: '' })
    expect(mocks.inspect).not.toHaveBeenCalled()
})
