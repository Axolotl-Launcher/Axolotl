import { expect, it } from 'vitest'
import { ref } from 'vue'

import ScrollToTopButton from '../components/base/ScrollToTopButton.vue'
import { I18N_INJECTION_KEY } from '../providers/i18n'
import { mountThemed } from './visual-harness'

const mountOptions = {
    attachTo: document.body,
    global: {
        provide: {
            [I18N_INJECTION_KEY as symbol]: {
                locale: ref('en-US'),
                t: (key: string) => key,
                setLocale: () => {},
            },
        },
        directives: { tooltip: {} },
    },
}

async function mountScrolledControl() {
    const viewport = document.createElement('div')
    viewport.className = 'app-viewport'
    viewport.style.cssText = 'height:600px;overflow:auto'
    viewport.innerHTML = '<div style="height:1000px"></div>'
    document.body.append(viewport)

    const wrapper = await mountThemed(ScrollToTopButton, {}, 'dark', mountOptions)
    viewport.scrollTop = 500
    viewport.dispatchEvent(new Event('scroll'))
    await wrapper.vm.$nextTick()

    return {
        wrapper,
        button: wrapper.find<HTMLElement>('.scroll-to-top-btn'),
        cleanup() {
            wrapper.unmount()
            viewport.remove()
            document.documentElement.style.removeProperty('--right-bar-width')
            document.documentElement.style.removeProperty('--floating-action-bar-clearance')
        },
    }
}

it('anchors the floating control to the safe bottom-right of the content pane', async () => {
    document.documentElement.style.setProperty('--right-bar-width', '300px')
    const { button, cleanup } = await mountScrolledControl()

    try {
        expect(button.exists()).toBe(true)

        const placement = getComputedStyle(button.element.parentElement!)
        expect(placement.position).toBe('fixed')
        expect(Number.parseFloat(placement.right)).toBeGreaterThanOrEqual(324)
        expect(Number.parseFloat(placement.bottom)).toBeGreaterThanOrEqual(48)
    } finally {
        cleanup()
    }
})

it('stays above the globally managed floating action bar', async () => {
    document.documentElement.style.setProperty('--floating-action-bar-clearance', '44px')
    const { button, cleanup } = await mountScrolledControl()

    try {
        expect(button.exists()).toBe(true)

        const placement = getComputedStyle(button.element.parentElement!)
        expect(Number.parseFloat(placement.bottom)).toBeGreaterThanOrEqual(108)
    } finally {
        cleanup()
    }
})
