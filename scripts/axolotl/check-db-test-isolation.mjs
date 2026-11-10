#!/usr/bin/env node
/**
 * Fails when the database test wiring or the app-database write policy drifts
 * back into the shape that broke main CI:
 *
 * - theseus DB/state suites must run under nextest (one process per test), or
 *   at minimum `cargo test -- --test-threads=1`. libtest otherwise shares the
 *   first `State::init_for_test` root and its app.db across every test in the
 *   binary.
 * - every nextest invocation must fail on zero matched tests, and the runner
 *   must be installed at the pinned version.
 * - `open_app_db_pool` must keep WAL, a non-zero busy timeout and a bounded
 *   connection count.
 * - read-then-write helpers on the content path must take the write lock up
 *   front (`BEGIN IMMEDIATE`) instead of upgrading a deferred WAL snapshot.
 * - `State::init_for_test` must be given a tempfile-derived root.
 *
 * Usage: node scripts/axolotl/check-db-test-isolation.mjs
 * `analyze*` functions are exported so the paired `.test.mjs` can exercise
 * each policy with inline fixtures.
 */
import { lstatSync, readFileSync, readdirSync } from 'node:fs'
import { join, relative } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

export const NEXTEST_VERSION = '0.9.150'
export const MAX_APP_DB_CONNECTIONS = 100

export const REQUIRED_TEST_SELECTORS = [
    'writing_version_info_creates_missing_parent_directory',
    'curseforge',
    'derived_db',
    'state::documents',
    'state::store::tests',
    'scheduler::',
    'install::critical_section',
    'telemetry::reconcile',
    'state::process::playtime_clock',
    'state::process::l0_separation_tests',
    'state::process::stopped_detector_tests',
    'store::tests::secrets',
    'marks_whose_account_secrets_were_moved',
    'content_refresh',
    'remove_instance',
    'resources',
]

export const TRANSACTION_HELPERS = {
    'packages/app-lib/src/state/instances/adapters/sqlite/files.rs': [
        'upsert_instance_file_from_parts',
        'rename_instance_file',
    ],
    'packages/app-lib/src/state/instances/adapters/sqlite/content_entries.rs': [
        'upsert_content_entry_from_parts',
        'upsert_content_provider_ref',
    ],
}

const NEXTEST_INSTALL_SHA = /uses:\s*taiki-e\/install-action@[0-9a-f]{40}\b/

function lineAt(source, index) {
    let line = 1
    for (let i = 0; i < index && i < source.length; i++) {
        if (source[i] === '\n') line += 1
    }
    return line
}

function issue(file, line, message) {
    return { file, line, message }
}

function escapeAnnotation(value) {
    return value.replaceAll('%', '%25').replaceAll('\r', '%0D').replaceAll('\n', '%0A')
}

export function analyzeWorkflow(source, file = '.github/workflows/axolotl-ci.yml') {
    const issues = []
    const lines = source.split('\n')
    let installLine = null
    let installVersion = null
    let installPinned = false
    let firstNextestLine = null
    let nextestCount = 0
    let theseusCommandCount = 0

    lines.forEach((raw, index) => {
        const lineNumber = index + 1
        const line = raw.trim()
        if (line === '' || line.startsWith('#')) return

        if (line.includes('THESEUS_CONFIG_DIR')) {
            issues.push(
                issue(
                    file,
                    lineNumber,
                    'CI must not set THESEUS_CONFIG_DIR; it overrides the temp roots used by State::init_for_test.',
                ),
            )
        }

        if (line.includes('tool:') && line.includes('cargo-nextest@')) {
            if (installLine === null || lineNumber < installLine) {
                installLine = lineNumber
                installVersion = line
                    .slice(line.indexOf('cargo-nextest@') + 'cargo-nextest@'.length)
                    .split(/[,\s]/)[0]
                    .trim()
            }
        }
        if (NEXTEST_INSTALL_SHA.test(line)) installPinned = true

        const nextestAt = line.indexOf('cargo nextest run')
        if (nextestAt !== -1) {
            nextestCount += 1
            if (firstNextestLine === null) firstNextestLine = lineNumber
            if (!line.includes('--package theseus')) {
                issues.push(
                    issue(
                        file,
                        lineNumber,
                        'Nextest commands in CI must target --package theseus so the DB/state suites stay covered.',
                    ),
                )
            }
            if (!line.includes('--no-fail-fast')) {
                issues.push(issue(file, lineNumber, 'Nextest commands must pass --no-fail-fast.'))
            }
            if (!line.includes('--no-tests=fail')) {
                issues.push(
                    issue(
                        file,
                        lineNumber,
                        'Nextest commands must pass --no-tests=fail so a renamed filter cannot silently run zero tests.',
                    ),
                )
            }
            if (/&&|;|\|\|/.test(line)) {
                issues.push(
                    issue(
                        file,
                        lineNumber,
                        'Do not chain nextest with shell operators; keep one test command per line for isolation and attribution.',
                    ),
                )
            }
        }

        const testAt = line.indexOf('cargo test')
        if (testAt !== -1 && /(?:--package\s+theseus|-p\s+theseus)/.test(line)) {
            theseusCommandCount += 1
            if (!line.includes('-- --test-threads=1')) {
                issues.push(
                    issue(
                        file,
                        lineNumber,
                        'Theseus tests are not process-isolated: use cargo nextest run (preferred) or cargo test -- --test-threads=1.',
                    ),
                )
            }
        }
    })

    if (firstNextestLine !== null) {
        if (installLine === null || !installPinned) {
            issues.push(
                issue(
                    file,
                    1,
                    'A pinned taiki-e/install-action cargo-nextest install step is required before theseus nextest commands.',
                ),
            )
        } else if (installLine > firstNextestLine) {
            issues.push(
                issue(file, installLine, 'Install cargo-nextest before the first nextest command.'),
            )
        }
        if (installVersion !== NEXTEST_VERSION) {
            issues.push(
                issue(
                    file,
                    installLine,
                    `Pin cargo-nextest@${NEXTEST_VERSION}; found ${installVersion ?? 'none'}.`,
                ),
            )
        }
    }

    return {
        issues,
        nextestCount,
        theseusCommandCount,
        installVersion,
        installLine,
        firstNextestLine,
        source,
    }
}

export function validateRequiredSelectors(source, file = '.github/workflows/axolotl-ci.yml') {
    const issues = []
    const lines = source.split('\n')
    const isIsolatedCommand = (line) => {
        if (line.trim().startsWith('#')) return false
        if (line.includes('cargo nextest run') && line.includes('--package theseus')) {
            return true
        }
        return (
            line.includes('cargo test') &&
            line.includes('--package theseus') &&
            line.includes('-- --test-threads=1')
        )
    }

    for (const selector of REQUIRED_TEST_SELECTORS) {
        const found = lines.some((line) => isIsolatedCommand(line) && line.includes(selector))
        if (!found) {
            issues.push(issue(file, 1, `Required database test selector is missing: ${selector}`))
        }
    }
    const javaTarget = lines.some(
        (line) => isIsolatedCommand(line) && line.includes('--test java_build_cache'),
    )
    if (!javaTarget) {
        issues.push(issue(file, 1, 'Required test target is missing: java_build_cache'))
    }
    return issues
}

function extractRustFunction(source, name) {
    const signature = new RegExp(`(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${name}\\s*\\(`)
    const match = signature.exec(source)
    if (!match) return null

    const bodyStart = source.indexOf('{', match.index)
    if (bodyStart === -1) return null
    let depth = 0
    for (let i = bodyStart; i < source.length; i++) {
        if (source[i] === '{') depth += 1
        else if (source[i] === '}') {
            depth -= 1
            if (depth === 0) {
                return {
                    index: match.index,
                    line: lineAt(source, match.index),
                    body: source.slice(bodyStart, i + 1),
                }
            }
        }
    }
    return null
}

export function analyzeDbPool(source, file = 'packages/app-lib/src/state/db.rs') {
    const issues = []
    const fn = extractRustFunction(source, 'open_app_db_pool')
    if (!fn) {
        return [
            issue(file, 1, 'open_app_db_pool was not found; the pool invariant check is vacuous.'),
        ]
    }

    const timeoutSecs = /busy_timeout\(\s*Duration::from_secs\((\d+)\)\s*\)/.exec(fn.body)
    const timeoutMillis = /busy_timeout\(\s*Duration::from_millis\((\d+)\)\s*\)/.exec(fn.body)
    if (!timeoutSecs && !timeoutMillis) {
        issues.push(issue(file, fn.line, 'open_app_db_pool must set a busy_timeout.'))
    } else if (timeoutMillis && Number(timeoutMillis[1]) < 1000) {
        issues.push(
            issue(file, fn.line, 'open_app_db_pool busy_timeout must be at least one second.'),
        )
    }

    if (!/journal_mode\(\s*(?:sqlx::sqlite::)?SqliteJournalMode::Wal\s*\)/.test(fn.body)) {
        issues.push(issue(file, fn.line, 'open_app_db_pool must enable WAL journal mode.'))
    }

    const maxConnections = /max_connections\((\d+)\)/.exec(fn.body)
    if (!maxConnections) {
        issues.push(issue(file, fn.line, 'open_app_db_pool must set a literal max_connections.'))
    } else {
        const value = Number(maxConnections[1])
        if (value < 1 || value > MAX_APP_DB_CONNECTIONS) {
            issues.push(
                issue(
                    file,
                    fn.line,
                    `open_app_db_pool max_connections(${value}) must be between 1 and ${MAX_APP_DB_CONNECTIONS}.`,
                ),
            )
        }
    }
    return issues
}

export function analyzeTransactionHelpers(files) {
    const issues = []
    for (const [file, names] of Object.entries(TRANSACTION_HELPERS)) {
        const source = files[file]
        if (typeof source !== 'string') {
            issues.push(issue(file, 1, `Transaction helper source is missing: ${file}`))
            continue
        }
        for (const name of names) {
            const fn = extractRustFunction(source, name)
            if (!fn) {
                issues.push(issue(file, 1, `Required transaction helper is missing: ${name}`))
                continue
            }
            if (!fn.body.includes('begin_with("BEGIN IMMEDIATE")')) {
                issues.push(
                    issue(
                        file,
                        fn.line,
                        `${name} must start its read-then-write transaction with BEGIN IMMEDIATE to avoid SQLITE_BUSY_SNAPSHOT under WAL.`,
                    ),
                )
            }
        }
    }
    return issues
}

function findMatchingParen(source, start) {
    let depth = 0
    for (let i = start; i < source.length; i++) {
        if (source[i] === '(') depth += 1
        else if (source[i] === ')') {
            depth -= 1
            if (depth === 0) return i
        }
    }
    return -1
}

export function analyzeInitForTest(source, file) {
    const issues = []
    const marker = 'State::init_for_test('
    let cursor = 0
    let count = 0

    while ((cursor = source.indexOf(marker, cursor)) !== -1) {
        count += 1
        const line = lineAt(source, cursor)
        const open = source.indexOf('(', cursor)
        const close = findMatchingParen(source, open)
        if (close === -1) {
            issues.push(issue(file, line, 'State::init_for_test argument is not closed.'))
            cursor += marker.length
            continue
        }
        const argument = source.slice(open + 1, close)
        const tempDerived = /tempfile::|TempDir|temp_dir\s*\(/.test(argument)

        let boundId = null
        if (!tempDerived) {
            const id = /^\s*(?:&)?\s*(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)/.exec(argument)
            if (id) {
                boundId = id[1]
                const binding = new RegExp(`let\\s+${boundId}\\s*=\\s*([^;]+);`)
                const before = source.slice(Math.max(0, cursor - 8000), cursor)
                const match = binding.exec(before)
                if (!match || !/tempfile::|TempDir|temp_dir\s*\(/.test(match[1])) {
                    issues.push(
                        issue(
                            file,
                            line,
                            `State::init_for_test must use a tempfile-derived root; ${boundId} is not one.`,
                        ),
                    )
                }
            } else {
                issues.push(
                    issue(file, line, 'State::init_for_test must use a tempfile-derived root.'),
                )
            }
        }
        cursor = close + 1
    }

    return { issues, count }
}

export function analyzeRepository({ workflows, rustSources, dbSources }) {
    const issues = []
    let nextestCommands = 0
    let initCalls = 0
    let workflowCommands = 0
    let nextestVersions = new Set()

    if (Object.keys(workflows).length === 0) {
        issues.push(issue('.github/workflows', 1, 'No workflow files were scanned.'))
    }

    for (const [file, source] of Object.entries(workflows)) {
        const result = analyzeWorkflow(source, file)
        issues.push(...result.issues)
        nextestCommands += result.nextestCount
        workflowCommands += result.nextestCount + result.theseusCommandCount
        if (result.installVersion) nextestVersions.add(result.installVersion)
        if (file.endsWith('axolotl-ci.yml')) {
            issues.push(...validateRequiredSelectors(source, file))
        }
    }

    if (workflowCommands === 0) {
        issues.push(issue('.github/workflows', 1, 'No cargo test/nextest command was found.'))
    }
    if (nextestVersions.size > 1) {
        issues.push(
            issue(
                '.github/workflows',
                1,
                `Multiple cargo-nextest versions are pinned: ${[...nextestVersions].join(', ')}.`,
            ),
        )
    }

    const dbFile = 'packages/app-lib/src/state/db.rs'
    if (typeof dbSources[dbFile] !== 'string') {
        issues.push(issue(dbFile, 1, 'The app database source was not scanned.'))
    } else {
        issues.push(...analyzeDbPool(dbSources[dbFile], dbFile))
    }

    issues.push(...analyzeTransactionHelpers(dbSources))

    if (Object.keys(rustSources).length === 0) {
        issues.push(issue('packages', 1, 'No Rust source files were scanned.'))
    }
    for (const [file, source] of Object.entries(rustSources)) {
        const result = analyzeInitForTest(source, file)
        issues.push(...result.issues)
        initCalls += result.count
    }
    if (initCalls === 0) {
        issues.push(issue('packages', 1, 'No State::init_for_test call sites were found.'))
    }

    return { issues, nextestCommands, initCalls, workflowCommands }
}

export function formatIssue(entry) {
    return `::error file=${entry.file},line=${entry.line}::${escapeAnnotation(entry.message)}`
}

function walk(directory) {
    const files = []
    for (const entry of readdirSync(directory)) {
        if (entry === 'target' || entry === 'node_modules' || entry === '.git') continue
        if (entry === 'dist' || entry === '.output' || entry === '.nuxt') continue
        const path = join(directory, entry)
        const stat = lstatSync(path)
        if (stat.isSymbolicLink()) continue
        if (stat.isDirectory()) files.push(...walk(path))
        else files.push(path)
    }
    return files
}

function main() {
    const root = fileURLToPath(new URL('../..', import.meta.url))
    const workflowsDir = join(root, '.github/workflows')
    const workflows = {}
    for (const file of readdirSync(workflowsDir)) {
        if (!/\.ya?ml$/.test(file)) continue
        const path = join(workflowsDir, file)
        workflows[`.github/workflows/${file}`] = readFileSync(path, 'utf8')
    }

    const rustFiles = [
        ...walk(join(root, 'packages')).filter((path) => path.endsWith('.rs')),
        ...walk(join(root, 'apps')).filter((path) => path.endsWith('.rs')),
    ]
    const rustSources = {}
    for (const path of rustFiles) {
        rustSources[relative(root, path).replaceAll('\\', '/')] = readFileSync(path, 'utf8')
    }

    const dbSources = {}
    for (const file of Object.keys(TRANSACTION_HELPERS)) {
        dbSources[file] = rustSources[file]
    }
    dbSources['packages/app-lib/src/state/db.rs'] = rustSources['packages/app-lib/src/state/db.rs']

    const result = analyzeRepository({ workflows, rustSources, dbSources })
    if (result.issues.length > 0) {
        for (const entry of result.issues) {
            console.error(`${entry.file}:${entry.line}: ${entry.message}`)
            console.log(formatIssue(entry))
        }
        console.error(`Database test isolation gate failed: ${result.issues.length} issue(s).`)
        process.exitCode = 1
        return
    }

    console.log(
        `Database test isolation gate passed: ${result.nextestCommands} nextest command(s), ` +
            `${result.initCalls} temp-rooted State::init_for_test call site(s), ` +
            'pool and transaction invariants intact.',
    )
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
    try {
        main()
    } catch (error) {
        console.error(error.message)
        process.exitCode = 1
    }
}
