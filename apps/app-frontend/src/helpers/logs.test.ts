import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { beforeEach, expect, it, vi } from 'vitest'

import { export_crash_context, export_log_file } from './logs'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn() }))

beforeEach(() => {
    vi.resetAllMocks()
})

it('returns the saved path after exporting the crash context', async () => {
    vi.mocked(save).mockResolvedValue('C:/exports/crash.zip')
    vi.mocked(invoke).mockResolvedValue(undefined)

    const path = await export_crash_context('instance', 'Test instance')

    expect(path).toBe('C:/exports/crash.zip')
    expect(save).toHaveBeenCalledOnce()
    expect(String(vi.mocked(save).mock.calls[0]![0]!.defaultPath)).toMatch(
        /^Test instance crash context .+\.zip$/,
    )
    expect(invoke).toHaveBeenCalledExactlyOnceWith('plugin:logs|logs_export_crash_context', {
        instanceId: 'instance',
        outputPath: 'C:/exports/crash.zip',
    })
})

it('returns null without invoking the export when the save dialog is cancelled', async () => {
    vi.mocked(save).mockResolvedValue(null)

    const path = await export_crash_context('instance', 'Test instance')

    expect(path).toBeNull()
    expect(invoke).not.toHaveBeenCalled()
})

it('exports the selected original gzip log without passing the display text through IPC', async () => {
    vi.mocked(save).mockResolvedValue('C:/exports/original.log')
    await export_log_file('instance', 'InfoLog', 'old.log.gz')
    expect(save).toHaveBeenCalledWith({
        defaultPath: 'old.log',
        filters: [{ name: 'Log', extensions: ['log', 'txt'] }],
    })
    expect(invoke).toHaveBeenCalledExactlyOnceWith('plugin:logs|logs_export_log_file', {
        instanceId: 'instance',
        logType: 'InfoLog',
        filename: 'old.log.gz',
        outputPath: 'C:/exports/original.log',
    })
})
it('does not export a log after cancellation', async () => {
    vi.mocked(save).mockResolvedValue(null)
    await export_log_file('instance', 'InfoLog', 'latest.log')
    expect(invoke).not.toHaveBeenCalled()
})
