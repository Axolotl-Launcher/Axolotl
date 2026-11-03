import { flushPromises, mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import { analyzeAreaOres, type AreaOreResult } from '@/lab/seed-map/area-analysis'

import SeedMapAreaAnalysis from './SeedMapAreaAnalysis.vue'

vi.mock('@/lab/seed-map/area-analysis', async (original) => ({
    ...(await original<typeof import('@/lab/seed-map/area-analysis')>()),
    analyzeAreaOres: vi.fn(),
}))
vi.mock('@/lab/seed-map/structure-labels', () => ({ createSeedMapLabels: () => ({}) }))
vi.mock('@modrinth/assets', () => ({ XIcon: { render: () => null } }))
vi.mock('@modrinth/ui', () => ({
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    StyledInput: { render: () => null },
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        locale: { value: 'en' },
        formatMessage: (message: { defaultMessage: string }, values?: Record<string, unknown>) =>
            new IntlMessageFormat(message.defaultMessage, 'en').format(values) as string,
    }),
}))

it('shows mineral counts while searching and keeps them, marked incomplete, after canceling', async () => {
    let publish!: (result: AreaOreResult) => void
    vi.mocked(analyzeAreaOres).mockImplementation(
        (_context, _area, _kinds, _min, _max, signal, _progress, _scan, onPartial) =>
            new Promise((_resolve, reject) => {
                publish = onPartial!
                signal.addEventListener('abort', () => reject(signal.reason), { once: true })
            }),
    )
    const wrapper = mount(SeedMapAreaAnalysis, {
        props: {
            area: { minX: 0, minZ: 0, maxX: 15, maxZ: 15 },
            context: { seed: '1', edition: 'java', version: '1.21', dimension: 'overworld' },
            ores: ['iron'],
            yRange: [-64, 320] as const,
            featureMask: 0,
            featureLabel: (kind: string) => kind,
            oreLabel: (kind: string) => kind,
        },
    })
    try {
        const button = (label: string) =>
            wrapper.findAll('button').find((candidate) => candidate.text() === label)!
        await button('Count mineral blocks').trigger('click')
        await flushPromises()
        publish({ counts: [{ ore: 'iron', count: 7 }], truncated: false })
        await new Promise(requestAnimationFrame)
        await flushPromises()
        expect(wrapper.text()).toContain('Results so far')
        expect(wrapper.find('tbody').text()).toContain('7')

        await button('Cancel').trigger('click')
        await flushPromises()
        expect(wrapper.find('tbody').text()).toContain('7')
        expect(wrapper.text()).toContain('cover only the part of the area searched')
        expect(wrapper.text()).not.toContain('No mineral candidates were found')
    } finally {
        wrapper.unmount()
    }
})
