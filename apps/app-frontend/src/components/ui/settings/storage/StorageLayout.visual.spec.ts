import { Checkbox, I18N_INJECTION_KEY } from '@modrinth/ui'
import { flushPromises } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { createPinia } from 'pinia'
import { afterEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { defineComponent, h, ref } from 'vue'

import { purgeLauncherCache } from '@/helpers/cache.js'
import {
    analyzeMeta,
    cleanMeta,
    openStoragePaths,
    scanStorage,
    type StorageNode,
} from '@/helpers/storage'
import zh from '@/locales/zh-CN/index.json'
import { mountThemed, waitFor } from '@/test/visual-harness'

import StorageSettings from '../StorageSettings.vue'

vi.mock('@/helpers/storage', () => ({
    scanStorage: vi.fn(),
    openStoragePaths: vi.fn(),
    analyzeMeta: vi.fn(),
    cleanMeta: vi.fn(),
}))
vi.mock('@/helpers/storage/preview-cache-usage', () => ({ previewCacheUsage: async () => 1200000 }))
vi.mock('@/helpers/cache.js', () => ({ purgeLauncherCache: vi.fn() }))
vi.mock('vue-router', async (original) => ({
    ...(await original<typeof import('vue-router')>()),
    useRouter: () => ({ push: vi.fn() }),
}))
const mounted: Awaited<ReturnType<typeof mountThemed>>[] = []
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.resetAllMocks()
})
const node = (
    id: string,
    type: StorageNode['type'],
    bytes: number,
    name?: string,
    children?: StorageNode[],
): StorageNode => ({
    id,
    type,
    name,
    size: { actual: bytes, symlink: 0 },
    paths: [{ path: `D:/Minecraft/Axolotl/${id}`, kind: 'directory' }],
    children,
})
const categories = () => [
    node('instances', 'instances', 4800000000, undefined, [
        {
            ...node('instance-1', 'instance', 4800000000, '红石生存 · Fabric 1.21.1', [
                node('mods', 'mods', 1200000000, undefined, [
                    {
                        ...node('mod.jar', 'folder', 1000, 'mod.jar'),
                        paths: [{ path: 'D:/Minecraft/Axolotl/mods/mod.jar', kind: 'file' }],
                    },
                ]),
            ]),
            instance_id: 'instance-1',
        },
    ]),
    node('cache', 'cache', 1200000000, undefined, [
        node('store', 'content_store', 900000000),
        node('resource-db', 'resource_cache', 300000000),
    ]),
    node('meta', 'meta', 3200000000, undefined, [
        node('versions', 'versions', 1200000000),
        node('libraries', 'libraries', 1000000000),
        node('assets', 'assets', 1000000000),
    ]),
    node('backups', 'backups', 800000000),
    node('db', 'database', 9000000, undefined, [
        node('release', 'database', 5000000, 'release'),
        node('beta', 'database', 4000000, 'beta'),
    ]),
]

it.each([1024, 680])('storage overview and cleanup use real components at %i px', async (width) => {
    await page.viewport(width, 1200)
    document.documentElement.style.height = 'auto'
    document.documentElement.style.overflow = 'auto'
    document.body.style.height = 'auto'
    document.body.style.overflow = 'auto'
    document.body.style.background = 'var(--surface-2)'
    vi.mocked(scanStorage).mockResolvedValue({
        scannedAt: '2026-10-15T00:00:00Z',
        total: { actual: 10009000000, symlink: 0 },
        categories: categories(),
        rootOther: null,
        external: [],
        warnings: [],
    })
    const initialSnapshot = await vi.mocked(scanStorage).getMockImplementation()!(false)
    let finishInitial!: (tree: typeof initialSnapshot) => void
    vi.mocked(scanStorage).mockImplementationOnce((_force, report) => {
        report?.({ kind: 'progress', completed: 0, total: 16, category: 'instances' })
        return new Promise((resolve) => {
            finishInitial = resolve
        })
    })
    vi.mocked(analyzeMeta).mockResolvedValue({
        id: 'plan',
        scanned_at: '2026-10-15T00:00:00Z',
        warnings: [],
        groups: [
            {
                id: 'versions/1.21.1-21.1.248',
                category: 'versions',
                name: '1.21.1-21.1.248',
                used: 190000000,
                unused: 0,
                unknown: 0,
                files: 2,
            },
            {
                id: 'versions/1.20.1-47.4.20',
                category: 'versions',
                name: '1.20.1-47.4.20',
                used: 0,
                unused: 240000000,
                unknown: 0,
                files: 3,
            },
            {
                id: 'libraries',
                category: 'libraries',
                name: '',
                used: 510000000,
                unused: 380000000,
                unknown: 12000000,
                files: 120,
            },
            {
                id: 'assets',
                category: 'assets',
                name: '',
                used: 1200000000,
                unused: 0,
                unknown: 0,
                files: 23456,
            },
            {
                id: 'java_versions/java21',
                category: 'java_versions',
                name: 'java21',
                used: 0,
                unused: 0,
                unknown: 300000000,
                files: 1100,
            },
        ],
    })
    const Host = defineComponent({
        setup: () => () =>
            h(
                'main',
                { class: 'mx-auto min-h-screen max-w-5xl bg-surface-2 p-4' },
                h(StorageSettings),
            ),
    })
    const wrapper = await mountThemed(Host, {}, 'dark', {
        global: {
            plugins: [createPinia()],
            directives: { tooltip: {} },
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref('zh-CN'),
                    t: (id: string, values: Record<string, unknown>) => {
                        const message = (zh as Record<string, { message: string }>)[id]?.message
                        return message ? new IntlMessageFormat(message, 'zh-CN').format(values) : id
                    },
                    setLocale: () => {},
                },
            },
        },
    })
    mounted.push(wrapper)
    await flushPromises()
    expect(wrapper.find('strong').exists()).toBe(false)
    expect(wrapper.find('[role="progressbar"]').exists()).toBe(true)
    expect(wrapper.find('.animate-pulse').exists()).toBe(false)
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/scan-initial-${width}.png`,
            element: wrapper.element,
        })
    finishInitial(initialSnapshot)
    await flushPromises()
    await waitFor(() => wrapper.find('strong').exists())
    const categoryRows = wrapper.findAll('.tree-row')
    const cacheRow = categoryRows.find((row) => row.text().includes('缓存数据'))!
    const referenceRow = categoryRows.find((row) => row.text().includes('实例数据'))!
    for (const row of categoryRows.filter(
        (row) => row.element.getBoundingClientRect().height > 0,
    )) {
        const category = row.element.closest('details') ?? row.element.parentElement!
        expect(getComputedStyle(category).borderLeftWidth).toBe('0px')
        expect(getComputedStyle(category).borderRightWidth).toBe('0px')
        expect(row.element.getBoundingClientRect().left).toBe(
            referenceRow.element.getBoundingClientRect().left,
        )
    }
    const previous = await vi.mocked(scanStorage).mock.results[0].value
    let finishScan!: (tree: typeof previous) => void
    vi.mocked(scanStorage).mockImplementationOnce((_force, report) => {
        report?.({ kind: 'progress', completed: 3, total: 16, category: 'meta' })
        return new Promise((resolve) => {
            finishScan = resolve
        })
    })
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '更新')!
        .trigger('click')
    await flushPromises()
    expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBe('19')
    expect(wrapper.findAll('.tree-row').length).toBe(categoryRows.length)
    expect(wrapper.find('.animate-pulse').exists()).toBe(false)
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/scan-progress-${width}.png`,
            element: wrapper.element,
        })
    finishScan(previous)
    await flushPromises()
    expect(wrapper.find('[role="progressbar"]').exists()).toBe(false)
    vi.mocked(openStoragePaths).mockResolvedValue({ opened: [], failed: [] })
    referenceRow.element.click()
    const instanceRow = wrapper.findAll('.tree-row').find((row) => row.text().includes('红石生存'))!
    await waitFor(() => instanceRow.element.getBoundingClientRect().height > 0)
    const openInstance = instanceRow.get('button[aria-label="在文件管理器中打开文件夹"]')
    const disclosure = instanceRow.element.closest('details')!
    openInstance.element.click()
    await flushPromises()
    expect(disclosure.open).toBe(false)
    expect(openStoragePaths).toHaveBeenLastCalledWith([
        { path: 'D:/Minecraft/Axolotl/instance-1', kind: 'directory' },
    ])
    expect(instanceRow.element.getBoundingClientRect().height).toBe(
        referenceRow.element.getBoundingClientRect().height,
    )
    instanceRow.element.click()
    const modsRow = wrapper
        .findAll('.tree-row')
        .find((row) => row.text().includes('模组') && !row.text().includes('mod.jar'))!
    await waitFor(() => modsRow.element.getBoundingClientRect().height > 0)
    modsRow.get('button[aria-label="在文件管理器中打开文件夹"]').element.click()
    await flushPromises()
    expect(openStoragePaths).toHaveBeenLastCalledWith([
        { path: 'D:/Minecraft/Axolotl/mods', kind: 'directory' },
    ])
    const fileRow = wrapper.findAll('.tree-row').find((row) => row.text().includes('mod.jar'))!
    expect(fileRow.find('button[aria-label="在文件管理器中打开文件夹"]').exists()).toBe(false)
    expect(cacheRow.find('button[aria-label="在文件管理器中打开文件夹"]').exists()).toBe(false)
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/instance-folders-${width}.png`,
            element: wrapper.element,
        })
    referenceRow.element.click()
    await waitFor(() => !referenceRow.element.closest('details')!.open)
    expect(cacheRow.element.getBoundingClientRect().height).toBe(
        referenceRow.element.getBoundingClientRect().height,
    )
    expect(cacheRow.findAll('button').some((button) => button.text() === '清理缓存')).toBe(true)
    const cacheDisclosure = cacheRow.element.closest('details')!
    const wasOpen = cacheDisclosure.open
    cacheRow
        .findAll('button')
        .find((button) => button.text() === '清理缓存')!
        .element.click()
    await waitFor(() => !!document.querySelector('[role="dialog"]'))
    expect(cacheDisclosure.open).toBe(wasOpen)
    expect(cacheRow.element.getBoundingClientRect().height).toBe(
        referenceRow.element.getBoundingClientRect().height,
    )
    vi.mocked(purgeLauncherCache).mockResolvedValueOnce({
        removed_bytes: 1000000000,
        remaining_bytes: 200000000,
        failures: [],
    })
    const confirmCache = [
        ...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button'),
    ].find((button) => button.textContent === '清理缓存')!
    confirmCache.click()
    await flushPromises()
    await waitFor(() => !!document.querySelector('[role="dialog"] [role="status"]'))
    expect(document.querySelector('[role="dialog"]')!.textContent).toContain('已释放')
    expect(cacheRow.element.getBoundingClientRect().height).toBe(
        referenceRow.element.getBoundingClientRect().height,
    )
    await waitFor(() => {
        const dialog = document.querySelector('[role="dialog"]')!
        const container = dialog.closest('.modal-container')!
        return (
            container.classList.contains('shown') &&
            getComputedStyle(container).opacity === '1' &&
            getComputedStyle(dialog).opacity === '1' &&
            dialog.getBoundingClientRect().left >= 0
        )
    })
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/cache-result-${width}.png`,
        })
    const closeCache = [
        ...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button'),
    ].find((button) => button.textContent === '关闭')!
    closeCache.click()
    await waitFor(() => !document.querySelector('[role="dialog"]'))

    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/overview-${width}.png`,
            element: wrapper.element,
        })
    const analyze = wrapper
        .findAll('button')
        .find((button) => button.text().includes('检查可清理'))!
    await analyze.trigger('click')
    await flushPromises()
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/metadata-${width}.png`,
            element: wrapper.element,
        })
    const checkbox = wrapper.findAllComponents(Checkbox)[0]
    await checkbox.trigger('click')
    await flushPromises()
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '清理所选文件')!
        .trigger('click')
    await waitFor(() => !!document.querySelector('[role="dialog"]'))
    const dialog = () => document.querySelector('[role="dialog"]')!
    await waitFor(() => {
        const container = dialog().closest('.modal-container')!
        return (
            container.classList.contains('shown') &&
            getComputedStyle(container).opacity === '1' &&
            getComputedStyle(dialog()).opacity === '1' &&
            dialog().getBoundingClientRect().top >= 0
        )
    })
    const actions = dialog().querySelectorAll<HTMLButtonElement>('button')
    const cancel = [...actions].find((button) => button.textContent === '取消')!
    const confirm = [...actions].find((button) => button.textContent === '清理所选文件')!
    expect(
        confirm.getBoundingClientRect().left - cancel.getBoundingClientRect().right,
    ).toBeGreaterThanOrEqual(8)
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/review-${width}.png`,
        })
    let finish!: (result: Awaited<ReturnType<typeof cleanMeta>>) => void
    vi.mocked(cleanMeta).mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    confirm.click()
    await flushPromises()
    expect(dialog().textContent).toContain('正在删除文件')
    expect(dialog().textContent).not.toContain('释放')
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/deleting-${width}.png`,
        })
    vi.mocked(scanStorage).mockReturnValueOnce(new Promise(() => {}))
    const nextAnalysis = await vi.mocked(analyzeMeta).mock.results[0].value
    vi.mocked(analyzeMeta).mockResolvedValue({
        ...nextAnalysis,
        id: 'next-plan',
        groups: nextAnalysis.groups.map((group) =>
            group.id === 'libraries' ? { ...group, unused: 0 } : group,
        ),
    })
    finish({
        removed_files: 2,
        removed_bytes: 380000000,
        removed_groups: { libraries: { files: 2, bytes: 380000000 } },
        skipped_files: 0,
        failures: [],
    })
    await flushPromises()
    await waitFor(() => !document.querySelector('[role="dialog"]'))
    expect(wrapper.text()).toContain('正在刷新占用统计')
    expect(wrapper.text()).toContain('已删除 2 个文件')
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/done-${width}.png`,
            element: wrapper.element,
        })
    expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(width)
})
