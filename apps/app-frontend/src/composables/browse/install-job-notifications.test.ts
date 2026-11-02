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
        ) =>
            new IntlMessageFormat(
                zhMessages[descriptor.id as keyof typeof zhMessages]?.message ??
                    descriptor.defaultMessage,
                'zh-CN',
            ).format(values),
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
