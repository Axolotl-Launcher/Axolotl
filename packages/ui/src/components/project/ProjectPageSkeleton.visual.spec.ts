import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'

import ProjectPageSkeleton from './ProjectPageSkeleton.vue'

it('renders a known title while keeping its loading layout', () => {
    const wrapper = mount(ProjectPageSkeleton, { props: { title: 'Known project' } })
    try {
        expect(wrapper.get('h1').text()).toBe('Known project')
        expect(wrapper.attributes('aria-hidden')).toBeUndefined()
    } finally {
        wrapper.unmount()
    }
})
