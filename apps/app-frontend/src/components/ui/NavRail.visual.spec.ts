import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import { defineComponent, h, nextTick } from 'vue'
import type { RouteLocationNormalizedLoaded } from 'vue-router'
import { createMemoryHistory, createRouter } from 'vue-router'

import { libraryRoutes } from '@/routes/library'
import { mountThemed, waitFor } from '@/test/visual-harness'

import NavButton from './NavButton.vue'
import NavRail from './NavRail.vue'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).forEach((fn) => fn()))

it('keeps the library parent selected through every child route and switches the rail between regions', async () => {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            ...libraryRoutes.map((route) => ({
                ...route,
                component: { render: () => null },
                children: route.children?.map((child) => ({
                    ...child,
                    component: { render: () => null },
                })),
            })),
            ...['/lab', '/library-other', '/instance/demo'].map((path) => ({
                path,
                component: { render: () => null },
            })),
        ],
    })
    await router.push('/library')
    const Harness = defineComponent({
        setup: () => () =>
            h(
                NavRail,
                {},
                {
                    default: () => [
                        h(
                            NavButton,
                            {
                                to: '/library',
                                isPrimary: (route: RouteLocationNormalizedLoaded) =>
                                    route.matched.some((record) => record.name === 'Library'),
                                isSubpage: (route: RouteLocationNormalizedLoaded) =>
                                    route.path.startsWith('/instance/'),
                            },
                            () => 'Library',
                        ),
                        h(NavButton, { to: '/lab' }, () => 'Lab'),
                    ],
                },
            ),
    })
    const wrapper = await mountThemed(Harness, {}, 'dark', { global: { plugins: [router] } })
    cleanup.push(() => wrapper.unmount())
    const library = wrapper.get('a[href="/library"]')
    const lab = wrapper.get('a[href="/lab"]')
    const slider = wrapper.get<HTMLElement>('.nav-rail-slider')
    for (const path of [
        '/library',
        ...libraryRoutes[0]
            .children!.filter((child) => child.path)
            .map((child) => `/library/${child.path}`),
    ]) {
        await router.push(path)
        await waitFor(() => slider.element.style.opacity === '1')
        expect(library.classes()).toContain('router-link-active')
        expect(library.classes()).not.toContain('subpage-active')
        expect(lab.classes()).not.toContain('router-link-active')
        expect(slider.element.style.top).toBe(`${(library.element as HTMLElement).offsetTop}px`)
    }
    await router.push('/lab')
    await waitFor(() => slider.element.style.top === `${(lab.element as HTMLElement).offsetTop}px`)
    expect(lab.classes()).toContain('router-link-active')
    expect(library.classes()).not.toContain('router-link-active')
    await router.push('/instance/demo')
    await waitFor(() => library.classes().includes('subpage-active'))
    expect(slider.element.style.opacity).toBe('1')
    await router.push('/library-other')
    await waitFor(() => slider.element.style.opacity === '0')
    expect(library.classes()).not.toContain('router-link-active')
    expect(library.classes()).not.toContain('subpage-active')
})

it('honors an explicit primary predicate instead of the router default when both apply', async () => {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/browse', component: { render: () => null } }],
    })
    await router.push('/browse?i=demo')
    const wrapper = await mountThemed(
        NavButton,
        {
            to: '/browse',
            isPrimary: (route: RouteLocationNormalizedLoaded) => !route.query.i,
            isSubpage: (route: RouteLocationNormalizedLoaded) => !!route.query.i,
        },
        'dark',
        { global: { plugins: [router] } },
    )
    cleanup.push(() => wrapper.unmount())
    expect(wrapper.classes()).not.toContain('router-link-active')
    expect(wrapper.classes()).toContain('subpage-active')
    await wrapper.setProps({ disabled: true })
    expect(wrapper.element.tagName).toBe('BUTTON')
    expect(wrapper.classes()).not.toContain('router-link-active')
    expect(wrapper.classes()).toContain('subpage-active')
})

function basicRouter() {
    return createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/library', component: { render: () => null } },
            { path: '/lab', component: { render: () => null } },
        ],
    })
}

async function mountBasicRail(router: ReturnType<typeof basicRouter>) {
    const Harness = defineComponent({
        setup: () => () =>
            h(
                NavRail,
                {},
                {
                    default: () => [
                        h(NavButton, { to: '/library' }, () => 'Library'),
                        h(NavButton, { to: '/lab' }, () => 'Lab'),
                    ],
                },
            ),
    })
    const wrapper = await mountThemed(Harness, {}, 'dark', { global: { plugins: [router] } })
    cleanup.push(() => wrapper.unmount())
    await waitFor(() => wrapper.get<HTMLElement>('.nav-rail-slider').element.style.opacity === '1')
    return wrapper
}

function pointerDown(el: Element) {
    el.dispatchEvent(
        new PointerEvent('pointerdown', {
            bubbles: true,
            button: 0,
            pointerType: 'mouse',
            isPrimary: true,
        }),
    )
}

it('moves the slider on pointerdown before the router commits', async () => {
    let release!: () => void
    let hold = false
    const router = basicRouter()
    router.beforeEach((to) => {
        if (hold && to.path === '/lab') {
            return new Promise<void>((resolve) => {
                release = resolve
            })
        }
    })
    await router.push('/library')
    const wrapper = await mountBasicRail(router)
    const library = wrapper.get('a[href="/library"]')
    const lab = wrapper.get('a[href="/lab"]')
    const slider = wrapper.get<HTMLElement>('.nav-rail-slider')

    hold = true
    pointerDown(lab.element)
    await nextTick()

    expect(router.currentRoute.value.path).toBe('/library')
    await waitFor(() => slider.element.style.top === `${(lab.element as HTMLElement).offsetTop}px`)

    await lab.trigger('click', { button: 0 })
    await flushPromises()
    release()
    await flushPromises()
    await waitFor(() => router.currentRoute.value.path === '/lab')
    await nextTick()

    expect(slider.element.style.top).toBe(`${(lab.element as HTMLElement).offsetTop}px`)
    expect(library.classes()).not.toContain('router-link-active')
})

it('rolls the slider back when navigation is aborted', async () => {
    const router = basicRouter()
    router.beforeEach((to) => (to.path === '/lab' ? false : undefined))
    await router.push('/library')
    const wrapper = await mountBasicRail(router)
    const library = wrapper.get('a[href="/library"]')
    const lab = wrapper.get('a[href="/lab"]')
    const slider = wrapper.get<HTMLElement>('.nav-rail-slider')

    pointerDown(lab.element)
    await nextTick()
    await waitFor(() => slider.element.style.top === `${(lab.element as HTMLElement).offsetTop}px`)

    await lab.trigger('click', { button: 0 })
    await flushPromises()

    await waitFor(
        () => slider.element.style.top === `${(library.element as HTMLElement).offsetTop}px`,
    )
    expect(router.currentRoute.value.path).toBe('/library')
    expect(library.classes()).toContain('router-link-active')
})

it('rolls back a pointerdown that is released outside the target', async () => {
    const router = basicRouter()
    await router.push('/library')
    const wrapper = await mountBasicRail(router)
    const library = wrapper.get('a[href="/library"]')
    const lab = wrapper.get('a[href="/lab"]')
    const slider = wrapper.get<HTMLElement>('.nav-rail-slider')

    pointerDown(lab.element)
    await nextTick()
    await waitFor(() => slider.element.style.top === `${(lab.element as HTMLElement).offsetTop}px`)

    document.body.dispatchEvent(new PointerEvent('pointerup', { bubbles: true }))
    await flushPromises()

    await waitFor(
        () => slider.element.style.top === `${(library.element as HTMLElement).offsetTop}px`,
    )
    expect(router.currentRoute.value.path).toBe('/library')
})

it('ignores modifier pointerdowns', async () => {
    const router = basicRouter()
    await router.push('/library')
    const wrapper = await mountBasicRail(router)
    const library = wrapper.get('a[href="/library"]')
    const lab = wrapper.get('a[href="/lab"]')
    const slider = wrapper.get<HTMLElement>('.nav-rail-slider')
    const before = slider.element.style.top

    lab.element.dispatchEvent(
        new PointerEvent('pointerdown', {
            bubbles: true,
            button: 0,
            pointerType: 'mouse',
            isPrimary: true,
            ctrlKey: true,
        }),
    )
    await nextTick()

    expect(slider.element.style.top).toBe(before)
    expect(library.classes()).toContain('router-link-active')
})
