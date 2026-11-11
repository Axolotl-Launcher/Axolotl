import { describe, expect, it } from 'vitest'

import SelectAllButton from '../layouts/shared/browse-tab/components/SelectAllButton.vue'
import { assertTokensLoaded, mountThemed } from './visual-harness'

describe('select all button', () => {
    it('loads the token layer', () => {
        assertTokensLoaded()
    })

    it('renders the candidate count and emits select', async () => {
        const wrapper = await mountThemed(SelectAllButton, { count: 3 }, 'dark')

        expect(wrapper.text()).toContain('Select all (3)')
        await wrapper.find('button').trigger('click')

        expect(wrapper.emitted('select')).toHaveLength(1)
    })

    it('shows progress and disables the action while selecting', async () => {
        const wrapper = await mountThemed(
            SelectAllButton,
            { count: 3, progress: { completed: 1, total: 3 } },
            'dark',
        )

        expect(wrapper.text()).toContain('Selecting 1/3')
        expect(wrapper.find('button').attributes('disabled')).toBeDefined()
    })

    it('renders nothing when there are no candidates', async () => {
        const wrapper = await mountThemed(SelectAllButton, { count: 0 }, 'dark')

        expect(wrapper.find('button').exists()).toBe(false)
    })
})
