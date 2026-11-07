import '@/test/visual-styles.scss'

import { I18N_INJECTION_KEY } from '@modrinth/ui'
import { mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'

import StudioImageEditor from './StudioImageEditor.vue'

// A real 1×1 PNG: the editor decodes the bytes onto a fabric canvas, so a
// placeholder would only prove that the element mounted.
const PNG_BYTES = Uint8Array.from(
    atob(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
    ),
    (character) => character.charCodeAt(0),
)

// The editor renders the real buttons and reads its labels through the locale
// the app provides at startup, so the test provides the same contract instead of
// stubbing the components it is meant to exercise.
const i18n = {
    locale: ref('en-US'),
    t: (key: string) => key,
    setLocale: async () => undefined,
}

const hosts: HTMLElement[] = []
const mounted: ReturnType<typeof mount>[] = []

afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    hosts.splice(0).forEach((host) => host.remove())
})

function mountEditor(overrides: Record<string, unknown> = {}) {
    const host = document.createElement('div')
    host.style.width = '480px'
    host.style.height = '360px'
    host.style.display = 'flex'
    document.body.appendChild(host)
    hosts.push(host)

    const wrapper = mount(StudioImageEditor, {
        props: {
            content: PNG_BYTES,
            mimeType: 'image/png',
            name: 'block.png',
            ...overrides,
        },
        attachTo: host,
        global: {
            directives: { tooltip: {} },
            provide: { [I18N_INJECTION_KEY as symbol]: i18n },
        },
    })
    mounted.push(wrapper)

    return wrapper
}

function toolLabels(wrapper: ReturnType<typeof mountEditor>) {
    return wrapper
        .findAll('[role="toolbar"] button')
        .map((button) => button.attributes('aria-label'))
}

it('offers the annotation tools the studio needs', () => {
    const wrapper = mountEditor()

    expect(toolLabels(wrapper)).toEqual([
        'Select',
        'Crop',
        'Pen',
        'Highlight',
        'Eraser',
        'Arrow',
        'Rectangle',
        'Ellipse',
        'Text',
    ])
})

it('leaves the censor brush out of the studio toolbar', () => {
    const wrapper = mountEditor()

    expect(toolLabels(wrapper)).not.toContain('Censor')
})

it('reports the image size once the canvas is ready', async () => {
    const wrapper = mountEditor()

    await vi.waitFor(() => expect(wrapper.text()).toContain('1 × 1'))
})

it('switches the active tool from the toolbar', async () => {
    const wrapper = mountEditor()
    // Initialising the canvas ends by selecting the move tool, so a click that
    // arrives before that finishes would be replaced by the initialiser and the
    // assertion would only ever see the selection it set itself.
    await vi.waitFor(() => expect(wrapper.text()).toContain('1 × 1'))

    const pen = wrapper
        .findAll('[role="toolbar"] button')
        .find((b) => b.attributes('aria-label') === 'Pen')

    await pen!.trigger('click')

    expect(pen!.attributes('aria-pressed')).toBe('true')
    expect(
        wrapper
            .findAll('[role="toolbar"] button')
            .find((b) => b.attributes('aria-label') === 'Select')!
            .attributes('aria-pressed'),
    ).toBe('false')
})

it('saves the edit as a PNG the file can hold', async () => {
    const wrapper = mountEditor()
    await vi.waitFor(() => expect(wrapper.text()).toContain('1 × 1'))

    const save = wrapper.findAll('button').find((button) => button.text() === 'Save')
    await save!.trigger('click')

    await vi.waitFor(() => expect(wrapper.emitted('save')).toBeTruthy())
    const bytes = wrapper.emitted('save')![0][0] as Uint8Array
    // A PNG signature: the file was read as PNG, so the bytes have to stay PNG
    // rather than be re-encoded into a format the extension does not match.
    expect([...bytes.slice(0, 4)]).toEqual([0x89, 0x50, 0x4e, 0x47])
})

it('releases the canvas when it is closed before it finishes opening', async () => {
    const revoke = vi.spyOn(URL, 'revokeObjectURL')
    const wrapper = mountEditor()
    // The fabric import and the image decode are still running; closing now used
    // to leave an initialiser that created a canvas and an object URL nothing
    // owned any more.
    await nextTick()
    wrapper.unmount()

    await vi.waitFor(() => expect(revoke).toHaveBeenCalled())
})

it('cancels without saving', async () => {
    const wrapper = mountEditor()

    const cancel = wrapper.findAll('button').find((button) => button.text() === 'Cancel')
    await cancel!.trigger('click')

    expect(wrapper.emitted('cancel')).toHaveLength(1)
    expect(wrapper.emitted('save')).toBeUndefined()
})

it('leaves escape to a focused property control instead of closing the editor', async () => {
    const wrapper = mountEditor()
    // The property controls only exist once a tool that has one is active, which
    // the initialiser resets; wait for it so the pen is the last word.
    await vi.waitFor(() => expect(wrapper.text()).toContain('1 × 1'))
    const pen = wrapper
        .findAll('[role="toolbar"] button')
        .find((button) => button.attributes('aria-label') === 'Pen')
    await pen!.trigger('click')

    const width = wrapper.find('input[type="number"]')
    expect(width.exists()).toBe(true)
    ;(width.element as HTMLInputElement).focus()
    width.element.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await nextTick()

    // Escape inside the control belongs to the control; discarding the whole
    // edit because the user pressed it while adjusting a value loses their work.
    expect(wrapper.emitted('cancel')).toBeUndefined()

    // The engine spends the first Escape returning to the select tool, so the
    // editor only closes on the one after it.
    for (let press = 0; press < 2; press += 1) {
        document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
        await nextTick()
    }
    expect(wrapper.emitted('cancel')).toHaveLength(1)
})
