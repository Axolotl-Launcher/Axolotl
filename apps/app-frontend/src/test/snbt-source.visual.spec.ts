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

it('indents a new line between a bracket pair', async () => {
    const { editor, text } = await mountSource({
        modelValue: '',
        kind: 'entity',
        version: '1.21',
        domain: 'entity',
    })

    editor.setValue('{}')
    editor.setPosition({ lineNumber: 1, column: 2 })
    editor.focus()
    await flushPromises()
    editor.trigger('keyboard', 'type', { text: '\n' })
    await new Promise((resolve) => setTimeout(resolve, 300))

    // 三行：光标落在缩进后的空行，右括号自己退到下一行。
    expect(text().replace(/\r\n/g, '\n')).toBe('{\n    \n}')
    expect(editor.getPosition()).toEqual({ lineNumber: 2, column: 5 })
})

it('expands the one-liner coming from another mode', async () => {
    const { text } = await mountSource({
        modelValue: '[custom_name="a",lore=["b"]]',
        kind: 'item',
        version: '1.21',
        domain: 'item',
    })

    expect(text()).toBe('[\n    custom_name="a",\n    lore=[\n        "b"\n    ]\n]')
})

it('expands and collapses with the toolbar buttons', async () => {
    const { wrapper, editor, text } = await mountSource({
        modelValue: '',
        kind: 'item',
        version: '1.21',
        domain: 'item',
    })

    editor.setValue('[custom_name="a",lore=["b"]]')
    await flushPromises()

    const [expandButton, collapseButton] = wrapper.findAll('button')
    await expandButton.trigger('click')
    await flushPromises()
    expect(text()).toBe('[\n    custom_name="a",\n    lore=[\n        "b"\n    ]\n]')

    await collapseButton.trigger('click')
    await flushPromises()
    expect(text()).toBe('[custom_name="a",lore=["b"]]')
})

it('accepts any tag as a generic data value', async () => {
    const { wrapper, editor, text } = await mountSource({
        modelValue: '',
        kind: 'generic',
        version: '1.21',
    })

    // `/data … value` 可以是标量或普通列表，不该铺骨架
    expect(text()).toBe('')

    const [, collapseButton] = wrapper.findAll('button')
    for (const value of ['1', '[1,2]']) {
        editor.setValue(value)
        await flushPromises()
        await collapseButton.trigger('click')
        await flushPromises()
        expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([value])
    }
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

it('adds a pair of quotes for a string component', async () => {
    const { editor, text } = await mountSource({
        modelValue: '',
        kind: 'item',
        version: '1.21',
        domain: 'item',
    })

    editor.setValue('[custom_nam')
    editor.setPosition({ lineNumber: 1, column: 12 })
    editor.focus()
    await flushPromises()
    editor.trigger('keyboard', 'editor.action.triggerSuggest', {})

    await waitFor(() => Boolean(document.querySelector('.suggest-widget .monaco-list-row')), {
        timeoutMs: 15_000,
        label: 'rows',
    })
    editor.trigger('keyboard', 'acceptSelectedSuggestion', {})
    await new Promise((resolve) => setTimeout(resolve, 300))

    expect(text()).toBe('[custom_name=""')
})

it('lists ids with their sprites after a namespaced prefix', async () => {
    const { editor } = await mountSource({
        modelValue: '',
        kind: 'entity',
        version: '1.21',
        domain: 'entity',
    })

    editor.setValue('{id:"minecraft:zom')
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
        label: row.querySelector('.label-name')?.textContent?.trim() ?? '',
        sprite: (row.querySelector('.icon') as HTMLElement | null)?.style.backgroundImage ?? '',
    }))
    console.log('ID ROWS:', JSON.stringify(rows.slice(0, 3)))
    expect(rows.length).toBeGreaterThan(0)
    expect(rows.every((row) => row.label.startsWith('minecraft:zom'))).toBe(true)
    expect(rows.some((row) => row.sprite.includes('texture-atlas'))).toBe(true)
    editor.trigger('keyboard', 'hideSuggestWidget', {})
})

/** 用户报的场景：bundle_contents 里手打 `id`，光标停在中间那行。 */
const BUNDLE = [
    '[',
    '    bundle_contents=[',
    '        {',
    '            id',
    '        }',
    '    ]',
    ']',
].join('\n')

it('suggests item ids while typing inside the quotes of a nested item stack', async () => {
    const { editor, text } = await mountSource({
        modelValue: '',
        kind: 'item',
        version: '1.21',
        domain: 'item',
    })

    editor.setValue(BUNDLE)
    editor.setPosition({ lineNumber: 4, column: 15 })
    editor.focus()
    await flushPromises()
    editor.trigger('keyboard', 'editor.action.triggerSuggest', {})

    await waitFor(() => Boolean(document.querySelector('.suggest-widget .monaco-list-row')), {
        timeoutMs: 15_000,
        label: 'rows',
    })
    // 接受 `id`：光标应该落在补出来的那对引号中间。
    editor.trigger('keyboard', 'acceptSelectedSuggestion', {})
    await new Promise((resolve) => setTimeout(resolve, 300))
    expect(text()).toContain('id:""')

    // 接着打字，弹窗要自己出来（snippet 会话不该挡住快速补全）。
    for (const char of 'dia') {
        editor.trigger('keyboard', 'type', { text: char })
        await new Promise((resolve) => setTimeout(resolve, 150))
    }
    console.log('BUNDLE WIDGET:', widgetText().slice(0, 160))
    expect(document.querySelectorAll('.suggest-widget .monaco-list-row').length).toBeGreaterThan(0)
    expect(widgetText()).toContain('minecraft:diamond')
})
