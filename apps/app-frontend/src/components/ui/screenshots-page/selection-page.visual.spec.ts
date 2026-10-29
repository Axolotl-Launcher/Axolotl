import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { afterEach, expect, it, vi } from 'vitest'

import { mountThemed, waitFor } from '@/test/visual-harness'

import ScreenshotsPage from './index.vue'

const fixture = vi.hoisted(() => ({
    screenshots: [] as unknown[],
    failScreenshots: false,
    failGroups: false,
    screenshotCalls: 0,
    groupCalls: 0,
    deleted: vi.fn(),
    moved: vi.fn(),
    preview: vi.fn(),
    skipped: [] as string[],
    notify: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({
    create_screenshot_group: vi.fn(),
    delete_screenshot_group: vi.fn(),
    delete_screenshots: fixture.deleted,
    export_screenshots: vi.fn(),
    getInstanceIconUrl: () => null,
    import_screenshot_groups: vi.fn(),
    list_all_screenshots: async () => fixture.screenshots,
    move_screenshots: fixture.moved,
    open_screenshot: vi.fn(),
    rename_screenshot_group: vi.fn(),
    save_edited_screenshot: vi.fn(),
    set_screenshot_group_memberships: vi.fn(),
}))
vi.mock('@/helpers/instance-groups', () => ({ MAX_INSTANCE_GROUP_NAME_LENGTH: 100 }))
vi.mock('@/composables/use-app-event', () => ({ useAppEvent: () => {} }))
vi.mock('@/composables/use-image-thumbnail', async () => {
    const { ref } = await import('vue')
    return { useImageThumbnail: () => ref(null) }
})
vi.mock('@/pages/instance/query-options', () => ({
    instanceListQueryOptions: () => ({ queryKey: ['instances'], queryFn: async () => [] }),
    instanceScreenshotsQueryOptions: () => ({
        queryKey: ['screenshots'],
        queryFn: async () => ({ screenshots: fixture.screenshots, skipped_instances: [] }),
    }),
    syncedScreenshotsQueryOptions: () => ({
        queryKey: ['screenshots'],
        queryFn: async () => {
            fixture.screenshotCalls++
            if (fixture.failScreenshots) throw new Error('Screenshot scan failed')
            return { screenshots: fixture.screenshots, skipped_instances: fixture.skipped }
        },
    }),
    screenshotGroupsQueryOptions: () => ({
        queryKey: ['groups'],
        queryFn: async () => {
            fixture.groupCalls++
            if (fixture.failGroups) throw new Error('Group scan failed')
            return []
        },
    }),
    screenshotKeys: {
        groups: () => ['groups'],
        global: () => ['screenshots'],
        synced: () => ['screenshots'],
        instance: () => ['screenshots'],
    },
}))
vi.mock('vue-router', () => ({
    useRoute: () => ({ query: {} }),
    useRouter: () => ({ push: vi.fn(), replace: vi.fn() }),
}))
vi.mock('@/components/ui/ContextMenu.vue', async () => {
    const { defineComponent } = await import('vue')
    return {
        default: defineComponent({
            setup: (_, { expose }) => {
                expose({ close: () => {} })
                return () => null
            },
        }),
    }
})
vi.mock('./toolbar.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: ['search'],
            setup:
                (props, { emit }) =>
                () =>
                    h('input', {
                        'aria-label': 'Search screenshots',
                        value: props.search,
                        onInput: (event: Event) =>
                            emit('update:search', (event.target as HTMLInputElement).value),
                    }),
        }),
    }
})
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, computed, ref } = await import('vue')
    const { default: Accordion } =
        await import('../../../../../../packages/ui/src/components/base/Accordion.vue')
    const { default: InlineEditableText } =
        await import('../../../../../../packages/ui/src/components/base/InlineEditableText.vue')
    const { default: IconButton } =
        await import('../../../../../../packages/ui/src/components/base/buttons/IconButton.vue')
    const { useScrollViewport } =
        await import('../../../../../../packages/ui/src/composables/virtual-scroll')
    const { default: IntlMessageFormat } = await import('intl-messageformat')
    const container = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
    return {
        Accordion,
        InlineEditableText,
        IconButton,
        useScrollViewport,
        Avatar: container,
        Admonition: defineComponent({
            props: ['header'],
            setup:
                (props, { slots }) =>
                () =>
                    h('aside', { role: 'alert' }, [props.header, slots.default?.()]),
        }),
        EmptyState: defineComponent({
            props: ['heading', 'description'],
            setup:
                (props, { slots }) =>
                () =>
                    h('section', { 'data-empty-state': '' }, [
                        props.heading,
                        props.description,
                        slots.default?.(),
                        slots.actions?.(),
                    ]),
        }),
        ReadyTransition: container,
        TagItem: container,
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        FloatingActionBar: defineComponent({
            props: ['shown'],
            setup:
                (props, { slots }) =>
                () =>
                    props.shown
                        ? h('aside', { 'data-selection-bar': '' }, slots.default?.())
                        : null,
        }),
        ConfirmModal: defineComponent({
            props: ['title'],
            setup: (props, { emit, expose }) => {
                const shown = ref(false)
                expose({
                    show: () => {
                        shown.value = true
                    },
                })
                return () =>
                    shown.value
                        ? h(
                              'button',
                              { 'data-confirm': '', onClick: () => emit('proceed') },
                              props.title,
                          )
                        : null
            },
        }),
        ImageViewerEditor: defineComponent({
            setup: (_, { expose }) => {
                expose({ show: fixture.preview })
                return () => null
            },
        }),
        useReadyState: (query: { isPending: { value: boolean } }) =>
            computed(() => query.isPending.value),
        defineMessages: (messages: unknown) => messages,
        commonMessages: {
            retryButton: { defaultMessage: 'Retry' },
            clearButton: { defaultMessage: 'Clear' },
            deleteLabel: { defaultMessage: 'Delete' },
            actionsLabel: { defaultMessage: 'Actions' },
        },
        useVIntl: () => ({
            formatMessage: (
                message: { defaultMessage: string },
                values?: Record<string, string | number>,
            ) => new IntlMessageFormat(message.defaultMessage, 'en').format(values),
        }),
        useDebugLogger: () => () => {},
        useFormatDateTime: () => (value: string) => value,
        injectNotificationManager: () => ({
            handleError: vi.fn(),
            addNotification: fixture.notify,
        }),
    }
})

const cleanup: (() => void)[] = []
afterEach(() => {
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn())
    localStorage.clear()
    fixture.skipped = []
    fixture.failScreenshots = false
    fixture.failGroups = false
    fixture.screenshotCalls = 0
    fixture.groupCalls = 0
    fixture.notify.mockReset()
})

it('mounts only visible screenshots plus two below the viewport in a large collection', async () => {
    const createdAt = new Date().toISOString()
    fixture.screenshots = Array.from({ length: 1000 }, (_, index) => ({
        id: `large-${index}`,
        instance_id: 'instance',
        instance_name: 'Test instance',
        file_name: `large-${String(index).padStart(4, '0')}.png`,
        created_at: createdAt,
        modified_at: 1,
        group_id: null,
        path: '',
        url: '',
    }))
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    cleanup.push(() => client.clear())
    const host = document.createElement('div')
    host.style.cssText = 'width:600px;height:600px;overflow:auto'
    document.body.append(host)
    cleanup.push(() => host.remove())
    const wrapper = await mountThemed(ScreenshotsPage, {}, 'dark', {
        attachTo: host,
        global: {
            plugins: [[VueQueryPlugin, { queryClient: client }]],
            directives: { tooltip: {} },
        },
    })
    cleanup.push(() => wrapper.unmount())
    const cards = () => wrapper.findAll('[data-screenshot-card]')
    const belowViewport = () =>
        cards().filter(
            (card) =>
                card.element.getBoundingClientRect().top >= host.getBoundingClientRect().bottom,
        )
    await waitFor(() => cards().length > 2 && belowViewport().length === 2)
    expect(cards().length).toBeLessThan(30)
    const firstId = cards()[0].attributes('data-screenshot-id')
    host.scrollTop = 30000
    host.dispatchEvent(new Event('scroll'))
    await waitFor(
        () => cards().length > 2 && cards()[0].attributes('data-screenshot-id') !== firstId,
    )
    expect(cards().length).toBeLessThan(30)
    expect(belowViewport()).toHaveLength(2)
})

it('renders healthy screenshots and reports one warning after a partial scan completes', async () => {
    fixture.skipped = ['missing', 'denied']
    fixture.screenshots = [
        {
            id: 'healthy',
            instance_id: 'available',
            instance_name: 'Available',
            file_name: 'healthy.png',
            created_at: new Date().toISOString(),
            modified_at: 1,
            group_id: null,
            path: '',
            url: '',
        },
    ]
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    cleanup.push(() => client.clear())
    const wrapper = await mountThemed(ScreenshotsPage, {}, 'dark', {
        global: {
            plugins: [[VueQueryPlugin, { queryClient: client }]],
            directives: { tooltip: {} },
        },
    })
    cleanup.push(() => wrapper.unmount())
    await waitFor(() => wrapper.findAll('[data-screenshot-card]').length === 1)
    await waitFor(() => fixture.notify.mock.calls.length === 1)
    expect(fixture.notify).toHaveBeenCalledExactlyOnceWith({
        type: 'warning',
        title: 'Skipped 2 inaccessible instance folders',
    })
    expect(wrapper.text()).not.toContain('Failed to load screenshots')
    await wrapper.get('input[aria-label="Search screenshots"]').setValue('healthy')
    expect(fixture.notify).toHaveBeenCalledOnce()
})

it('uses page range and group selection and limits deletion to the current filtered selection', async () => {
    localStorage.clear()
    const now = new Date()
    const yesterday = new Date(now)
    yesterday.setDate(now.getDate() - 1)
    fixture.screenshots = Array.from({ length: 5 }, (_, index) => ({
        id: `shot-${index}`,
        instance_id: 'instance',
        instance_name: 'Test instance',
        file_name: `shot-${index}.png`,
        created_at: (index < 2 ? now : yesterday).toISOString(),
        modified_at: 1,
        group_id: null,
        path: '',
        url: '',
    }))
    fixture.deleted.mockReset().mockResolvedValue(undefined)
    fixture.moved.mockReset()
    fixture.preview.mockReset()
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    cleanup.push(() => client.clear())
    const host = document.createElement('div')
    host.style.cssText = 'width:360px;height:600px;overflow:auto'
    document.body.append(host)
    cleanup.push(() => host.remove())
    const wrapper = await mountThemed(ScreenshotsPage, {}, 'dark', {
        attachTo: host,
        global: {
            plugins: [[VueQueryPlugin, { queryClient: client }]],
            directives: { tooltip: {} },
        },
    })
    cleanup.push(() => wrapper.unmount())
    await waitFor(() => wrapper.findAll('[data-screenshot-card]').length === 5)
    await wrapper.get('[data-screenshot-id="shot-0"]').trigger('click')
    expect(fixture.preview).toHaveBeenCalledTimes(1)
    await wrapper.get('[data-screenshot-id="shot-2"]').trigger('click', { shiftKey: true })
    expect(wrapper.get('[data-selection-bar]').text()).toContain('3 selected')
    const groups = wrapper.findAll('input[type="checkbox"]')
    expect(groups).toHaveLength(2)
    await groups[1].setValue(true)
    expect(wrapper.get('[data-selection-bar]').text()).toContain('5 selected')
    await groups[0].setValue(false)
    expect(wrapper.get('[data-selection-bar]').text()).toContain('3 selected')
    await wrapper.get('input[aria-label="Search screenshots"]').setValue('shot-4')
    expect(wrapper.get('[data-selection-bar]').text()).toContain('1 selected')
    const deleteButton = wrapper
        .findAll('[data-selection-bar] button')
        .find((button) => button.text() === 'Delete')!
    await deleteButton.trigger('click')
    await wrapper.get('[data-confirm]').trigger('click')
    await waitFor(() => fixture.deleted.mock.calls.length === 1)
    expect(fixture.deleted).toHaveBeenCalledWith([
        { instance_id: 'instance', file_name: 'shot-4.png' },
    ])
    expect(fixture.moved).not.toHaveBeenCalled()
    await waitFor(() => !wrapper.find('[data-selection-bar]').exists())
    await wrapper.get('input[aria-label="Search screenshots"]').setValue('')
    localStorage.setItem('screenshots-group-v2-global', 'instance')
    window.dispatchEvent(
        new StorageEvent('storage', {
            key: 'screenshots-group-v2-global',
            oldValue: 'date',
            newValue: 'instance',
            storageArea: localStorage,
        }),
    )
    await waitFor(
        () =>
            wrapper.findAll('input[type="checkbox"]').length === 1 &&
            wrapper.findAll('[data-screenshot-card]').length === 5,
    )
    await new Promise((resolve) => requestAnimationFrame(resolve))
    const card = wrapper.get('[data-screenshot-id="shot-0"]').element
    const box = card.getBoundingClientRect()
    card.dispatchEvent(
        new PointerEvent('pointerdown', {
            bubbles: true,
            pointerType: 'mouse',
            pointerId: 1,
            button: 0,
            ctrlKey: true,
            clientX: box.left + 4,
            clientY: box.top + 4,
        }),
    )
    document.dispatchEvent(
        new PointerEvent('pointermove', {
            bubbles: true,
            pointerType: 'mouse',
            pointerId: 1,
            ctrlKey: true,
            clientX: box.right - 4,
            clientY: box.bottom - 4,
        }),
    )
    document.dispatchEvent(
        new PointerEvent('pointerup', {
            bubbles: true,
            pointerType: 'mouse',
            pointerId: 1,
            ctrlKey: true,
            clientX: box.right - 4,
            clientY: box.bottom - 4,
        }),
    )
    card.dispatchEvent(new MouseEvent('click', { bubbles: true, ctrlKey: true }))
    await wrapper.vm.$nextTick()
    expect(wrapper.get('[data-selection-bar]').text()).toContain('1 selected')
    expect(fixture.preview).toHaveBeenCalledTimes(1)
    expect(fixture.moved).not.toHaveBeenCalled()
})

async function renderFailureCase() {
    fixture.screenshots = [
        {
            id: 'kept',
            instance_id: 'one',
            instance_name: 'One',
            file_name: 'kept.png',
            created_at: new Date().toISOString(),
            modified_at: 1,
            group_id: null,
            path: '',
            url: '',
        },
    ]
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    cleanup.push(() => client.clear())
    const wrapper = await mountThemed(ScreenshotsPage, {}, 'dark', {
        global: {
            plugins: [[VueQueryPlugin, { queryClient: client }]],
            directives: { tooltip: {} },
        },
    })
    cleanup.push(() => wrapper.unmount())
    return { client, wrapper }
}

it('retries the failed group query instead of scanning healthy screenshots again', async () => {
    localStorage.setItem('screenshots-group-v2-global', 'custom')
    fixture.failGroups = true
    const { wrapper } = await renderFailureCase()
    await waitFor(() => wrapper.text().includes('Group scan failed'))
    expect(wrapper.findAll('[data-screenshot-card]')).toHaveLength(0)
    const previousScans = fixture.screenshotCalls
    fixture.failGroups = false
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Retry')!
        .trigger('click')
    await waitFor(() => wrapper.findAll('[data-screenshot-card]').length === 1)
    expect(fixture.groupCalls).toBe(2)
    expect(fixture.screenshotCalls).toBe(previousScans)
})

it('keeps cached screenshots and selection after a background scan failure', async () => {
    const { client, wrapper } = await renderFailureCase()
    await waitFor(() => wrapper.findAll('[data-screenshot-card]').length === 1)
    await wrapper.get('[data-screenshot-id="kept"]').trigger('click', { ctrlKey: true })
    fixture.failScreenshots = true
    await client.refetchQueries({ queryKey: ['screenshots'] })
    await waitFor(() => wrapper.find('[role="alert"]').exists())
    expect(wrapper.findAll('[data-screenshot-card]')).toHaveLength(1)
    expect(wrapper.get('[data-selection-bar]').text()).toContain('1 selected')
    fixture.failScreenshots = false
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Retry')!
        .trigger('click')
    await waitFor(() => !wrapper.find('[role="alert"]').exists())
    expect(wrapper.findAll('[data-screenshot-card]')).toHaveLength(1)
})
