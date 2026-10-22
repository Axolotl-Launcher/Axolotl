import { Checkbox, FileTreeSelect, I18N_INJECTION_KEY, MultiStageModal } from '@modrinth/ui'
import { save } from '@tauri-apps/plugin-dialog'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import {
    type ExportResolutionSession,
    resourceExportClose,
    resourceExportQueue,
    resourceExportRefresh,
    resourceExportRefreshRetry,
    type ResourceExportScan,
    resourceExportScan,
} from '@/helpers/resources'
import { mountThemed, waitFor } from '@/test/visual-harness'

import ExportModal from '../ExportModal.vue'

vi.mock('@/helpers/resources', async (importOriginal) => ({
    ...(await importOriginal<typeof import('@/helpers/resources')>()),
    resourceExportScan: vi.fn(),
    resourceExportRefresh: vi.fn(),
    resourceExportRefreshRetry: vi.fn(),
    resourceExportClose: vi.fn().mockResolvedValue(undefined),
    resourceExportQueue: vi.fn(),
}))
vi.mock('@/helpers/utils', () => ({ highlightInFolder: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn() }))
beforeEach(() => {
    const teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.appendChild(teleports)
    vi.resetAllMocks()
    vi.mocked(resourceExportClose).mockResolvedValue(undefined)
    vi.mocked(resourceExportRefresh).mockResolvedValue(resolved())
    vi.mocked(resourceExportQueue).mockResolvedValue({
        job_id: 'export-job',
        status: 'queued',
    } as never)
})
afterEach(() => document.getElementById('teleports')?.remove())

const snapshot = (id = 'fresh'): ResourceExportScan => ({
    sessionId: id,
    instanceId: 'instance',
    instanceRevision: 1,
    categories: [
        {
            id: 'mods',
            defaultSelected: true,
            paths: ['mods/'],
            children: [],
            title: null,
            modifiedAt: null,
        },
    ],
    files: ['mods/a.jar', 'mods/deep/b.jar'].map((relativePath) => ({
        relativePath,
        integrity: { size: 10, hashes: [] },
        revision: 1,
        identifiable: true,
        modifiedAt: 1000,
        categoryId: 'mods',
        categorySelected: true,
    })),
    directories: [
        { relativePath: 'mods', modifiedAt: 1000 },
        { relativePath: 'mods/deep', modifiedAt: 1000 },
    ],
    excludedPaths: ['assets'],
    requirements: { gameVersion: '1.21.1', loader: 'neoforge', loaderVersion: '21.1.1' },
})
async function mountExport() {
    return mountThemed(ExportModal, { instance: { id: 'instance', name: 'Demo' } }, 'dark', {
        global: {
            directives: { tooltip: {} },
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref('en-US'),
                    t: (key: string) => key,
                    setLocale: () => {},
                },
                [Symbol.for('modrinth:notificationManager')]: { handleError: vi.fn() },
                [Symbol.for('modrinth:popupNotificationManager')]: {
                    addPopupNotification: vi.fn(),
                },
            },
        },
    })
}

function resolved(revision = 1): ExportResolutionSession {
    return {
        id: 'fresh',
        requestId: 'request',
        selectionRevision: revision,
        instanceRevision: 1,
        state: 'ready',
        files: snapshot().files.map((file) => ({
            snapshot: {
                ...file,
                integrity: {
                    size: 10,
                    hashes: ['sha1', 'sha256', 'sha512'].map((algorithm) => ({
                        algorithm,
                        value: 'abc',
                    })),
                },
            },
            localError: null,
            resolutions: [
                { kind: 'noMatch', source: 'modrinth' },
                { kind: 'noMatch', source: 'curseforge' },
            ],
        })),
    }
}

const exportButton = () =>
    [...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')].find(
        (button) => button.textContent?.trim() === 'Export',
    )!

it('does not queue an export after unmount while the native save dialog is open', async () => {
    vi.mocked(resourceExportScan).mockResolvedValue(snapshot())
    let finish!: (path: string) => void
    vi.mocked(save).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    const wrapper = await mountExport()
    wrapper.vm.show()
    await flushPromises()
    wrapper.findComponent(MultiStageModal).vm.setStage('package')
    await flushPromises()
    exportButton().click()
    await flushPromises()
    wrapper.unmount()
    finish('abandoned.mrpack')
    await flushPromises()
    expect(resourceExportQueue).not.toHaveBeenCalled()
    expect(resourceExportClose).toHaveBeenCalledWith('fresh')
})

it('releases a cancelled scan session even when its result arrives after another window session starts', async () => {
    let finish!: (value: ResourceExportScan) => void
    vi.mocked(resourceExportScan)
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    finish = resolve
                }),
        )
        .mockResolvedValueOnce(snapshot('current'))
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await flushPromises()
        wrapper.vm.show()
        await flushPromises()
        finish(snapshot('abandoned'))
        await flushPromises()
        expect(resourceExportClose).toHaveBeenCalledWith('abandoned')
        expect(resourceExportRefresh).toHaveBeenCalledTimes(1)
        expect(resourceExportRefresh).toHaveBeenCalledWith(
            'instance',
            'current',
            expect.any(Array),
            expect.any(Number),
            expect.any(Function),
            expect.any(AbortSignal),
        )
    } finally {
        wrapper.unmount()
    }
})

it('starts verification when opened and keeps its progress in the fixed header across stages', async () => {
    vi.mocked(resourceExportScan).mockResolvedValue(snapshot())
    vi.mocked(resourceExportRefresh).mockImplementation(() => new Promise(() => {}))
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await flushPromises()
        expect(resourceExportRefresh).toHaveBeenCalledTimes(1)
        const progress = document.querySelector('[data-export-progress]')!
        expect(progress.closest('[data-modal-content]')).toBeNull()
        const modal = wrapper.findComponent(MultiStageModal)
        modal.vm.nextStage()
        await flushPromises()
        expect(document.querySelector('[data-export-progress]')).toBe(progress)
        modal.vm.nextStage()
        await flushPromises()
        const signal = vi.mocked(resourceExportRefresh).mock.calls[0][5]!
        modal.vm.prevStage()
        await flushPromises()
        expect(signal.aborted).toBe(false)
        expect(resourceExportRefresh).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
    }
})

it('shows cumulative verification progress across local and source checks', async () => {
    vi.mocked(resourceExportScan).mockResolvedValue(snapshot())
    let update!: NonNullable<Parameters<typeof resourceExportRefresh>[4]>
    vi.mocked(resourceExportRefresh).mockImplementation(
        (_instance, _session, _files, _revision, progress) => {
            update = progress
            return new Promise(() => {})
        },
    )
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await flushPromises()
        update({
            sessionId: 'fresh',
            requestId: 'request',
            selectionRevision: 1,
            phase: 'hashing',
            completed: 1,
            total: 2,
            overallCompleted: 1,
            overallTotal: 6,
            currentFileFraction: 0.5,
        })
        await flushPromises()
        const progress = document.querySelector<HTMLElement>(
            '[data-export-progress] [role="progressbar"]',
        )!
        expect(progress.getAttribute('aria-valuenow')).toBe('25')
        expect(document.querySelector('[data-export-progress]')?.textContent).toContain('25%')
        update({
            sessionId: 'fresh',
            requestId: 'request',
            selectionRevision: 1,
            phase: 'modrinth',
            completed: 2,
            total: 2,
            overallCompleted: 4,
            overallTotal: 6,
        })
        await flushPromises()
        expect(progress.getAttribute('aria-valuenow')).toBe('67')
        expect(document.querySelector('[data-export-progress]')?.textContent).toContain('Modrinth')
        update({
            sessionId: 'fresh',
            requestId: 'request',
            selectionRevision: 1,
            phase: 'complete',
            completed: 2,
            total: 2,
            overallCompleted: 6,
            overallTotal: 6,
        })
        await flushPromises()
        expect(document.querySelector('[data-export-progress]')?.textContent).toContain(
            'Preparing verification results',
        )
    } finally {
        wrapper.unmount()
    }
})

it('queues unfinished verification, freezes the current selection and closes the modal', async () => {
    vi.mocked(resourceExportScan).mockResolvedValue(snapshot())
    vi.mocked(resourceExportRefresh).mockImplementation(() => new Promise(() => {}))
    vi.mocked(save).mockResolvedValue('out.zip')
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await flushPromises()
        const modal = wrapper.findComponent(MultiStageModal)
        modal.vm.setStage('package')
        await flushPromises()
        const inputs = document.querySelectorAll<HTMLInputElement>('[role="dialog"] input')
        inputs[0].value = 'Edited package'
        inputs[0].dispatchEvent(new Event('input', { bubbles: true }))
        inputs[2].value = 'Author'
        inputs[2].dispatchEvent(new Event('input', { bubbles: true }))
        wrapper.findComponent({ name: 'Combobox' }).vm.$emit('update:modelValue', 'curseForge')
        await flushPromises()
        expect(exportButton().disabled).toBe(false)
        exportButton().click()
        await flushPromises()
        expect(resourceExportQueue).toHaveBeenCalledWith(
            'fresh',
            snapshot().files,
            'curseForge',
            { name: 'Edited package', version: '1.0.0', summary: '', author: 'Author' },
            [],
            'out.zip',
        )
        expect(vi.mocked(resourceExportRefresh).mock.calls[0][5]!.aborted).toBe(true)
        await waitFor(() => document.querySelector('[role="dialog"]') === null)
        expect(resourceExportClose).toHaveBeenCalledWith('fresh')
    } finally {
        wrapper.unmount()
    }
})

it('checks the full snapshot once while selection changes preserve the in-flight check', async () => {
    const scan = snapshot()
    scan.files[1].categorySelected = false
    vi.mocked(resourceExportScan).mockResolvedValue(scan)
    let finish!: (value: ExportResolutionSession) => void
    vi.mocked(resourceExportRefresh).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await flushPromises()
        const signal = vi.mocked(resourceExportRefresh).mock.calls[0][5]!
        expect(vi.mocked(resourceExportRefresh).mock.calls[0][2]).toEqual(scan.files)
        const modal = wrapper.findComponent(MultiStageModal)
        const category = wrapper
            .findAllComponents(Checkbox)
            .find((checkbox) => checkbox.props('label') === 'Mods and plugins')!
        category.vm.$emit('update:modelValue', false)
        await flushPromises()
        category.vm.$emit('update:modelValue', true)
        await flushPromises()
        modal.vm.setStage('files')
        await flushPromises()
        const tree = wrapper.findComponent(FileTreeSelect)
        tree.vm.$emit('update:modelValue', [])
        await flushPromises()
        tree.vm.$emit('update:modelValue', ['mods/deep/b.jar'])
        await flushPromises()
        expect(signal.aborted).toBe(false)
        expect(resourceExportRefresh).toHaveBeenCalledTimes(1)
        finish(resolved())
        await flushPromises()
        tree.vm.$emit('update:modelValue', ['mods/a.jar', 'mods/deep/b.jar'])
        await flushPromises()
        const restore = [
            ...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button'),
        ].find((button) => button.textContent?.includes('Restore category selection'))!
        restore.click()
        await flushPromises()
        await new Promise((resolve) => setTimeout(resolve, 200))
        expect(resourceExportRefresh).toHaveBeenCalledTimes(1)
        expect(resourceExportRefreshRetry).not.toHaveBeenCalled()
        expect(document.querySelector('[data-export-progress]')?.textContent).toContain(
            'Background verification finished',
        )
    } finally {
        wrapper.unmount()
    }
})

it('retries exact failed references and sends explicit fallback choices to the queued task', async () => {
    vi.mocked(resourceExportScan).mockResolvedValue(snapshot())
    const failed = resolved()
    failed.state = 'needsDecision'
    failed.files[0].resolutions[1] = {
        kind: 'failed',
        source: 'curseforge',
        code: 'offline',
        message: 'Offline',
    }
    vi.mocked(resourceExportRefresh).mockResolvedValue(failed)
    vi.mocked(resourceExportRefreshRetry).mockResolvedValue({ ...failed, selectionRevision: 2 })
    vi.mocked(save).mockResolvedValue('out.mrpack')
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await flushPromises()
        wrapper.findComponent(MultiStageModal).vm.setStage('package')
        await flushPromises()
        const retry = [
            ...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button'),
        ].find((button) => button.textContent?.includes('Retry analysis'))!
        retry.click()
        await flushPromises()
        expect(vi.mocked(resourceExportRefreshRetry).mock.calls[0][1]).toEqual([
            { snapshot: failed.files[0].snapshot, source: 'curseforge' },
        ])
        const checkbox = [
            ...document.querySelectorAll<HTMLButtonElement>('[role="checkbox"]'),
        ].find((button) => button.textContent?.includes('Embed this file'))!
        checkbox.click()
        await flushPromises()
        exportButton().click()
        await flushPromises()
        expect(vi.mocked(resourceExportQueue).mock.calls[0][4]).toEqual([
            { snapshot: failed.files[0].snapshot, source: 'curseforge' },
        ])
    } finally {
        wrapper.unmount()
    }
})

it('retains package input and selection when queue submission fails', async () => {
    vi.mocked(resourceExportScan).mockResolvedValue(snapshot())
    vi.mocked(save).mockResolvedValue('out.mrpack')
    vi.mocked(resourceExportQueue).mockRejectedValue({
        message: 'Queue unavailable',
        code: 'resource',
    })
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await flushPromises()
        wrapper.findComponent(MultiStageModal).vm.setStage('package')
        await flushPromises()
        const input = document.querySelector<HTMLInputElement>('[role="dialog"] input')!
        input.value = 'Retained package'
        input.dispatchEvent(new Event('input', { bubbles: true }))
        exportButton().click()
        await flushPromises()
        expect(document.querySelector('[role="alert"]')?.textContent).toContain(
            'Could not complete the content operation. Refresh and try again.',
        )
        expect(input.value).toBe('Retained package')
        expect(exportButton().disabled).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('retains completed evidence and fallback choices through deselection and retries only on request', async () => {
    vi.mocked(resourceExportScan).mockResolvedValue(snapshot())
    const failed = resolved()
    failed.state = 'needsDecision'
    failed.files[0].resolutions[1] = {
        kind: 'failed',
        source: 'curseforge',
        code: 'offline',
        message: 'Offline',
    }
    vi.mocked(resourceExportRefresh).mockResolvedValueOnce(failed)
    vi.mocked(resourceExportRefreshRetry).mockResolvedValue({ ...failed, selectionRevision: 2 })
    vi.mocked(save).mockResolvedValue('out.mrpack')
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await flushPromises()
        const modal = wrapper.findComponent(MultiStageModal)
        modal.vm.setStage('package')
        await flushPromises()
        const checkbox = [
            ...document.querySelectorAll<HTMLButtonElement>('[role="checkbox"]'),
        ].find((button) => button.textContent?.includes('Embed this file'))!
        checkbox.click()
        await flushPromises()
        modal.vm.setStage('files')
        await flushPromises()
        const tree = wrapper.findComponent(FileTreeSelect)
        tree.vm.$emit('update:modelValue', ['mods/deep/b.jar'])
        await flushPromises()
        tree.vm.$emit('update:modelValue', ['mods/a.jar'])
        await flushPromises()
        modal.vm.nextStage()
        await flushPromises()
        expect(resourceExportRefresh).toHaveBeenCalledTimes(1)
        expect(resourceExportRefreshRetry).not.toHaveBeenCalled()
        expect(
            [...document.querySelectorAll<HTMLButtonElement>('[role="checkbox"]')]
                .find((button) => button.textContent?.includes('Embed this file'))
                ?.getAttribute('aria-checked'),
        ).toBe('true')
        exportButton().click()
        await flushPromises()
        expect(
            vi.mocked(resourceExportQueue).mock.calls[0][1].map((file) => file.relativePath),
        ).toEqual(['mods/a.jar'])
        expect(vi.mocked(resourceExportQueue).mock.calls[0][4]).toEqual([
            { snapshot: failed.files[0].snapshot, source: 'curseforge' },
        ])
    } finally {
        wrapper.unmount()
    }
})

it('locks decisions during queue submission and preserves them when the save dialog is canceled', async () => {
    vi.mocked(resourceExportScan).mockResolvedValue(snapshot())
    let finish!: (value: string | null) => void
    vi.mocked(save).mockImplementation(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await flushPromises()
        wrapper.findComponent(MultiStageModal).vm.setStage('package')
        await flushPromises()
        exportButton().click()
        await flushPromises()
        expect(document.querySelector<HTMLInputElement>('[role="dialog"] input')!.disabled).toBe(
            true,
        )
        expect(exportButton().disabled).toBe(true)
        finish(null)
        await flushPromises()
        expect(document.querySelector<HTMLInputElement>('[role="dialog"] input')!.disabled).toBe(
            false,
        )
        expect(resourceExportQueue).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})

it('keeps exact selection and directory through category backtracking without rescanning', async () => {
    vi.mocked(resourceExportScan).mockResolvedValue(snapshot())
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await flushPromises()
        const modal = wrapper.findComponent(MultiStageModal)
        modal.vm.nextStage()
        await flushPromises()
        let tree = wrapper.findComponent(FileTreeSelect)
        tree.vm.$emit('update:modelValue', ['mods/a.jar'])
        tree.vm.$emit('update:currentPath', 'mods')
        await flushPromises()
        modal.vm.prevStage()
        await flushPromises()
        const mods = wrapper
            .findAllComponents(Checkbox)
            .find((checkbox) => checkbox.props('label') === 'Mods and plugins')!
        mods.vm.$emit('update:modelValue', false)
        await flushPromises()
        mods.vm.$emit('update:modelValue', true)
        modal.vm.nextStage()
        await flushPromises()
        tree = wrapper.findComponent(FileTreeSelect)
        expect(tree.props('modelValue')).toEqual(['mods/a.jar'])
        expect(tree.props('currentPath')).toBe('mods')
        expect(resourceExportScan).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
    }
})

it('cancels a hidden scan and rejects its late response after reopening', async () => {
    let resolveOld!: (value: ResourceExportScan) => void
    vi.mocked(resourceExportScan)
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    resolveOld = resolve
                }),
        )
        .mockResolvedValueOnce(snapshot())
    const wrapper = await mountExport()
    try {
        wrapper.vm.show()
        await waitFor(() => vi.mocked(resourceExportScan).mock.calls.length > 0)
        const signal = vi.mocked(resourceExportScan).mock.calls[0][1]!
        wrapper.findComponent(MultiStageModal).vm.hide()
        await flushPromises()
        expect(signal.aborted).toBe(true)
        wrapper.vm.show()
        await flushPromises()
        resolveOld({ ...snapshot('old'), files: [] })
        await flushPromises()
        wrapper.findComponent(MultiStageModal).vm.nextStage()
        await flushPromises()
        expect(
            wrapper
                .findComponent(FileTreeSelect)
                .props('items')
                .filter((item) => item.type === 'file'),
        ).toHaveLength(2)
    } finally {
        wrapper.unmount()
    }
})
