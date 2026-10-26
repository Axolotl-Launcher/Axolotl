import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, ref } from 'vue'

import { useSearch } from './search'

vi.mock('vue-router', () => ({
    useRoute: () => ({ query: { v: '1.21.1', g: 'categories:fabric', q: 'sodium' } }),
}))
vi.mock('../composables/i18n', async () => {
    const { ref } = await import('vue')
    return {
        defineMessage: (value: unknown) => value,
        defineMessages: (value: unknown) => value,
        useVIntl: () => ({
            locale: ref('en-US'),
            formatMessage: (value: { defaultMessage: string }) => value.defaultMessage,
        }),
    }
})

it('restores metadata-dependent URL filters exactly once after metadata arrives', () => {
    const tags = ref({ gameVersions: [], loaders: [], categories: [] })
    let state!: ReturnType<typeof useSearch>
    const wrapper = mount(
        defineComponent({
            setup() {
                state = useSearch(ref(['mod']), tags, ref([]), undefined, undefined, true)
                return () => null
            },
        }),
    )
    try {
        expect(state.currentFilters.value).toEqual([])
        Object.assign(tags.value, {
            gameVersions: [{ version: '1.21.1', version_type: 'release' }],
            loaders: [{ name: 'fabric', supported_project_types: ['mod'] }],
        })
        state.initializeQueryParams()
        expect(state.currentFilters.value).toEqual(
            expect.arrayContaining([
                expect.objectContaining({ type: 'game_version', option: '1.21.1' }),
                expect.objectContaining({ option: 'fabric' }),
            ]),
        )
        expect(state.query.value).toBe('sodium')
        const count = state.currentFilters.value.length
        state.initializeQueryParams()
        expect(state.currentFilters.value).toHaveLength(count)
    } finally {
        wrapper.unmount()
    }
})
