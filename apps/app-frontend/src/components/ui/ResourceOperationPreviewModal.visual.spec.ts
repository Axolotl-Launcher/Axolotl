import { config, flushPromises, mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { RESOURCE_ANALYSIS_TIMEOUT_MS } from '@/helpers/resource-analysis-request'
import {
    resourceAnalysisClose,
    resourceAnalysisOpen,
    resourceAnalysisUpdate,
    resourceGet,
    type ResourceOperationPlan,
    type ResourceOperationProgress,
    resourcePrepareEntryOperation,
    resourcePrepareOperation,
    resourceQueuePlan,
    resourceRevisePlan,
} from '@/helpers/resources'
import { get_instance_worlds } from '@/helpers/worlds'
import zhMessages from '@/locales/zh-CN/index.json'
import { createContentAnalysis } from '@/providers/content-analysis'

import ResourceOperationPreviewModal from './ResourceOperationPreviewModal.vue'

const closeTransition = vi.fn().mockResolvedValue(undefined)
const testLocale: { language: string; messages: Record<string, { message: string }> | null } = {
    language: 'en',
    messages: null,
}
config.global.directives.tooltip = {}

vi.mock('@/helpers/resources', () => ({
    resourceAnalysisOpen: vi.fn(),
    resourceAnalysisClose: vi.fn().mockResolvedValue(undefined),
    resourceAnalysisUpdate: vi.fn(),
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
    Admonition: defineComponent({
        props: ['header', 'type'],
        setup:
            (props, { slots }) =>
            () =>
                h('aside', [h('h2', props.header), slots.default?.(), slots.actions?.()]),
    }),
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
            message: { id?: string; defaultMessage: string },
            values?: Record<string, string | number>,
        ) =>
            new IntlMessageFormat(
                testLocale.messages?.[message.id ?? '']?.message ?? message.defaultMessage,
                testLocale.language,
            ).format(values),
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

it('keeps unknown compatibility visible and queues without requiring exceptions', async () => {
    ready()
    const local = node('local-file', 'satisfied')
    local.compatibility = { gameVersion: 'unknown', loader: 'unknown', reasons: [] }
    const remote = node('remote-file', 'install', { reference, fileName: 'Remote.jar' })
    remote.compatibility = { gameVersion: 'unknown', loader: 'unknown', reasons: [] }
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        ...plan(),
        nodes: [local, remote],
    } as unknown as ResourceOperationPlan)
    vi.mocked(resourceQueuePlan).mockResolvedValueOnce({ job_id: 'unknown-job' } as never)
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
        ).toHaveLength(0)
        expect(
            wrapper
                .findAll('button')
                .filter((button) => button.text() === 'Accept loader compatibility exception'),
        ).toHaveLength(0)
        expect(wrapper.text()).toContain('Already satisfied')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).not.toHaveBeenCalled()
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith('plan', 1)
        expect(await result).toEqual({ job_id: 'unknown-job' })
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
    testLocale.language = 'en'
    testLocale.messages = null
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

it.each(['en', 'zh-CN'])(
    'shows distinct localized conflict details at the top and a brief install tooltip (%s)',
    async (language) => {
        ready()
        testLocale.language = language
        testLocale.messages = language === 'zh-CN' ? zhMessages : null
        const sodium = node('sodium', 'selectedRoot', { reference, fileName: 'Sodium.jar' })
        vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
            ...plan(),
            nodes: [root, sodium],
            issues: [
                {
                    code: 'versionConflict',
                    selectionId: 'root',
                    blocking: true,
                    message: 'incompatible resource is active',
                    conflict: {
                        reason: 'incompatibleResource',
                        subject: 'Canvas Renderer',
                        related: 'Sodium.jar',
                        required: '*',
                        actual: null,
                    },
                },
                {
                    code: 'versionConflict',
                    selectionId: 'root',
                    blocking: true,
                    message:
                        'parent dependency requirement is not satisfied by the selected artifact',
                    conflict: {
                        reason: 'dependencyVersion',
                        subject: 'Canvas Renderer',
                        related: 'Sodium.jar',
                        required: '>=0.6',
                        actual: '0.5.13',
                    },
                },
            ],
        } as ResourceOperationPlan)
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
                                reference.kind === 'source' &&
                                reference.project.projectId === 'sodium'
                                    ? 'Sodium'
                                    : 'Canvas Renderer',
                            iconUrl: null,
                        },
                    ],
                }) as Awaited<ReturnType<typeof resourceGet>>,
        )
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
        const result = wrapper.vm.show(input)
        try {
            await flushPromises()
            const expected =
                language === 'zh-CN'
                    ? [
                          'Canvas Renderer 声明与 Sodium 不兼容（冲突范围：所有版本）。',
                          'Canvas Renderer 要求 Sodium 满足 >=0.6，当前所选版本或文件为 0.5.13。',
                      ]
                    : [
                          'Canvas Renderer declares an incompatibility with Sodium (all versions).',
                          'Canvas Renderer requires Sodium to match >=0.6; the selected version or file is 0.5.13.',
                      ]
            const queueLabel =
                language === 'zh-CN'
                    ? zhMessages['app.resources.operation.apply'].message
                    : 'Queue changes'
            const button = wrapper.findAll('button').find((button) => button.text() === queueLabel)!
            const details = wrapper.find('[data-resource-conflicts]')
            expect(details.exists()).toBe(true)
            expect(details.element.parentElement?.firstElementChild).toBe(details.element)
            expect(details.findAll('li')).toHaveLength(2)
            for (const text of expected) {
                expect(details.text()).toContain(text)
                expect(button.attributes('data-tooltip')).not.toContain(text)
            }
            expect(button.attributes('data-tooltip')).toBe(
                language === 'zh-CN' ? '检测到版本冲突。' : 'A version conflict was detected.',
            )
            expect(button.attributes('data-tooltip')).not.toContain('0.5.13')
            expect(wrapper.text()).not.toContain('incompatible resource is active')
            expect(wrapper.text()).not.toContain('parent dependency requirement is not satisfied')
            expect(button.attributes('disabled')).toBeUndefined()
            await wrapper.vm.cancel()
            expect(await result).toBeNull()
        } finally {
            wrapper.unmount()
        }
    },
)

it('explains a selected mod resolution failure at the top without calling it a dependency failure', async () => {
    ready()
    testLocale.language = 'zh-CN'
    testLocale.messages = zhMessages
    const failed = {
        ...plan(),
        nodes: [],
        actions: [],
        issues: [
            {
                code: 'missingDependency',
                selectionId: 'root',
                blocking: true,
                message: 'opaque selected release diagnostic',
                context: { dependency: false, subject: 'Example Mod', source: 'modrinth' },
            },
        ],
    } as ResourceOperationPlan
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue(failed)
    vi.mocked(resourceRevisePlan).mockResolvedValue({ ...failed, revision: 2 })
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
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        const details = wrapper.find('[data-resource-conflicts]')
        expect(details.element.parentElement?.firstElementChild).toBe(details.element)
        expect(details.text()).toContain('Example Mod（Modrinth）')
        expect(details.text()).toContain('无法准备所选内容')
        expect(details.text()).toContain('跳过依赖检查无法解决此问题')
        expect(details.text()).not.toContain('无法解析所需的依赖')
        expect(details.text()).not.toContain('opaque selected release diagnostic')
        const button = wrapper
            .findAll('button')
            .find(
                (button) => button.text() === zhMessages['app.resources.operation.apply'].message,
            )!
        expect(button.attributes('data-tooltip')).toBe('无法准备所选内容。')
        expect(button.attributes('disabled')).toBeUndefined()
        await button.trigger('click')
        await flushPromises()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        expect(wrapper.find('[data-resource-conflicts]').findAll('li')).toHaveLength(1)
        await wrapper.vm.cancel()
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
    }
})

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
        analysis: { signal: controller.signal, prepare, cancel: vi.fn() },
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

it('cancels a dependency analysis that never finishes from the Cancel button and stops the cart analysis', async () => {
    ready()
    const controller = new AbortController()
    let analysisSignal: AbortSignal | undefined
    let late!: (value: ResourceOperationPlan) => void
    const prepare = vi.fn(
        (signal: AbortSignal) =>
            new Promise<ResourceOperationPlan>((resolve) => {
                analysisSignal = signal
                late = resolve
            }),
    )
    const cancelAnalysis = vi.fn(() => controller.abort())
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.showInstallSelection({
        instanceId: 'instance',
        instanceName: 'Instance',
        selections: [],
        installDependencies: true,
        analysis: { signal: controller.signal, prepare, cancel: cancelAnalysis },
    })
    try {
        await flushPromises()
        expect(wrapper.find('[data-operation-skeleton]').exists()).toBe(true)
        const cancel = wrapper.findAll('button').find((button) => button.text() === 'Cancel')!
        expect(cancel.attributes('disabled')).toBeUndefined()
        await cancel.trigger('click')
        await flushPromises()
        expect(analysisSignal?.aborted).toBe(true)
        expect(cancelAnalysis).toHaveBeenCalledOnce()
        expect(await result).toBeNull()
        expect(wrapper.find('section').exists()).toBe(false)
        late(plan())
        await flushPromises()
        expect(wrapper.find('section').exists()).toBe(false)
        expect(resourceQueuePlan).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})

it('keeps a finished cart analysis when its reviewed preview is closed', async () => {
    ready()
    const cancelAnalysis = vi.fn()
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.showInstallSelection({
        instanceId: 'instance',
        instanceName: 'Instance',
        selections: [],
        installDependencies: true,
        analysis: {
            signal: new AbortController().signal,
            prepare: vi.fn().mockResolvedValue(plan()),
            cancel: cancelAnalysis,
        },
    })
    try {
        await flushPromises()
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Cancel')!
            .trigger('click')
        expect(await result).toBeNull()
        expect(cancelAnalysis).not.toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})

it('uses a warning tooltip and queues selected content in one click after dependency analysis fails', async () => {
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
        expect(button.attributes('disabled')).toBeUndefined()
        expect(button.attributes('color')).toBe('orange')
        expect(button.attributes('type')).toBe('colored')
        expect(button.attributes('data-tooltip')).toBe('Could not connect to the content source.')
        expect(wrapper.text()).not.toContain('Force install selected content only')
        await button.trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).toHaveBeenCalledWith(
            reviewed.id,
            reviewed.revision,
            [{ kind: 'ignoreDependencies' }],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith(reviewed.id, 2)
        expect((await result)?.job_id).toBe('queued')
    } finally {
        wrapper.unmount()
    }
})

it('queues selected content in one click when dependency conflicts have other blocking issues', async () => {
    ready()
    const dependency = {
        ...node('dependency', 'conflict', { reference, fileName: 'Dependency.jar' }),
        compatibility: { gameVersion: 'incompatible', loader: 'compatible', reasons: [] },
    }
    const reviewed = {
        ...plan(),
        nodes: [root, dependency],
        issues: [
            {
                code: 'versionConflict',
                selectionId: 'root',
                blocking: true,
                message: 'required dependency conflicts with an active root release or artifact',
            },
            {
                code: 'compatibilityMismatch',
                selectionId: 'dependency',
                blocking: true,
                message: 'game version mismatch',
            },
            {
                code: 'sourceUnavailable',
                selectionId: 'other-dependency',
                blocking: true,
                message: 'source offline',
            },
        ],
    } as ResourceOperationPlan
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue(reviewed)
    vi.mocked(resourceRevisePlan).mockResolvedValue({
        ...plan(2),
        nodes: [root],
    })
    vi.mocked(resourceQueuePlan).mockResolvedValue({ job_id: 'queued' } as never)
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        const queue = () =>
            wrapper.findAll('button').find((button) => button.text() === 'Queue changes')!
        expect(queue().attributes('disabled')).toBeUndefined()
        expect(queue().attributes('color')).toBe('orange')
        await queue().trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).toHaveBeenCalledExactlyOnceWith(
            reviewed.id,
            reviewed.revision,
            [{ kind: 'ignoreDependencies' }],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith(reviewed.id, 2)
        expect((await result)?.job_id).toBe('queued')
    } finally {
        wrapper.unmount()
    }
})

it('preserves blocking root file validation after preparing a roots-only preview', async () => {
    ready()
    const invalidPath = {
        code: 'invalidPath' as const,
        selectionId: 'root',
        entryId: null,
        blocking: true,
        message: 'invalid filename',
    }
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
            invalidPath,
        ],
    } as ResourceOperationPlan
    vi.mocked(resourcePrepareOperation).mockResolvedValue(reviewed)
    vi.mocked(resourceRevisePlan).mockResolvedValue({
        ...reviewed,
        revision: 2,
        issues: [invalidPath],
    })
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
        expect(button.attributes('disabled')).toBeUndefined()
        await button.trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).toHaveBeenCalledWith(
            reviewed.id,
            reviewed.revision,
            [{ kind: 'ignoreDependencies' }],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        expect(wrapper.text()).toContain('The installation path is invalid')
        expect(button.attributes('disabled')).toBeUndefined()
        expect(button.attributes('color')).toBe('orange')
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
        expect(queue().attributes('disabled')).toBeUndefined()
        await queue().trigger('click')
        await flushPromises()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
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

it('retries the one-click install without dependencies after analysis rejects the entire preview', async () => {
    ready()
    vi.mocked(resourcePrepareOperation)
        .mockRejectedValueOnce(new Error('dependency analysis failed'))
        .mockRejectedValueOnce(new Error('temporary failure'))
        .mockResolvedValueOnce({ ...plan(), intent: 'install' })
    vi.mocked(resourceQueuePlan).mockResolvedValue({ job_id: 'queued' } as never)
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
            .find((button) => button.text() === 'Queue changes')!
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
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith('plan', 1)
        expect((await result)?.job_id).toBe('queued')
    } finally {
        wrapper.unmount()
    }
})

it('accepts the selected release compatibility exceptions and queues it in the same click', async () => {
    ready()
    const incompatible = {
        ...root,
        compatibility: { gameVersion: 'incompatible', loader: 'unknown', reasons: [] },
    }
    const issues = [
        {
            code: 'compatibilityMismatch',
            selectionId: 'root',
            blocking: true,
            message: 'game version mismatch',
        },
    ]
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        ...plan(),
        nodes: [incompatible],
        issues,
    } as ResourceOperationPlan)
    vi.mocked(resourceRevisePlan)
        .mockResolvedValueOnce({
            ...plan(2),
            nodes: [incompatible],
            issues,
        } as ResourceOperationPlan)
        .mockResolvedValueOnce(plan(3))
    vi.mocked(resourceQueuePlan).mockResolvedValueOnce({ job_id: 'forced-job' } as never)
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        const button = wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
        expect(button.attributes('color')).toBe('orange')
        expect(button.attributes('disabled')).toBeUndefined()
        await button.trigger('click')
        await flushPromises()
        expect(resourceRevisePlan).toHaveBeenNthCalledWith(
            1,
            'plan',
            1,
            [{ kind: 'ignoreDependencies' }],
            expect.any(AbortSignal),
            expect.any(Function),
        )
        expect(resourceRevisePlan).toHaveBeenNthCalledWith(
            2,
            'plan',
            2,
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
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith('plan', 3)
        expect((await result)?.job_id).toBe('forced-job')
        expect(wrapper.find('section').exists()).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('locks the one-click fallback against duplicate submission and reopening but lets its revision be canceled', async () => {
    ready()
    vi.mocked(resourcePrepareEntryOperation).mockResolvedValue({
        ...plan(),
        issues: [
            { code: 'versionConflict', selectionId: 'root', blocking: true, message: 'conflict' },
        ],
    } as ResourceOperationPlan)
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
        const button = wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
        await button.trigger('click')
        await flushPromises()
        expect(button.attributes('disabled')).toBeDefined()
        await button.trigger('click')
        expect(await wrapper.vm.show({ ...input, instanceId: 'other' })).toBeNull()
        expect(resourceRevisePlan).toHaveBeenCalledOnce()
        const cancel = wrapper.findAll('button').find((button) => button.text() === 'Cancel')!
        expect(cancel.attributes('disabled')).toBeUndefined()
        await cancel.trigger('click')
        await flushPromises()
        expect(wrapper.find('section').exists()).toBe(false)
        expect(await result).toBeNull()
        finish(plan(2))
        await flushPromises()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
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

it('keeps compatibility exception controls and an enabled warning install button for installed dependencies', async () => {
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
        ).toBeUndefined()
        await wrapper.vm.cancel()
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
    }
})

it('shows each parsed item while retaining the skeleton and locking every button except Cancel until the final plan arrives', async () => {
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
            kind: 'node',
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
                .every(
                    (button) =>
                        (button.attributes('disabled') === undefined) ===
                        (button.text() === 'Cancel'),
                ),
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

it('shows what the analysis is doing and drops nodes a later resolution round removed', async () => {
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
    const stage = () => wrapper.get('[data-operation-phase]').text()
    const update = (value: object) => progress(value as ResourceOperationProgress)
    const emit = (value: ReturnType<typeof node>, title: string, round: number) =>
        update({
            kind: 'node',
            planId: 'plan',
            revision: 1,
            round,
            node: value,
            metadata: { title, iconUrl: null, summary: '' },
        })
    try {
        await flushPromises()
        expect(stage()).toBe('Resolving releases and dependencies…')
        update({ kind: 'phase', phase: 'collectingConstraints' })
        await flushPromises()
        expect(stage()).toBe('Collecting the version requirements of installed content…')

        update({ kind: 'roundStarted', planId: 'plan', revision: 1, round: 0 })
        await flushPromises()
        expect(stage()).toBe('Resolving releases and dependencies…')
        emit(root, 'Parsed root', 0)
        emit(node('stale', 'install', { reference, fileName: 'Stale.jar' }), 'Stale dependency', 0)
        await flushPromises()
        expect(stage()).toBe('Resolving releases and dependencies…')
        expect(wrapper.text()).toContain('Stale dependency')

        update({ kind: 'roundStarted', planId: 'plan', revision: 1, round: 1 })
        update({ kind: 'nodeRemoved', planId: 'plan', revision: 1, round: 1, nodeId: 'stale' })
        await flushPromises()
        expect(stage()).toBe('Resolving dependencies again to settle version conflicts…')
        expect(wrapper.text()).not.toContain('Stale dependency')
        expect(wrapper.text()).toContain('Parsed root')

        update({ kind: 'phase', phase: 'validating' })
        await flushPromises()
        expect(stage()).toBe('Checking that the files can be written…')
        // Closing the window releases the run without going through cancel().
        await (wrapper.vm as unknown as { modal: { hide: () => Promise<void> } }).modal.hide()
        expect(await result).toBeNull()
        complete({ ...plan(), nodes: [root] } as ResourceOperationPlan)

        // A data pack preview waits for the world list before it analyzes; its
        // skeleton starts from the default text, not the closed run's stage.
        vi.mocked(get_instance_worlds).mockReturnValueOnce(new Promise(() => {}))
        const next = wrapper.vm.showInstall({
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
        await flushPromises()
        expect(stage()).toBe('Resolving releases and dependencies…')
        await wrapper.vm.cancel()
        expect(await next).toBeNull()
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
            kind: 'node',
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
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        await flushPromises()
        expect(resourcePrepareOperation).not.toHaveBeenCalled()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
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
        ).toBeUndefined()
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        vi.mocked(resourcePrepareOperation).mockResolvedValueOnce(plan())
        vi.mocked(resourceQueuePlan).mockResolvedValueOnce({ job_id: 'world-job' } as never)
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        await flushPromises()
        const forcedWorld = vi.mocked(resourcePrepareOperation).mock.lastCall!
        expect(forcedWorld[2][0].scope).toEqual({ kind: 'world', relativePath: 'saves/two' })
        expect(forcedWorld[8]).toBe(false)
        expect((await result)?.job_id).toBe('world-job')
        expect(resourceQueuePlan).toHaveBeenCalledExactlyOnceWith('plan', 1)
    } finally {
        wrapper.unmount()
    }
})

it('allows adding a missing dependency or using the warning install button when automatic installation is off', async () => {
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
        ).toBeUndefined()
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
            'The content source is unavailable. Check the connection and source settings, then retry.',
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

it('warns about unaccepted compatibility and binds manual exceptions to exactly one release', async () => {
    ready()
    const incompatible = {
        ...root,
        compatibility: {
            gameVersion: 'incompatible',
            loader: 'incompatible',
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
        ).toBeUndefined()
        vi.mocked(resourceRevisePlan).mockResolvedValue({
            ...plan(2),
            nodes: [incompatible],
            exceptions: [
                { selectionId: 'root', release: reference.release, dimensions: ['gameVersion'] },
            ],
            issues: [
                {
                    selectionId: 'root',
                    code: 'compatibilityMismatch',
                    blocking: true,
                    message: 'incompatible loader',
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
        ).toBeUndefined()
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

it('locks preparation controls except Cancel and cancels an obsolete session without allowing old responses to replace it', async () => {
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
                .every(
                    (button) =>
                        (button.attributes('disabled') === undefined) ===
                        (button.text() === 'Cancel'),
                ),
        ).toBe(true)
        const current = wrapper.vm.show({ ...input, instanceId: 'other' })
        expect(await old).toBeNull()
        expect(signal?.aborted).toBe(true)
        await flushPromises()
        resolveOld({ ...plan(9), actions: [] })
        oldProgress?.({
            kind: 'node',
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

it('refreshes a stale reviewed plan and queues the selected root draft without another click', async () => {
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
        vi.mocked(resourceQueuePlan).mockResolvedValueOnce({ job_id: 'refreshed-job' } as Awaited<
            ReturnType<typeof resourceQueuePlan>
        >)
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
        expect(resourceQueuePlan).toHaveBeenCalledTimes(2)
        expect(resourceQueuePlan).toHaveBeenLastCalledWith('refreshed', 7)
        expect(await result).toEqual({ job_id: 'refreshed-job' })
    } finally {
        wrapper.unmount()
    }
})

it('lets the user cancel the re-analysis that follows a stale queue attempt', async () => {
    ready()
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await flushPromises()
        vi.mocked(resourceQueuePlan).mockRejectedValueOnce({
            message: 'instance changed after review',
        })
        let refreshSignal: AbortSignal | undefined
        vi.mocked(resourcePrepareOperation).mockImplementationOnce((...args) => {
            refreshSignal = args[4]
            return new Promise(() => {})
        })
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Queue changes')!
            .trigger('click')
        await flushPromises()
        expect(refreshSignal).toBeDefined()
        const cancel = wrapper.findAll('button').find((button) => button.text() === 'Cancel')!
        expect(cancel.attributes('disabled')).toBeUndefined()
        await cancel.trigger('click')
        await flushPromises()
        expect(refreshSignal!.aborted).toBe(true)
        expect(await result).toBeNull()
        expect(wrapper.find('section').exists()).toBe(false)
        expect(resourceQueuePlan).toHaveBeenCalledOnce()
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
        expect(wrapper.text()).toContain('The source requires a manual download')
        expect(wrapper.text()).toContain('Another file already occupies the destination')
        expect(wrapper.text()).toContain('mods/conflict.jar')
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
                .every(
                    (button) =>
                        (button.attributes('disabled') === undefined) ===
                        (button.text() === 'Cancel'),
                ),
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

it('recovers a timed-out cart analysis in place and retries with a new native session', async () => {
    ready()
    vi.useFakeTimers()
    vi.mocked(resourceAnalysisOpen)
        .mockResolvedValueOnce('old-session')
        .mockResolvedValueOnce('new-session')
    let report!: (value: ResourceOperationProgress) => void
    let late!: (value: ResourceOperationPlan) => void
    vi.mocked(resourceAnalysisUpdate)
        .mockImplementationOnce((...args) => {
            report = args[5]
            return new Promise((resolve) => (late = resolve))
        })
        .mockResolvedValue(plan())
    vi.mocked(resourcePrepareOperation).mockResolvedValue(plan())
    const selections = [
        {
            id: 'root',
            resource: { kind: 'source', project: reference.release.project },
            release: { kind: 'automatic' },
            resourceType: 'mod',
            scope: { kind: 'instance' },
        },
    ] as ResourceOperationPlan['selections']
    const cart = createContentAnalysis()
    cart.update({
        instanceId: 'instance',
        context: 'review',
        selections,
        installDependencies: true,
    })
    const analysis = cart.review()!
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.showInstallSelection({
        instanceId: 'instance',
        instanceName: 'Instance',
        selections,
        installDependencies: true,
        analysis,
    })
    try {
        await vi.advanceTimersByTimeAsync(RESOURCE_ANALYSIS_TIMEOUT_MS - 1)
        report({
            kind: 'phase',
            phase: 'validating',
            selectionRevision: 1,
        } as ResourceOperationProgress)
        await vi.advanceTimersByTimeAsync(1)
        expect(wrapper.find('section').exists()).toBe(true)
        expect(wrapper.find('[data-operation-skeleton]').exists()).toBe(false)
        expect(wrapper.text()).toContain('The request timed out. Check your connection and retry.')
        expect(analysis.signal.aborted).toBe(false)
        expect(vi.mocked(resourceAnalysisUpdate).mock.calls[0][4].aborted).toBe(true)
        expect(resourceAnalysisClose).toHaveBeenCalledWith('old-session')
        report({
            kind: 'node',
            selectionRevision: 1,
            node: { ...root, id: 'late' },
        } as ResourceOperationProgress)
        late({ ...plan(), id: 'late-plan' })
        await vi.advanceTimersByTimeAsync(0)
        expect(wrapper.text()).not.toContain('late')
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        const retry = wrapper.findAll('button').find((button) => button.text() === 'Retry')!
        expect(retry.attributes('disabled')).toBeUndefined()
        await retry.trigger('click')
        await vi.advanceTimersByTimeAsync(0)
        expect(resourceAnalysisOpen).toHaveBeenCalledTimes(2)
        expect(vi.mocked(resourceAnalysisUpdate).mock.calls[1][2]).toEqual(selections)
        expect(vi.mocked(resourcePrepareOperation).mock.calls[0][9]).toEqual({
            sessionId: 'new-session',
            selectionRevision: 2,
        })
        expect(wrapper.find('[data-operation-skeleton]').exists()).toBe(false)
        expect(wrapper.text()).toContain('mods/custom.jar.disabled')
        expect(wrapper.text()).not.toContain('The request timed out.')
        expect(vi.getTimerCount()).toBe(0)
        await wrapper.vm.cancel()
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
        cart.clear()
        vi.useRealTimers()
    }
})

it('bounds an unresponsive direct preview and keeps retry available after late native callbacks', async () => {
    ready()
    vi.useFakeTimers()
    let report!: (value: ResourceOperationProgress) => void
    let late!: (value: ResourceOperationPlan) => void
    vi.mocked(resourcePrepareEntryOperation).mockImplementationOnce((...args) => {
        report = args[6]!
        return new Promise((resolve) => (late = resolve))
    })
    const wrapper = mount(ResourceOperationPreviewModal)
    const result = wrapper.vm.show(input)
    try {
        await vi.advanceTimersByTimeAsync(RESOURCE_ANALYSIS_TIMEOUT_MS)
        expect(vi.mocked(resourcePrepareEntryOperation).mock.calls[0][5]?.aborted).toBe(true)
        expect(wrapper.find('[data-operation-skeleton]').exists()).toBe(false)
        expect(wrapper.find('section').exists()).toBe(true)
        expect(wrapper.text()).toContain('The request timed out. Check your connection and retry.')
        report({ kind: 'node', node: { ...root, id: 'late' } } as ResourceOperationProgress)
        late({ ...plan(), id: 'late-plan' })
        await vi.advanceTimersByTimeAsync(0)
        expect(wrapper.text()).not.toContain('late')
        expect(wrapper.text()).not.toContain('mods/custom.jar.disabled')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Retry')!
            .trigger('click')
        await vi.advanceTimersByTimeAsync(0)
        expect(resourcePrepareEntryOperation).toHaveBeenCalledTimes(2)
        expect(wrapper.text()).toContain('mods/custom.jar.disabled')
        expect(wrapper.text()).not.toContain('The request timed out.')
        expect(resourceQueuePlan).not.toHaveBeenCalled()
        await wrapper.vm.cancel()
        expect(await result).toBeNull()
    } finally {
        wrapper.unmount()
        vi.useRealTimers()
    }
})
