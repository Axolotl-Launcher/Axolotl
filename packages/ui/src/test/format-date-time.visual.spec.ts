import { afterEach, expect, it } from 'vitest'
import { defineComponent, h } from 'vue'

import { useFormatDateTime } from '../composables/format-date-time'
import { mountThemed } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).forEach((fn) => fn()))

it('returns an empty string for missing or invalid dates', async () => {
    const results: string[] = []
    const wrapper = await mountThemed(
        defineComponent({
            setup() {
                const format = useFormatDateTime({ dateStyle: 'medium' })
                results.push(format(undefined), format(null), format('not-a-date'), format(0))
                return () => h('span')
            },
        }),
        {},
        'dark',
    )
    cleanup.push(() => wrapper.unmount())

    expect(results.slice(0, 3)).toEqual(['', '', ''])
    expect(results[3]).toBe('Jan 1, 1970')
})
