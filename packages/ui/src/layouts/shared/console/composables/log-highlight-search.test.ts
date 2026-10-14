import assert from 'node:assert/strict'
import test from 'node:test'

import { highlightLine, markSearchMatches } from './log-highlight.ts'

test('returns the html untouched when there is no usable query', () => {
    const html = '<span class="level-info">hello</span>'

    assert.equal(markSearchMatches(html, ''), html)
    assert.equal(markSearchMatches(html, '   '), html)
    assert.equal(markSearchMatches(html, 'missing'), html)
})

test('marks every whitespace separated term, case-insensitively', () => {
    assert.equal(
        markSearchMatches('Player Steve joined the game', 'STEVE joined'),
        'Player <mark>Steve</mark> <mark>joined</mark> the game',
    )
})

test('never marks text inside tags or attributes', () => {
    assert.equal(
        markSearchMatches('<span class="level-info">level info</span>', 'level'),
        '<span class="level-info"><mark>level</mark> info</span>',
    )
})

test('matches against the decoded visible text', () => {
    assert.equal(
        markSearchMatches('File &quot;/srv/app.py&quot;', '"'),
        'File <mark>&quot;</mark>/srv/app.py<mark>&quot;</mark>',
    )
})

test('closes and reopens the mark when a match spans a tag', () => {
    assert.equal(
        markSearchMatches('foo<span class="x">bar</span>baz', 'obar'),
        'fo<mark>o</mark><mark>bar</mark></span>baz',
    )
})

test('treats regexp metacharacters in the query as literals', () => {
    assert.equal(markSearchMatches('a+b (c)', 'a+b'), '<mark>a+b</mark> (c)')
    assert.equal(markSearchMatches('x.y', '.'), 'x<mark>.</mark>y')
})

test('keeps line highlighting when a search term matches the frame keyword', () => {
    const rendered = markSearchMatches(
        highlightLine('\tat com.example.Main.main(Main.java:12)'),
        'at',
    )

    assert.match(rendered, /<mark>at<\/mark>/)
    assert.match(rendered, /level-stack-frame/)
})

test('keeps line highlighting when a search term matches the exception marker', () => {
    const rendered = markSearchMatches(
        highlightLine('Caused by: java.lang.NullPointerException: boom'),
        'Caused',
    )

    assert.match(rendered, /<span class="level-stack-caused-by"><mark>Caused<\/mark> by:/)
    assert.match(rendered, /level-stack-caused-by/)
})

test('marks long lines instead of leaking escaped mark tags', () => {
    const rendered = markSearchMatches(highlightLine('y'.repeat(4100) + ' needle'), 'needle')

    assert.match(rendered, /<mark>needle<\/mark>/)
    assert.doesNotMatch(rendered, /&lt;mark&gt;/)
})
