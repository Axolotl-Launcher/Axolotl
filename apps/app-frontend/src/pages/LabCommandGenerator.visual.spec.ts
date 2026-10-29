import { config, flushPromises, mount } from '@vue/test-utils'
import { NbtTag } from 'deepslate/nbt'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import CommandGeneratorArgument from '@/components/lab/command-generator/CommandGeneratorArgument.vue'
import CommandGeneratorItemNbt from '@/components/lab/command-generator/CommandGeneratorItemNbt.vue'
import CommandGeneratorNbtAdvanced from '@/components/lab/command-generator/CommandGeneratorNbtAdvanced.vue'
import CommandGeneratorNbtEditor from '@/components/lab/command-generator/CommandGeneratorNbtEditor.vue'
import { initializeRegistry } from '@/lab/command-generator/commands/registry.ts'
import { buildCommand } from '@/lab/command-generator/generator.ts'
import { loadCommandsForVersion } from '@/lab/command-generator/loader.ts'
import { parseNbt } from '@/lab/command-generator/nbt-tree.ts'
import { saveUserPreset } from '@/lab/command-generator/presets.ts'

import LabCommandGenerator from './LabCommandGenerator.vue'

vi.mock('@/components/lab/command-generator/CommandGeneratorItemPickerModal.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        commonMessages: { clearButton: { id: 'button.clear', defaultMessage: 'Clear' } },
        Button: defineComponent({
            props: { label: String, iconOnly: Boolean, nativeType: String },
            setup:
                (props, { slots }) =>
                () =>
                    h(
                        'button',
                        {
                            'aria-label': props.iconOnly ? props.label : undefined,
                            type: props.nativeType ?? 'button',
                        },
                        slots.default?.(),
                    ),
        }),
        Toggle: defineComponent({ render: () => null }),
        StyledInput: defineComponent({
            props: ['modelValue'],
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
        Combobox: defineComponent({
            props: ['modelValue', 'options'],
            emits: ['update:modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h(
                        'select',
                        {
                            value: props.modelValue,
                            onChange: (event: Event) =>
                                emit(
                                    'update:modelValue',
                                    (event.target as HTMLSelectElement).value,
                                ),
                        },
                        (props.options as Array<{ value: string; label: string }>).map((option) =>
                            h('option', { value: option.value }, option.label),
                        ),
                    ),
        }),
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (message: { id: string; defaultMessage?: string }) =>
                message.defaultMessage ?? message.id,
        }),
    }
})

const wrappers: Array<{ unmount: () => void }> = []
beforeEach(() => {
    localStorage.removeItem('axolotl:command-generator:user-presets')
    config.global.directives.tooltip = () => {}
})
afterEach(() => wrappers.splice(0).forEach((wrapper) => wrapper.unmount()))

function mountCoordinates(commandId: string, name: string, initial?: string) {
    const arg = loadCommandsForVersion('1.16')
        .find((command) => command.id === commandId)!
        .arguments.find((argument) => argument.name === name)!
    const value = ref<string | number | boolean | undefined>(initial)
    const wrapper = mount(
        defineComponent({
            setup: () => () =>
                h(CommandGeneratorArgument, {
                    arg,
                    modelValue: value.value,
                    'onUpdate:modelValue': (next: string | number | boolean) => {
                        value.value = next
                    },
                }),
        }),
    )
    wrappers.push(wrapper)
    return { wrapper, value }
}

it('sets all three axes to absolute coordinates without resetting previous axes', async () => {
    const { wrapper, value } = mountCoordinates('setblock', 'pos', '~ ~ ~')
    for (const [index, coordinate] of ['10', '64', '-20'].entries()) {
        await wrapper.findAll('.coordinate-mode')[index].trigger('click')
        await wrapper.findAll('input')[index].setValue(coordinate)
    }
    expect(value.value).toBe('10 64 -20')
})

it('enters and leaves local coordinates as a complete tuple', async () => {
    const { wrapper, value } = mountCoordinates('setblock', 'pos', '~1 64 ~-2')
    await wrapper.findAll('.coordinate-mode')[1].trigger('click')
    expect(value.value).toBe('^1 ^64 ^-2')
    await wrapper.findAll('.coordinate-mode')[0].trigger('click')
    expect(value.value).toBe('~1 ~64 ~-2')
})

it('omits untouched optional coordinates and can clear them after editing', async () => {
    const { wrapper, value } = mountCoordinates('forceload', 'to')
    initializeRegistry('1.16')
    const command = () =>
        buildCommand('forceload', { action: 'query', from: '0 0', to: value.value ?? '' })
    expect(command()).toBe('/forceload query 0 0')
    await wrapper.findAll('input')[0].setValue('16')
    expect(value.value).toBe('~16 ~')
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Clear')!
        .trigger('click')
    expect(command()).toBe('/forceload query 0 0')
    expect(
        wrapper.findAll('input').map((input) => (input.element as HTMLInputElement).value),
    ).toEqual(['', ''])
    await wrapper.findAll('input')[1].setValue('32')
    expect(value.value).toBe('~ ~32')
})

it('restores both entity and coordinate destinations in an already mounted editor', async () => {
    const { wrapper, value } = mountCoordinates('teleport', 'destination', '~ ~ ~')
    value.value = '@p'
    await flushPromises()
    expect(wrapper.find('.coordinate-mode').exists()).toBe(false)
    expect((wrapper.get('input').element as HTMLInputElement).value).toBe('@p')
    await wrapper.get('input').setValue('')
    expect(wrapper.find('.coordinate-mode').exists()).toBe(false)
    await wrapper.get('input').setValue('1')
    expect(wrapper.find('.coordinate-mode').exists()).toBe(false)
    await wrapper.get('input').setValue('123Alex')
    expect((wrapper.get('input').element as HTMLInputElement).value).toBe('123Alex')
    expect(wrapper.find('.coordinate-mode').exists()).toBe(false)
    value.value = '10 64 -20'
    await flushPromises()
    expect(wrapper.findAll('.coordinate-mode').map((button) => button.text())).toEqual([
        '=',
        '=',
        '=',
    ])
    await wrapper.findAll('input')[1].setValue('70')
    expect(value.value).toBe('10 70 -20')
})

it.each([
    ['forceload', 'from'],
    ['spreadplayers', 'center'],
])('offers only world coordinate modes for %s', async (commandId, name) => {
    const { wrapper, value } = mountCoordinates(commandId, name, '~1 ~2')
    await wrapper.findAll('.coordinate-mode')[0].trigger('click')
    await wrapper.findAll('.coordinate-mode')[1].trigger('click')
    expect(value.value).toBe('1 2')
    await wrapper.findAll('.coordinate-mode')[0].trigger('click')
    await wrapper.findAll('.coordinate-mode')[1].trigger('click')
    expect(value.value).toBe('~1 ~2')
})

it('loads and edits advanced NBT without emitting an empty or wrapped replacement on load', async () => {
    const wrapper = mount(CommandGeneratorNbtAdvanced, { props: { modelValue: '{Health:20f}' } })
    wrappers.push(wrapper)
    await flushPromises()
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
    expect((wrapper.get('.nbt-name').element as HTMLInputElement).value).toBe('Health')
    await wrapper.setProps({ modelValue: '{Health:5f}' })
    expect((wrapper.get('.nbt-value').element as HTMLInputElement).value).toBe('5')
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
    await wrapper.get('.nbt-value').setValue('10')
    const output = wrapper.emitted('update:modelValue')!.at(-1)![0] as string
    expect(NbtTag.fromString(output).equals(NbtTag.fromString('{Health:10f}'))).toBe(true)
    await wrapper.setProps({ modelValue: '' })
    expect(wrapper.findAll('.nbt-row')).toHaveLength(0)
})

it.each(['1.16', '1.21'])('escapes nested JSON in names and lore for %s', async (version) => {
    const wrapper = mount(CommandGeneratorItemNbt, { props: { version, modelValue: '' } })
    wrappers.push(wrapper)
    const text = 'Sword "A" \\ path Bob\'s'
    await wrapper.get('input.text-input').setValue(text)
    await wrapper.get('.lore-list .ghost-add').trigger('click')
    await wrapper.get('.lore-row input').setValue(text)
    const output = wrapper.emitted('update:modelValue')!.at(-1)![0] as string
    const parsed = parseNbt(output)
    const display = parsed.components
        ? parsed.nodes
        : parsed.nodes.find((n) => n.name === 'display')!.children!
    const name = display.find((n) => n.name === (parsed.components ? 'custom_name' : 'Name'))!
    const lore = display.find((n) => n.name === (parsed.components ? 'lore' : 'Lore'))!.children![0]
    expect(JSON.parse(String(name.value)).text).toBe(text)
    expect(JSON.parse(String(lore.value)).text).toBe(text)
    await wrapper.setProps({ modelValue: output })
    expect((wrapper.get('input.text-input').element as HTMLInputElement).value).toBe(text)
})

it.each([
    ['1.16', '{display:{Name:\'{"text":"Sword"}\'},CustomModelData:123}'],
    ['1.21', '[custom_name=\'{"text":"Sword"}\',custom_data={test:1b}]'],
])(
    'preserves item data when switching between simple and advanced modes for %s',
    async (version, input) => {
        const value = ref(input)
        const host = defineComponent({
            setup: () => () =>
                h(CommandGeneratorNbtEditor, {
                    kind: 'item',
                    version,
                    modelValue: value.value,
                    'onUpdate:modelValue': (next: string) => {
                        value.value = next
                    },
                }),
        })
        const wrapper = mount(host)
        wrappers.push(wrapper)
        await flushPromises()
        await wrapper.findAll('.nbt-mode-tab')[1].trigger('click')
        await flushPromises()
        expect(value.value).toBe(input)
        await wrapper.findAll('.nbt-mode-tab')[0].trigger('click')
        await flushPromises()
        expect(value.value).toBe(input)
        expect((wrapper.get('input.text-input').element as HTMLInputElement).value).toBe('Sword')
        await wrapper.get('input.text-input').setValue('Changed name')
        const parsed = parseNbt(value.value)
        expect(
            parsed.nodes.some(
                (node) => node.name === (parsed.components ? 'custom_data' : 'CustomModelData'),
            ),
        ).toBe(true)
    },
)

it.each(['give', 'execute'])(
    'opens a %s preset from another version and generates the restored command',
    async (commandId) => {
        saveUserPreset(
            'Older preset',
            commandId,
            '1.16',
            { target: '@s', item: 'minecraft:stone', count: 3 },
            {
                executeState: {
                    chain: [{ subcommandId: 'as', values: { target: '@a' } }],
                    finalCommand: 'say hello',
                },
            },
        )
        const wrapper = mount(LabCommandGenerator)
        wrappers.push(wrapper)
        await flushPromises()
        await wrapper.get('.global-presets-button').trigger('click')
        await wrapper.get('.global-preset-item').trigger('click')
        await flushPromises()
        expect((wrapper.findAll('select')[0].element as HTMLSelectElement).value).toBe('1.16')
        expect((wrapper.findAll('select')[3].element as HTMLSelectElement).value).toBe(commandId)
        await wrapper.get('.action-bar button').trigger('click')
        expect(wrapper.get('.result-code').text()).toBe(
            commandId === 'give' ? '/give @s minecraft:stone 3' : '/execute as @a run say hello',
        )
    },
)

it('loads and edits an entity destination from a saved teleport preset', async () => {
    saveUserPreset('Entity destination', 'teleport', '1.16', {
        target: '@s',
        destination: '@p',
    })
    const wrapper = mount(LabCommandGenerator)
    wrappers.push(wrapper)
    await flushPromises()
    await wrapper.get('.global-presets-button').trigger('click')
    await wrapper.get('.global-preset-item').trigger('click')
    await flushPromises()
    const destination = wrapper.get('[data-testid="arg-card-destination"]')
    expect(destination.find('.coordinate-mode').exists()).toBe(false)
    expect((destination.get('input').element as HTMLInputElement).value).toBe('@p')
    await destination.get('input').setValue('Steve')
    await wrapper.get('.action-bar button').trigger('click')
    expect(wrapper.get('.result-code').text()).toBe('/teleport @s Steve')
})

it('updates effect fields and output when switching from give to clear', async () => {
    const wrapper = mount(LabCommandGenerator)
    wrappers.push(wrapper)
    await flushPromises()
    await wrapper.findAll('select')[3].setValue('effect')
    const card = (name: string) => wrapper.findAll(`[data-testid="arg-card-${name}"]`)[0]!
    await card('target').get('input').setValue('@s')
    await card('effect').get('input').setValue('minecraft:speed')
    expect(card('duration')).toBeDefined()
    await card('action').get('select').setValue('clear')
    expect(card('duration')).toBeUndefined()
    expect(card('amplifier')).toBeUndefined()
    await wrapper.get('.action-bar button').trigger('click')
    expect(wrapper.get('.result-code').text()).toBe('/effect clear @s minecraft:speed')
})

it('saves and deletes a preset without deleting the current command inputs', async () => {
    const wrapper = mount(LabCommandGenerator)
    wrappers.push(wrapper)
    await flushPromises()
    await wrapper.findAll('select')[3].setValue('give')
    const fields = wrapper.findAll('.argument-card input')
    await fields[0].setValue('@s')
    await fields[1].setValue('minecraft:stone')
    await wrapper
        .findAll('aside button')
        .find((button) => button.text().includes('Save as preset'))!
        .trigger('click')
    await wrapper.get('form input').setValue('My stone command')
    await wrapper.get('form').trigger('submit')
    await wrapper.get('.global-presets-button').trigger('click')
    expect(wrapper.get('.global-preset-item').text()).toContain('My stone command')
    await wrapper.get('button[aria-label="Delete preset"]').trigger('click')
    expect(wrapper.find('.global-preset-item').exists()).toBe(false)
    expect((fields[1].element as HTMLInputElement).value).toBe('minecraft:stone')
    await wrapper.get('.action-bar button').trigger('click')
    expect(wrapper.get('.result-code').text()).toBe('/give @s minecraft:stone 1')
    await fields[1].setValue('minecraft:diamond')
    expect(wrapper.find('.result-code').exists()).toBe(false)
})

it.each(['26.1', '26.2', '26.3'])(
    'round trips inline item names and lore for %s',
    async (version) => {
        const name = 'Sword "A" \\ path Bob\'s'
        const input = `[custom_name={text:"Original"},custom_data={test:1b}]`
        const wrapper = mount(CommandGeneratorItemNbt, { props: { version, modelValue: input } })
        wrappers.push(wrapper)
        await wrapper.get('input.text-input').setValue(name)
        await wrapper.get('.lore-list .ghost-add').trigger('click')
        await wrapper.get('.lore-row input').setValue(name)
        const output = wrapper.emitted('update:modelValue')!.at(-1)![0] as string
        const nodes = parseNbt(output).nodes
        expect(nodes.find((node) => node.name === 'custom_name')!.children![0].value).toBe(name)
        expect(nodes.find((node) => node.name === 'lore')!.children![0].children![0].value).toBe(
            name,
        )
        expect(nodes.find((node) => node.name === 'custom_data')).toBeDefined()
        await wrapper.setProps({ modelValue: output })
        expect((wrapper.get('input.text-input').element as HTMLInputElement).value).toBe(name)
    },
)

it('restores a 26.3 preset and exposes the new compute form', async () => {
    saveUserPreset('Computed value', 'compute', '26.3', {
        context: 'default',
        type: 'integer',
        provider: '5',
    })
    const wrapper = mount(LabCommandGenerator)
    wrappers.push(wrapper)
    await flushPromises()
    await wrapper.get('.global-presets-button').trigger('click')
    await wrapper.get('.global-preset-item').trigger('click')
    await flushPromises()
    expect((wrapper.findAll('select')[0].element as HTMLSelectElement).value).toBe('26.3')
    await wrapper.get('.action-bar button').trigger('click')
    expect(wrapper.get('.result-code').text()).toBe('/compute default integer 5')
})

it('offers modern gamerule presets without legacy camelCase choices for 26.x', async () => {
    const wrapper = mount(LabCommandGenerator)
    wrappers.push(wrapper)
    await flushPromises()
    await wrapper.findAll('select')[0].setValue('26.1')
    await wrapper.findAll('select')[3].setValue('gamerule')
    const rule = wrapper.findAll('.argument-card')[0]
    await rule.findAll('button')[0].trigger('click')
    expect((rule.get('input').element as HTMLInputElement).value).toBe('minecraft:keep_inventory')
    await wrapper.findAll('.argument-card')[1].get('input').setValue('true')
    await wrapper.get('.action-bar button').trigger('click')
    expect(wrapper.get('.result-code').text()).toBe('/gamerule minecraft:keep_inventory true')
})

it('uses component syntax when adding tags or applying advanced item presets in 26.3', async () => {
    const wrapper = mount(CommandGeneratorNbtAdvanced, {
        props: { kind: 'item', version: '26.3', modelValue: '' },
    })
    wrappers.push(wrapper)
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Add root tag')!
        .trigger('click')
    expect(String(wrapper.emitted('update:modelValue')!.at(-1)![0])).toMatch(/^\[/)
    await wrapper
        .findAll('button')
        .find((button) => button.text() === 'Enchanted item')!
        .trigger('click')
    const output = String(wrapper.emitted('update:modelValue')!.at(-1)![0])
    expect(parseNbt(output).nodes.find((node) => node.name === 'custom_name')!.type).toBe(
        'compound',
    )
})
