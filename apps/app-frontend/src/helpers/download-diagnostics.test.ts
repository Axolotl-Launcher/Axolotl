import { invoke } from '@tauri-apps/api/core'
import { expect, it, vi } from 'vitest'

import { download_diagnostics } from './download-diagnostics'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

it('uses the registered diagnostics command with a bounded selected-file query', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ enabled: false, files: [] })
    await download_diagnostics('job', 'file')
    expect(invoke).toHaveBeenCalledWith('plugin:install|download_diagnostics', {
        jobId: 'job',
        fileId: 'file',
    })
})
