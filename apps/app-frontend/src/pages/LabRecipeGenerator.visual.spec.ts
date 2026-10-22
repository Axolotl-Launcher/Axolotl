import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'

import LabRecipeGenerator from './LabRecipeGenerator.vue'

const resourceLoads = vi.hoisted(() => ({
    deferred: false,
    pending: [] as { version: string; resolve: (value: unknown) => void }[],
}))

vi.mock('@/components/lab/recipe-generator/InstanceExportModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/recipe-generator/ItemPalette.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/recipe-generator/RecipeGeneratorCopyrightModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/recipe-generator/RecipeItemIcon.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/recipe-generator/RecipeSlotGrid.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/recipe-generator/TagPalette.vue', () => ({
    default: { render: () => null },
}))

vi.mock('@/lab/recipe-generator/resources', async (importOriginal) => ({
    ...(await importOriginal<typeof import('@/lab/recipe-generator/resources')>()),
    loadVersionResources: (version: string) => {
        const result = { version, items: [], itemsById: {}, vanillaTags: {} }
        if (!resourceLoads.deferred) return Promise.resolve(result)
        return new Promise((resolve) => resourceLoads.pending.push({ version, resolve }))
    },
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref } = await import('vue')
    const control = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
    return {
        Button: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('button', slots.default?.()),
        }),
        StyledInput: defineComponent({
            props: ['modelValue', 'size'],
            emits: ['update:modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h('input', {
                        value: props.modelValue,
                        onInput: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLInputElement).value),
                    }),
        }),
        Checkbox: control,
        Combobox: control,
        Toggle: control,
        NewModal: { render: () => null },
        defineMessages: (messages: unknown) => messages,
        defineMessage: (message: unknown) => message,
        injectNotificationManager: () => ({ handleError: vi.fn(), addNotification: vi.fn() }),
        useVIntl: () => ({
            locale: ref('en-US'),
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

beforeEach(() => localStorage.removeItem('axolotl.lab.recipe-generator.v1'))

it('row clone and delete operate on that row while another recipe is selected', async () => {
    const wrapper = mount(LabRecipeGenerator, {
        global: {
            directives: { tooltip: () => {} },
            provide: { replayOnboarding: async () => {} },
        },
    })
    try {
        await flushPromises()
        const group = () => wrapper.get('input[placeholder="Group"]')
        const rows = () => wrapper.findAll('.recipe-sidebar-row')
        await group().setValue('Recipe A')
        await wrapper.get('.recipe-sidebar-heading button').trigger('click')
        expect(rows()).toHaveLength(2)
        expect((group().element as HTMLInputElement).value).toBe('')
        await group().setValue('Recipe B')
        expect((group().element as HTMLInputElement).value).toBe('Recipe B')
        await rows()[0].get('.recipe-sidebar-select').trigger('click')
        expect((group().element as HTMLInputElement).value).toBe('Recipe A')
        await rows()[1].get('.recipe-row-actions button').trigger('click')
        expect(rows()).toHaveLength(3)
        expect((group().element as HTMLInputElement).value).toBe('Recipe B')
        await group().setValue('Independent copy')
        await rows()[0].get('.recipe-sidebar-select').trigger('click')
        await rows()[1].findAll('.recipe-row-actions button')[1].trigger('click')
        expect(rows()).toHaveLength(2)
        expect(rows()[0].classes()).toContain('active')
        expect((group().element as HTMLInputElement).value).toBe('Recipe A')
        await rows()[1].get('.recipe-sidebar-select').trigger('click')
        expect((group().element as HTMLInputElement).value).toBe('Independent copy')
        await rows()[1].findAll('.recipe-row-actions button')[1].trigger('click')
        expect((group().element as HTMLInputElement).value).toBe('Recipe A')
        await rows()[0].findAll('.recipe-row-actions button')[1].trigger('click')
        expect(rows()).toHaveLength(1)
        expect((group().element as HTMLInputElement).value).toBe('')
    } finally {
        wrapper.unmount()
    }
})

it('keeps resources and loading state owned by the latest selected version', async () => {
    resourceLoads.deferred = true
    resourceLoads.pending = []
    const wrapper = mount(LabRecipeGenerator, {
        global: {
            directives: { tooltip: () => {} },
            provide: { replayOnboarding: async () => {} },
        },
    })
    try {
        await flushPromises()
        expect(resourceLoads.pending.map(({ version }) => version)).toEqual(['26.3'])
        ;(wrapper.vm as { store: { selectedVersion: string } }).store.selectedVersion = '1.20'
        await flushPromises()
        expect(resourceLoads.pending.map(({ version }) => version)).toEqual(['26.3', '1.20'])
        const oldResult = {
            version: '26.3',
            items: [{ id: 'old' }],
            itemsById: {},
            vanillaTags: {},
        }
        resourceLoads.pending[0].resolve(oldResult)
        await flushPromises()
        expect(wrapper.text()).toContain('Loading version data')
        const currentResult = {
            version: '1.20',
            items: [{ id: 'current' }],
            itemsById: {},
            vanillaTags: {},
        }
        resourceLoads.pending[1].resolve(currentResult)
        await flushPromises()
        expect(wrapper.text()).not.toContain('Loading version data')
        expect((wrapper.vm as { resources: { version: string } }).resources.version).toBe('1.20')
    } finally {
        resourceLoads.deferred = false
        wrapper.unmount()
    }
})

it('keeps brewing fields across incompatible versions and restores saved projects', async () => {
    const wrapper = mount(LabRecipeGenerator, {
        global: {
            directives: { tooltip: () => {} },
            provide: { replayOnboarding: async () => {} },
        },
    })
    const vm = wrapper.vm as unknown as {
        store: {
            selectedVersion: string
            recipes: {
                recipeType: string
                brewing: { outputPotion: string }
                slots: Record<string, unknown>
            }[]
        }
        setRecipeType: (type: string) => void
        generatedJson: unknown
    }
    await flushPromises()
    vm.setRecipeType('brewing')
    vm.store.recipes[0].slots = {
        'brewing.input': { kind: 'item', id: 'minecraft:potion' },
        'brewing.reagent': { kind: 'item', id: 'minecraft:nether_wart' },
        'brewing.result': { kind: 'item', id: 'minecraft:potion', count: 2 },
    }
    vm.store.recipes[0].brewing.outputPotion = 'minecraft:awkward'
    await flushPromises()
    expect(wrapper.find('[data-onboarding-id="recipe-brewing-editor"]').exists()).toBe(true)
    expect(vm.generatedJson).toMatchObject({ type: 'minecraft:brewing', output: { count: 2 } })
    ;(vm.store.recipes[0].slots['brewing.result'] as { count: number }).count = 99
    await flushPromises()
    await wrapper.get('[data-recipe-slot="brewing.result"]').trigger('wheel', { deltaY: 1 })
    expect((vm.store.recipes[0].slots['brewing.result'] as { count: number }).count).toBe(98)
    vm.store.selectedVersion = '26.2'
    await flushPromises()
    expect(vm.store.recipes[0].recipeType).toBe('brewing')
    expect(vm.store.recipes[0].brewing.outputPotion).toBe('minecraft:awkward')
    expect(vm.generatedJson).toBeNull()
    wrapper.unmount()
    const restored = mount(LabRecipeGenerator, {
        global: {
            directives: { tooltip: () => {} },
            provide: { replayOnboarding: async () => {} },
        },
    })
    try {
        await flushPromises()
        const project = (restored.vm as unknown as { store: typeof vm.store }).store
        expect(project.selectedVersion).toBe('26.2')
        expect(project.recipes[0].recipeType).toBe('brewing')
        expect(project.recipes[0].brewing.outputPotion).toBe('minecraft:awkward')
    } finally {
        restored.unmount()
    }
})
