import assert from 'node:assert/strict'
import test from 'node:test'

import { coveragePasses, formatSummary, summarizeLcov } from './rust-coverage.mjs'

const record = (file, lines, hit) =>
    `TN:\nSF:${file}\nLF:${lines}\nLH:${hit}\nend_of_record\n`

test('the shell is measured separately from the crates it feature-unifies', () => {
    const [libraries, shell] = coveragePasses()
    assert.deepEqual(libraries.slice(0, 3), ['llvm-cov', '--workspace', '--exclude'])
    assert.equal(libraries[3], 'theseus_gui')
    assert.ok(libraries.includes('--no-report'))
    assert.deepEqual(shell.slice(0, 3), ['llvm-cov', '--package', 'theseus_gui'])
    assert.ok(shell.includes('--no-report'))
})

test('the LCOV summary sums every record', () => {
    const summary = summarizeLcov(record('a.rs', 10, 7) + record('b.rs', 30, 20))
    assert.deepEqual(summary, { lines: 40, covered: 27, percent: 67.5 })
    assert.equal(formatSummary(summary), '27/40 lines (67.50%)')
})

test('coverage without line records is rejected instead of reporting zero', () => {
    assert.throws(() => summarizeLcov('TN:\nend_of_record\n'), /no line records/)
    assert.throws(() => summarizeLcov(''), /no line records/)
})

test('duplicate file records accumulate rather than replace each other', () => {
    const summary = summarizeLcov(record('a.rs', 4, 4) + record('a.rs', 6, 0))
    assert.equal(summary.lines, 10)
    assert.equal(summary.covered, 4)
})
