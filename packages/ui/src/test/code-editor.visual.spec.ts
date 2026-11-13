import { flushPromises } from '@vue/test-utils'
import type * as Monaco from 'monaco-editor/editor/editor.api.js'
import { afterEach, expect, it, vi } from 'vitest'
import { userEvent } from 'vitest/browser'
import { ref } from 'vue'

import CodeEditor from '../components/code-editor/CodeEditor.vue'
import EditorFindReplace from '../layouts/shared/files-tab/components/editor/EditorFindReplace.vue'
import FileEditor from '../layouts/shared/files-tab/components/editor/FileEditor.vue'
import { mountThemed, waitFor } from './visual-harness'

// Monaco setup plus two editors can exceed the default timeout when the whole
// visual suite runs in parallel; it completes in a few seconds on its own.
vi.setConfig({ testTimeout: 30_000 })

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).forEach((dispose) => dispose()))

async function openEditor(content = '{"name":"original"}') {
    const wrapper = await mountThemed(
        CodeEditor,
        {
            content,
            filePath: 'config.json',
            language: 'json',
        },
        'dark',
        { attrs: { style: 'width: 640px; height: 320px' } },
    )
    cleanup.push(() => wrapper.unmount())
    await waitFor(() => Boolean(wrapper.emitted('ready') || wrapper.emitted('error')), {
        timeoutMs: 20000,
        label: 'Monaco editor ready',
    })
    expect(wrapper.emitted('error')).toBeUndefined()
    const [editor, runtime] = wrapper.emitted('ready')![0] as [
        Monaco.editor.IStandaloneCodeEditor,
        typeof Monaco,
    ]
    return { wrapper, editor, runtime }
}

it('keeps simultaneous documents separate and synchronizes edits, external content and model disposal', async () => {
    const first = await openEditor()
    const second = await openEditor('{"name":"second"}')
    const oldModel = first.editor.getModel()!
    expect(oldModel.uri.toString()).not.toBe(second.editor.getModel()!.uri.toString())

    first.editor.executeEdits('test-input', [
        {
            range: oldModel.getFullModelRange(),
            text: '{"name":"edited"}',
        },
    ])
    expect(first.wrapper.emitted('update:content')!.at(-1)).toEqual(['{"name":"edited"}'])
    expect(second.editor.getValue()).toBe('{"name":"second"}')

    const changes = first.wrapper.emitted('update:content')!.length
    await first.wrapper.setProps({ content: '{"name":"external"}', readOnly: true })
    expect(first.editor.getValue()).toBe('{"name":"external"}')
    expect(first.editor.getOption(first.runtime.editor.EditorOption.readOnly)).toBe(true)
    expect(first.wrapper.emitted('update:content')!.length).toBe(changes)

    await first.wrapper.setProps({
        filePath: 'next.toml',
        language: 'toml',
        content: 'name = "next"',
    })
    expect(oldModel.isDisposed()).toBe(true)
    expect(first.editor.getModel()!.getLanguageId()).toBe('toml')
    expect(first.editor.getValue()).toBe('name = "next"')
    const lastModel = first.editor.getModel()!
    first.wrapper.unmount()
    expect(lastModel.isDisposed()).toBe(true)
    cleanup.shift()
})

it('formats JSON through its worker and emits save from the editor shortcut', async () => {
    const { wrapper, editor } = await openEditor('{"name":"format","enabled":true}')
    await wrapper.vm.formatDocument()
    expect(editor.getValue()).toContain('\n')
    expect(JSON.parse(editor.getValue())).toEqual({ name: 'format', enabled: true })
    editor.focus()
    await userEvent.keyboard('{Control>}s{/Control}')
    expect(wrapper.emitted('save')).toHaveLength(1)
}, 30000)

it('waits for a formatter provider and releases the wait when its model is disposed', async () => {
    const { runtime } = await openEditor()
    const { prepareFormatting } = await import('../components/code-editor/monaco-runtime')
    const { jsonDefaults } = await import('monaco-editor/language/json/monaco.contribution.js')
    const { StandaloneServices } =
        await import('monaco-editor/editor/standalone/browser/standaloneServices.js')
    const { ILanguageService } = await import('monaco-editor/editor/common/languages/language.js')
    const modeConfiguration = jsonDefaults.modeConfiguration
    jsonDefaults.setModeConfiguration({
        ...modeConfiguration,
        documentFormattingEdits: false,
        documentRangeFormattingEdits: false,
    })
    const model = runtime.editor.createModel('{}', 'json')
    const disposedModel = runtime.editor.createModel('{}', 'json')
    const changedModel = runtime.editor.createModel('{}', 'json')
    const activation = vi.spyOn(
        StandaloneServices.get(ILanguageService),
        'requestRichLanguageFeatures',
    )
    let provider: Monaco.IDisposable | undefined
    try {
        let ready = false
        const waiting = prepareFormatting(model).then(() => {
            ready = true
        })
        await waitFor(() => activation.mock.calls.length === 1, {
            label: 'formatting to request language providers',
        })
        expect(ready).toBe(false)
        provider = runtime.languages.registerDocumentFormattingEditProvider('json', {
            provideDocumentFormattingEdits: () => [],
        })
        await waiting
        expect(ready).toBe(true)
        provider.dispose()

        let disposedWaitFinished = false
        const disposedWait = prepareFormatting(disposedModel).then(() => {
            disposedWaitFinished = true
        })
        await waitFor(() => activation.mock.calls.length === 2, {
            label: 'the disposable model to wait for its formatter',
        })
        expect(disposedWaitFinished).toBe(false)
        disposedModel.dispose()
        await disposedWait
        expect(disposedWaitFinished).toBe(true)

        let changedWaitFinished = false
        const changedWait = prepareFormatting(changedModel).then(() => {
            changedWaitFinished = true
        })
        await waitFor(() => activation.mock.calls.length === 3, {
            label: 'the changing model to wait for its formatter',
        })
        expect(changedWaitFinished).toBe(false)
        runtime.editor.setModelLanguage(changedModel, 'toml')
        await changedWait
        expect(changedWaitFinished).toBe(true)

        const unsupported = runtime.editor.createModel('name = "test"', 'toml')
        try {
            await prepareFormatting(unsupported)
        } finally {
            unsupported.dispose()
        }
    } finally {
        provider?.dispose()
        activation.mockRestore()
        model.dispose()
        disposedModel.dispose()
        changedModel.dispose()
        jsonDefaults.setModeConfiguration(modeConfiguration)
    }
})

it('finds and replaces file text with undo and blocks replacement while busy', async () => {
    const busy = ref(false)
    const wrapper = await mountThemed(
        FileEditor,
        {
            file: { name: 'options.txt', path: 'options.txt' },
        },
        'dark',
        {
            attrs: { style: 'width: 640px' },
            global: {
                provide: {
                    [Symbol.for('modrinth:fileManagerContext')]: {
                        isBusy: busy,
                        readFile: async () => 'alpha beta ALPHA',
                        writeFile: async () => {},
                    },
                    [Symbol.for('modrinth:notificationManager')]: { addNotification: () => {} },
                    [Symbol.for('modrinth:modrinthClient')]: {},
                },
            },
        },
    )
    cleanup.push(() => wrapper.unmount())
    await flushPromises()
    await waitFor(
        () => {
            const codeEditor = wrapper.findComponent(CodeEditor)
            return codeEditor.exists() && Boolean(codeEditor.vm.getEditor())
        },
        {
            timeoutMs: 20000,
            label: 'file editor ready',
        },
    )
    const editor = wrapper.findComponent(CodeEditor).vm.getEditor()!
    const find = wrapper.findComponent(EditorFindReplace)
    find.vm.$emit('update:findQuery', 'alpha')
    await flushPromises()
    expect(find.props('findMatchCount')).toBe(2)
    find.vm.$emit('replace', 'first')
    await flushPromises()
    expect(editor.getValue()).toBe('first beta ALPHA')
    expect(wrapper.vm.hasUnsavedChanges).toBe(true)
    await editor.getModel()!.undo()
    await flushPromises()
    expect(editor.getValue()).toBe('alpha beta ALPHA')
    find.vm.$emit('replaceAll', 'all')
    await flushPromises()
    expect(editor.getValue()).toBe('all beta all')
    find.vm.$emit('update:findQuery', 'all')
    busy.value = true
    await flushPromises()
    find.vm.$emit('replaceAll', 'blocked')
    await flushPromises()
    expect(editor.getValue()).toBe('all beta all')

    busy.value = false
    await flushPromises()
    editor.executeEdits('large-document', [
        {
            range: editor.getModel()!.getFullModelRange(),
            text: 'alpha '.repeat(1005),
        },
    ])
    find.vm.$emit('update:findQuery', 'alpha')
    await flushPromises()
    expect(find.props('findMatchCount')).toBe(1005)
    expect(editor.getModel()!.getValueInRange(editor.getSelection()!)).toBe('alpha')
    find.vm.$emit('replaceAll', 'omega')
    await flushPromises()
    expect(editor.getValue()).toBe('omega '.repeat(1005))
    expect(find.props('findMatchCount')).toBe(0)

    wrapper.vm.revertChanges()
    await flushPromises()
    expect(editor.getValue()).toBe('alpha beta ALPHA')
    expect(find.props('findMatchCount')).toBe(2)
}, 30000)
