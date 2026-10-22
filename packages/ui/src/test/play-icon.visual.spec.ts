import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import { afterEach, expect, it } from 'vitest'
import { h } from 'vue'

import Button from '../components/base/buttons/Button.vue'
import { mountThemed } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).forEach((fn) => fn()))

it('centers the play glyph and its box consistently across circular button sizes and scaling', async () => {
    for (const size of ['sm', 'md', 'xl'] as const) {
        const wrapper = await mountThemed(
            Button,
            { size, circular: true, iconOnly: true },
            'dark',
            { slots: { default: () => h(PlayIcon) } },
        )
        cleanup.push(() => wrapper.unmount())
        const button = wrapper.element as HTMLElement
        const svg = button.querySelector('svg')!
        for (const zoom of [0.8, 1, 1.25]) {
            button.style.zoom = String(zoom)
            const outer = button.getBoundingClientRect()
            const box = svg.getBoundingClientRect()
            expect(Math.abs(box.left + box.width / 2 - outer.left - outer.width / 2)).toBeLessThan(
                0.6,
            )
            expect(Math.abs(box.top + box.height / 2 - outer.top - outer.height / 2)).toBeLessThan(
                0.6,
            )
            const image = new Image()
            const copy = svg.cloneNode(true) as SVGElement
            copy.setAttribute('stroke', 'black')
            copy.setAttribute('width', String(box.width))
            copy.setAttribute('height', String(box.height))
            image.src = `data:image/svg+xml,${encodeURIComponent(new XMLSerializer().serializeToString(copy))}`
            await image.decode()
            const canvas = document.createElement('canvas')
            canvas.width = Math.round(box.width * 4)
            canvas.height = Math.round(box.height * 4)
            const context = canvas.getContext('2d')!
            context.drawImage(image, 0, 0, canvas.width, canvas.height)
            const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data
            let weight = 0
            let x = 0
            let y = 0
            for (let index = 3; index < pixels.length; index += 4) {
                const alpha = pixels[index]
                const pixel = (index - 3) / 4
                weight += alpha
                x += ((pixel % canvas.width) + 0.5) * alpha
                y += (Math.floor(pixel / canvas.width) + 0.5) * alpha
            }
            expect(weight).toBeGreaterThan(0)
            expect(Math.abs(x / weight - canvas.width / 2) / 4).toBeLessThan(0.6)
            expect(Math.abs(y / weight - canvas.height / 2) / 4).toBeLessThan(0.6)
        }
    }
})
