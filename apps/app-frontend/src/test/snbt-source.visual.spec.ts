import { CodeEditor, I18N_INJECTION_KEY } from '@modrinth/ui'
import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import CommandGeneratorNbtSource from '@/components/lab/command-generator/CommandGeneratorNbtSource.vue'
import { mountThemed, waitFor } from '@/test/visual-harness'

vi.setConfig({ testTimeout: 40_000 })

interface Driver {
    setValue: (value: string) => void
    setPosition: (position: { lineNumber: number; column: number }) => void
    getPosition: () => { lineNumber: number; column: number }
    getModel: () => { getValue: () => string } | null
    focus: () => void
    trigger: (source: string, id: string, payload: object) => void
}

const mounted: { unmount: () => void }[] = []

afterEach(() => {
    while (mounted.length) mounted.pop()?.unmount()
})

async function mountSource(props: Record<string, unknown>) {
    const wrapper = await mountThemed(CommandGeneratorNbtSource, props, 'dark', {
        global: {
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref('en-US'),
                    t: (key: string) => key,
                    setLocale: () => {},
                },
            },
        },
    })
    mounted.push(wrapper)

    const editorOf = () =>
        (
            wrapper.findComponent(CodeEditor).vm as unknown as { getEditor: () => Driver | null }
        ).getEditor()
    await waitFor(() => Boolean(editorOf()), { timeoutMs: 20_000, label: 'editor' })
    await flushPromises()

    const editor = editorOf()!
    const model = editor.getModel()!
    return { wrapper, editor, text: () => model.getValue() }
}

function widgetText() {
    return document.querySelector('.suggest-widget')?.textContent ?? ''
}

it('seeds a component list for 1.20.5+ items', async () => {
    const { editor, text } = await mountSource({
        modelValue: '',
        kind: 'item',
        version: '1.21',
        domain: 'item',
    })

    expect(text()).toBe('[\n    \n]')
    expect(editor.getPosition()).toEqual({ lineNumber: 2, column: 5 })
})

it('seeds a compound for legacy items', async () => {
    const { text } = await mountSource({
        modelValue: '',
        kind: 'item',
        version: '1.20.4',
        domain: 'item',
        forceDataComponents: false,
    })

    expect(text()).toBe('{\n    \n}')
})

it('seeds a compound for entity nbt', async () => {
    const { text } = await mountSource({
        modelValue: '',
        kind: 'entity',
        version: '1.21',
        domain: 'entity',
    })

    expect(text()).toBe('{\n    \n}')
})

it('opens the component list right away on the seeded skeleton', async () => {
    const { editor } = await mountSource({
        modelValue: '',
        kind: 'item',
        version: '1.21',
        domain: 'item',
    })

    await waitFor(() => Boolean(document.querySelector('.suggest-widget .monaco-list-row')), {
        timeoutMs: 15_000,
        label: 'rows',
    })
    console.log('SEEDED WIDGET:', widgetText().slice(0, 200))
    expect(widgetText()).toContain('banner_patterns')
    editor.trigger('keyboard', 'hideSuggestWidget', {})
})

it('inserts the brackets along with the component name', async () => {
    const { editor, text } = await mountSource({
        modelValue: '',
        kind: 'item',
        version: '1.21',
        domain: 'item',
    })

    editor.setValue('[banner_')
    editor.setPosition({ lineNumber: 1, column: 9 })
    editor.focus()
    await flushPromises()
    editor.trigger('keyboard', 'editor.action.triggerSuggest', {})

    await waitFor(() => Boolean(document.querySelector('.suggest-widget .monaco-list-row')), {
        timeoutMs: 15_000,
        label: 'rows',
    })
    console.log('BANNER WIDGET:', widgetText().slice(0, 200))
    editor.trigger('keyboard', 'acceptSelectedSuggestion', {})
    await new Promise((resolve) => setTimeout(resolve, 300))
    console.log('AFTER ACCEPT:', JSON.stringify(text()))

    expect(text()).toBe('[banner_patterns=[]')
})

it('lists ids with their sprites after a namespaced prefix', async () => {
    const { editor } = await mountSource({
        modelValue: '{id:"minecraft:zom',
        kind: 'entity',
        version: '1.21',
        domain: 'entity',
    })

    editor.setPosition({ lineNumber: 1, column: 19 })
    editor.focus()
    await flushPromises()
    editor.trigger('keyboard', 'editor.action.triggerSuggest', {})

    await waitFor(() => Boolean(document.querySelector('.suggest-widget .monaco-list-row')), {
        timeoutMs: 15_000,
        label: 'rows',
    })
    await new Promise((resolve) => setTimeout(resolve, 600))

    const rows = [...document.querySelectorAll('.suggest-widget .monaco-list-row')].map((row) => ({
        label: row.querySelector('.label-name')?.textContent?.trim(),
        sprite: (row.querySelector('.icon') as HTMLElement | null)?.style.backgroundImage ?? '',
    }))
    console.log('ID ROWS:', JSON.stringify(rows.slice(0, 3)))
    expect(rows[0]?.label).toBe('minecraft:zombie')
    expect(rows[0]?.sprite).toContain('texture-atlas')
    editor.trigger('keyboard', 'hideSuggestWidget', {})
})
