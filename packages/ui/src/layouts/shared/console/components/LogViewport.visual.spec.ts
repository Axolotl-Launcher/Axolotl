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
