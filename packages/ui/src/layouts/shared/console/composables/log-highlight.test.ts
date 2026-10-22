import assert from 'node:assert/strict'
import test from 'node:test'

import { highlightLine, type LogHighlightMode, markSearchMatches } from './log-highlight.ts'

const pythonFrame = '  File "/srv/app/main.py", line 12, in <module>'
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
