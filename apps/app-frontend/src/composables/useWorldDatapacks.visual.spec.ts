import { beforeEach, expect, it, vi } from 'vitest'

import { listDatapacks, setDatapackEnabled, type WorldWithDatapacks } from '@/helpers/datapacks'

import { useWorldDatapacks } from './useWorldDatapacks'

vi.mock('@modrinth/ui', () => ({
    useDebugLogger: () => () => {},
    defineMessage: (message: unknown) => message,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))
vi.mock('@/helpers/datapacks', () => ({
    listDatapacks: vi.fn(),
    setDatapackEnabled: vi.fn(),
    deleteDatapack: vi.fn(),
}))

beforeEach(() => vi.resetAllMocks())

function world(path = 'world', enabled: boolean | undefined = true): WorldWithDatapacks {
    return {
        type: 'singleplayer',
        path,
        name: path,
        game_mode: 'survival',
        hardcore: false,
        locked: false,
        display_status: 'normal',
        datapacks: [
            { file_name: 'pack.zip', display_name: 'pack', kind: 'zip', enabled, size: 10 },
        ],
    }
}

it('keeps previous rows and reports failure, including on first load', async () => {
    const state = useWorldDatapacks(() => 'instance')
    vi.mocked(listDatapacks)
        .mockRejectedValueOnce(new Error('offline'))
        .mockResolvedValueOnce([world()])
        .mockRejectedValueOnce(new Error('offline'))
    await state.loadWorldDatapacks()
    expect(state.worldDatapackErrors.value).toEqual(['offline'])
    await state.loadWorldDatapacks()
    expect(state.worldDatapackErrors.value).toEqual([])
    await state.loadWorldDatapacks()
    expect(state.worldDatapackItems.value).toHaveLength(1)
    expect(state.worldDatapackErrors.value).toEqual(['offline'])
})

it('merges successful worlds while preserving failed worlds and disabling unknown state', async () => {
    const state = useWorldDatapacks(() => 'instance')
    vi.mocked(listDatapacks)
        .mockResolvedValueOnce([world(), world('second')])
        .mockResolvedValueOnce([
            { ...world(), datapacks: [], read_error: 'denied' },
            world('second', false),
            {
                ...world('third'),
                datapacks: [{ ...world('third').datapacks[0], enabled: undefined }],
                state_error: 'corrupt',
            },
        ])
    await state.loadWorldDatapacks()
    await state.loadWorldDatapacks()
    const items = state.worldDatapackItems.value
    expect(items).toHaveLength(3)
    expect(items[0].instanceCapabilities?.canToggle).toBe(false)
    expect(items[1].enabled).toBe(false)
    expect(items[1].instanceCapabilities?.canToggle).toBe(true)
    expect(items[2].enabled).toBeUndefined()
    expect(items[2].instanceCapabilities?.canToggle).toBe(false)
    await expect(state.toggleWorldDatapackItem(items[2], true)).rejects.toThrow()
    expect(setDatapackEnabled).not.toHaveBeenCalled()
    expect(state.worldDatapackErrors.value).toEqual(['world: denied', 'third: corrupt'])
})

it('discards responses for a different instance and rolls back failed toggles', async () => {
    let instanceId = 'first'
    const state = useWorldDatapacks(() => instanceId)
    let complete!: (value: WorldWithDatapacks[]) => void
    vi.mocked(listDatapacks)
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    complete = resolve
                }),
        )
        .mockResolvedValueOnce([world('new')])
    const old = state.loadWorldDatapacks()
    instanceId = 'second'
    await state.loadWorldDatapacks()
    complete([world('old')])
    await old
    expect(state.worldDatapacks.value.map((entry) => entry.path)).toEqual(['new'])
    vi.mocked(setDatapackEnabled).mockRejectedValue(new Error('world locked'))
    await expect(
        state.toggleWorldDatapackItem(state.worldDatapackItems.value[0], false),
    ).rejects.toThrow('world locked')
    expect(state.worldDatapackItems.value[0].enabled).toBe(true)
})
