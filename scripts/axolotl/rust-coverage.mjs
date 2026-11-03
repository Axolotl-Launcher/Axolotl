import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { parseArgs } from 'node:util'
import { pathToFileURL } from 'node:url'

// `theseus_gui` enables `theseus/tauri`, and that feature compiles out the
// helpers the `theseus` test modules need. Cargo unifies features per
// invocation, so the workspace cannot be measured in one pass: the library
// crates are measured without the shell, then the shell is measured alone.
//
// Runs go through nextest because it gives every test its own process. Several
// suites share process-global state (tracked servers, the initialized launcher
// state) and fail intermittently when they see each other's leftovers, which
// they do under libtest's in-process thread pool.
//
// nextest is fail-fast like libtest, so `--no-fail-fast` is always passed: a
// red run must list every broken test, not just the first one. `--ignore-run-fail`
// is the opposite trade-off and replaces it when a report is wanted from a run
// whose tests already failed; cargo-llvm-cov rejects the two together.
export function coveragePasses({ ignoreRunFail = false } = {}) {
    const common = ['llvm-cov', 'nextest', '--no-report', '--locked']
    common.push(ignoreRunFail ? '--ignore-run-fail' : '--no-fail-fast')
    return [
        [...common, '--workspace', '--exclude', 'theseus_gui'],
        [...common, '--package', 'theseus_gui'],
    ]
}

// Sums the LF/LH records of an LCOV tracefile. `lines` is the total number of
// instrumented lines and `covered` the subset the tests executed.
export function summarizeLcov(text) {
    let lines = 0
    let covered = 0
    for (const line of text.split(/\r?\n/)) {
        if (line.startsWith('LF:')) lines += Number(line.slice(3))
        else if (line.startsWith('LH:')) covered += Number(line.slice(3))
    }
    if (!Number.isFinite(lines) || !Number.isFinite(covered) || lines <= 0)
        throw new Error('Coverage data contains no line records')
    return { lines, covered, percent: (covered / lines) * 100 }
}

export function formatSummary({ lines, covered, percent }) {
    return `${covered}/${lines} lines (${percent.toFixed(2)}%)`
}

function run(program, args, options = {}) {
    execFileSync(program, args, {
        stdio: 'inherit',
        ...options,
    })
}

function main() {
    const { values } = parseArgs({
        options: {
            lcov: { type: 'string', default: path.join('target', 'coverage', 'lcov.info') },
            html: { type: 'string', default: path.join('target', 'coverage', 'html') },
            summary: { type: 'string', default: path.join('target', 'coverage', 'summary.txt') },
            total: { type: 'string', default: path.join('target', 'coverage', 'total.txt') },
            open: { type: 'boolean', default: false },
            'fail-under': { type: 'string' },
            'ignore-run-fail': { type: 'boolean', default: false },
            // CI sets this so a failing test suite still produces the report:
            // a red run is exactly when the lcov and HTML matter.
            'keep-report-on-test-fail': { type: 'boolean', default: false },
        },
    })

    const lcov = path.resolve(values.lcov)
    const html = path.resolve(values.html)
    const summary = path.resolve(values.summary)
    const total = path.resolve(values.total)
    for (const output of [lcov, summary, total])
        fs.mkdirSync(path.dirname(output), { recursive: true })

    // A measurement run must not merge profiling data left by an earlier run.
    run('cargo', ['llvm-cov', 'clean', '--workspace'])
    let testsFailed = false
    for (const args of coveragePasses({ ignoreRunFail: values['ignore-run-fail'] })) {
        try {
            run('cargo', args)
        } catch (error) {
            if (!values['keep-report-on-test-fail']) throw error
            testsFailed = true
            console.error(`Coverage run failed; reporting what was measured: ${error.message}`)
        }
    }

    run('cargo', ['llvm-cov', 'report', '--lcov', '--output-path', lcov])
    run('cargo', ['llvm-cov', 'report', '--html', '--output-dir', html])

    const text = execFileSync('cargo', ['llvm-cov', 'report'], {
        encoding: 'utf8',
        maxBuffer: 16 * 1024 * 1024,
    })
    fs.writeFileSync(summary, text)
    const totals = summarizeLcov(fs.readFileSync(lcov, 'utf8'))
    const line = `Rust line coverage: ${formatSummary(totals)}`
    console.log(line)
    fs.writeFileSync(total, `${line}\n`)

    if (values.open) {
        const indexPath = path.join(html, 'index.html')
        try {
            if (process.platform === 'win32') run('cmd', ['/c', 'start', '', indexPath])
            else run(process.platform === 'darwin' ? 'open' : 'xdg-open', [indexPath])
        } catch {
            console.error(`Open the report manually: ${indexPath}`)
        }
    }

    if (values['fail-under'] !== undefined) {
        const minimum = Number(values['fail-under'])
        if (!Number.isFinite(minimum) || minimum < 0 || minimum > 100)
            throw new Error(`Invalid --fail-under value: ${values['fail-under']}`)
        if (totals.percent < minimum) {
            console.error(
                `Rust line coverage ${totals.percent.toFixed(2)}% is below the required ${minimum}%`,
            )
            process.exitCode = 1
        }
    }
    if (testsFailed) process.exitCode = 1
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
    try {
        main()
    } catch (error) {
        console.error(error.message)
        process.exitCode = 1
    }
}
