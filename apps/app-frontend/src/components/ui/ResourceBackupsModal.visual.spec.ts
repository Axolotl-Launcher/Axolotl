import { flushPromises, mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import {
    type ResourceBackupFile,
    resourceExecuteHistoryCleanup,
    resourcePrepareHistoryCleanup,
    resourceScanBackups,
} from '@/helpers/resources'

import ResourceBackupsModal from './ResourceBackupsModal.vue'

vi.mock('@/helpers/resources', () => ({
    resourceScanBackups: vi.fn(),
    resourcePrepareHistoryCleanup: vi.fn(),
    resourceExecuteHistoryCleanup: vi.fn(),
}))
vi.mock('@modrinth/ui', () => ({
    Admonition: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    }),
    StyledInput: defineComponent({ setup: () => () => h('input') }),
    Combobox: defineComponent({ props: ['options', 'modelValue'], setup: () => () => h('select') }),
    useFormatBytes: () => (size: number) => `${size} bytes`,
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    Checkbox: defineComponent({
        props: ['modelValue', 'disabled', 'description'],
        emits: ['update:modelValue'],
        setup:
            (props, { emit }) =>
            () =>
                h('button', {
                    disabled: props.disabled,
                    role: 'checkbox',
                    'aria-label': props.description,
                    'aria-checked': props.modelValue,
                    onClick: () => emit('update:modelValue', !props.modelValue),
                }),
    }),
    NewModal: defineComponent({
        props: ['onHide', 'disableClose'],
        setup(props, { slots, expose }) {
            const visible = ref(false)
            expose({
                show: () => {
                    visible.value = true
                },
                hide: () => {
                    if (!props.disableClose) {
                        visible.value = false
                        props.onHide?.()
                    }
                },
            })
            return () =>
                visible.value ? h('section', [slots.default?.(), slots.actions?.()]) : null
        },
    }),
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }, values?: Record<string, number>) =>
            new IntlMessageFormat(message.defaultMessage, 'en').format(values),
    }),
}))

const file = (id: string, changes: Partial<ResourceBackupFile> = {}): ResourceBackupFile => ({
    id,
    historyId: id,
    relativePath: `mods/${id}.old`,
    integrity: { size: 123, hashes: [] },
    rollbackAvailable: true,
    recognized: true,
    exists: true,
    ...changes,
})
const mounted: ReturnType<typeof mount>[] = []
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.resetAllMocks()
})
async function open(files: ResourceBackupFile[]) {
    vi.mocked(resourceScanBackups).mockResolvedValue(files)
    const wrapper = mount(ResourceBackupsModal)
    mounted.push(wrapper)
    wrapper.vm.show('instance')
    await flushPromises()
    return wrapper
}
function button(wrapper: ReturnType<typeof mount>, text: string) {
    return wrapper.findAll('button').find((value) => value.text() === text)!
}

it('scans all backups and reviews the selected immutable plan before deleting', async () => {
    const valid = file('valid')
    const orphan = file('orphan', { historyId: null, rollbackAvailable: false })
    const wrapper = await open([
        valid,
        orphan,
        file('unknown', { recognized: false }),
        file('missing', { exists: false }),
    ])
    expect(
        wrapper.get('button[aria-label="mods/unknown.old"]').attributes('disabled'),
    ).toBeDefined()
    expect(
        wrapper.get('button[aria-label="mods/missing.old"]').attributes('disabled'),
    ).toBeDefined()
    await wrapper.get('button[aria-label="mods/valid.old"]').trigger('click')
    vi.mocked(resourcePrepareHistoryCleanup).mockResolvedValue({
        id: 'plan',
        revision: 3,
        instanceId: 'instance',
        files: [valid],
    })
    await button(wrapper, 'Review selected cleanup').trigger('click')
    await flushPromises()
    expect(resourcePrepareHistoryCleanup).toHaveBeenCalledWith('instance', ['valid'])
    expect(wrapper.text()).toContain('permanently removes 1 available rollback backups')
    expect(resourceExecuteHistoryCleanup).not.toHaveBeenCalled()
    vi.mocked(resourceExecuteHistoryCleanup).mockResolvedValue({ removed: ['valid'], failures: [] })
    await button(wrapper, 'Delete these backups').trigger('click')
    await flushPromises()
    expect(resourceExecuteHistoryCleanup).toHaveBeenCalledWith('plan', 3)
    expect(wrapper.text()).toContain('1 files deleted.')
    expect(wrapper.emitted('cleaned')).toHaveLength(1)
    expect(wrapper.get('button[aria-label="mods/valid.old"]').attributes('disabled')).toBeDefined()
})

it('closing or returning from a cleanup review does not delete files', async () => {
    const valid = file('valid')
    const wrapper = await open([valid])
    await wrapper.get('button[aria-label="mods/valid.old"]').trigger('click')
    vi.mocked(resourcePrepareHistoryCleanup).mockResolvedValue({
        id: 'plan',
        revision: 1,
        instanceId: 'instance',
        files: [valid],
    })
    await button(wrapper, 'Review selected cleanup').trigger('click')
    await flushPromises()
    await button(wrapper, 'Back to selection').trigger('click')
    expect(wrapper.get('button[aria-label="mods/valid.old"]').attributes('aria-checked')).toBe(
        'true',
    )
    await button(wrapper, 'Close').trigger('click')
    expect(resourceExecuteHistoryCleanup).not.toHaveBeenCalled()
})

it('late scans cannot replace another instance and scan failures remain retryable', async () => {
    let resolve!: (files: ResourceBackupFile[]) => void
    vi.mocked(resourceScanBackups).mockReturnValueOnce(
        new Promise((value) => {
            resolve = value
        }),
    )
    const wrapper = mount(ResourceBackupsModal)
    mounted.push(wrapper)
    wrapper.vm.show('old')
    await flushPromises()
    await button(wrapper, 'Close').trigger('click')
    vi.mocked(resourceScanBackups).mockResolvedValueOnce([file('new')])
    wrapper.vm.show('new')
    await flushPromises()
    resolve([file('old')])
    await flushPromises()
    expect(wrapper.text()).toContain('mods/new.old')
    expect(wrapper.text()).not.toContain('mods/old.old')
    vi.mocked(resourceScanBackups).mockRejectedValueOnce(new Error('scan failed'))
    await button(wrapper, 'Scan again').trigger('click')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain(
        'Could not complete the content operation',
    )
    expect(wrapper.text()).toContain('mods/new.old')
})

it('a failed first scan does not claim the file collection or search is empty', async () => {
    vi.mocked(resourceScanBackups).mockRejectedValueOnce(new Error('offline'))
    const wrapper = mount(ResourceBackupsModal)
    mounted.push(wrapper)
    wrapper.vm.show('instance')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain(
        'Could not complete the content operation',
    )
    expect(wrapper.text()).not.toContain('No files match your search or filter.')
    expect(wrapper.text()).not.toContain('No old files or rollback backups found.')
})

it('partial cleanup marks staged failures unavailable and counts physical files once', async () => {
    const valid = file('valid')
    const shared = file('shared', { relativePath: valid.relativePath })
    const failure = file('failure')
    const wrapper = await open([valid, shared, failure])
    await wrapper.get('button[aria-label="mods/valid.old"]').trigger('click')
    await wrapper.get('button[aria-label="mods/failure.old"]').trigger('click')
    vi.mocked(resourcePrepareHistoryCleanup).mockResolvedValue({
        id: 'plan',
        revision: 1,
        instanceId: 'instance',
        files: [valid, failure],
    })
    await button(wrapper, 'Review selected cleanup').trigger('click')
    await flushPromises()
    vi.mocked(resourceExecuteHistoryCleanup).mockResolvedValue({
        removed: ['valid', 'shared'],
        failures: [['failure', 'locked file']],
    })
    await button(wrapper, 'Delete these backups').trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('1 files could not be deleted')
    expect(wrapper.text()).not.toContain('Verified rollback backup')
    expect(button(wrapper, 'Review selected cleanup').attributes('disabled')).toBeDefined()
})

it('duplicate history ids do not inflate the successful file count', async () => {
    const valid = file('valid')
    const wrapper = await open([valid])
    await wrapper.get('button[aria-label="mods/valid.old"]').trigger('click')
    vi.mocked(resourcePrepareHistoryCleanup).mockResolvedValue({
        id: 'plan',
        revision: 1,
        instanceId: 'instance',
        files: [valid],
    })
    await button(wrapper, 'Review selected cleanup').trigger('click')
    await flushPromises()
    vi.mocked(resourceExecuteHistoryCleanup).mockResolvedValue({
        removed: ['valid', 'same-backup-other-history'],
        failures: [],
    })
    await button(wrapper, 'Delete these backups').trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('1 files deleted.')
})
