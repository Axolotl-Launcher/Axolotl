import { flushPromises } from '@vue/test-utils'
import type * as Monaco from 'monaco-editor/editor/editor.api.js'
import { expect, it, vi } from 'vitest'

import CodeEditor from '../components/code-editor/CodeEditor.vue'
import {
    attachSnbtCompletion,
    registerSnbt,
    type SnbtCompletionSource,
} from '../components/code-editor/snbt/index.ts'
import { mountThemed, waitFor } from './visual-harness'

vi.setConfig({ testTimeout: 30_000 })

it('shows suggestions from the attached source', async () => {
    const source: SnbtCompletionSource = {
        candidates: () => [
            { label: 'banner_patterns', insertText: 'banner_patterns: ', kind: 'field', score: 90 },
        ],
    }

    const wrapper = await mountThemed(
        CodeEditor,
        { content: '[banner_', filePath: 'nbt.snbt', language: 'snbt' },
        'dark',
        { attrs: { style: 'width: 640px; height: 320px' } },
    )

    await waitFor(() => Boolean(wrapper.emitted('ready')), { timeoutMs: 20_000, label: 'ready' })
    const [editor, runtime] = wrapper.emitted('ready')![0] as [
        Monaco.editor.IStandaloneCodeEditor,
        typeof Monaco,
    ]

    registerSnbt(runtime)
    const model = editor.getModel()!
    attachSnbtCompletion(editor, model, source)

    editor.setPosition({ lineNumber: 1, column: 8 })
    editor.focus()
    await flushPromises()
    editor.trigger('keyboard', 'editor.action.triggerSuggest', {})

    await waitFor(
        () => {
            const widget = document.querySelector('.suggest-widget')
            return Boolean(widget && widget.getBoundingClientRect().height > 0)
        },
        { timeoutMs: 5000, label: 'suggest widget' },
    )

    const text = document.querySelector('.suggest-widget')?.textContent ?? ''
    expect(text).toContain('banner_patterns')

    wrapper.unmount()
})
