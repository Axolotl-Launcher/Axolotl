import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import { mountThemed } from '../../../test/visual-harness'
import ProjectCard from './ProjectCard.vue'

it('keeps compact sources on one bounded line and preserves the card action', async () => {
    const open = vi.fn()
    const wrapper = await mountThemed(
        ProjectCard,
        {
            layout: 'compact',
            title: 'Example',
            link: open,
            providers: ['modrinth', 'curseforge', 'mcarchive', 'planet_minecraft'],
        },
        'dark',
    )
    try {
        await flushPromises()
        const source = wrapper.get<HTMLElement>('.project-card--compact__provider')
        expect(source.text()).toBe('Modrinth, CurseForge, MCArchive, Planet Minecraft')
        expect(getComputedStyle(source.element).whiteSpace).toBe('nowrap')
        expect(getComputedStyle(source.element).textOverflow).toBe('ellipsis')
        expect(getComputedStyle(source.element).pointerEvents).toBe('all')
        await source.trigger('click')
        expect(open).toHaveBeenCalledOnce()
    } finally {
        wrapper.unmount()
    }
})
