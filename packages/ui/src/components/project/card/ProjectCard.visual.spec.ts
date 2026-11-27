import { CheckIcon, FolderIcon, HeartIcon } from '@modrinth/assets'
import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { h } from 'vue'

import { mountThemed, waitFor } from '../../../test/visual-harness'
import { Button } from '../../base'
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

/**
 * The compact card lays its trailing cluster out as fixed grid tracks. Before
 * this guard the actions track was 5.5rem while the install button plus two
 * circular buttons need roughly 150px, so `justify-end` pushed the button
 * cluster leftwards over the download count ("12.31MB" disappearing under
 * "Installed"). Reproduce the row at a width where statistics are visible and
 * assert the two boxes never intersect.
 */
async function mountCompactRow(width: number, downloads: number) {
    const host = document.createElement('div')
    host.style.cssText = `width: ${width}px`
    document.body.append(host)

    const wrapper = await mountThemed(
        ProjectCard,
        {
            layout: 'compact',
            title: 'Alternate Current',
            summary: 'An efficient and non-locational redstone dust implementation',
            link: () => {},
            downloads,
            provider: 'modrinth',
        },
        'dark',
        {
            attachTo: host,
            slots: {
                actions: () =>
                    h('div', { class: 'flex gap-2' }, [
                        h(
                            Button,
                            {
                                color: 'green',
                                type: 'outlined',
                                size: '2xs',
                                disabled: true,
                            },
                            () => [h(CheckIcon), 'Installed'],
                        ),
                        h(
                            Button,
                            {
                                circular: true,
                                'icon-only': true,
                                type: 'quiet',
                                size: '2xs',
                                label: 'Manage groups',
                            },
                            () => h(FolderIcon),
                        ),
                        h(
                            Button,
                            {
                                circular: true,
                                'icon-only': true,
                                color: 'brand',
                                type: 'quiet',
                                size: '2xs',
                                label: 'Remove',
                            },
                            () => h(HeartIcon),
                        ),
                    ]),
            },
        },
    )

    await flushPromises()
    return { wrapper, host }
}

for (const width of [900, 560]) {
    it(`keeps the compact download count clear of the row actions at ${width}px`, async () => {
        const { wrapper, host } = await mountCompactRow(width, 12_310_000)
        try {
            const card = wrapper.get<HTMLElement>('.project-card--compact').element
            const stats = wrapper.get<HTMLElement>('.project-card--compact__downloads').element
            const actions = wrapper.get<HTMLElement>('.project-card--compact__actions').element
            // Measure the buttons themselves: the actions cell keeps its track width while an
            // over-wide cluster overflows outside it, so the cell's own rect hides the overlap.
            const cluster = actions.firstElementChild as HTMLElement

            const cardRect = card.getBoundingClientRect()
            const statsRect = stats.getBoundingClientRect()
            const clusterRect = cluster.getBoundingClientRect()

            expect(statsRect.width).toBeGreaterThan(0)
            expect(clusterRect.width).toBeGreaterThan(0)
            // The button cluster sits entirely to the right of the download count.
            expect(clusterRect.left).toBeGreaterThanOrEqual(statsRect.right - 0.5)
            // ...and entirely inside the card's content box.
            expect(clusterRect.right).toBeLessThanOrEqual(cardRect.right - 0.5)
        } finally {
            wrapper.unmount()
            host.remove()
        }
    })
}

it('keeps compact server actions on the card row instead of a clipped second row', async () => {
    const host = document.createElement('div')
    host.style.cssText = 'width: 900px'
    document.body.append(host)

    const wrapper = await mountThemed(
        ProjectCard,
        {
            layout: 'compact',
            title: 'Survival Server',
            link: () => {},
            provider: 'modrinth',
            serverOnlinePlayers: 12,
            serverStatusOnline: true,
            isServerProject: true,
        },
        'dark',
        {
            attachTo: host,
            slots: {
                actions: () =>
                    h('div', { class: 'flex gap-2' }, [
                        h(Button, { color: 'brand', size: '2xs' }, () => 'Join'),
                    ]),
            },
        },
    )

    try {
        await flushPromises()
        const card = wrapper.get<HTMLElement>('.project-card--compact').element
        const actions = wrapper.get<HTMLElement>('.project-card--compact__actions').element

        const cardRect = card.getBoundingClientRect()
        const clusterRect = (actions.firstElementChild as HTMLElement).getBoundingClientRect()

        expect(clusterRect.width).toBeGreaterThan(0)
        expect(clusterRect.top).toBeGreaterThanOrEqual(cardRect.top)
        expect(clusterRect.bottom).toBeLessThanOrEqual(cardRect.bottom)
        expect(clusterRect.right).toBeLessThanOrEqual(cardRect.right - 0.5)
    } finally {
        wrapper.unmount()
        host.remove()
    }
})

it('keeps a local grid banner backdrop when project images are missing or cannot decode', async () => {
    const brokenImage = 'data:image/png;base64,invalid'
    const validImage = document.createElement('canvas').toDataURL()
    const wrapper = await mountThemed(
        ProjectCard,
        { layout: 'grid', title: 'Example', banner: brokenImage, iconUrl: brokenImage },
        'dark',
    )
    try {
        const banner = wrapper.get<HTMLElement>('.bg-project-gradient')
        await waitFor(() => wrapper.find('svg.avatar').exists(), { label: 'the local avatar' })
        await waitFor(
            () =>
                !banner.find('img').exists() ||
                banner.get<HTMLImageElement>('img').element.complete,
            { label: 'the invalid banner to finish loading' },
        )
        expect(banner.find('img').exists()).toBe(false)
        expect(getComputedStyle(banner.element).backgroundImage).toContain('linear-gradient(')
        const height = banner.element.getBoundingClientRect().height
        expect(height).toBeGreaterThan(0)

        await wrapper.setProps({ banner: validImage, iconUrl: validImage })
        await waitFor(() => banner.get<HTMLImageElement>('img').element.naturalWidth > 0, {
            label: 'the replacement banner',
        })
        await waitFor(() => wrapper.get<HTMLImageElement>('img.avatar').element.naturalWidth > 0, {
            label: 'the replacement avatar',
        })
        expect(banner.element.getBoundingClientRect().height).toBe(height)

        await wrapper.setProps({ banner: undefined, iconUrl: undefined })
        expect(banner.find('img').exists()).toBe(false)
        expect(wrapper.find('svg.avatar').exists()).toBe(true)
    } finally {
        wrapper.unmount()
    }
})
