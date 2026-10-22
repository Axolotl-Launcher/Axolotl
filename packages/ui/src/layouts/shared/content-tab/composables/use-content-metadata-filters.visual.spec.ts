import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, ref } from 'vue'

import type { ContentItem } from '../types'
import { useContentMetadataFilters } from './use-content-metadata-filters'

vi.mock('#ui/composables/i18n', () => ({
    defineMessages: (value: unknown) => value,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

it('filters actual platforms and ownership independently, including recognized local files', async () => {
    const items = ref([
        {
            id: 'mr-user',
            origin_provider: 'modrinth',
            instanceOwnershipKind: 'user_added',
            source_kind: 'local',
        },
        {
            id: 'cf-pack',
            origin_provider: 'curseforge',
            instanceOwnershipKind: 'pack_managed',
            source_kind: 'imported_modpack',
            provider_refs: [{ provider: 'modrinth' }],
        },
        {
            id: 'mr-pack',
            origin_provider: 'modrinth',
            instanceOwnershipKind: 'pack_managed',
            source_kind: 'imported_modpack',
        },
        {
            id: 'recognized-local',
            origin_provider: null,
            instanceOwnershipKind: 'local_discovered',
            provider_refs: [{ provider: 'modrinth' }],
        },
    ] as ContentItem[])
    let filters!: ReturnType<typeof useContentMetadataFilters>
    const wrapper = mount(
        defineComponent({
            setup() {
                filters = useContentMetadataFilters(items)
                return () => h('div')
            },
        }),
    )
    try {
        expect(
            filters.metadataFilterCategories.value
                .find((category) => category.key === 'source')
                ?.options.map((option) => option.value),
        ).toEqual(['modrinth', 'curseforge', 'unknown'])
        filters.setCategorySelection('source', ['modrinth'])
        expect(filters.applyMetadataFilters(items.value).map((item) => item.id)).toEqual([
            'mr-user',
            'mr-pack',
        ])
        filters.setCategorySelection('ownership', ['pack_managed'])
        expect(filters.applyMetadataFilters(items.value).map((item) => item.id)).toEqual([
            'mr-pack',
        ])
        items.value[1] = { ...items.value[1], origin_provider: 'modrinth' }
        await nextTick()
        expect(filters.applyMetadataFilters(items.value).map((item) => item.id)).toEqual([
            'cf-pack',
            'mr-pack',
        ])
        filters.setExcludedValues({})
        expect(filters.applyMetadataFilters(items.value)).toHaveLength(4)
    } finally {
        wrapper.unmount()
    }
})
