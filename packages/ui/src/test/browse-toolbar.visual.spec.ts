import { afterEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { computed, defineComponent, h, ref, shallowRef } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import Button from '../components/base/buttons/Button.vue'
import PopoutMenu from '../components/base/PopoutMenu.vue'
import BrowsePageLayout from '../layouts/shared/browse-tab/layout.vue'
import {
    type BrowseManagerContext,
    provideBrowseManager,
} from '../layouts/shared/browse-tab/providers/browse-manager'
import { mountThemed, waitFor } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn()),
)

async function mountBrowseToolbar() {
    const debug = vi.spyOn(console, 'debug').mockImplementation(() => {})
    cleanup.push(() => debug.mockRestore())
    const teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.append(teleports)
    cleanup.push(() => teleports.remove())
    const host = document.createElement('div')
    // Reserve the desktop app's 64px navigation rail and 300px sidebar.
    host.style.cssText =
        'position:absolute;left:64px;top:48px;width:calc(100vw - 364px);box-sizing:border-box;padding:24px'
    document.body.append(host)
    cleanup.push(() => host.remove())
    const query = ref('')
    const translate = vi.fn()
    const chooseInstance = vi.fn()
    const source = ref('All sources')
    const sort = { name: 'relevance', display: 'Relevance' }
    const context: BrowseManagerContext = {
        variant: 'app',
        tags: ref({ gameVersions: [], loaders: [], categories: [] }),
        projectType: ref('mod'),
        query,
        filters: computed(() => []),
        currentFilters: ref([]),
        toggledGroups: ref([]),
        overriddenProvidedFilterTypes: ref([]),
        serverFilterTypes: computed(() => []),
        serverCurrentFilters: ref([]),
        serverToggledGroups: ref([]),
        effectiveSortTypes: computed(() => [sort]),
        effectiveCurrentSortType: ref(sort),
        loading: ref(false),
        projectHits: shallowRef([]),
        serverHits: shallowRef([]),
        totalHits: ref(0),
        pageCount: computed(() => 274),
        maxResults: ref(20),
        currentPage: ref(1),
        isServerType: computed(() => false),
        effectiveLayout: computed(() => 'list'),
        deprioritizedTags: computed(() => []),
        excludeLoaders: computed(() => false),
        refreshSearch: vi.fn(async () => {}),
        setPage: vi.fn(async () => {}),
        clearSearch: () => {
            query.value = ''
        },
        onFilterChange: vi.fn(),
        getProjectLink: () => '/',
        getServerProjectLink: () => '/',
        selectableProjectTypes: computed(() =>
            ['Mods', 'Resource packs', 'Worlds', 'Shaders', 'Favorites'].map((label, index) => ({
                label,
                href: `/browse/${index}`,
            })),
        ),
        showProjectTypeTabs: computed(() => true),
    }
    const Harness = defineComponent({
        setup() {
            provideBrowseManager(context)
            return () =>
                h(
                    BrowsePageLayout,
                    {},
                    {
                        'nav-tabs-actions': () =>
                            h(
                                Button,
                                {
                                    type: 'quiet',
                                    size: 'xl',
                                    'data-testid': 'translate',
                                    onClick: translate,
                                },
                                () => 'Show original',
                            ),
                        'search-bar-actions': () => [
                            h(
                                Button,
                                {
                                    size: 'md',
                                    class: 'flex min-w-0 items-center gap-2',
                                    'data-testid': 'instance',
                                    onClick: chooseInstance,
                                },
                                () => [
                                    h('span', { class: 'size-5 shrink-0' }, '◇'),
                                    h(
                                        'span',
                                        { class: 'max-w-40 truncate' },
                                        'A very long Fabric survival instance name',
                                    ),
                                    h('span', { class: 'size-4 shrink-0' }, '⌄'),
                                ],
                            ),
                            h(
                                PopoutMenu,
                                { placement: 'bottom-end' },
                                {
                                    trigger: () =>
                                        h(
                                            Button,
                                            {
                                                size: 'md',
                                                'data-testid': 'source',
                                            },
                                            () => source.value,
                                        ),
                                    menu: () =>
                                        h(
                                            Button,
                                            {
                                                'data-testid': 'source-option',
                                                onClick: () => {
                                                    source.value = 'Modrinth'
                                                },
                                            },
                                            () => 'Modrinth',
                                        ),
                                },
                            ),
                        ],
                    },
                )
        },
    })
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/browse/:id', component: { render: () => null } }],
    })
    await router.push('/browse/0')
    await router.isReady()
    const wrapper = await mountThemed(Harness, {}, 'light', {
        attachTo: host,
        global: { plugins: [router], stubs: { SearchFilterControl: true } },
    })
    cleanup.push(() => wrapper.unmount())
    return { wrapper, host, query, translate, chooseInstance }
}

it('keeps browsing controls usable when the content column shrinks and expands', async () => {
    const { wrapper, host, query, translate, chooseInstance } = await mountBrowseToolbar()
    const input = wrapper.get<HTMLInputElement>('input[type="text"]')
    const translation = wrapper.get<HTMLElement>('[data-testid="translate"]')
    const instance = wrapper.get<HTMLElement>('[data-testid="instance"]')
    const source = wrapper.get<HTMLElement>('[data-testid="source"]')
    const tabs = wrapper.get('nav')

    for (const width of [1440, 1100, 864, 1440]) {
        await page.viewport(width, 900)
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
        const bounds = host.getBoundingClientRect()
        for (const control of [input, translation, instance, source, tabs]) {
            const rect = control.element.getBoundingClientRect()
            expect(rect.width).toBeGreaterThan(0)
            expect(rect.left).toBeGreaterThanOrEqual(bounds.left - 1)
            expect(rect.right).toBeLessThanOrEqual(bounds.right + 1)
        }
        expect(host.scrollWidth).toBeLessThanOrEqual(host.clientWidth)
        expect(input.element.getBoundingClientRect().width).toBeGreaterThanOrEqual(240)

        if (width === 864) {
            expect(instance.element.getBoundingClientRect().top).toBeGreaterThanOrEqual(
                input.element.getBoundingClientRect().bottom,
            )
            expect(translation.element.getBoundingClientRect().top).toBeGreaterThanOrEqual(
                tabs.element.getBoundingClientRect().bottom,
            )
            await page.elementLocator(input.element).fill('Just Enough Items')
            expect(query.value).toBe('Just Enough Items')
            await page.elementLocator(translation.element).click()
            await page.elementLocator(instance.element).click()
            expect(translate).toHaveBeenCalledTimes(1)
            expect(chooseInstance).toHaveBeenCalledTimes(1)
            await page.elementLocator(source.element).click()
            await waitFor(() => !!document.querySelector('[data-testid="source-option"]'))
            await page.getByTestId('source-option').click()
            expect(source.text()).toBe('Modrinth')
            await page.elementLocator(source.element).click()
        } else if (width === 1440) {
            const search = input.element.getBoundingClientRect()
            const action = source.element.getBoundingClientRect()
            expect(action.top).toBeLessThan(search.bottom)
            expect(action.bottom).toBeGreaterThan(search.top)
        }
    }
})
