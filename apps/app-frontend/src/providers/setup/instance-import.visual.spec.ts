import type { InstanceImportProvider } from '@modrinth/ui'
import { expect, it, vi } from 'vitest'

import { get_default_launcher_path, get_importable_instances } from '@/helpers/import.js'

import { setupInstanceImportProvider } from './instance-import'

const provided = vi.hoisted(() => ({ value: null as InstanceImportProvider | null }))
vi.mock('@modrinth/ui', () => ({
    provideInstanceImport: (provider: InstanceImportProvider) => {
        provided.value = provider
    },
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('@/helpers/events.js', () => ({ import_plan_listener: vi.fn() }))
vi.mock('@/helpers/import.js', () => ({
    cancel_import_plan: vi.fn(),
    get_default_launcher_path: vi.fn(),
    get_importable_instances: vi.fn(),
    import_instance: vi.fn(),
    start_import_plan: vi.fn(),
}))
vi.mock('@/helpers/install', () => ({ wait_for_install_job: vi.fn() }))
vi.mock('@/helpers/metadata.js', () => ({ get_loader_versions: vi.fn() }))
vi.mock('@/helpers/utils.js', () => ({ openPath: vi.fn() }))

it('reports each launcher once found, without waiting for a slow or failing one', async () => {
    let finishSlow!: (path: string) => void
    vi.mocked(get_default_launcher_path).mockImplementation(async (name: string) => {
        if (name === 'ModrinthApp')
            return await new Promise<string>((resolve) => {
                finishSlow = resolve
            })
        if (name === 'MultiMC') throw new Error('unreadable')
        return name === 'PCL2' ? 'C:/PCL2' : null
    })
    vi.mocked(get_importable_instances).mockImplementation(async (name: string) => [
        `${name} instance`,
    ])
    setupInstanceImportProvider({ handleError: vi.fn() } as never)
    const seen: string[] = []
    const detected = provided.value!.getDetectedLaunchers((launcher) => seen.push(launcher.name))
    await vi.waitFor(() => expect(seen).toEqual(['PCL2']))
    finishSlow('C:/Modrinth')
    const launchers = await detected
    expect(seen).toEqual(['PCL2', 'ModrinthApp'])
    expect(launchers.map((launcher) => launcher.name)).toEqual(['ModrinthApp', 'PCL2'])
})
