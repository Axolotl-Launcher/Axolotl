import { config, flushPromises, mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import {
    resourceGet,
    type ResourceOperationPlan,
    type ResourceOperationProgress,
    resourcePrepareEntryOperation,
    resourcePrepareOperation,
    resourceQueuePlan,
    resourceRevisePlan,
} from '@/helpers/resources'
import { get_instance_worlds } from '@/helpers/worlds'

import ResourceOperationPreviewModal from './ResourceOperationPreviewModal.vue'

const closeTransition = vi.fn().mockResolvedValue(undefined)
config.global.directives.tooltip = {}

vi.mock('@/helpers/resources', () => ({
    resourceGet: vi.fn(),
    resourcePrepareEntryOperation: vi.fn(),
    resourcePrepareOperation: vi.fn(),
    resourceProjectDocuments: vi.fn().mockResolvedValue([]),
    resourceQueuePlan: vi.fn(),
    resourceRevisePlan: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({ refresh_content: vi.fn() }))
vi.mock('@/helpers/worlds', () => ({ get_instance_worlds: vi.fn().mockResolvedValue([]) }))
vi.mock('@modrinth/ui', () => ({
    Avatar: defineComponent({ setup: () => () => h('span') }),
    Combobox: defineComponent({
        props: ['modelValue', 'options', 'disabled'],
        emits: ['update:modelValue'],
        setup:
            (props, { emit }) =>
            () =>
                h(
                    'select',
                    {
                        value: props.modelValue,
                        disabled: props.disabled,
                        onChange: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLSelectElement).value),
                    },
                    [
                        h('option', { value: '' }, 'Choose world'),
                        ...props.options.map((option: { value: string; label: string }) =>
                            h('option', { value: option.value }, option.label),
                        ),
                    ],
                ),
    }),
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    NewModal: defineComponent({
        props: ['onHide', 'disableClose'],
        setup(props, { slots, expose }) {
            const visible = ref(false)
            expose({
                show: () => {
                    visible.value = true
                },
                hide: async () => {
                    if (props.disableClose) return
                    props.onHide?.()
                    await closeTransition()
                    visible.value = false
                },
            })
            return () =>
                visible.value ? h('section', [slots.default?.(), slots.actions?.()]) : null
        },
    }),
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (
            message: { defaultMessage: string },
            values?: Record<string, string | number>,
        ) => new IntlMessageFormat(message.defaultMessage, 'en').format(values),
    }),
}))

const reference = {
    release: { project: { source: 'modrinth', projectId: 'root0000' }, releaseId: 'version2' },
    artifactId: 'root.jar',
}
const node = (id: string, state: string, artifact: unknown = null) => ({
    id,
    resource: { kind: 'source', project: { source: 'modrinth', projectId: id } },
    scope: { kind: 'instance' },
    state,
    artifact,
    installedEntries: [],
    compatibility: { gameVersion: 'compatible', loader: 'compatible', reasons: [] },
})
const root = node('root', 'update', { reference, fileName: 'Root.jar' })
const optional = node('optional', 'optionalCandidate')

it('omits compatibility exceptions for local filename matches but keeps real unknown release warnings', async () => {
    ready()
    const local = node('local-file', 'satisfied')
    local.compatibility = { gameVersion: 'unknown', loader: 'unknown', reasons: [] }
    const remote = node('remote-file', 'install', { reference, fileName: 'Remote.jar' })
    remote.compatibility = { gameVersion: 'unknown', loader: 'unknown', reasons: [] }
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        ...plan(),
        nodes: [local, remote],
    } as unknown as ResourceOperationPlan)
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        const assessments = wrapper
            .findAll('p')
            .filter((paragraph) => paragraph.text() === 'Game version: Unknown; loader: Unknown')
        expect(assessments).toHaveLength(1)
        expect(
            wrapper
                .findAll('button')
                .filter((button) => button.text() === 'Accept Minecraft compatibility exception'),
        ).toHaveLength(1)
        expect(
            wrapper
                .findAll('button')
                .filter((button) => button.text() === 'Accept loader compatibility exception'),
        ).toHaveLength(1)
        expect(wrapper.text()).toContain('Already satisfied')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Cancel')!
            .trigger('click')
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
    }
})

function plan(revision = 1): ResourceOperationPlan {
    return {
        id: 'plan',
        revision,
        instanceId: 'instance',
        intent: 'update',
        selections: [],
        exceptions: [],
        activeRoots: ['root'],
        nodes: [root, optional],
        edges: [],
        actions: [
            {
                entryId: 'entry',
                nodeId: 'root',
                kind: 'replace',
                targetRelativePath: 'mods/custom.jar.disabled',
            },
        ],
        issues: [],
    } as unknown as ResourceOperationPlan
}
function ready() {
    vi.clearAllMocks()
    closeTransition.mockReset().mockResolvedValue(undefined)
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue(plan())
    vi.mocked(resourceGet).mockImplementation(
        async (reference) =>
            ({
                projects: [
                    {
                        project:
                            reference.kind === 'source'
                                ? reference.project
                                : { source: 'modrinth', projectId: reference.id },
                        title:
                            reference.kind === 'source'
                                ? reference.project.projectId
                                : reference.id,
                        iconUrl: null,
                    },
                ],
            }) as Awaited<ReturnType<typeof resourceGet>>,
    )
}

it('uses the cart analysis and closes its preview when that cart snapshot is invalidated', async () => {
    ready()
    const controller = new AbortController()
    const prepare = vi.fn().mockResolvedValue(plan())
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.showInstallSelection({
        instanceId: 'instance',
        instanceName: 'Instance',
        selections: [],
        installDependencies: true,
        analysis: { signal: controller.signal, prepare },
    })
    try {
        await flushPromises()
        expect(prepare).toHaveBeenCalledTimes(1)
        expect(resourcePrepareOperation).not.toHaveBeenCalled()
        controller.abort()
        await expect(result).resolves.toBeNull()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})

it('requires an explicit roots-only preview after dependency analysis fails', async () => {
    ready()
    const reviewed = {
        ...plan(),
        intent: 'install',
        issues: [
            {
                code: 'dependencyAnalysisFailed',
                selectionId: 'root',
                blocking: false,
                message: 'source offline',
            },
        ],
    } as ResourceOperationPlan
    vi.mocked(resourcePrepareOperation).mockResolvedValue(reviewed)
    vi.mocked(resourceRevisePlan).mockResolvedValue({
        ...reviewed,
        revision: 2,
        issues: [],
        nodes: [root],
    })
    vi.mocked(resourceQueuePlan).mockResolvedValue({ job_id: 'queued' } as never)
    const wrapper = mount(ResourceOperationPreviewModal, {
        global: {
            directives: {
                tooltip: {
                    mounted: (element, binding) => {
                        element.dataset.tooltip = binding.value ?? ''
                    },
                    updated: (element, binding) => {
                        element.dataset.tooltip = binding.value ?? ''
                    },
                },
            },
        },
    })
    const result = wrapper.vm.showInstall({
        instanceId: 'instance',
        instanceName: 'Instance',
        selections: [],
        installDependencies: true,
    })
    try {
        await flushPromises()
        const button = wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
        expect(button.attributes('disabled')).toBeDefined()
        expect(button.attributes('data-tooltip')).toBe(
            'Dependency analysis failed. The instance may not work correctly.',
        )
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Force install selected content only')!
            .trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).toHaveBeenCalledWith(
            reviewed.id,
            reviewed.revision,
            [{ kind: 'ignoreDependencies' }],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        expect(wrapper.text()).toContain('Dependencies will not be checked or installed')
        expect(button.attributes('disabled')).toBeUndefined()
        await button.trigger('click')
        await flushPromises()
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith(reviewed.id, 2)
        expect((await result)?.job_id).toBe('queued')
    } finally {
        wrapper.unmount()
    }
})

it('does not bypass blocking file validation when dependency analysis also failed', async () => {
    ready()
    vi.mocked(resourcePrepareOperation).mockResolvedValue({
        ...plan(),
        intent: 'install',
        issues: [
            {
                code: 'dependencyAnalysisFailed',
                selectionId: 'root',
                blocking: false,
                message: 'source offline',
            },
            {
                code: 'invalidPath',
                selectionId: 'root',
                blocking: true,
                message: 'invalid filename',
            },
        ],
    } as ResourceOperationPlan)
    const wrapper = mount(ResourceOperationPreviewModal, {
        global: { directives: { tooltip: {} } },
    })
    const result = wrapper.vm.showInstall({
        instanceId: 'instance',
        instanceName: 'Instance',
        selections: [],
        installDependencies: true,
    })
    try {
        await flushPromises()
        const button = wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
        expect(button.attributes('disabled')).toBeDefined()
        await button.trigger('click')
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        expect(
            wrapper.findAll('button').some((button) => button.text().startsWith('Force install')),
        ).toBe(false)
        await wrapper.vm.cancel()
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
    }
})

it('combines dependency fallback with skipped files and submits only the writable roots', async () => {
    ready()
    const writable = { ...root, id: 'writable', rootSelectionIds: ['writable'] }
    const action = {
        ...plan().actions[0],
        nodeId: 'writable',
        entryId: 'writable-entry',
        targetRelativePath: 'mods/writable.jar',
    }
    const reviewed = {
        ...plan(),
        intent: 'install',
        activeRoots: ['root', 'writable'],
        nodes: [root, writable],
        actions: [...plan().actions, action],
        issues: [
            {
                code: 'fileNotWritable',
                selectionId: 'root',
                blocking: true,
                message: 'mods/locked.jar',
            },
        ],
    } as ResourceOperationPlan
    vi.mocked(resourcePrepareOperation).mockResolvedValue({
        ...reviewed,
        issues: [
            ...reviewed.issues,
            {
                code: 'dependencyAnalysisFailed',
                selectionId: 'root',
                blocking: false,
                message: 'offline',
            },
        ],
    })
    vi.mocked(resourceRevisePlan).mockResolvedValueOnce(reviewed)
    vi.mocked(resourceRevisePlan).mockResolvedValue({
        ...reviewed,
        revision: 2,
        actions: [action],
        issues: [
            {
                code: 'fileWriteSkipped',
                selectionId: 'root',
                blocking: false,
                message: 'mods/locked.jar',
            },
        ],
    })
    vi.mocked(resourceQueuePlan).mockResolvedValue({ job_id: 'queued' } as never)
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.showInstallSelection({
        instanceId: 'instance',
        instanceName: 'Instance',
        selections: [],
        installDependencies: true,
    })
    try {
        await flushPromises()
        const queue = () =>
            wrapper.findAll('button').find((button) => button.text() === 'Queue changes')!
        expect(queue().attributes('disabled')).toBeDefined()
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Force install selected content only')!
            .trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain('Dependencies will not be checked or installed')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Force install writable files')!
            .trigger('click')
        await flushPromises()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        expect(resourceRevisePlan).toHaveBeenCalledWith(
            reviewed.id,
            1,
            [{ kind: 'skipUnwritableFiles' }],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        expect(wrapper.text()).toContain('mods/locked.jar will be skipped')
        expect(queue().attributes('disabled')).toBeUndefined()
        await queue().trigger('click')
        await flushPromises()
        expect((await result)?.activeRoots).toEqual(['writable'])
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith(reviewed.id, 2)
    } finally {
        wrapper.unmount()
    }
})

it('can prepare selected content without dependencies after analysis rejects the entire preview', async () => {
    ready()
    vi.mocked(resourcePrepareOperation)
        .mockRejectedValueOnce(new Error('dependency analysis failed'))
        .mockRejectedValueOnce(new Error('temporary failure'))
        .mockResolvedValueOnce({ ...plan(), intent: 'install' })
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.showInstall({
        instanceId: 'instance',
        instanceName: 'Instance',
        selections: [],
        installDependencies: true,
    })
    try {
        await flushPromises()
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Force install selected content only')!
            .trigger('click')
        await flushPromises()
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Retry')!
            .trigger('click')
        await flushPromises()
        expect(resourcePrepareOperation).toHaveBeenLastCalledWith(
            'instance',
            'install',
            [],
            true,
            expect.any(AbortSignal),
            undefined,
            undefined,
            expect.any(Function),
            false,
        )
        expect(wrapper.text()).toContain('Dependencies will not be checked or installed')
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        vi.mocked(resourceQueuePlan).mockRejectedValueOnce(
            new Error('instance changed after review'),
        )
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        await flushPromises()
        expect(resourcePrepareOperation).toHaveBeenLastCalledWith(
            'instance',
            'install',
            [],
            true,
            expect.any(AbortSignal),
            undefined,
            undefined,
            expect.any(Function),
            false,
        )
        expect(wrapper.text()).toContain('Dependencies will not be checked or installed')
        await wrapper.vm.cancel()
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
    }
})

it('keeps submission and reopening locked through the closing animation', async () => {
    ready()
    vi.mocked(resourceQueuePlan).mockResolvedValue({ job_id: 'queued' } as never)
    let finish!: () => void
    closeTransition.mockImplementationOnce(
        () =>
            new Promise<void>((resolve) => {
                finish = resolve
            }),
    )
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show({
        instanceId: 'instance',
        intent: 'update',
        entryIds: ['entry'],
        installDependencies: true,
    })
    try {
        await flushPromises()
        const button = wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
        await button.trigger('click')
        await flushPromises()
        expect(button.attributes('disabled')).toBeDefined()
        await button.trigger('click')
        expect(
            await wrapper.vm.show({
                instanceId: 'other',
                intent: 'update',
                entryIds: ['entry'],
                installDependencies: true,
            }),
        ).toBeNull()
        expect(resourceQueuePlan).toHaveBeenCalledOnce()
        finish()
        await flushPromises()
        expect((await result)?.job_id).toBe('queued')
        expect(wrapper.find('section').exists()).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('cancelling a context aborts parsing and ignores its late plan', async () => {
    ready()
    let complete!: (plan: ResourceOperationPlan) => void
    vi.mocked(resourcePrepareEntryOperation).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                complete = resolve
            }),
    )
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show({
        instanceId: 'instance',
        intent: 'update',
        entryIds: ['entry'],
        installDependencies: true,
    })
    try {
        await flushPromises()
        const signal = vi.mocked(resourcePrepareEntryOperation).mock.calls[0][5]!
        await wrapper.vm.cancel()
        expect(signal.aborted).toBe(true)
        complete(plan())
        await flushPromises()
        expect(await result).toBeNull()
        expect(wrapper.find('section').exists()).toBe(false)
        expect(resourceQueuePlan).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})
const input = {
    instanceId: 'instance',
    instanceName: 'Example',
    entryIds: ['entry'],
    intent: 'update' as const,
    installDependencies: true,
}

it('shows blocking compatibility and exception controls for byte-matched installed dependencies', async () => {
    ready()
    const dependency = {
        ...node('installed', 'satisfied', { reference, fileName: 'Installed.jar' }),
        compatibility: { gameVersion: 'incompatible', loader: 'compatible', reasons: [] },
    }
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        ...plan(),
        nodes: [root, dependency],
        issues: [
            {
                code: 'compatibilityMismatch',
                selectionId: 'installed',
                entryId: null,
                blocking: true,
                message: 'game version mismatch',
            },
        ],
    } as ResourceOperationPlan)
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        const accept = wrapper
            .findAll('button')
            .find((button) => button.text().includes('Minecraft'))!
        expect(accept.exists()).toBe(true)
        expect(
            wrapper
                .findAll('button')
                .find((button) => button.text() === 'Queue changes')!
                .attributes('disabled'),
        ).toBeDefined()
        await wrapper.vm.cancel()
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
    }
})

it('shows each parsed item while retaining the skeleton and locking every button until the final plan arrives', async () => {
    ready()
    let progress!: (value: ResourceOperationProgress) => void
    let complete!: (value: ResourceOperationPlan) => void
    vi.mocked(resourcePrepareEntryOperation).mockImplementationOnce((...args) => {
        progress = args[6]!
        return new Promise((resolve) => {
            complete = resolve
        })
    })
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    const emit = (value: ReturnType<typeof node>, title: string) =>
        progress({
            planId: 'plan',
            revision: 1,
            node: value,
            metadata: { title, iconUrl: null, summary: '' },
        } as ResourceOperationProgress)
    try {
        await flushPromises()
        expect(wrapper.find('[data-operation-skeleton]').exists()).toBe(true)
        emit(root, 'Parsed root')
        await flushPromises()
        expect(wrapper.text()).toContain('Parsed root')
        expect(wrapper.text()).not.toContain('Parsed dependency')
        const child = node('child', 'install', { reference, fileName: 'Child.jar' })
        emit(child, 'Parsed dependency')
        emit(child, 'Parsed dependency')
        await flushPromises()
        expect(wrapper.text().match(/Parsed dependency/g)).toHaveLength(1)
        expect(wrapper.text()).toContain('Parsed root')
        expect(wrapper.find('[data-operation-skeleton]').exists()).toBe(true)
        expect(
            wrapper
                .findAll('button')
                .every((button) => button.attributes('disabled') !== undefined),
        ).toBe(true)
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        complete({ ...plan(), nodes: [root, child] } as ResourceOperationPlan)
        await flushPromises()
        expect(wrapper.find('[data-operation-skeleton]').exists()).toBe(false)
        expect(
            wrapper
                .findAll('button')
                .every((button) => button.attributes('disabled') === undefined),
        ).toBe(true)
        emit(node('late', 'install'), 'Late item')
        await flushPromises()
        expect(wrapper.text()).not.toContain('Late item')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Cancel')!
            .trigger('click')
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
    }
})

it('discards partial revision items after failure and keeps the previously reviewed plan usable', async () => {
    ready()
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    let reject!: (error: Error) => void
    let progress!: (value: ResourceOperationProgress) => void
    try {
        await flushPromises()
        vi.mocked(resourceRevisePlan).mockImplementationOnce((...args) => {
            progress = args[4]!
            return new Promise((_, fail) => {
                reject = fail
            })
        })
        await wrapper
            .findAll('button')
            .find((button) => button.text() === '＋')!
            .trigger('click')
        progress({
            planId: 'plan',
            revision: 2,
            node: node('partial', 'install'),
            metadata: null,
        } as ResourceOperationProgress)
        await flushPromises()
        expect(wrapper.text()).toContain('partial')
        expect(wrapper.find('[data-operation-skeleton]').exists()).toBe(true)
        reject(new Error('source unavailable'))
        await flushPromises()
        expect(wrapper.text()).not.toContain('partial')
        expect(wrapper.find('[data-operation-skeleton]').exists()).toBe(false)
        expect(wrapper.text()).toContain('mods/custom.jar.disabled')
        vi.mocked(resourceQueuePlan).mockResolvedValueOnce({ job_id: 'original' } as Awaited<
            ReturnType<typeof resourceQueuePlan>
        >)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        await flushPromises()
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith('plan', 1)
        expect(await result).toEqual({ job_id: 'original' })
    } finally {
        wrapper.unmount()
    }
})

it('data packs require a real world and a failed world switch cannot queue the previous plan', async () => {
    ready()
    vi.mocked(get_instance_worlds)
        .mockRejectedValueOnce(new Error('world scan failed'))
        .mockResolvedValue([
            { type: 'singleplayer', path: 'one', name: 'One' },
            { type: 'singleplayer', path: 'two', name: 'Two' },
        ] as Awaited<ReturnType<typeof get_instance_worlds>>)
    vi.mocked(resourcePrepareOperation)
        .mockResolvedValueOnce(plan())
        .mockRejectedValueOnce(new Error('preparation failed'))
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.showInstall({
        instanceId: 'instance',
        instanceName: 'Example',
        installDependencies: true,
        selections: [
            {
                id: 'pack',
                resourceType: 'dataPack',
                scope: { kind: 'instance' },
                release: { kind: 'automatic' },
                resource: {
                    kind: 'source',
                    project: { source: 'modrinth', projectId: 'pack0000' },
                },
            },
        ],
    })
    try {
        await flushPromises()
        expect(resourcePrepareOperation).not.toHaveBeenCalled()
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Retry')!
            .trigger('click')
        await flushPromises()
        await wrapper.find('select').setValue('saves/one')
        await flushPromises()
        expect(vi.mocked(resourcePrepareOperation).mock.calls[0][2][0].scope).toEqual({
            kind: 'world',
            relativePath: 'saves/one',
        })
        await wrapper.find('select').setValue('saves/two')
        await flushPromises()
        expect(
            wrapper
                .findAll('button')
                .find((button) => button.text() === 'Queue changes')
                ?.attributes('disabled'),
        ).toBeDefined()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        vi.mocked(resourcePrepareOperation).mockResolvedValueOnce(plan())
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Force install selected content only')!
            .trigger('click')
        await flushPromises()
        const forcedWorld = vi.mocked(resourcePrepareOperation).mock.lastCall!
        expect(forcedWorld[2][0].scope).toEqual({ kind: 'world', relativePath: 'saves/two' })
        expect(forcedWorld[8]).toBe(false)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Cancel')!
            .trigger('click')
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
    }
})

it('requires explicit addition of a missing dependency when automatic installation is off', async () => {
    ready()
    const missing = node('required', 'missing', { reference, fileName: 'Required.jar' })
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        ...plan(),
        nodes: [root, missing],
        edges: [
            {
                parent: 'root',
                child: 'required',
                kind: 'required',
                declaredBy: reference,
                versionConstraint: null,
            },
        ],
        issues: [
            {
                code: 'missingDependency',
                selectionId: 'required',
                blocking: true,
                message: 'explicit addition required',
            },
        ],
    } as ResourceOperationPlan)
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show({ ...input, installDependencies: false })
    try {
        await flushPromises()
        expect(wrapper.text()).toContain('Automatic dependency installation is off')
        expect(
            wrapper
                .findAll('button')
                .find((button) => button.text() === 'Queue changes')!
                .attributes('disabled'),
        ).toBeDefined()
        vi.mocked(resourceRevisePlan).mockResolvedValue({
            ...plan(2),
            nodes: [root, { ...missing, state: 'selectedRoot' }],
            activeRoots: ['root', 'required'],
        })
        await wrapper
            .findAll('button')
            .find((button) => button.text() === '＋')!
            .trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).toHaveBeenCalledWith(
            'plan',
            1,
            [{ kind: 'addRequired', nodeId: 'required' }],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        expect(wrapper.text()).not.toContain('Automatic dependency installation is off')
        wrapper
            .findAll('button')
            .find((button) => button.text() === 'Cancel')!
            .trigger('click')
        await result
    } finally {
        wrapper.unmount()
    }
})

it('previews an exact path, retains the previous revision on optional failure, and queues only the successful reviewed revision', async () => {
    ready()
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        expect(wrapper.text()).toContain('mods/custom.jar.disabled')
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        vi.mocked(resourceRevisePlan).mockRejectedValueOnce({
            message: 'dependency source unavailable',
        })
        await wrapper
            .findAll('button')
            .find((button) => button.text() === '＋')!
            .trigger('click')
        await flushPromises()
        expect(wrapper.text()).toContain(
            'Could not complete the content operation. Refresh and try again.',
        )
        expect(wrapper.text()).toContain('mods/custom.jar.disabled')
        vi.mocked(resourceRevisePlan).mockResolvedValueOnce({
            ...plan(2),
            nodes: [root, node('optional', 'install', { reference, fileName: 'Optional.jar' })],
            activeRoots: ['root', 'optional'],
        } as ResourceOperationPlan)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Retry')!
            .trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).toHaveBeenLastCalledWith(
            'plan',
            1,
            [{ kind: 'addOptional', nodeId: 'optional' }],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        vi.mocked(resourceQueuePlan).mockResolvedValue({ job_id: 'job' } as Awaited<
            ReturnType<typeof resourceQueuePlan>
        >)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        await flushPromises()
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith('plan', 2)
        expect(await result).toEqual({ job_id: 'job' })
        expect(wrapper.find('section').exists()).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('blocks unaccepted compatibility and binds an exception to exactly one release', async () => {
    ready()
    const incompatible = {
        ...root,
        compatibility: {
            gameVersion: 'incompatible',
            loader: 'unknown',
            reasons: ['gameVersion', 'loader'],
        },
    }
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        ...plan(),
        nodes: [incompatible],
        issues: [
            {
                selectionId: 'root',
                code: 'compatibilityMismatch',
                blocking: true,
                message: 'incompatible game version',
            },
        ],
    } as ResourceOperationPlan)
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        expect(
            wrapper
                .findAll('button')
                .find((button) => button.text() === 'Queue changes')!
                .attributes('disabled'),
        ).toBeDefined()
        vi.mocked(resourceRevisePlan).mockResolvedValue({
            ...plan(2),
            nodes: [incompatible],
            exceptions: [
                { selectionId: 'root', release: reference.release, dimensions: ['gameVersion'] },
            ],
            issues: [
                {
                    selectionId: 'root',
                    code: 'compatibilityUnknown',
                    blocking: true,
                    message: 'unknown loader',
                },
            ],
        } as ResourceOperationPlan)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Accept Minecraft compatibility exception')!
            .trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).toHaveBeenCalledWith(
            'plan',
            1,
            [
                {
                    kind: 'acceptCompatibility',
                    exception: {
                        selectionId: 'root',
                        release: reference.release,
                        dimensions: ['gameVersion'],
                    },
                },
            ],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        expect(
            wrapper
                .findAll('button')
                .find((button) => button.text() === 'Queue changes')!
                .attributes('disabled'),
        ).toBeDefined()
        expect(
            wrapper
                .findAll('button')
                .some((button) => button.text() === 'Accept Minecraft compatibility exception'),
        ).toBe(false)
        vi.mocked(resourceRevisePlan).mockResolvedValue(plan(3))
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Accept loader compatibility exception')!
            .trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).toHaveBeenLastCalledWith(
            'plan',
            2,
            [
                {
                    kind: 'acceptCompatibility',
                    exception: {
                        selectionId: 'root',
                        release: reference.release,
                        dimensions: ['loader'],
                    },
                },
            ],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Cancel')!
            .trigger('click')
        expect(await result).toBeNull()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})

it('locks preparation controls and cancels an obsolete session without allowing old responses to replace it', async () => {
    ready()
    let signal: AbortSignal | undefined
    let oldProgress: ((value: ResourceOperationProgress) => void) | undefined
    let resolveOld!: (value: ResourceOperationPlan) => void
    vi.mocked(resourcePrepareEntryOperation).mockImplementationOnce((...args) => {
        signal = args[5]
        oldProgress = args[6]
        return new Promise((resolve) => {
            resolveOld = resolve
        })
    })
    const wrapper = mount(ResourceOperationPreviewModal)
    const old = wrapper.vm.show(input)
    try {
        await flushPromises()
        expect(
            wrapper
                .findAll('button')
                .every((button) => button.attributes('disabled') !== undefined),
        ).toBe(true)
        const current = wrapper.vm.show({ ...input, instanceId: 'other' })
        expect(await old).toBeNull()
        expect(signal?.aborted).toBe(true)
        await flushPromises()
        resolveOld({ ...plan(9), actions: [] })
        oldProgress?.({
            planId: 'old',
            revision: 9,
            node: node('obsolete-progress', 'install'),
            metadata: null,
        } as ResourceOperationProgress)
        await flushPromises()
        expect(wrapper.text()).toContain('mods/custom.jar.disabled')
        expect(wrapper.text()).not.toContain('obsolete-progress')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Cancel')!
            .trigger('click')
        expect(await current).toBeNull()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})

it('refreshes a stale reviewed plan before queueing and keeps the selected root draft', async () => {
    ready()
    const wrapper = mount(ResourceOperationPreviewModal)
    const importFiles = { root: 'C:/Downloads/exact.jar' }
    const locators = { root: 'project-slug' }
    const result = wrapper.vm.show({ ...input, importFiles, locators })
    try {
        await flushPromises()
        vi.mocked(resourceQueuePlan).mockRejectedValueOnce({
            message: 'instance changed after review',
        })
        vi.mocked(resourcePrepareOperation).mockResolvedValueOnce({ ...plan(7), id: 'refreshed' })
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        await flushPromises()
        expect(wrapper.text()).not.toContain('instance changed after review')
        expect(resourcePrepareOperation).toHaveBeenCalledWith(
            'instance',
            'update',
            [],
            true,
            expect.any(AbortSignal),
            importFiles,
            locators,
            expect.any(Function),
            true,
        )
        vi.mocked(resourceQueuePlan).mockResolvedValueOnce({ job_id: 'refreshed-job' } as Awaited<
            ReturnType<typeof resourceQueuePlan>
        >)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        await flushPromises()
        expect(resourceQueuePlan).toHaveBeenLastCalledWith('refreshed', 7)
        expect(await result).toEqual({ job_id: 'refreshed-job' })
    } finally {
        wrapper.unmount()
    }
})

it('returns the submitted root range after a selection revision and modal closure', async () => {
    ready()
    vi.mocked(resourcePrepareOperation).mockResolvedValueOnce({ ...plan(), intent: 'install' })
    vi.mocked(resourceRevisePlan).mockResolvedValueOnce({
        ...plan(2),
        intent: 'install',
        activeRoots: ['added-optional'],
    })
    vi.mocked(resourceQueuePlan).mockResolvedValueOnce({ job_id: 'selected-job' } as Awaited<
        ReturnType<typeof resourceQueuePlan>
    >)
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.showInstallSelection({
        instanceId: 'instance',
        instanceName: 'Example',
        selections: [],
        installDependencies: true,
    })
    try {
        await flushPromises()
        await wrapper
            .findAll('button')
            .find((button) => button.text() === '＋')!
            .trigger('click')
        await flushPromises()
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        await flushPromises()
        expect(await result).toEqual({
            job: { job_id: 'selected-job' },
            activeRoots: ['added-optional'],
        })
        expect(resourceQueuePlan).toHaveBeenLastCalledWith('plan', 2)
    } finally {
        wrapper.unmount()
    }
})

it('a second install request cannot replace a preview while the first reviewed job is being queued', async () => {
    ready()
    let finish!: (job: Awaited<ReturnType<typeof resourceQueuePlan>>) => void
    vi.mocked(resourceQueuePlan).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        const next = wrapper.vm.show({ ...input, instanceId: 'other-instance' })
        expect(await next).toBeNull()
        finish({ job_id: 'original-job' } as Awaited<ReturnType<typeof resourceQueuePlan>>)
        await flushPromises()
        expect(await result).toEqual({ job_id: 'original-job' })
        expect(resourcePrepareEntryOperation).not.toHaveBeenCalledWith(
            'other-instance',
            expect.anything(),
            expect.anything(),
            expect.anything(),
            expect.anything(),
            expect.anything(),
        )
    } finally {
        wrapper.unmount()
    }
})

it('shows satisfied entries, labels activated optionals, and phrases blocking issues in place', async () => {
    ready()
    const installed = node('installed', 'satisfied', { reference, fileName: 'Installed.jar' })
    const dependency = node('dependency', 'install', { reference, fileName: 'Dependency.jar' })
    const addedOptional = node('added-optional', 'selectedRoot', {
        reference,
        fileName: 'AddedOptional.jar',
    })
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        ...plan(),
        nodes: [root, installed, dependency, addedOptional, optional],
        edges: [{ parent: 'dependency', child: 'added-optional', kind: 'optional' }],
        issues: [
            {
                selectionId: 'root',
                code: 'manualDownloadRequired',
                blocking: true,
                message: 'source requires an exact manual download',
            },
            {
                selectionId: 'dependency',
                code: 'invalidPath',
                blocking: true,
                message: 'target already exists: mods/conflict.jar',
            },
        ],
    } as unknown as ResourceOperationPlan)
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        expect(wrapper.text()).toContain('Already satisfied')
        expect(wrapper.text()).not.toContain('source requires an exact manual download')
        expect(wrapper.text()).not.toContain('target already exists')
        expect(wrapper.text()).toContain('requires a manual download')
        expect(wrapper.text()).toContain('mods/conflict.jar is already used by another file')
        expect(wrapper.text()).toContain('Optional dependency of dependency')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Cancel')!
            .trigger('click')
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
    }
})

it('ignores repeated optional activation and late revisions after cancellation', async () => {
    ready()
    let finish!: (value: ResourceOperationPlan) => void
    vi.mocked(resourceRevisePlan).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        const plus = wrapper.findAll('button').find((button) => button.text() === '＋')!
        await plus.trigger('click')
        await plus.trigger('click')
        expect(resourceRevisePlan).toHaveBeenCalledTimes(1)
        expect(
            wrapper
                .findAll('button')
                .every((button) => button.attributes('disabled') !== undefined),
        ).toBe(true)
        wrapper.unmount()
        expect(await result).toBeNull()
        finish(plan(2))
        await flushPromises()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})

it('removes one explicit choice from a shared node without removing another choice', async () => {
    ready()
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        ...plan(),
        activeRoots: ['first', 'second'],
        nodes: [{ ...root, rootSelectionIds: ['first', 'second'] }],
    } as ResourceOperationPlan)
    vi.mocked(resourceRevisePlan).mockResolvedValue({
        ...plan(2),
        activeRoots: ['first'],
        nodes: [{ ...root, rootSelectionIds: ['first'] }],
    } as ResourceOperationPlan)
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        const removals = wrapper.findAll('button').filter((button) => button.text() === '×')
        expect(removals).toHaveLength(2)
        await removals[1].trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).toHaveBeenCalledWith(
            'plan',
            1,
            [{ kind: 'removeRoot', selectionId: 'second' }],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        expect(wrapper.findAll('button').filter((button) => button.text() === '×')).toHaveLength(1)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Cancel')!
            .trigger('click')
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
    }
})
