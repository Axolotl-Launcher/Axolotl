// Exercises the downgrade script against throwaway databases.
//
// The script rewrites a real installation's database, so every guard it has -
// refusing on unknown schema, refusing to guess the channel, refusing to touch
// a database it does not recognize - is worth a regression test.
//
// Run directly: node scripts/axolotl/downgrade-app-db.test.mjs

import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { fileURLToPath } from 'node:url'

const script = fileURLToPath(new URL('downgrade-app-db.mjs', import.meta.url))
const migrationsDir = fileURLToPath(new URL('../../packages/app-lib/migrations', import.meta.url))
const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-downgrade-'))

const CLOSE_BEHAVIOR = 20260903120000
const OFFICIAL_LOGIN_PROOF = 20261002120000
const UNMAPPED = 20260101000000

// THESEUS_CONFIG_DIR takes priority over the platform directories, so the tests
// run the same way everywhere instead of only on Windows.
let settingsDir

function run(args, { configDir = settingsDir, env = {} } = {}) {
	const environment = { ...process.env, ...env }
	delete environment.THESEUS_CONFIG_DIR
	if (configDir) environment.THESEUS_CONFIG_DIR = configDir

	const result = spawnSync(process.execPath, [script, ...args], {
		encoding: 'utf8',
		env: environment,
	})
	return { status: result.status, output: `${result.stdout}${result.stderr}` }
}

function sampleDatabase(
	file,
	{ withColumn = true, withProof = false, applied = [CLOSE_BEHAVIOR] } = {},
) {
	fs.mkdirSync(path.dirname(file), { recursive: true })
	const database = new DatabaseSync(file)
	database.exec('CREATE TABLE settings (id INTEGER PRIMARY KEY, max_memory REAL)')
	if (withColumn) {
		database.exec(
			"ALTER TABLE settings ADD COLUMN close_behavior TEXT NOT NULL DEFAULT 'ask' CHECK (close_behavior IN ('ask', 'close', 'lightweight'))",
		)
	}
	if (withProof) {
		database.exec(
			'CREATE TABLE official_login_proof (id INTEGER PRIMARY KEY, verified INTEGER NOT NULL, version INTEGER NOT NULL, mac TEXT NOT NULL)',
		)
		database.exec("INSERT INTO official_login_proof VALUES (0, 1, 1, 'test')")
	}
	database.exec('INSERT INTO settings (id, max_memory) VALUES (1, 4096)')
	database.exec(`CREATE TABLE _sqlx_migrations (
		version BIGINT PRIMARY KEY,
		description TEXT NOT NULL,
		installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
		success BOOLEAN NOT NULL,
		checksum BLOB NOT NULL,
		execution_time BIGINT NOT NULL
	)`)
	const insert = database.prepare(
		`INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
		 VALUES (?, ?, TRUE, X'00', 0)`,
	)
	for (const version of applied) insert.run(version, `migration-${version}`)
	database.close()
}

function migrations(file) {
	const database = new DatabaseSync(file, { readOnly: true })
	const rows = database
		.prepare('SELECT version FROM _sqlx_migrations ORDER BY version')
		.all()
		.map((row) => Number(row.version))
	database.close()
	return rows
}

function columns(file, table) {
	const database = new DatabaseSync(file, { readOnly: true })
	const names = database
		.prepare('SELECT name FROM pragma_table_info(?)')
		.all(table)
		.map((row) => row.name)
	database.close()
	return names
}

function backupsOf(file) {
	const dir = path.dirname(file)
	return fs
		.readdirSync(dir)
		.filter((name) => name.startsWith(`${path.basename(file)}.before-downgrade-`))
}

function check(name, condition, detail) {
	assert.ok(condition, detail ? `${name}: ${detail}` : name)
	console.log(`  ok  ${name}`)
}

// --- a database with an applied, revertible migration -------------------------

settingsDir = path.join(directory, 'settings')
const database = path.join(settingsDir, 'beta', 'app.db')
sampleDatabase(database)

console.log('dry run')
{
	const before = migrations(database)
	const result = run(['--to', String(CLOSE_BEHAVIOR)])
	check('reports success', result.status === 0)
	check('names the migration to drop', result.output.includes(String(CLOSE_BEHAVIOR)))
	check('announces the dry run', result.output.includes('dry run'))
	check('leaves the migrations alone', migrations(database).join() === before.join())
	check('leaves the column in place', columns(database, 'settings').includes('close_behavior'))
	check('writes no backup', backupsOf(database).length === 0)
}

console.log('apply')
{
	const result = run(['--to', String(CLOSE_BEHAVIOR), '--apply'])
	check('reports success', result.status === 0)
	check('removes the migration record', !migrations(database).includes(CLOSE_BEHAVIOR))
	check('drops the column', !columns(database, 'settings').includes('close_behavior'))
	check('keeps the remaining columns', columns(database, 'settings').includes('max_memory'))
	check(
		'keeps the row',
		new DatabaseSync(database, { readOnly: true })
			.prepare('SELECT COUNT(*) AS count FROM settings')
			.get().count === 1,
	)
	check('writes one backup', backupsOf(database).length === 1)
	check('explains how to restore it', result.output.includes('Restore the backup'))

	const [backup] = backupsOf(database)
	const backupPath = path.join(path.dirname(database), backup)
	check(
		'the backup is a database in its own right',
		migrations(backupPath).includes(CLOSE_BEHAVIOR),
	)
	check(
		'the backup still has the column',
		columns(backupPath, 'settings').includes('close_behavior'),
	)
	check(
		'the backup needs no sidecar files',
		!fs.existsSync(`${backupPath}-wal`) && !fs.existsSync(`${backupPath}-shm`),
	)
}

console.log('apply again')
{
	const before = migrations(database)
	const result = run(['--to', String(CLOSE_BEHAVIOR), '--apply'])
	check('reports success', result.status === 0)
	check('changes nothing', migrations(database).join() === before.join())
	check('writes no second backup', backupsOf(database).length === 1)
}

console.log('official login proof downgrade')
{
	const proofDb = path.join(settingsDir, 'beta', 'proof.db')
	sampleDatabase(proofDb, { withColumn: false, withProof: true, applied: [OFFICIAL_LOGIN_PROOF] })
	const result = run(['--db', proofDb, '--to', String(OFFICIAL_LOGIN_PROOF), '--apply'])
	check(
		'removes the proof migration',
		result.status === 0 && !migrations(proofDb).includes(OFFICIAL_LOGIN_PROOF),
	)
	check('drops the proof table', columns(proofDb, 'official_login_proof').length === 0)
	const [backup] = backupsOf(proofDb)
	check(
		'preserves the proof table in backup',
		!!backup &&
			columns(path.join(path.dirname(proofDb), backup), 'official_login_proof').includes('mac'),
	)
}

// --- refusals -----------------------------------------------------------------

console.log('refusals')
{
	const unknown = path.join(settingsDir, 'beta', 'unknown.db')
	sampleDatabase(unknown, { withColumn: false, applied: [UNMAPPED] })
	const before = migrations(unknown)

	const refused = run(['--db', unknown, '--to', String(UNMAPPED)])
	check('refuses a migration with no known schema', refused.status === 1)
	check('explains why', refused.output.includes('no known schema'))
	check('points at the override', refused.output.includes('--allow-unmapped'))
	check('changed nothing', migrations(unknown).join() === before.join())

	const allowed = run(['--db', unknown, '--to', String(UNMAPPED), '--allow-unmapped', '--apply'])
	check('proceeds with --allow-unmapped', allowed.status === 0)
	check('removes the record', !migrations(unknown).includes(UNMAPPED))
}

{
	// The column is already gone, so this script's mapping cannot describe the
	// database. Deleting the record anyway would leave a database no build opens.
	const mismatched = path.join(settingsDir, 'beta', 'mismatched.db')
	sampleDatabase(mismatched, { withColumn: false })

	const result = run(['--db', mismatched, '--to', String(CLOSE_BEHAVIOR)])
	check('refuses a mapping that does not match', result.status === 1)
	check('does not claim the column is fine', result.output.includes('NOT in the database'))
	check('keeps the migration record', migrations(mismatched).includes(CLOSE_BEHAVIOR))
}

{
	const stranger = path.join(settingsDir, 'beta', 'stranger.db')
	fs.mkdirSync(path.dirname(stranger), { recursive: true })
	new DatabaseSync(stranger).exec('CREATE TABLE unrelated (a)')

	const result = run(['--db', stranger, '--to', String(CLOSE_BEHAVIOR)])
	check("refuses a database that is not the launcher's", result.status === 1)
	check('says which table is missing', result.output.includes('_sqlx_migrations'))
}

console.log('argument validation')
{
	for (const [args, expected] of [
		[['--to', '2026'], '14 digit'],
		[['--to', '1e5'], '14 digit'],
		[['--to', String(CLOSE_BEHAVIOR), '--db'], 'needs a value'],
		[['--to', String(CLOSE_BEHAVIOR), '--channel', '../..'], 'must be one of'],
		[['--totally-bogus'], 'unknown argument'],
	]) {
		const result = run(args)
		check(`rejects ${args.join(' ')}`, result.status === 1 && result.output.includes(expected))
	}
}

// --- resolving which database to touch ---------------------------------------

console.log('resolving the settings directory')
{
	const suffixed = `${settingsDir}-pr538`
	sampleDatabase(path.join(suffixed, 'beta', 'app.db'))
	sampleDatabase(path.join(settingsDir, 'release', 'app.db'))

	const result = run(['--suffix', 'pr538', '--to', String(CLOSE_BEHAVIOR), '--apply'])
	check(
		'a suffix targets the built directory',
		result.status === 0 && result.output.includes('settings-pr538'),
	)
	check(
		'and leaves the unsuffixed directory untouched',
		migrations(path.join(settingsDir, 'release', 'app.db')).includes(CLOSE_BEHAVIOR),
	)

	const unknown = run(['--to', String(CLOSE_BEHAVIOR), '--suffix', 'does-not-exist'])
	check('an unknown suffix reports the database as missing', unknown.status === 1)
	check('without falling back to another directory', unknown.output.includes('does-not-exist'))
}

{
	// Without --db or --suffix, a database under a suffixed directory is worth
	// naming: that is the directory a suffixed build writes to, and silently
	// reporting "missing" would send the reader looking in the wrong place.
	const nearby = path.join(directory, 'nearby')
	sampleDatabase(path.join(nearby, 'beta', 'app.db'))
	sampleDatabase(path.join(`${nearby}-pr538`, 'beta', 'app.db'))
	fs.rmSync(path.join(nearby, 'beta', 'app.db'))

	const result = run(['--to', String(CLOSE_BEHAVIOR)], { configDir: nearby })
	check('a missing database names the suffixed one that exists', result.status === 1)
	check(
		'with its full path',
		result.output.includes(path.join(`${nearby}-pr538`, 'beta', 'app.db')),
	)
	check('and how to select it', result.output.includes('--suffix'))
}

{
	const ambiguous = path.join(directory, 'ambiguous')
	sampleDatabase(path.join(ambiguous, 'beta', 'app.db'))
	sampleDatabase(path.join(ambiguous, 'release', 'app.db'))

	const result = run(['--to', String(CLOSE_BEHAVIOR)], { configDir: ambiguous })
	check('two channels need an explicit choice', result.status === 1)
	check('and say so', result.output.includes('--channel'))
}

{
	const empty = path.join(directory, 'empty')
	fs.mkdirSync(empty, { recursive: true })

	const result = run(['--to', String(CLOSE_BEHAVIOR)], { configDir: empty })
	check('an empty directory needs an explicit channel', result.status === 1)
	check('and says which flag to pass', result.output.includes('--channel'))
}

// --- keeping the migration mapping honest -------------------------------------

console.log('keeping the migration mapping honest')
{
	const source = fs.readFileSync(script, 'utf8')
	const block = source.slice(source.indexOf('const REVERTIBLE_SCHEMA = {'))
	const body = block.slice(0, block.indexOf('\n}'))
    const registered = [...body.matchAll(/^[ \t]+(\d{14}):/gm)].map((match) => Number(match[1]))

	const files = fs.readdirSync(migrationsDir).filter((name) => name.endsWith('.sql'))
	const versions = files.map((name) => Number(name.slice(0, 14)))

	// A mapping for a migration that does not exist is dead weight: it can never
	// match anything, and it makes the table look considered when it is not.
	const unknown = registered.filter((version) => !versions.includes(version))
	check('every mapped migration exists', unknown.length === 0, unknown.join(', '))

	// The script refuses any applied migration it has no mapping for, so a
	// column added without one turns the documented recovery into a refusal.
	const oldest = Math.min(...registered)
	const addingColumns = files
		.filter((name) => /ADD COLUMN/i.test(fs.readFileSync(path.join(migrationsDir, name), 'utf8')))
		.map((name) => Number(name.slice(0, 14)))
		.filter((version) => version >= oldest)
	const unmapped = addingColumns.filter((version) => !registered.includes(version))
	check('every migration that adds a column is mapped', unmapped.length === 0, unmapped.join(', '))
}

// --- refusing a database a running launcher holds open ------------------------

console.log('resource identities: real upgrade, downgrade and re-upgrade')
{
    const version = 20261011120000
    const file = path.join(directory, 'resources', 'beta', 'app.db')
    fs.mkdirSync(path.dirname(file), { recursive: true })
    const database = new DatabaseSync(file)
    database.exec('PRAGMA foreign_keys = ON')
    database.exec(`CREATE TABLE _sqlx_migrations (
        version BIGINT PRIMARY KEY, description TEXT NOT NULL,
        installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
        success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL
    )`)
    const files = fs.readdirSync(migrationsDir).filter((name) => name.endsWith('.sql')).sort()
    function apply(name) {
        const sql = fs.readFileSync(path.join(migrationsDir, name), 'utf8')
        database.exec('BEGIN')
        try {
            database.exec(sql)
            database.prepare('INSERT INTO _sqlx_migrations(version, description, success, checksum, execution_time) VALUES (?, ?, 1, ?, 0)')
                .run(Number(name.slice(0, 14)), name, createHash('sha384').update(sql).digest())
            database.exec('COMMIT')
        } catch (error) {
            database.exec('ROLLBACK')
            throw error
        }
    }
    for (const name of files.filter((name) => Number(name.slice(0, 14)) < version)) apply(name)
    database.exec(`
        INSERT INTO content_favorites VALUES ('modrinth', '12345678', 'mod', 123), ('curseforge', '12345678', 'mod', 456),
            ('modrinth', 'old-slug', 'mod', 789), ('curseforge', '000123', 'shader', 800);
        INSERT INTO cache(id, data_type, data, expires) VALUES ('malformed', 'project', 'broken JSON', 0);
        INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified)
            VALUES ('sample', '/sample', 'installed', '1', 'Sample', 10, 20);
        INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified)
            VALUES ('set', 'sample', 'Root', 'local', 'applied', '1.20.1', 'fabric', 10, 20);
        INSERT INTO instance_files(id, instance_id, relative_path, file_name, enabled, sha1, size, missing, added_at, modified_at)
            VALUES ('file', 'sample', 'mods/custom.jar', 'custom.jar', 0, 'hash', 100, 0, 10, 20);
        INSERT INTO instance_content_entries(id, instance_id, content_set_id, file_id, project_type, source_kind, server_requirement, client_requirement, enabled, added_at, modified_at)
            VALUES ('entry', 'sample', 'set', 'file', 'mod', 'modrinth', 'unknown', 'unknown', 0, 11, 21);
        INSERT INTO instance_content_provider_refs(content_entry_id, provider, provider_project_id, provider_release_id, provider_file_id, is_origin)
            VALUES ('entry', 'modrinth', '12345678', 'release1', 'artifact1', 1);
        INSERT INTO instance_links(instance_id, link_kind, curseforge_project_id, curseforge_file_id)
            VALUES ('sample', 'curseforge_modpack', 100, 200);
        INSERT INTO instance_pack_members(id, content_set_id, content_entry_id, member_key, project_type, expected_relative_path, provider, provider_project_id, provider_release_id, created_at, modified_at)
            VALUES ('member', 'set', 'entry', 'key', 'mod', 'mods/custom.jar', 'curseforge', '12345678', '101', 30, 40);
        INSERT INTO install_jobs(id, instance_id, kind, status, state, created, modified)
            VALUES ('frozen', 'sample', 'install_content', 'queued',
                '{"request":{"type":"install_content","project_id":"12345678","version_id":"release1"}}', 1, 2);
    `)
    const businessTables = ['content_favorites', 'instance_files', 'instance_content_entries', 'instance_content_provider_refs', 'instance_pack_members', 'instance_links', 'install_jobs', 'cache']
    const original = businessTables.map((table) => database.prepare(`SELECT * FROM ${table}`).all())
    for (const name of files.filter((name) => Number(name.slice(0, 14)) >= version)) apply(name)
    check('resource naming preferences default on when upgrading', database.prepare('SELECT resource_chinese_filenames, resource_preserve_filenames FROM settings WHERE id = 0').get().resource_chinese_filenames === 1 && database.prepare('SELECT resource_preserve_filenames FROM settings WHERE id = 0').get().resource_preserve_filenames === 1)
    database.exec("INSERT INTO resource_pack_unlinks(instance_id, content_set_id, unlinked_at) VALUES ('sample', 'set', 42)")
    assert.deepEqual(businessTables.map((table) => database.prepare(`SELECT * FROM ${table}`).all()), original)
    check('provider-qualified IDs remain separate', database.prepare("SELECT COUNT(DISTINCT resource_id) AS count FROM resource_source_projects WHERE project_id = '12345678'").get().count === 2)
    check('malformed cache and slug do not become business identities', database.prepare("SELECT COUNT(*) AS count FROM resource_source_projects WHERE project_id IN ('old-slug', '000123', 'malformed')").get().count === 0)
    check('online pack has a durable identity', database.prepare('SELECT COUNT(*) AS count FROM resource_instance_pack_links').get().count === 1)
    check('upgrade has no broken foreign keys', database.prepare('PRAGMA foreign_key_check').all().length === 0)
    const lookupPlan = database.prepare("EXPLAIN QUERY PLAN SELECT source, project_id, content_type FROM resource_legacy_projects WHERE source = 'curseforge' AND project_id = '12345678'").all().map(row => row.detail).join('\n')
    check('resource trigger lookup searches pending downloads through its covering index', lookupPlan.includes('SEARCH instance_pending_manual_downloads USING COVERING INDEX resource_pending_download_project'), lookupPlan)
    check('resource favorites preserve exact source timestamps', database.prepare('SELECT saved_at FROM resource_favorites ORDER BY saved_at').all().map((row) => row.saved_at).join(',') === database.prepare("SELECT saved_at FROM content_favorites WHERE project_id = '12345678' ORDER BY saved_at").all().map((row) => row.saved_at).join(','))
    database.exec(`
        INSERT INTO resource_operation_plans(id, instance_id, revision, payload) VALUES ('frozen-plan', 'sample', 1, '{"exact":"retained"}');
        INSERT INTO resource_execution_journals(plan_id, payload) VALUES ('frozen-plan', '[]');
        INSERT INTO resource_entry_graphs(entry_id, nodes, edges) VALUES ('entry', '[]', '[]');
        INSERT INTO resource_installed_artifacts(entry_id, artifact, scope) VALUES ('entry', '{"source":"modrinth"}', '{"kind":"instance"}');
        INSERT INTO resource_history(id, instance_id, entry_id, recorded_at, payload, backup_relative_path, backup_size, backup_sha1, backup_available)
            VALUES ('history', 'sample', 'entry', 30, '{"previous":"exact"}', 'mods/backup.old', 100, 'hash', 1);
    `)
    database.exec(`
        INSERT INTO install_jobs(id, instance_id, kind, status, state, created, modified, finished, dismissed) VALUES ('resource-job', 'sample', 'change_content', 'succeeded', '{}', 1, 1, 2, 0);
        UPDATE resource_operation_plans SET job_id = 'resource-job', queued = 1, completed = 1 WHERE id = 'frozen-plan';
        DELETE FROM install_jobs WHERE id = 'resource-job';
    `)
    check('clearing task history preserves the durable reviewed resource plan', database.prepare("SELECT COUNT(*) AS count FROM resource_operation_plans WHERE id = 'frozen-plan' AND job_id IS NULL AND completed = 1").get().count === 1)
    check('frozen resource tasks and populated history preserve foreign keys', database.prepare('PRAGMA foreign_key_check').all().length === 0)
    database.exec("BEGIN; DELETE FROM instances WHERE id = 'sample'; ROLLBACK;")
    check('resource execution and history tables were created', database.prepare("SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name IN ('resource_history', 'resource_entry_graphs', 'resource_execution_journals')").get().count === 3)
    check('cascading deletion does not conflict with identity triggers', database.prepare("SELECT COUNT(*) AS count FROM instances WHERE id = 'sample'").get().count === 1)
    database.close()
    const result = run(['--db', file, '--to', String(version), '--apply'])
    check('real downgrade succeeds', result.status === 0, result.output)
    const old = new DatabaseSync(file)
    assert.deepEqual(businessTables.map((table) => old.prepare(`SELECT * FROM ${table}`).all()), original)
    check('downgrade removes resource schema including triggers and views', old.prepare("SELECT COUNT(*) AS count FROM sqlite_master WHERE name = 'resources' OR name LIKE 'resource_%'").get().count === 0)
    check('downgrade removes naming columns', !old.prepare('PRAGMA table_info(settings)').all().some(row => row.name === 'resource_chinese_filenames' || row.name === 'resource_preserve_filenames'))
    old.exec('BEGIN')
    for (const name of files.filter((name) => Number(name.slice(0, 14)) >= version)) {
        const sql = fs.readFileSync(path.join(migrationsDir, name))
        old.exec(sql.toString())
        old.prepare('INSERT INTO _sqlx_migrations(version, description, success, checksum, execution_time) VALUES (?, ?, 1, ?, 0)')
            .run(Number(name.slice(0, 14)), name, createHash('sha384').update(sql).digest())
    }
    old.exec('COMMIT')
    check('re-upgrade restores source-qualified identities', old.prepare("SELECT COUNT(DISTINCT resource_id) AS count FROM resource_source_projects WHERE project_id = '12345678'").get().count === 2)
    check('re-upgrade keeps foreign keys valid', old.prepare('PRAGMA foreign_key_check').all().length === 0)
    check('re-upgrade restores naming defaults without duplicate schema', old.prepare('SELECT resource_chinese_filenames, resource_preserve_filenames FROM settings WHERE id = 0').get().resource_preserve_filenames === 1)
    check('resource favorites are re-seeded with their original timestamps', old.prepare('SELECT saved_at FROM resource_favorites ORDER BY saved_at').all().map((row) => row.saved_at).join(',') === old.prepare("SELECT saved_at FROM content_favorites WHERE project_id = '12345678' ORDER BY saved_at").all().map((row) => row.saved_at).join(','))
    assert.deepEqual(businessTables.map((table) => old.prepare(`SELECT * FROM ${table}`).all()), original)
    old.exec('DROP TRIGGER resource_content_favorites_insert')
    old.close()
    const refused = run(['--db', file, '--to', String(version), '--apply'])
    check('missing mapped trigger prevents unsafe downgrade', refused.status === 1 && refused.output.includes('does not match'))
}

if (process.platform !== 'win32') {
	console.log('skipping the running launcher checks outside Windows')
} else {
	console.log('refusing a database a running launcher holds open')

	const held = path.join(directory, 'held')
	const heldDb = path.join(held, 'beta', 'app.db')
	sampleDatabase(heldDb)

	// tasklist matches on the name of the executable, so the probe has to be a
	// process that is certainly running: the test itself. A filter covering only
	// the first candidate would miss it and rewrite the database regardless.
	const runningImage = path.basename(process.execPath)

	const running = run(['--to', String(CLOSE_BEHAVIOR), '--apply'], {
		configDir: held,
		env: { AXOLOTL_LAUNCHER_IMAGES: `definitely-not-running.exe,${runningImage}` },
	})
	check('a launcher running under a later candidate is found', running.status === 1)
	check('and named in the refusal', running.output.includes(`${runningImage} is running`))
	check('the migration record stays', migrations(heldDb).includes(CLOSE_BEHAVIOR))

	const absent = run(['--to', String(CLOSE_BEHAVIOR), '--apply'], {
		configDir: held,
		env: { AXOLOTL_LAUNCHER_IMAGES: 'definitely-not-running.exe' },
	})
	check('no launcher running lets the downgrade through', absent.status === 0)
	check('and the record goes', !migrations(heldDb).includes(CLOSE_BEHAVIOR))

	// A database with nothing left to remove returns before the launcher check,
	// so this needs one of its own.
	const namelessDir = path.join(directory, 'nameless')
	sampleDatabase(path.join(namelessDir, 'beta', 'app.db'))

	const nameless = run(['--to', String(CLOSE_BEHAVIOR), '--apply'], {
		configDir: namelessDir,
		env: { AXOLOTL_LAUNCHER_IMAGES: ' , ' },
	})
	check('a list naming no process is rejected', nameless.status === 1)
	check('and says so', nameless.output.includes('names no process'))
}

fs.rmSync(directory, { recursive: true, force: true })
console.log('\nAll downgrade script checks passed.')
