import assert from 'node:assert/strict'
import test from 'node:test'

import { coveragePasses, formatSummary, summarizeLcov } from './rust-coverage.mjs'

const record = (file, lines, hit) =>
    `TN:\nSF:${file}\nLF:${lines}\nLH:${hit}\nend_of_record\n`

test('the shell is measured separately from the crates it feature-unifies', () => {
    const [libraries, shell] = coveragePasses()
    assert.deepEqual(libraries.slice(0, 2), ['llvm-cov', 'nextest'])
    assert.ok(libraries.includes('--workspace'))
    assert.ok(
        libraries.includes('--exclude') &&
            libraries[libraries.indexOf('--exclude') + 1] === 'theseus_gui',
    )
    assert.ok(libraries.includes('--no-report'))
    assert.deepEqual(shell.slice(0, 2), ['llvm-cov', 'nextest'])
    assert.ok(
        shell.includes('--package') &&
            shell[shell.indexOf('--package') + 1] === 'theseus_gui',
    )
    assert.ok(!shell.includes('--workspace'))
    assert.ok(shell.includes('--no-report'))
})

test('tests run in their own processes so global state cannot leak between them', () => {
    for (const args of [...coveragePasses(), ...coveragePasses({ ignoreRunFail: true })])
        assert.equal(args[1], 'nextest')
})

test('run failures can be tolerated so a report still gets generated', () => {
    for (const args of coveragePasses())
        assert.ok(args.includes('--no-fail-fast'), 'every failure must be reported')
    const tolerant = coveragePasses({ ignoreRunFail: true })
    for (const args of tolerant) {
        assert.ok(args.includes('--ignore-run-fail'))
        // cargo-llvm-cov rejects the two flags together.
        assert.ok(!args.includes('--no-fail-fast'))
    }
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
