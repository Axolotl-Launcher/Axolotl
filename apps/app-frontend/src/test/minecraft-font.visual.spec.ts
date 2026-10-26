import '@/assets/stylesheets/global.scss'

import { expect, it } from 'vitest'

it('loads the nametag Minecraft font from a bundled file', async () => {
    const label = document.createElement('div')
    label.className = 'font-minecraft'
    label.textContent = 'Player'
    document.body.appendChild(label)
    try {
        const fonts = await document.fonts.load('24px "bundled-minecraft-font-mrapp"', 'Player')
        expect(fonts).toHaveLength(1)
        expect(fonts[0].status).toBe('loaded')
        expect(getComputedStyle(label).fontFamily).toContain('bundled-minecraft-font-mrapp')
        expect(
            performance
                .getEntriesByType('resource')
                .some((entry) => entry.name.includes('/font/minecraft_font.ttf')),
        ).toBe(true)
    } finally {
        label.remove()
    }
})
