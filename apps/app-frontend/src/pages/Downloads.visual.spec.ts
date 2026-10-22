import '@/test/visual-styles.scss'

import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { nextTick, reactive, ref } from 'vue'

import type { InstallJobSnapshot } from '@/helpers/install'
import { preserveMonotonicProgress } from '@/helpers/install-progress'

import Downloads from './Downloads.vue'

const fixture = vi.hoisted(() => ({
    manager: {} as unknown,
    instances: [] as { id: string; name: string }[],
    clipboard: vi.fn(),
    route: { query: {} as Record<string, unknown> },
}))

vi.mock('@tanstack/vue-query', async () => {
    const { ref } = await import('vue')
    return {
        useQuery: () => ({ data: ref(fixture.instances) }),
        useQueryClient: () => ({ invalidateQueries: vi.fn() }),
    }
})
vi.mock('@/pages/instance/query-options', () => ({
    instanceKeys: { list: () => ['instances', 'list'] },
    instanceListQueryOptions: () => ({}),
}))
vi.mock('@/composables/use-app-event', () => ({ useAppEvent: () => {} }))

// Keep the actual page and ProgressBar; isolate native APIs and unrelated controls.
vi.mock('@/providers/download-manager', () => ({
    injectDownloadManager: () => fixture.manager,
}))
vi.mock('@/providers/content-install', () => ({
    injectContentInstall: () => ({ showCurseForgeManualDownloads: vi.fn() }),
}))
vi.mock('@/helpers/install', () => ({ download_job_support_details: vi.fn() }))
vi.mock('@/helpers/curseforge', () => ({ listPendingCurseForgeManualDownloads: vi.fn() }))
vi.mock('@/components/ui/modal/MissingModpackContentModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('vue-router', () => ({
    useRoute: () => fixture.route,
    useRouter: () => ({ push: vi.fn() }),
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const { default: ProgressBar } =
        await import('../../../../packages/ui/src/components/base/ProgressBar.vue')
    const control = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
    return {
        ...Object.fromEntries(
            [
                'Admonition',
                'BulletDivider',
                'Card',
                'Combobox',
                'ConfirmModal',
                'EmptyState',
                'NavTabs',
                'StyledInput',
                'TagItem',
            ].map((name) => [name, control]),
        ),
        Badge: defineComponent({
            props: ['type'],
            setup: (props) => () => h('span', { 'data-status-label': '' }, props.type),
        }),
        ProgressBar,
        Button: (await import('../../../../packages/ui/src/components/base/buttons/Button.vue'))
            .default,
        Table: (await import('../../../../packages/ui/src/components/base/Table.vue')).default,
        NewModal: defineComponent({
            props: ['header'],
            setup: (props, { expose, slots }) => {
                const visible = ref(false)
                expose({
                    show: () => {
                        visible.value = true
                    },
                })
                return () =>
                    visible.value
                        ? h('section', { role: 'dialog' }, [props.header, slots.default?.()])
                        : null
            },
        }),
        commonMessages: { retryButton: { defaultMessage: 'Retry' } },
        defineMessages: (messages: unknown) => messages,
        defineMessage: (message: unknown) => message,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useFormatBytes: () => (value: number) => String(value),
        useVIntl: () => ({
            formatMessage: (
                message: { defaultMessage: string },
                values: Record<string, unknown> = {},
            ) =>
                message.defaultMessage.replace(/\{(\w+)\}/g, (_, key) =>
                    String(values[key] ?? `{${key}}`),
                ),
        }),
    }
})

function downloadingJob(provider: InstallJobSnapshot['provider']): InstallJobSnapshot {
    return {
        job_id: 'curseforge-progress-regression',
        instance_deleted: false,
        kind: 'create_modpack_instance',
        status: 'running',
        execution_mode: 'normal',
        provider,
        target: { type: 'new_instance' },
        phase: 'downloading_content',
        progress: { current: 10, total: 10, secondary: { current: 1000, total: 1000 } },
        details: { type: 'modpack', title: 'Progress regression pack' },
        created: '2026-09-26T06:34:06Z',
        modified: '2026-09-26T06:34:06Z',
        summary: {
            files_completed: 10,
            files_total: 10,
            bytes_downloaded: 1000,
            bytes_total: 1000,
            fallback_count: 0,
        },
        items: [],
    }
}

it('renders archive byte progress and aligns source and status with manual download actions', async () => {
    const pack: InstallJobSnapshot = {
        ...downloadingJob('modrinth'),
        phase: 'downloading_pack_file',
        progress: { current: 25, total: 100 },
        items: [
            {
                id: 'pack.mrpack',
                name: 'Pack archive.mrpack',
                source: 'official',
                status: 'downloading',
                bytes_downloaded: 25,
                bytes_total: 100,
                manual_url: 'https://example.test/manual',
            },
        ],
    }
    const jobs = ref([pack])
    fixture.manager = {
        initializationState: ref('ready'),
        jobs,
        activeJobs: jobs,
        historyJobs: ref([]),
        legacyDownloads: ref([]),
    }
    const wrapper = mount(Downloads, {
        attachTo: document.body,
        global: { directives: { tooltip: () => {} } },
    })
    try {
        const bar = wrapper.get('[role="progressbar"]')
        expect(bar.attributes('aria-valuenow')).toBe('25')
        expect(bar.attributes('aria-label')).toBe('25 / 100')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Details')!
            .trigger('click')
        const row = wrapper.get('tbody tr')
        const cells = row.findAll('td')
        const source = cells[0].get('span.text-xs')
        const status = cells[1].get('[data-status-label]')
        expect(source.text()).toBe('Official')
        expect(status.text()).toBe('Downloading')
        const sourceRect = source.element.getBoundingClientRect()
        const statusRect = status.element.getBoundingClientRect()
        expect(
            Math.abs(sourceRect.y + sourceRect.height / 2 - statusRect.y - statusRect.height / 2),
        ).toBeLessThan(1)
        expect(row.element.getBoundingClientRect().height).toBeLessThanOrEqual(56)
        expect(cells[0].text()).toContain('Open')
        expect(cells[1].text()).not.toContain('Official')
    } finally {
        wrapper.unmount()
    }
})

it('returning from history without a valid tab query displays active downloads', async () => {
    fixture.route = reactive({ query: { tab: 'history' } as Record<string, unknown> })
    const jobs = ref([downloadingJob('modrinth')])
    fixture.manager = {
        initializationState: ref('ready'),
        jobs,
        activeJobs: jobs,
        historyJobs: ref([]),
        legacyDownloads: ref([]),
    }
    const wrapper = mount(Downloads, { global: { directives: { tooltip: () => {} } } })
    try {
        const tabs = () => wrapper.get('[data-onboarding-id="downloads-tabs"] > div')
        expect(tabs().attributes('active-index')).toBe('1')
        for (const value of [undefined, 'invalid', 'active']) {
            fixture.route.query.tab = value
            await nextTick()
            expect(tabs().attributes('active-index')).toBe('0')
        }
    } finally {
        wrapper.unmount()
        fixture.route = { query: {} }
    }
})

it.each(['downloading_content', 'downloading_pack_file', 'downloading_minecraft'] as const)(
    '%s keeps byte progress visible through individual file activity',
    async (phase) => {
        const jobs = ref<InstallJobSnapshot[]>([
            {
                ...downloadingJob('local'),
                phase,
                progress:
                    phase === 'downloading_content'
                        ? { current: 5, total: 10, secondary: { current: 50, total: 100 } }
                        : { current: 50, total: 100 },
                items: [
                    {
                        id: 'working-file',
                        name: 'working.jar',
                        status: 'verifying',
                        bytes_downloaded: 10,
                    },
                    {
                        id: 'downloading-file',
                        name: 'downloading.jar',
                        status: 'downloading',
                        bytes_downloaded: 40,
                    },
                ],
            },
        ])
        fixture.manager = {
            initializationState: ref('ready'),
            jobs,
            activeJobs: jobs,
            historyJobs: ref([]),
            legacyDownloads: ref([]),
        }
        const wrapper = mount(Downloads, { global: { directives: { tooltip: () => {} } } })
        try {
            for (const status of [
                'verifying',
                'writing',
                'connecting',
                'waiting_for_resource',
                'waiting_for_database',
                'finalizing',
            ] as const) {
                jobs.value[0].items[0].status = status
                await nextTick()
                const bar = wrapper.get('[role="progressbar"]')
                expect(bar.attributes('aria-label')).toBe('50 / 100')
                expect(bar.attributes('aria-valuenow')).toBe('50')
            }
            jobs.value[0] = {
                ...jobs.value[0],
                phase: 'extracting_overrides',
                progress: null,
            }
            await nextTick()
            const bar = wrapper.get('[role="progressbar"]')
            expect(bar.attributes('aria-label')).toBe('Extracting overrides')
            expect(bar.attributes('aria-valuenow')).toBeUndefined()
        } finally {
            wrapper.unmount()
        }
    },
)

it.each(['curse_forge', 'local'] as const)(
    '%s extraction clears the previous download percentage and accepts new phase progress',
    async (provider) => {
        const jobs = ref([downloadingJob(provider)])
        fixture.manager = {
            initializationState: ref('ready'),
            jobs,
            activeJobs: jobs,
            historyJobs: ref([]),
            legacyDownloads: ref([]),
        }
        const wrapper = mount(Downloads, { global: { directives: { tooltip: () => {} } } })
        const bar = () => wrapper.get('[role=progressbar]')
        try {
            expect(bar().attributes('aria-valuenow')).toBe('100')
            jobs.value = [
                preserveMonotonicProgress(jobs.value[0], {
                    ...jobs.value[0],
                    phase: 'extracting_overrides',
                    progress: null,
                }),
            ]
            await nextTick()

            // CurseForge emits no counters here; download summary bytes remain populated.
            expect(bar().attributes('aria-label')).toBe('Extracting overrides')
            expect(bar().attributes('aria-valuenow')).toBeUndefined()
            expect(wrapper.text()).not.toMatch(/[0-9]+%/)
            expect(bar().find('.progress-bar--waiting').exists()).toBe(true)

            // A measured phase uses its own counters, even below the old download value.
            jobs.value = [
                preserveMonotonicProgress(jobs.value[0], {
                    ...jobs.value[0],
                    progress: { current: 1, total: 100 },
                }),
            ]
            await nextTick()
            expect(bar().attributes('aria-valuenow')).toBe('1')
            expect(wrapper.text()).toContain('1%')
            expect(bar().find('.progress-bar--waiting').exists()).toBe(false)
        } finally {
            wrapper.unmount()
        }
    },
)

it('shows the target name in the task summary and keeps long errors and raw identifiers in file details', async () => {
    fixture.instances = [{ id: 'internal-instance-id', name: 'My Minecraft world' }]
    const error =
        'Connection failed: ' +
        'very-long-filename-'.repeat(25) +
        ' permission denied at the destination'
    const item = {
        id: 'file-one',
        name: 'example.jar',
        manual_url: 'https://example.test/manual',
        status: 'failed',
        attempts: 2,
        error,
        source: 'official',
        request_url: 'https://files.example.test/' + 'path/'.repeat(30) + 'example.jar',
        project_id: 'internal-project',
        version_id: 'internal-version',
        bytes_downloaded: 1,
        bytes_total: 10,
    }
    const job = {
        ...downloadingJob('modrinth'),
        instance_id: 'internal-instance-id',
        items: [item],
    } as InstallJobSnapshot
    const jobs = ref([job])
    fixture.manager = {
        initializationState: ref('ready'),
        jobs,
        activeJobs: jobs,
        historyJobs: ref([]),
        legacyDownloads: ref([]),
    }
    Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: { writeText: fixture.clipboard.mockResolvedValue(undefined) },
    })
    const wrapper = mount(Downloads, {
        attachTo: document.body,
        global: { directives: { tooltip: () => {} } },
    })
    try {
        const card = wrapper.get('[data-install-job-id]')
        expect(card.text()).toContain('My Minecraft world')
        expect(card.text()).not.toContain('internal-instance-id')
        expect(card.text()).not.toContain('https://files.example.test')
        await card
            .findAll('button')
            .find((button) => button.text() === 'Details')!
            .trigger('click')
        expect(card.text()).toContain('internal-instance-id')
        expect(card.find('table').text()).not.toContain(error)
        expect(card.get('tbody tr').element.getBoundingClientRect().height).toBeLessThanOrEqual(56)
        await card.get('[aria-label="View details for example.jar"]').trigger('click')
        const dialog = wrapper.get('[role="dialog"]')
        expect(dialog.text()).toContain(error)
        expect(dialog.text()).toContain(item.request_url)
        expect(dialog.text()).toContain('internal-project')
        expect(dialog.text()).toContain('internal-version')
        expect(dialog.get('p').classes()).not.toContain('truncate')
        await dialog
            .findAll('button')
            .find((button) => button.text() === 'Copy file details')!
            .trigger('click')
        expect(fixture.clipboard).toHaveBeenCalledWith(expect.stringContaining(error))
        jobs.value = [{ ...job, items: [{ ...item, error: 'Updated error' }] }]
        await nextTick()
        expect(dialog.text()).toContain('Updated error')
    } finally {
        wrapper.unmount()
        fixture.instances = []
    }
})
