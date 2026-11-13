import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import { defineComponent, h, nextTick, ref } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import NavTabs from '../components/base/NavTabs.vue'
import { mountThemed, waitFor } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn()),
)

const links = [
    { label: 'Alpha', href: '/alpha' },
    { label: 'Beta', href: '/beta' },
]

function makeRouter() {
    return createRouter({
        history: createMemoryHistory(),
        routes: ['/alpha', '/beta', '/gamma'].map((path) => ({
            path,
            component: { render: () => null },
        })),
    })
}

async function mountTabs(
    router: ReturnType<typeof makeRouter>,
    props: Record<string, unknown> = {},
) {
    const Harness = defineComponent({
        setup: () => () => h(NavTabs, { links, ...props }),
    })
    const wrapper = await mountThemed(Harness, {}, 'dark', {
        global: { plugins: [router] },
    })
    cleanup.push(() => wrapper.unmount())
    await nextTick()
    return wrapper
}

function pointerDown(el: Element, init: PointerEventInit = {}) {
    el.dispatchEvent(
        new PointerEvent('pointerdown', {
            bubbles: true,
            button: 0,
            pointerType: 'mouse',
            isPrimary: true,
            ...init,
        }),
    )
}

function click(el: Element) {
    el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 }))
}

it('animates on pointerdown, before the router commits the tab', async () => {
    let release!: () => void
    let hold = false
    const router = makeRouter()
    router.beforeEach((to) => {
        if (hold && to.path === '/beta') {
            return new Promise<void>((resolve) => {
                release = resolve
            })
        }
    })
    await router.push('/alpha')
    const wrapper = await mountTabs(router)
    const beta = wrapper.get('a[href="/beta"]')
    const slider = wrapper.get<HTMLElement>('.navtabs-slider')

    hold = true
    pointerDown(beta.element)
    await nextTick()

    expect(router.currentRoute.value.path).toBe('/alpha')
    await waitFor(
        () => slider.element.style.left === `${(beta.element as HTMLElement).offsetLeft}px`,
    )

    click(beta.element)
    await flushPromises()
    release()
    await flushPromises()
    await waitFor(() => router.currentRoute.value.path === '/beta')
    await nextTick()

    expect(slider.element.style.left).toBe(`${(beta.element as HTMLElement).offsetLeft}px`)
})

it('ignores external route changes by default', async () => {
    const router = makeRouter()
    await router.push('/alpha')
    const wrapper = await mountTabs(router)
    const alpha = wrapper.get('a[href="/alpha"]')
    const slider = wrapper.get<HTMLElement>('.navtabs-slider')
    await waitFor(
        () => slider.element.style.left === `${(alpha.element as HTMLElement).offsetLeft}px`,
    )

    await router.push('/beta')
    await flushPromises()
    await nextTick()

    expect(slider.element.style.left).toBe(`${(alpha.element as HTMLElement).offsetLeft}px`)
})

it('follows external route changes when route-sync is enabled', async () => {
    const router = makeRouter()
    await router.push('/alpha')
    const wrapper = await mountTabs(router, { routeSync: true })
    const beta = wrapper.get('a[href="/beta"]')
    const slider = wrapper.get<HTMLElement>('.navtabs-slider')

    await router.push('/beta')
    await waitFor(
        () => slider.element.style.left === `${(beta.element as HTMLElement).offsetLeft}px`,
    )
})

it('rolls back to the committed tab when navigation is aborted', async () => {
    const router = makeRouter()
    router.beforeEach((to) => (to.path === '/beta' ? false : undefined))
    await router.push('/alpha')
    const wrapper = await mountTabs(router)
    const alpha = wrapper.get('a[href="/alpha"]')
    const beta = wrapper.get('a[href="/beta"]')
    const slider = wrapper.get<HTMLElement>('.navtabs-slider')

    pointerDown(beta.element)
    await nextTick()
    await waitFor(
        () => slider.element.style.left === `${(beta.element as HTMLElement).offsetLeft}px`,
    )

    click(beta.element)
    await flushPromises()

    await waitFor(
        () => slider.element.style.left === `${(alpha.element as HTMLElement).offsetLeft}px`,
    )
    expect(router.currentRoute.value.path).toBe('/alpha')
})

it('does not activate on modifier pointerdown', async () => {
    const router = makeRouter()
    await router.push('/alpha')
    const wrapper = await mountTabs(router)
    const alpha = wrapper.get('a[href="/alpha"]')
    const beta = wrapper.get('a[href="/beta"]')
    const slider = wrapper.get<HTMLElement>('.navtabs-slider')
    const before = slider.element.style.left

    pointerDown(beta.element, { ctrlKey: true })
    await nextTick()

    expect(slider.element.style.left).toBe(before)
    await waitFor(
        () => slider.element.style.left === `${(alpha.element as HTMLElement).offsetLeft}px`,
    )
})

it('keeps local mode controlled by activeIndex rather than the route', async () => {
    const router = makeRouter()
    await router.push('/alpha')
    const activeIndex = ref(0)
    const Harness = defineComponent({
        setup: () => () => h(NavTabs, { links, mode: 'local', activeIndex: activeIndex.value }),
    })
    const wrapper = await mountThemed(Harness, {}, 'dark', { global: { plugins: [router] } })
    cleanup.push(() => wrapper.unmount())
    const beta = wrapper.findAll('button[role="tab"]')[1]
    const slider = wrapper.get<HTMLElement>('.navtabs-slider')
    await waitFor(() => slider.element.style.left !== '')

    activeIndex.value = 1
    await nextTick()

    await waitFor(
        () => slider.element.style.left === `${(beta.element as HTMLElement).offsetLeft}px`,
    )
    expect(router.currentRoute.value.path).toBe('/alpha')
})

it('snaps a fresh instance to the committed tab instead of replaying the old snapshot', async () => {
    const router = makeRouter()
    await router.push('/alpha')
    const first = await mountTabs(router)
    first.unmount()
    cleanup.splice(cleanup.length - 1, 1)

    await router.push('/beta')
    const transitions: string[] = []
    const listener = (event: Event) => {
        if (
            event instanceof TransitionEvent &&
            (event.target as Element | null)?.classList.contains('navtabs-slider')
        ) {
            transitions.push(event.propertyName)
        }
    }
    document.addEventListener('transitionstart', listener, true)

    const second = await mountTabs(router)
    const beta = second.get('a[href="/beta"]')
    const slider = second.get<HTMLElement>('.navtabs-slider')
    await waitFor(
        () => slider.element.style.left === `${(beta.element as HTMLElement).offsetLeft}px`,
    )

    expect(transitions).toEqual([])
    document.removeEventListener('transitionstart', listener, true)
})

it('shows the slider when a tab is activated from the no-active state', async () => {
    const router = makeRouter()
    await router.push('/gamma')
    const wrapper = await mountTabs(router)
    const alpha = wrapper.get('a[href="/alpha"]')
    expect(wrapper.find('.navtabs-slider').exists()).toBe(false)

    pointerDown(alpha.element)
    await nextTick()

    const slider = wrapper.get<HTMLElement>('.navtabs-slider')
    await waitFor(
        () => slider.element.style.left === `${(alpha.element as HTMLElement).offsetLeft}px`,
    )
})
