import './visual-styles.scss'

import { expect, it } from 'vitest'

it('lets explicit button widths fill a grid column while defaults remain compact', () => {
    const host = document.createElement('div')
    host.className = 'universal-body'
    host.style.width = '600px'
    host.innerHTML = `
        <div style="display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px">
            <a href="#">Community</a>
            <button class="w-full">QQ</button>
        </div>
        <button>Default</button>
    `
    document.body.appendChild(host)
    try {
        const link = host.querySelector('a')!
        const button = host.querySelector('button')!
        const compact = host.lastElementChild!
        expect(link.getBoundingClientRect().width).toBeGreaterThan(0)
        expect(button.getBoundingClientRect().width).toBe(link.getBoundingClientRect().width)
        button.disabled = true
        expect(button.getBoundingClientRect().width).toBe(link.getBoundingClientRect().width)
        expect(compact.getBoundingClientRect().width).toBeLessThan(
            link.getBoundingClientRect().width,
        )
    } finally {
        host.remove()
    }
})
