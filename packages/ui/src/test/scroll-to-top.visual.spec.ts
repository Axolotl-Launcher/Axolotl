import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { ref } from 'vue'

import ScrollToTopButton from '../components/base/ScrollToTopButton.vue'
import NotificationPanel from '../components/nav/NotificationPanel.vue'
import { I18N_INJECTION_KEY } from '../providers/i18n'
import { mountThemed } from './visual-harness'

const originalViewport = { width: window.innerWidth, height: window.innerHeight }

beforeEach(async () => {
    await page.viewport(1024, 768)
})

afterEach(async () => {
    vi.restoreAllMocks()
    await page.viewport(originalViewport.width, originalViewport.height)
})

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

async function settleNotificationLayout() {
    // Let ResizeObserver delivery and the following coalesced frame finish too.
    for (let frame = 0; frame < 3; frame++) {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    }
}

async function mountNotificationPanel() {
    const location = ref<'left' | 'right'>('right')
    const notificationManager = {
        getNotifications: () => [{ id: 1, title: 'World imported', text: 'Ready to play' }],
        getNotificationLocation: () => location.value,
        stopNotificationTimer: () => {},
        setNotificationTimer: () => {},
        removeNotification: () => {},
    }
    const panel = await mountThemed(NotificationPanel, { hasSidebar: false }, 'dark', {
        ...mountOptions,
        global: {
            ...mountOptions.global,
            provide: {
                ...mountOptions.global.provide,
                [Symbol.for('modrinth:notificationManager')]: notificationManager,
            },
        },
    })
    await settleNotificationLayout()

    return {
        panel,
        group: panel.get<HTMLElement>('.vue-notification-group').element,
        location,
    }
}

function publishedClearance() {
    return document.documentElement.style.getPropertyValue('--notification-panel-clearance')
}

async function expectPanelClearance(group: HTMLElement, bottom: number) {
    await vi.waitFor(() => {
        expect(Number.parseFloat(getComputedStyle(group).bottom)).toBeCloseTo(bottom, 1)
        expect(publishedClearance()).toBe(
            `${Math.ceil(window.innerHeight - group.getBoundingClientRect().top + 16)}px`,
        )
    })
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
            document.documentElement.style.removeProperty('--notification-panel-clearance')
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
        expect(Number.parseFloat(placement.bottom)).toBeCloseTo(16, 1)
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
        expect(Number.parseFloat(placement.bottom)).toBeCloseTo(60, 1)
    } finally {
        cleanup()
    }
})

it('publishes clearance from a visible notification panel and lifts the button', async () => {
    const { panel } = await mountNotificationPanel()

    try {
        const published = Number.parseFloat(publishedClearance())
        expect(published).toBeGreaterThan(0)
        const { button, cleanup } = await mountScrolledControl()

        try {
            expect(button.exists()).toBe(true)

            const placement = getComputedStyle(button.element.parentElement!)
            expect(Number.parseFloat(placement.bottom)).toBeCloseTo(16 + published, 1)
        } finally {
            cleanup()
        }
    } finally {
        panel.unmount()
    }
})

it('ignores unrelated descendant attributes and coalesces necessary measurements without redundant writes', async () => {
    const unrelated = document.createElement('div')
    document.body.append(unrelated)
    const { panel, group } = await mountNotificationPanel()
    const measure = vi.spyOn(group, 'getBoundingClientRect')
    const write = vi.spyOn(document.documentElement.style, 'setProperty')

    try {
        unrelated.classList.add('unrelated-state')
        unrelated.style.opacity = '0.5'
        const notification = panel.get<HTMLElement>('.vue-notification-wrapper').element
        notification.classList.add('unrelated-state')
        notification.style.opacity = '0.5'
        await settleNotificationLayout()

        expect(measure).not.toHaveBeenCalled()
        expect(write).not.toHaveBeenCalled()

        window.dispatchEvent(new Event('resize'))
        window.dispatchEvent(new Event('resize'))
        group.style.zIndex = '201'
        group.dispatchEvent(new TransitionEvent('transitionend', { propertyName: 'bottom' }))
        await settleNotificationLayout()

        expect(measure).toHaveBeenCalledTimes(1)
        expect(write).not.toHaveBeenCalled()
    } finally {
        panel.unmount()
        unrelated.remove()
    }
})

it('tracks body positioning classes, inherited offsets and viewport changes', async () => {
    const { panel, group } = await mountNotificationPanel()

    try {
        await expectPanelClearance(group, 24)
        const initialClearance = publishedClearance()

        document.body.classList.add('floating-action-bar-shown')
        await expectPanelClearance(group, 90)
        expect(publishedClearance()).not.toBe(initialClearance)

        document.body.classList.remove('floating-action-bar-shown')
        await expectPanelClearance(group, 24)
        expect(publishedClearance()).toBe(initialClearance)

        document.body.style.setProperty('--size-mobile-navbar-height', '56px')
        await page.viewport(700, 768)
        await expectPanelClearance(group, 66)

        document.body.style.setProperty('--size-mobile-navbar-height', '96px')
        await expectPanelClearance(group, 106)
    } finally {
        panel.unmount()
        document.body.classList.remove('floating-action-bar-shown')
        document.body.style.removeProperty('--size-mobile-navbar-height')
    }
})

it('tracks panel positioning, content resizing and notification location', async () => {
    const { panel, group, location } = await mountNotificationPanel()

    try {
        group.style.bottom = '48px'
        await expectPanelClearance(group, 48)
        const initialClearance = Number.parseFloat(publishedClearance())

        panel.get<HTMLElement>('.vue-notification-wrapper').element.style.minHeight = '160px'
        await vi.waitFor(() => {
            expect(Number.parseFloat(publishedClearance())).toBeGreaterThan(initialClearance)
        })
        await expectPanelClearance(group, 48)

        location.value = 'left'
        await vi.waitFor(() => expect(publishedClearance()).toBe('0px'))
        location.value = 'right'
        await expectPanelClearance(group, 48)
    } finally {
        panel.unmount()
    }
})

it('tracks intercom insertion and removal without observing descendant attributes', async () => {
    const { panel, group } = await mountNotificationPanel()
    const intercom = document.createElement('div')
    intercom.className = 'intercom-lightweight-app'

    try {
        document.body.append(intercom)
        await expectPanelClearance(group, 80)

        intercom.remove()
        await expectPanelClearance(group, 24)
    } finally {
        panel.unmount()
        intercom.remove()
    }
})

it('cancels pending frames and removes observers, listeners and clearance on unmount', async () => {
    const { panel, group } = await mountNotificationPanel()
    const measure = vi.spyOn(group, 'getBoundingClientRect')
    const write = vi.spyOn(document.documentElement.style, 'setProperty')
    const requestFrame = vi.spyOn(window, 'requestAnimationFrame')
    const cancelFrame = vi.spyOn(window, 'cancelAnimationFrame')

    try {
        window.dispatchEvent(new Event('resize'))
        group.style.bottom = '48px'
        expect(requestFrame).toHaveBeenCalledTimes(1)
        const pendingFrame = requestFrame.mock.results[0].value

        panel.unmount()
        expect(cancelFrame).toHaveBeenCalledWith(pendingFrame)
        expect(publishedClearance()).toBe('')
        requestFrame.mockRestore()
        cancelFrame.mockRestore()

        window.dispatchEvent(new Event('resize'))
        group.dispatchEvent(new TransitionEvent('transitionend', { propertyName: 'bottom' }))
        group.style.bottom = '64px'
        document.body.classList.add('floating-action-bar-shown')
        await settleNotificationLayout()

        expect(measure).not.toHaveBeenCalled()
        expect(write).not.toHaveBeenCalled()
        expect(publishedClearance()).toBe('')
    } finally {
        if (panel.exists()) panel.unmount()
        document.body.classList.remove('floating-action-bar-shown')
    }
})
