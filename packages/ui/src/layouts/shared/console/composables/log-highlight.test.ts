import assert from 'node:assert/strict'
import test from 'node:test'

import { highlightLine } from './log-highlight.ts'

test('highlights python traceback frames including the file location', () => {
    const heading = highlightLine('Traceback (most recent call last):')
    assert.match(heading, /<span class="level-stack-frame">Traceback \(<\/span>/)
    assert.match(heading, /<span class="level-stack-location">most recent call last<\/span>/)

    const frame = highlightLine('  File "/srv/app/main.py", line 12, in <module>')
    assert.match(frame, /<span class="level-stack-frame">\s*File\s+<\/span>/)
    assert.match(frame, /<span class="level-stack-location">&quot;/)
    assert.match(frame, /&quot;,\s*line\s+<\/span>/)
    assert.match(frame, /<span class="level-stack-class">12<\/span>/)
    assert.match(frame, /, in &lt;module&gt;$/)
})
