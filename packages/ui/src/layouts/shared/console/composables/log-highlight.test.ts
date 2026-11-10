import assert from 'node:assert/strict'
import test from 'node:test'

import {
    highlightLine,
    type LogHighlightMode,
    markSearchMatches,
    visibleLogText,
} from './log-highlight.ts'

test('ANSI SGR supports basic, indexed, RGB, resets and safe literal HTML', () => {
    const html = highlightLine(
        '\x1b[31m<script>red</script>\x1b[0m plain \x1b[38;5;196mindexed\x1b[0m \x1b[38;2;1;2;3mrgb\x1b[0m',
    )
    assert.match(html, /color:rgb\(187,0,0\)/)
    assert.match(html, /color:rgb\(255,0,0\)/)
    assert.match(html, /color:rgb\(1,2,3\)/)
    assert.ok(!html.includes('\x1b'))
    assert.doesNotMatch(html, /<script>/)
    assert.match(html, /&lt;script&gt;red&lt;\/script&gt;<\/span> plain/)
    assert.equal(highlightLine('\x1b[31mred', 'raw'), '\x1b[31mred')
    assert.equal(highlightLine('uncolored'), 'uncolored')
    assert.doesNotMatch(
        highlightLine('\x1b[31m\x1b]8;;https://example.com\x07link\x1b]8;;\x07'),
        /<a/,
    )
    assert.match(markSearchMatches(html, 'indexed'), /<mark>indexed<\/mark>/)
})

const pythonFrame = '  File "/srv/app/main.py", line 12, in <module>'

test('search marks apostrophes escaped by the ANSI renderer as visible characters', () => {
    const html = highlightLine("\x1b[31mcan't stop\x1b[0m")
    assert.match(html, /can&#x27;t/)
    assert.match(markSearchMatches(html, "can't"), /<mark>can&#039;t<\/mark>/)
    assert.equal(markSearchMatches(html, 'x27'), html)
})
const withoutMarks = (html: string) => html.replace(/<\/?mark>/g, '')

test('Python frames highlight escaped quotes, paths, and line numbers', () => {
    const html = highlightLine(pythonFrame)
    assert.match(html, /<span class="level-stack-frame">\s*File\s+<\/span>/)
    assert.match(html, /<span class="level-stack-location">&quot;/)
    assert.match(html, /<span class="level-stack-class">12<\/span>/)
    assert.match(html, /&lt;module&gt;/)
})

for (const [text, queries] of [
    [pythonFrame, ['File', '"/srv/app/main.py"', '12', '<module>', pythonFrame]],
    ['Traceback (most recent call last):', ['Traceback', '(most', 'last):']],
    ['Caused by: java.lang.NullPointerException: boom', ['Caused', 'by:', 'NullPointerException']],
    ['\tat com.example.Main.main(Main.java:12)', ['at', 'Main.main(Main.java:12)', '12']],
    ['Suppressed: java.io.IOException: boom', ['Suppressed:', 'IOException']],
] as const) {
    for (const query of queries) {
        test(`search ${JSON.stringify(query)} preserves syntax in ${JSON.stringify(text)}`, () => {
            const syntax = highlightLine(text)
            const marked = markSearchMatches(syntax, query)
            assert.match(marked, /<mark>/)
            assert.equal(withoutMarks(marked), syntax)
        })
    }
}

test('search only matches visible text, not tags, class names, or entity spellings', () => {
    const html = '<span class="level-stack-frame">File &quot;A&amp;B&lt;C&gt;&#039;&quot;</span>'
    for (const query of ['span', 'class', 'level-stack-frame', 'quot', 'amp', '039']) {
        assert.equal(markSearchMatches(html, query), html)
    }
    assert.equal(
        markSearchMatches(html, '"A&B<C>\'"'),
        '<span class="level-stack-frame">File <mark>&quot;A&amp;B&lt;C&gt;&#039;&quot;</mark></span>',
    )
})

test('search matches across nested syntax spans with balanced mark elements', () => {
    const html = '<span class="outer">ab<span class="inner">cd</span>ef</span>'
    assert.equal(
        markSearchMatches(html, 'bcde'),
        '<span class="outer">a<mark>b</mark><span class="inner"><mark>cd</mark></span><mark>e</mark>f</span>',
    )
})

test('search terms are literal, case insensitive, and never create nested marks for overlapping terms', () => {
    assert.equal(
        markSearchMatches('Foo Foobar FOO [a.*]', 'foo foobar [a.*] foo'),
        '<mark>Foo</mark> <mark>Foobar</mark> <mark>FOO</mark> <mark>[a.*]</mark>',
    )
    assert.equal(markSearchMatches('unchanged', ' \t '), 'unchanged')
    assert.equal(markSearchMatches('', 'x'), '')
})

for (const mode of ['full', 'lite', 'line-only', 'raw'] as LogHighlightMode[]) {
    for (const length of [20, 5000, 66000]) {
        test(`search stays rendered for ${mode} mode and a ${length}-character line`, () => {
            const text = 'x'.repeat(length) + ' target <mark>literal</mark> & <script>'
            const html = highlightLine(text, mode)
            const marked = markSearchMatches(html, 'target')
            assert.match(marked, /<mark>target<\/mark>/)
            assert.equal(withoutMarks(marked), html)
            assert.doesNotMatch(html, /<mark>|<script>/)
            assert.match(html, /&lt;mark&gt;literal&lt;\/mark&gt;/)
        })
    }
}

test('changing search terms cannot contaminate the syntax cache', () => {
    const syntax = highlightLine(pythonFrame)
    markSearchMatches(syntax, 'File')
    const second = markSearchMatches(highlightLine(pythonFrame), '12')
    assert.doesNotMatch(second, /<mark>File<\/mark>/)
    assert.equal(highlightLine(pythonFrame), syntax)
})

test('visible ANSI text matches rendering, decodes entities once, and respects raw and hard-limit fallbacks', () => {
    const visible = `<&"' &lt; literal>`
    const ansi = `\x1b[1;31m${visible}\x1b[0m`
    for (const mode of ['full', 'lite', 'line-only'] as LogHighlightMode[])
        assert.equal(visibleLogText(ansi, mode), visible)
    assert.equal(visibleLogText(ansi, 'raw'), ansi)
    assert.equal(visibleLogText(visible), visible)
    assert.equal(visibleLogText('\x1b[31m\x1b]8;;https://example.com\x07link\x1b]8;;\x07'), '')
    const tooLong = ansi + 'x'.repeat(65536)
    assert.equal(visibleLogText(tooLong), tooLong)
    assert.equal(visibleLogText('\x1b[31mred\x1b[0m uncolored'), 'red uncolored')
})
