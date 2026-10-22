import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { ref } from 'vue'

import type { InstallJobSnapshot } from '@/helpers/install'

import DownloadCenterButton from './DownloadCenterButton.vue'

const fixture = vi.hoisted(() => ({ manager: {} as unknown, handleError: vi.fn() }))
vi.mock('@/providers/download-manager', () => ({ injectDownloadManager: () => fixture.manager }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        ProgressBar: (
            await import('../../../../../packages/ui/src/components/base/ProgressBar.vue')
        ).default,
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        commonMessages: { retryButton: { defaultMessage: 'Retry' } },
        PopoutMenu: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', [slots.trigger?.(), slots.menu?.({ hide: vi.fn() })]),
        }),
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: fixture.handleError }),
        useFormatBytes: () => (value: number) => String(value),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

it('uses indeterminate progress without stale summary bytes and restores measured progress', async () => {
    const jobs = ref([
        {
            job_id: 'progress',
            status: 'running',
            phase: 'resolving_loader',
            progress: null,
            summary: {
                bytes_downloaded: 80,
                bytes_total: 100,
                files_completed: 8,
                files_total: 10,
            },
            items: [],
        } as unknown as InstallJobSnapshot,
    ])
    fixture.manager = {
        initializationState: ref('ready'),
        activeJobs: jobs,
        legacyDownloads: ref([]),
    }
    const wrapper = mount(DownloadCenterButton, { global: { directives: { tooltip: () => {} } } })
    const bar = () => wrapper.get('[role="progressbar"]')
    const trigger = () => wrapper.get('[aria-label="Downloads"]')
    try {
        expect(bar().attributes('aria-valuenow')).toBeUndefined()
        expect(bar().find('.progress-bar--waiting').exists()).toBe(true)
        expect(trigger().element.style.backgroundImage).toBe('')
        expect(wrapper.text()).not.toContain('8 / 10')

        jobs.value[0] = {
            ...jobs.value[0],
            phase: 'downloading_pack_file',
            progress: { current: 25, total: 100 },
        }
        await flushPromises()
        expect(bar().attributes('aria-valuenow')).toBe('25')
        expect(bar().find('.progress-bar--waiting').exists()).toBe(false)
        expect(trigger().element.style.backgroundImage).toContain('25%')
        expect(wrapper.text()).toContain('25 / 100')

        for (const phase of ['extracting_overrides', 'finalizing'] as const) {
            jobs.value[0] = { ...jobs.value[0], phase, progress: null }
            await flushPromises()
            expect(bar().attributes('aria-valuenow')).toBeUndefined()
            expect(bar().find('.progress-bar--waiting').exists()).toBe(true)
            expect(trigger().element.style.backgroundImage).toBe('')
            expect(wrapper.text()).not.toContain('80 / 100')
        }

        jobs.value[0] = { ...jobs.value[0], progress: { current: 2, total: 5 } }
        await flushPromises()
        expect(bar().attributes('aria-valuenow')).toBe('40')
        expect(wrapper.text()).toContain('2 / 5')

        jobs.value[0] = {
            ...jobs.value[0],
            status: 'waiting_for_user',
            phase: 'downloading_content',
            progress: { current: 8, total: 10 },
        }
        await flushPromises()
        expect(wrapper.text()).toContain('8 / 10')
    } finally {
        wrapper.unmount()
    }
})

it('cancel failures notify the user and release the task-specific busy state', async () => {
    let rejectCancel!: (reason: Error) => void
    const cancel = vi.fn(
        () =>
            new Promise((_, reject) => {
                rejectCancel = reject
            }),
    )
    fixture.handleError.mockReset()
    fixture.manager = {
        initializationState: ref('ready'),
        activeJobs: ref([
            {
                job_id: 'job',
                phase: 'downloading_content',
                progress: { current: 0, total: 1 },
                summary: { bytes_downloaded: 0, files_completed: 0, files_total: 1 },
            },
        ]),
        legacyDownloads: ref([]),
        cancel,
    }
    const wrapper = mount(DownloadCenterButton, { global: { directives: { tooltip: () => {} } } })
    try {
        const button = () => wrapper.get('[aria-label="Cancel download"]')
        await button().trigger('click')
        expect(button().attributes('disabled')).toBeDefined()
        await button().trigger('click')
        expect(cancel).toHaveBeenCalledTimes(1)
        const error = new Error('cancel failed')
        rejectCancel(error)
        await flushPromises()
        expect(fixture.handleError).toHaveBeenCalledWith(error)
        expect(button().attributes('disabled')).toBeUndefined()
    } finally {
        wrapper.unmount()
    }
})

it('keeps initial download loading local and offers retry after startup failure', async () => {
    const initializationState = ref('loading')
    const start = vi.fn().mockResolvedValue(undefined)
    fixture.manager = { initializationState, activeJobs: ref([]), legacyDownloads: ref([]), start }
    const wrapper = mount(DownloadCenterButton, { global: { directives: { tooltip: () => {} } } })
    try {
        expect(wrapper.text()).not.toContain('No active downloads')
        initializationState.value = 'error'
        await flushPromises()
        const retry = wrapper.findAll('button').find((button) => button.text() === 'Retry')!
        await retry.trigger('click')
        expect(start).toHaveBeenCalledOnce()
        initializationState.value = 'ready'
        await flushPromises()
        expect(wrapper.text()).toContain('No active downloads')
    } finally {
        wrapper.unmount()
    }
})
