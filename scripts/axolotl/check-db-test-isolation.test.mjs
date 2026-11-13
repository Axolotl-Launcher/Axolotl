import assert from 'node:assert/strict'
import test from 'node:test'

import {
    analyzeDbPool,
    analyzeInitForTest,
    analyzeRepository,
    analyzeTransactionHelpers,
    analyzeWorkflow,
    formatIssue,
    validateRequiredSelectors,
} from './check-db-test-isolation.mjs'

const NEXTEST_SHA = 'a'.repeat(40)

function commandLines() {
    const selectors = [
        'writing_version_info_creates_missing_parent_directory',
        'curseforge',
        'derived_db',
        'state::db::',
        'state::db_backup::',
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
    return [
        ...selectors.map(
            (selector) =>
                `          cargo nextest run --package theseus --no-fail-fast --no-tests=fail ${selector}`,
        ),
        '          cargo nextest run --package theseus --no-fail-fast --no-tests=fail --test java_build_cache',
    ].join('\n')
}

function workflow(overrides = '') {
    return `jobs:
  desktop:
    steps:
      - name: Install cargo-nextest
        uses: taiki-e/install-action@${NEXTEST_SHA}
        with:
          tool: cargo-nextest@0.9.150
      - name: Test database
        run: |
${commandLines()}
${overrides}
`
}

function poolSource({
    timeout = '.busy_timeout(Duration::from_secs(30))',
    wal = '.journal_mode(SqliteJournalMode::Wal)',
    max = '.max_connections(100)',
} = {}) {
    return `async fn open_app_db_pool(db_path: &Path) -> crate::Result<Pool<Sqlite>> {
    let conn_options = SqliteConnectOptions::new()
        .filename(db_path)
        ${timeout}
        ${wal}
        .create_if_missing(true);
    Ok(SqlitePoolOptions::new()
        ${max}
        .connect_with(conn_options)
        .await?)
}`
}

function helper(name, body) {
    return `pub(crate) async fn ${name}(pool: &SqlitePool) -> crate::Result<()> {
    ${body}
}`
}

test('checks pool settings through the ownership-aware builder', () => {
    const wrapper = `async fn open_app_db_pool(db_path: &Path) -> crate::Result<Pool<Sqlite>> {
        open_app_db_pool_with_ownership(db_path, None).await
    }`
    const builder = poolSource().replace('open_app_db_pool(', 'open_app_db_pool_with_ownership(')
    assert.deepEqual(analyzeDbPool(`${wrapper}\n${builder}`), [])
    assert.ok(
        analyzeDbPool(
            `${wrapper}\n${builder.replace('.journal_mode(SqliteJournalMode::Wal)', '')}`,
        ).some((entry) => entry.message.includes('WAL')),
    )
    assert.ok(analyzeDbPool(wrapper).some((entry) => entry.message.includes('not found')))
})

function transactionSources({ immediate = true } = {}) {
    const begin = immediate
        ? 'let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;'
        : 'let mut tx = pool.begin().await?;'
    return {
        'packages/app-lib/src/state/instances/adapters/sqlite/files.rs': [
            helper('upsert_instance_file_from_parts', begin),
            helper('rename_instance_file', begin),
        ].join('\n'),
        'packages/app-lib/src/state/instances/adapters/sqlite/content_entries.rs': [
            helper('upsert_content_entry_from_parts', begin),
            helper('upsert_content_provider_ref', begin),
        ].join('\n'),
    }
}

test('accepts a canonical nextest workflow', () => {
    const result = analyzeWorkflow(workflow())
    assert.deepEqual(result.issues, [])
    assert.equal(result.nextestCount, 19)
    assert.equal(result.installVersion, '0.9.150')
})

test('rejects an unisolated cargo test command', () => {
    const result = analyzeWorkflow(workflow('      - run: cargo test --package theseus curseforge'))
    assert.ok(result.issues.some((entry) => entry.message.includes('not process-isolated')))
})

test('allows cargo test with a single test thread as the weaker fallback', () => {
    const result = analyzeWorkflow(
        workflow('      - run: cargo test --package theseus curseforge -- --test-threads=1'),
    )
    assert.deepEqual(result.issues, [])
})

test('rejects nextest without its failure-safety flags', () => {
    const result = analyzeWorkflow(
        workflow('      - run: cargo nextest run --package theseus curseforge'),
    )
    assert.ok(result.issues.some((entry) => entry.message.includes('--no-fail-fast')))
    assert.ok(result.issues.some((entry) => entry.message.includes('--no-tests=fail')))
})

test('rejects a missing or mispinned nextest install', () => {
    const noInstall = workflow().replace(
        /      - name: Install cargo-nextest[\s\S]*?cargo-nextest@0\.9\.150\n/,
        '',
    )
    assert.ok(analyzeWorkflow(noInstall).issues.some((entry) => entry.message.includes('install')))

    const wrongVersion = workflow().replace('cargo-nextest@0.9.150', 'cargo-nextest@0.9.149')
    assert.ok(
        analyzeWorkflow(wrongVersion).issues.some((entry) =>
            entry.message.includes('cargo-nextest@0.9.150'),
        ),
    )
})

test('rejects THESEUS_CONFIG_DIR in CI', () => {
    const result = analyzeWorkflow(
        workflow('      - run: echo "THESEUS_CONFIG_DIR=$PWD" >> "$GITHUB_ENV"'),
    )
    assert.ok(result.issues.some((entry) => entry.message.includes('THESEUS_CONFIG_DIR')))
})

test('requires every database selector to remain wired', () => {
    const missing = workflow().replace(/.*derived_db.*\n/, '')
    const issues = validateRequiredSelectors(missing)
    assert.ok(issues.some((entry) => entry.message.includes('derived_db')))

    assert.deepEqual(validateRequiredSelectors(workflow()), [])
})

test('checks the app database pool invariants', () => {
    assert.deepEqual(analyzeDbPool(poolSource()), [])
    assert.ok(
        analyzeDbPool(poolSource({ timeout: '' })).some((entry) =>
            entry.message.includes('busy_timeout'),
        ),
    )
    assert.ok(analyzeDbPool(poolSource({ wal: '' })).some((entry) => entry.message.includes('WAL')))
    assert.ok(
        analyzeDbPool(poolSource({ max: '.max_connections(101)' })).some((entry) =>
            entry.message.includes('max_connections'),
        ),
    )
})

test('requires BEGIN IMMEDIATE on the content write helpers', () => {
    assert.deepEqual(analyzeTransactionHelpers(transactionSources()), [])
    const deferred = analyzeTransactionHelpers(transactionSources({ immediate: false }))
    assert.equal(deferred.length, 4)
    assert.ok(deferred.every((entry) => entry.message.includes('BEGIN IMMEDIATE')))
})

test('accepts a temp-rooted State::init_for_test and rejects a fixed path', () => {
    const temp = `#[tokio::test]
async fn t() {
    let root = tempfile::tempdir().unwrap().keep();
    let _ = State::init_for_test(root.to_string_lossy().to_string()).await;
}`
    assert.deepEqual(analyzeInitForTest(temp, 'x.rs').issues, [])

    const fixed = `#[tokio::test]
async fn t() {
    let root = std::path::PathBuf::from("target/test-state");
    let _ = State::init_for_test(root.to_string_lossy().to_string()).await;
}`
    const issues = analyzeInitForTest(fixed, 'x.rs').issues
    assert.equal(issues.length, 1)
    assert.ok(issues[0].message.includes('tempfile-derived'))
})

test('fails closed on empty scans', () => {
    const result = analyzeRepository({
        workflows: {},
        rustSources: {},
        dbSources: {},
    })
    assert.ok(result.issues.length > 0)
})

test('escapes GitHub workflow command payloads', () => {
    assert.equal(
        formatIssue({ file: 'a.yml', line: 3, message: '50% done\nnext' }),
        '::error file=a.yml,line=3::50%25 done%0Anext',
    )
})
