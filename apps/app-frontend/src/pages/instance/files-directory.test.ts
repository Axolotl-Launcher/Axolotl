import { beforeEach, expect, it, vi } from 'vitest'

import { readInstanceDirectory, readInstanceDirectoryCounts } from './files-directory'

const filesystem = vi.hoisted(() => ({
    readDir: vi.fn(),
    stat: vi.fn(),
    sep: vi.fn(() => '\\'),
}))

vi.mock('@tauri-apps/api/path', () => ({ sep: filesystem.sep }))
vi.mock('@tauri-apps/plugin-fs', () => ({
    readDir: filesystem.readDir,
    stat: filesystem.stat,
}))

beforeEach(() => {
    vi.clearAllMocks()
    filesystem.sep.mockReturnValue('\\')
    filesystem.stat.mockResolvedValue({
        mtime: new Date('2026-01-02T00:00:00Z'),
        birthtime: new Date('2026-01-01T00:00:00Z'),
        size: 42,
    })
})

it('does not enumerate unopened child folders to count their files', async () => {
    filesystem.readDir.mockResolvedValue([
        { name: 'schematics', isDirectory: true },
        { name: 'latest.log', isDirectory: false },
    ])

    const items = await readInstanceDirectory('E:\\instance', '', () => true)

    expect(filesystem.readDir.mock.calls).toEqual([['E:\\instance']])
    expect(filesystem.stat.mock.calls).toEqual([
        ['E:\\instance\\schematics'],
        ['E:\\instance\\latest.log'],
    ])
    expect(items[0]).toEqual({
        name: 'schematics',
        type: 'directory',
        path: 'schematics',
        modified: 1767312000,
        created: 1767225600,
    })
    expect(items[1]).toMatchObject({ name: 'latest.log', size: 42 })
})

it('bounds metadata requests and stops scheduling work after navigation', async () => {
    filesystem.readDir.mockResolvedValue(
        Array.from({ length: 5000 }, (_, index) => ({
            name: `${index}.litematic`,
            isDirectory: false,
        })),
    )
    const complete: Array<(value: unknown) => void> = []
    filesystem.stat.mockImplementation(() => new Promise((resolve) => complete.push(resolve)))
    let current = true
    const reading = readInstanceDirectory('E:\\instance\\schematics', 'schematics', () => current)
    await Promise.resolve()

    expect(filesystem.stat).toHaveBeenCalledTimes(16)
    current = false
    complete.forEach((resolve) => resolve({ size: 1 }))
    expect(await reading).toEqual([])
    expect(filesystem.stat).toHaveBeenCalledTimes(16)
})

it('retains directory order and relative paths when metadata finishes out of order', async () => {
    filesystem.sep.mockReturnValue('/')
    filesystem.readDir.mockResolvedValue([
        { name: 'a.litematic', isDirectory: false },
        { name: 'gone.litematic', isDirectory: false },
        { name: 'b.litematic', isDirectory: false },
    ])
    let completeFirst: (value: unknown) => void = () => {}
    filesystem.stat.mockImplementation(async (path: string) => {
        if (path.endsWith('a.litematic')) {
            return new Promise((resolve) => (completeFirst = resolve))
        }
        if (path.endsWith('gone.litematic')) throw new Error('removed')
        return { size: 2 }
    })

    const reading = readInstanceDirectory('/instance/schematics/', 'schematics', () => true)
    await Promise.resolve()
    completeFirst({ size: 1 })

    expect((await reading).map((item) => [item.path, item.size])).toEqual([
        ['schematics/a.litematic', 1],
        ['schematics/b.litematic', 2],
    ])
    expect(filesystem.stat.mock.calls[0]).toEqual(['/instance/schematics/a.litematic'])
})

it('does not start metadata work for a directory response made obsolete while reading', async () => {
    filesystem.readDir.mockResolvedValue([{ name: 'old.litematic', isDirectory: false }])
    expect(await readInstanceDirectory('/old', '', () => false)).toEqual([])
    expect(filesystem.stat).not.toHaveBeenCalled()
})

it('loads child counts separately, preserving zero and leaving unreadable folders unknown', async () => {
    filesystem.readDir.mockResolvedValue([
        { name: 'builds', isDirectory: true },
        { name: 'empty', isDirectory: true },
        { name: 'unreadable', isDirectory: true },
        { name: 'file.litematic', isDirectory: false },
    ])
    const items = await readInstanceDirectory('E:\\instance', '', () => true)
    filesystem.readDir.mockImplementation(async (path: string) => {
        if (path.endsWith('unreadable')) throw new Error('unreadable')
        return path.endsWith('builds') ? [{ name: 'castle.litematic' }] : []
    })
    await readInstanceDirectoryCounts('E:\\instance', items, () => true)
    expect(items.map((item) => item.count)).toEqual([1, 0, undefined, undefined])
    expect(filesystem.readDir.mock.calls).toEqual([
        ['E:\\instance'],
        ['E:\\instance\\builds'],
        ['E:\\instance\\empty'],
        ['E:\\instance\\unreadable'],
    ])
})

it('stops obsolete count requests and discards their results', async () => {
    filesystem.readDir.mockResolvedValue([
        { name: 'one', isDirectory: true },
        { name: 'two', isDirectory: true },
    ])
    const items = await readInstanceDirectory('/instance', '', () => true)
    let complete: (value: unknown) => void = () => {}
    filesystem.readDir.mockImplementation(() => new Promise((resolve) => (complete = resolve)))
    let current = true
    const counting = readInstanceDirectoryCounts('/instance', items, () => current)
    current = false
    complete([{ name: 'stale.litematic' }])
    await counting
    expect(items.every((item) => item.count === undefined)).toBe(true)
    expect(filesystem.readDir).toHaveBeenCalledTimes(2)
})
