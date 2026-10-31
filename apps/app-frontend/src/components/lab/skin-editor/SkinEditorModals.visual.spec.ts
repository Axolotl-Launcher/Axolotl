import { I18N_INJECTION_KEY } from '@modrinth/ui'
import { expect, it } from 'vitest'
import { ref } from 'vue'

import { DEFAULT_SKIN_EDITOR_PREFERENCES } from '@/lab/skin-editor/session'
import { mountThemed, waitFor } from '@/test/visual-harness'

import SkinEditorRecoveryModal from './SkinEditorRecoveryModal.vue'
import SkinEditorSettingsModal from './SkinEditorSettingsModal.vue'
import SkinEditorUnsavedChangesModal from './SkinEditorUnsavedChangesModal.vue'

const provide = {
    [I18N_INJECTION_KEY as symbol]: {
        locale: ref('en-US'),
        t: (key: string) => key,
        setLocale: () => undefined,
    },
}

const directives = { tooltip: () => {} }

async function waitForDialog() {
    await waitFor(() => !!document.querySelector('[role="dialog"]'), {
        label: 'skin editor dialog',
    })
}

function button(label: string) {
    return [...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')].find(
        (candidate) => candidate.textContent?.trim().includes(label),
    )
}

it('edits and saves skin editor preferences', async () => {
    const wrapper = await mountThemed(
        SkinEditorSettingsModal,
        { preferences: DEFAULT_SKIN_EDITOR_PREFERENCES },
        'dark',
        { global: { provide, directives } },
    )
    try {
        ;(wrapper.vm as unknown as { show: () => void }).show()
        await waitForDialog()

        const interval = document.querySelector<HTMLInputElement>('#skin-auto-save-interval')!
        interval.value = '45'
        interval.dispatchEvent(new Event('input', { bubbles: true }))

        const autoSaveToggle = document.querySelector<HTMLButtonElement>(
            '[role="dialog"] [role="switch"]',
        )!
        autoSaveToggle.click()
        await wrapper.vm.$nextTick()
        button('Save settings')!.click()
        await wrapper.vm.$nextTick()

        expect(wrapper.emitted('save')?.at(-1)?.[0]).toMatchObject({
            autoSaveEnabled: false,
            autoSaveIntervalSeconds: 45,
            warnBeforeLeave: true,
        })
    } finally {
        wrapper.unmount()
    }
})

it('offers save, discard and stay for unsaved skin changes', async () => {
    const wrapper = await mountThemed(SkinEditorUnsavedChangesModal, {}, 'dark', {
        global: { provide, directives },
    })
    try {
        const result = (
            wrapper.vm as unknown as { prompt: () => Promise<'save' | 'discard' | 'stay'> }
        ).prompt()
        await waitForDialog()

        expect(button('Save PNG')).toBeDefined()
        expect(button('Discard')).toBeDefined()
        expect(button('Stay')).toBeDefined()
        button('Discard')!.click()
        expect(await result).toBe('discard')
    } finally {
        wrapper.unmount()
    }
})

it('shares one prompt while an unsaved decision is already open', async () => {
    const wrapper = await mountThemed(SkinEditorUnsavedChangesModal, {}, 'dark', {
        global: { provide, directives },
    })
    try {
        const vm = wrapper.vm as unknown as {
            prompt: () => Promise<'save' | 'discard' | 'stay'>
        }
        const first = vm.prompt()
        const second = vm.prompt()
        expect(second).toBe(first)
        await waitForDialog()
        button('Stay')!.click()
        expect(await first).toBe('stay')
    } finally {
        wrapper.unmount()
    }
})

it('shows recovery time and emits the chosen recovery action', async () => {
    const wrapper = await mountThemed(SkinEditorRecoveryModal, {}, 'dark', {
        global: { provide, directives },
    })
    try {
        ;(
            wrapper.vm as unknown as {
                show: (recovery: { updatedAt: number; name?: string }) => void
            }
        ).show({ updatedAt: Date.UTC(2026, 9, 31, 8, 30), name: 'Axolotl.png' })
        await waitForDialog()

        expect(document.querySelector('[role="dialog"]')?.textContent).toContain(
            'Continue last skin?',
        )
        button('Continue')!.click()
        expect(wrapper.emitted('continue')).toHaveLength(1)
    } finally {
        wrapper.unmount()
    }
})
