import { expect, it } from 'vitest'

import { mountThemed } from '../../../../test/visual-harness'
import { highlightLine, markSearchMatches } from '../composables/log-highlight'
import LogViewport from './LogViewport.vue'

it('keeps stack styling and safely renders search marks as the query changes', async () => {
    const text = '  File "/srv/app/main.py", line 12, in <module>'
    const host = document.createElement('div')
    host.style.cssText = 'height:300px;width:800px;position:relative'
    document.body.append(host)
    const wrapper = await mountThemed(
        LogViewport,
        {
            lines: [{ line: { text, level: 'error' }, originalIndex: 0 }],
            searchQuery: 'File',
        },
        'dark',
        { attachTo: host },
    )
    try {
        const content = wrapper.get('.log-line-content')
        const originalClasses = Array.from(content.element.querySelectorAll('span')).map(
            (span) => span.className,
        )
        expect(originalClasses).toContain('level-stack-frame')
        expect(originalClasses).toContain('level-stack-location')
        expect(originalClasses).toContain('level-stack-class')
        const originalColors = Array.from(content.element.querySelectorAll('span')).map(
            (span) => getComputedStyle(span).color,
        )
        const lineNumber = wrapper.get('.log-line > span:first-child').element.textContent
        for (const query of ['File', '"/srv/app/main.py"', '12', '<module>']) {
            await wrapper.setProps({ searchQuery: query })
            expect(content.element.textContent).toBe(text)
            expect(
                content
                    .findAll('mark')
                    .map((mark) => mark.text())
                    .join(''),
            ).toBe(query)
            expect(
                Array.from(content.element.querySelectorAll('span')).map((span) => span.className),
            ).toEqual(originalClasses)
            expect(
                Array.from(content.element.querySelectorAll('span')).map(
                    (span) => getComputedStyle(span).color,
                ),
            ).toEqual(originalColors)
            expect(wrapper.get('.log-line > span:first-child').element.textContent).toBe(lineNumber)
            expect(content.find('mark mark').exists()).toBe(false)
        }
        await wrapper.setProps({ searchQuery: '' })
        expect(content.find('mark').exists()).toBe(false)
        expect(content.element.textContent).toBe(text)
    } finally {
        wrapper.unmount()
        host.remove()
    }
})

it('renders search results in long lines without exposing mark tags as log content', async () => {
    const text = 'x'.repeat(5000) + ' target <mark>literal</mark>'
    const wrapper = await mountThemed(
        LogViewport,
        {
            lines: [{ line: { text, level: 'info' }, originalIndex: 0 }],
            searchQuery: 'target',
        },
        'dark',
    )
    try {
        const content = wrapper.get('.log-line-content')
        expect(content.element.textContent).toBe(text)
        expect(content.get('mark').text()).toBe('target')
        expect(content.findAll('mark')).toHaveLength(1)
    } finally {
        wrapper.unmount()
    }
})

it('generates balanced HTML when matches cross syntax spans or escaped entities', () => {
    for (const text of [
        '\tat com.example.Main.main(Main.java:12)',
        '  File "/srv/app/main.py", line 12, in <module>',
        'Caused by: java.lang.NullPointerException: boom',
        'Traceback (most recent call last):',
        'quotes " & < > \' and literal <mark>text</mark>',
    ]) {
        const syntax = highlightLine(text)
        const html = markSearchMatches(syntax, text)
        const template = document.createElement('template')
        template.innerHTML = html
        expect(template.innerHTML).toBe(html.replace(/&quot;/g, '"').replace(/&#039;/g, "'"))
        expect(template.content.textContent).toBe(text)
        expect(template.content.querySelector('mark mark')).toBeNull()
        expect(template.content.querySelectorAll('mark').length).toBeGreaterThan(0)
    }
})

it('recalculates wrapped row heights when its real container width changes', async () => {
    const host = document.createElement('div')
    host.style.cssText = 'height:300px;width:800px;position:relative'
    document.body.append(host)
    const wrapper = await mountThemed(
        LogViewport,
        {
            lines: [{ line: { text: 'x'.repeat(500), level: 'info' }, originalIndex: 0 }],
            wrap: true,
        },
        'dark',
        { attachTo: host },
    )
    try {
        await expect
            .poll(() => wrapper.get('.log-line').element.getBoundingClientRect().height)
            .toBeGreaterThan(17)
        const wide = wrapper.get('.log-line').element.getBoundingClientRect().height
        host.style.width = '300px'
        await expect
            .poll(() => wrapper.get('.log-line').element.getBoundingClientRect().height)
            .toBeGreaterThan(wide)
    } finally {
        wrapper.unmount()
        host.remove()
    }
})

it('rebuilds wrapped offsets when filtering replaces interior rows with the same endpoints', async () => {
    const host = document.createElement('div')
    host.style.cssText = 'height:300px;width:400px;position:relative'
    document.body.append(host)
    const first = { line: { text: 'first', level: 'info' as const }, originalIndex: 0 }
    const last = { line: { text: 'last', level: 'info' as const }, originalIndex: 3 }
    const long = { line: { text: 'x'.repeat(1000), level: 'warn' as const }, originalIndex: 1 }
    const short = { line: { text: 'short', level: 'error' as const }, originalIndex: 2 }
    const wrapper = await mountThemed(
        LogViewport,
        { lines: [first, long, last], wrap: true },
        'dark',
        { attachTo: host },
    )
    const total = () => (wrapper.get('.log-viewport-spacer').element as HTMLElement).style.height
    try {
        await expect.poll(() => Number.parseFloat(total())).toBeGreaterThan(100)
        await wrapper.setProps({ lines: [first, short, last] })
        await expect.poll(total).toBe('51px')
        await wrapper.setProps({ lines: [first, long, last] })
        await expect.poll(() => Number.parseFloat(total())).toBeGreaterThan(100)
    } finally {
        wrapper.unmount()
        host.remove()
    }
})

it('measures ANSI-heavy wrapped rows like their visible plain-text equivalents', async () => {
    const host = document.createElement('div')
    host.style.cssText = 'height:300px;width:400px;position:relative'
    document.body.append(host)
    const visible = `<&"'abc>`.repeat(10)
    const ansi = [...visible].map((character) => `\x1b[38;2;1;2;3m${character}\x1b[0m`).join('')
    const makeLine = (text: string, originalIndex: number) => ({
        line: { text, level: 'info' as const },
        originalIndex,
    })
    const wrapper = await mountThemed(
        LogViewport,
        { lines: [makeLine(visible, 0), makeLine(ansi, 1)], wrap: true },
        'dark',
        { attachTo: host },
    )
    const heights = () =>
        wrapper.findAll('.log-line').map((row) => row.element.getBoundingClientRect().height)
    try {
        await expect.poll(() => heights()[0]).toBeGreaterThan(17)
        expect(wrapper.findAll('.log-line-content').map((row) => row.element.textContent)).toEqual([
            visible,
            visible,
        ])
        expect(heights()[1]).toBe(heights()[0])
        expect(
            Number.parseFloat(
                (wrapper.get('.log-viewport-spacer').element as HTMLElement).style.height,
            ),
        ).toBe(heights()[0] * 2)
        const wideHeight = heights()[0]
        host.style.width = '200px'
        await expect.poll(() => heights()[0]).toBeGreaterThan(wideHeight)
        expect(heights()[1]).toBe(heights()[0])
        await wrapper.setProps({
            lines: [makeLine(visible, 0), makeLine(ansi, 1), makeLine(ansi, 2)],
        })
        expect(heights()).toEqual([heights()[0], heights()[0], heights()[0]])
        await wrapper.setProps({ wrap: false })
        expect(heights()).toEqual([17, 17, 17])
    } finally {
        wrapper.unmount()
        host.remove()
    }
})
