import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'
import type { Router } from 'vue-router'

import type { InstallJobSnapshot } from '@/helpers/install'
import zhMessages from '@/locales/zh-CN/index.json'
import type { DownloadManager } from '@/providers/download-manager'

import { useInstallJobNotifications } from './install-job-notifications'

vi.mock('@modrinth/ui', () => ({
    defineMessage: (message: unknown) => message,
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (
            descriptor: { id?: string; defaultMessage: string },
            values?: Record<string, string | number>,
        ) => {
            const entry = descriptor.id
                ? zhMessages[descriptor.id as keyof typeof zhMessages]
                : undefined
            const template = entry && 'message' in entry ? entry.message : descriptor.defaultMessage
            return new IntlMessageFormat(template, 'zh-CN').format(values)
        },
    }),
}))
vi.mock('@/helpers/install', () => ({
    installJobInstanceId: () => null,
    install_job_dismiss: vi.fn(),
    install_job_repair_cache_and_retry: vi.fn(),
    install_job_retry: vi.fn(),
    install_job_support_details: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({ get_many: vi.fn().mockResolvedValue([]) }))

it('keeps notification titles consistent and terminal notifications free of ongoing phase copy', async () => {
    const initial = {
        job_id: 'batch',
        kind: 'install_content',
        status: 'running',
        phase: 'finalizing',
        display: { title: 'Install resources' },
        resource_names: ['Sodium', 'Iris'],
        details: { type: 'empty' },
        created: '2026-11-01T00:00:00Z',
        progress: null,
        summary: { files_completed: 0, bytes_downloaded: 0 },
        items: [],
    } as unknown as InstallJobSnapshot
    const jobs = ref([initial])
    const handleError = vi.fn()
    const notifications = await useInstallJobNotifications({
        manager: {
            jobs,
            refresh: vi.fn().mockResolvedValue(undefined),
        } as unknown as DownloadManager,
        router: { push: vi.fn() } as unknown as Router,
        handleError,
        onChange: vi.fn(),
        onFinishedJobExpired: vi.fn(),
    })
    try {
        expect(notifications.progressItems.value[0].title).toBe('安装内容：Sodium 等 2 项')
        for (const status of ['succeeded', 'failed', 'interrupted', 'canceled'] as const) {
            jobs.value = [{ ...initial, status }]
            await nextTick()
            const item = notifications.progressItems.value[0]
            expect(item.title).toBe('安装内容：Sodium 等 2 项')
            expect(item.text).not.toContain('正在完成安装')
            expect(item.showProgress).toBe(false)
            if (status === 'succeeded') expect(item.text).toBe('已完成')
        }
        jobs.value = [{ ...initial, display: { title: 'Custom task' } }]
        await nextTick()
        expect(notifications.progressItems.value[0].title).toBe('Custom task')
        jobs.value = [
            {
                ...initial,
                kind: 'change_content',
                resource_names: undefined,
                display: { title: 'Change resources' },
                items: [{ name: 'my-renamed-Iris-1.7.2.jar' }] as InstallJobSnapshot['items'],
            },
        ]
        await nextTick()
        expect(notifications.progressItems.value[0].title).toBe(
            '变更内容：my-renamed-Iris-1.7.2.jar',
        )
        jobs.value = [
            {
                ...initial,
                status: 'failed',
                error: { code: 'cache_repair_required', message: 'Cache repair required' },
            },
        ]
        await nextTick()
        expect(notifications.progressItems.value[0].title).toBe(
            zhMessages['app.action-bar.install.cache-repair.title'].message,
        )
        expect(handleError).not.toHaveBeenCalled()
    } finally {
        notifications.dispose()
    }
})

function installJob(job_id: string, status: InstallJobSnapshot['status']) {
    return {
        job_id,
        kind: 'change_content',
        status,
        phase: 'finalizing',
        display: { title: job_id },
        details: { type: 'empty' },
        created: '2026-11-09T00:00:00Z',
        progress: null,
        summary: { files_completed: 0, bytes_downloaded: 0 },
        items: [],
    } as unknown as InstallJobSnapshot
}

async function trackJobs(initial: InstallJobSnapshot[]) {
    const jobs = ref(initial)
    const onFinishedJobExpired = vi.fn()
    const notifications = await useInstallJobNotifications({
        manager: {
            jobs,
            refresh: vi.fn().mockResolvedValue(undefined),
        } as unknown as DownloadManager,
        router: { push: vi.fn() } as unknown as Router,
        handleError: vi.fn(),
        onChange: vi.fn(),
        onFinishedJobExpired,
    })
    const shownIds = () => notifications.progressItems.value.map((item) => item.id)
    return { jobs, onFinishedJobExpired, notifications, shownIds }
}

it('removes finished jobs from the popup shortly after they end, but keeps failures', async () => {
    vi.useFakeTimers()
    const { jobs, onFinishedJobExpired, notifications, shownIds } = await trackJobs([
        installJob('done', 'running'),
        installJob('broken', 'running'),
    ])
    try {
        jobs.value = [installJob('done', 'succeeded'), installJob('broken', 'failed')]
        await nextTick()
        expect(shownIds().sort()).toEqual(['broken', 'done'])

        await vi.advanceTimersByTimeAsync(10_000)

        expect(shownIds()).toEqual(['broken'])
        expect(onFinishedJobExpired).toHaveBeenCalledOnce()
    } finally {
        notifications.dispose()
        vi.useRealTimers()
    }
})

it('keeps cleared jobs away when a later job starts', async () => {
    vi.useFakeTimers()
    const { jobs, notifications, shownIds } = await trackJobs([
        installJob('done', 'running'),
        installJob('broken', 'failed'),
    ])
    try {
        jobs.value = [installJob('done', 'succeeded'), installJob('broken', 'failed')]
        await nextTick()

        notifications.clearFinishedJobs()
        expect(shownIds()).toEqual([])

        jobs.value = [
            installJob('done', 'succeeded'),
            installJob('broken', 'failed'),
            installJob('next', 'running'),
        ]
        await nextTick()
        expect(shownIds()).toEqual(['next'])

        jobs.value = [installJob('broken', 'running'), installJob('next', 'running')]
        await nextTick()
        expect(shownIds().sort()).toEqual(['broken', 'next'])
    } finally {
        notifications.dispose()
        vi.useRealTimers()
    }
})

it('shows loader installation as a percentage while preserving byte downloads and file counts', async () => {
    vi.stubGlobal('window', { setTimeout, clearTimeout })
    const initial = {
        job_id: 'loader',
        kind: 'create_instance',
        status: 'running',
        phase: 'running_loader_processors',
        details: { type: 'empty' },
        created: '2026-11-06T00:00:00Z',
        progress: { current: 7, total: 10 },
        summary: { files_completed: 0, bytes_downloaded: 0 },
        items: [],
    } as unknown as InstallJobSnapshot
    const jobs = ref([initial])
    const notifications = await useInstallJobNotifications({
        manager: {
            jobs,
            refresh: vi.fn().mockResolvedValue(undefined),
        } as unknown as DownloadManager,
        router: { push: vi.fn() } as unknown as Router,
        handleError: vi.fn(),
        onChange: vi.fn(),
        onFinishedJobExpired: vi.fn(),
    })
    try {
        const loader = notifications.progressItems.value[0]
        expect(loader.progressType).toBe('percentage')
        expect(loader.progress).toBe(0.7)
        expect(loader.progressCurrent).toBe(7)
        expect(loader.progressTotal).toBe(10)

        for (const [phase, expectedType] of [
            ['downloading_minecraft', 'bytes'],
            ['downloading_pack_file', 'bytes'],
            ['downloading_content', 'count'],
        ] as const) {
            jobs.value = [{ ...initial, phase }]
            await nextTick()
            expect(notifications.progressItems.value[0].progressType).toBe(expectedType)
        }

        jobs.value = [{ ...initial, progress: { current: 0, total: 0 } }]
        await nextTick()
        expect(notifications.progressItems.value[0].waiting).toBe(true)
        expect(notifications.progressItems.value[0].progressType).toBeUndefined()
    } finally {
        notifications.dispose()
        vi.unstubAllGlobals()
    }
})
