import { expect, it } from 'vitest'

import ProjectCard from '../components/project/card/ProjectCard.vue'
import ProjectCardDate from '../components/project/card/ProjectCardDate.vue'
import { mountThemed } from './visual-harness'

for (const layout of ['list', 'compact', 'grid', 'gallery'] as const) {
    it(`shows both source labels without source action buttons in ${layout}`, async () => {
        const wrapper = await mountThemed(
            ProjectCard,
            {
                title: 'Shared resource',
                layout,
                provider: 'modrinth',
                providers: ['modrinth', 'curseforge', 'modrinth'],
            },
            'dark',
        )
        expect(wrapper.text().match(/Modrinth/g)).toHaveLength(1)
        expect(wrapper.text().match(/CurseForge/g)).toHaveLength(1)
        expect(wrapper.findAll('button').some((button) => button.text() === 'CurseForge')).toBe(
            false,
        )
        wrapper.unmount()
    })
}

for (const type of [
    'created',
    'projectPublished',
    'firstRelease',
    'projectModified',
    'releaseUpdated',
] as const) {
    it(`keeps ${type} dates visible in compact resource cards`, async () => {
        const wrapper = await mountThemed(
            ProjectCard,
            {
                title: 'Sample',
                layout: 'compact',
                displayedDate: type,
                datePublished: '2020-01-01T00:00:00Z',
                dateUpdated: '2025-01-01T00:00:00Z',
            },
            'dark',
        )
        const date = wrapper.findComponent(ProjectCardDate)
        expect(date.exists()).toBe(true)
        expect(date.props('type')).toBe(type)
        expect(date.props('date').getUTCFullYear()).toBe(
            type === 'projectModified' || type === 'releaseUpdated' ? 2025 : 2020,
        )
        wrapper.unmount()
    })
}
