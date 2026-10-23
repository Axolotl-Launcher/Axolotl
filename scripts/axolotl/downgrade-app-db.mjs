// Downgrade the launcher app database so an older build can open it again.
//
// sqlx refuses to open a database whose applied migrations include a version the
// running binary does not know ("... was previously applied but is missing in
// the resolved migrations"). That is what an installed release reports after a
// local build carrying a new migration wrote into the shared database.
//
// The upgrade path is one-way, so undoing it means removing the newer migration
// records and the schema they added. Both halves are needed: dropping only the
// records makes the next install of a newer build fail on the duplicate column
// instead.
//
// Usage (dry run unless --apply is given):
//   node scripts/axolotl/downgrade-app-db.mjs --list
//   node scripts/axolotl/downgrade-app-db.mjs --to 20260903120000
//   node scripts/axolotl/downgrade-app-db.mjs --to 20260903120000 --apply
//
//   --to <version>    remove every applied migration at or after this version.
//                     A 14 digit sqlx timestamp, not a single migration: every
//                     newer migration goes too.
//   --db <path>       target app.db, when it is not in the default location
//   --channel <name>  release or beta; defaults to the launcher's active channel
//   --suffix <name>   settings directory of a build made with
//                     AXOLOTL_DATA_DIR_SUFFIX=<name>
//   --apply           perform the downgrade; without it nothing is written
//   --allow-unmapped  continue although a migration has no known schema here
//   --list            print the applied migrations and exit
//
// AXOLOTL_LAUNCHER_IMAGES=<name,...>  process names that count as the launcher
//                     running; defaults to Axolotl Launcher.exe, theseus_gui.exe
//
// Resolving the default location needs Windows; pass --db anywhere else. The
// launcher must be closed: a running instance keeps the database open and would
// keep using the schema it read at startup.

import { spawnSync } from 'node:child_process'
import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { join, basename, dirname } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

const SETTINGS_DIR_NAME = 'red.ghs.axolotl'
const APP_DB = 'app.db'
const CHANNELS = ['release', 'beta']

// Schema added by known migrations, used to undo it. Every migration a
// downgrade is allowed to pass needs an entry here: added columns and tables
// cannot be left behind, or the next install of a build carrying the migration
// fails on duplicate schema. A migration that only moves data (DELETE, UPDATE)
// gets an empty object; naming it keeps the downgrade from stopping on a
// migration it can safely pass.
//
// Only recent migrations are listed. Dropping to a threshold before them is
// refused rather than guessed at; --allow-unmapped accepts the risk explicitly.
const REVERTIBLE_SCHEMA = {
    20261022120000: { columns: [{ table: 'settings', column: 'curseforge_api_key' }] },
    // CodeFlow endpoint domain move; data only
    20261021120000: {},
    20261014210000: { tables: ['resource_legacy_rollback_journals'] },
    20261014190000: {
        objects: [
            { type: 'index', name: 'resource_pending_download_project' },
            { type: 'index', name: 'resource_link_modrinth_project' },
            { type: 'index', name: 'resource_link_content_project' },
            { type: 'index', name: 'resource_link_curseforge_project' },
        ],
    },
    20261013120000: {
        columns: [
            { table: 'settings', column: 'resource_chinese_filenames' },
            { table: 'settings', column: 'resource_preserve_filenames' },
        ],
        tables: ['resource_pack_unlinks'],
    },
    20261011164500: { objects: [{ type: 'trigger', name: 'resource_install_job_delete' }] },
    20261011163000: { tables: ['resource_entry_graphs', 'resource_execution_journals'] },
    20261011160000: { tables: ['resource_history_cleanup_plans', 'resource_history'] },
    20261011150000: { tables: ['resource_installed_artifacts', 'resource_operation_plans'] },
    20261011140000: { tables: ['resource_favorites'] },
    20261011123000: {},
    20261011120000: {
        objects: [
            { type: 'trigger', name: 'resource_instance_links_insert' },
            { type: 'trigger', name: 'resource_instance_links_update' },
            { type: 'view', name: 'resource_instance_pack_links' },
            { type: 'trigger', name: 'resource_instance_content_provider_refs_insert' },
            { type: 'trigger', name: 'resource_instance_content_provider_refs_update' },
            { type: 'trigger', name: 'resource_content_favorites_insert' },
            { type: 'trigger', name: 'resource_content_favorites_update' },
            { type: 'trigger', name: 'resource_instance_pack_members_insert' },
            { type: 'trigger', name: 'resource_instance_pack_members_update' },
            { type: 'trigger', name: 'resource_instance_pending_manual_downloads_insert' },
            { type: 'trigger', name: 'resource_instance_pending_manual_downloads_update' },
            { type: 'view', name: 'resource_pack_links' },
            { type: 'view', name: 'resource_favorite_links' },
            { type: 'view', name: 'resource_entry_links' },
            { type: 'view', name: 'resource_legacy_projects' },
            { type: 'view', name: 'resource_instance_pack_projects' },
        ],
        tables: ['resource_identity_evidence', 'resource_aliases', 'resource_source_types', 'resource_source_projects', 'resource_preferences', 'resources'],
    },
	20261004140000: {
		columns: [
			{ table: 'settings', column: 'doh_enabled' },
			{ table: 'settings', column: 'doh_server' },
		],
	},
	20261004120001: {
		tables: ['store_operations', 'store_retained_refs', 'store_instance_files', 'store_blobs'],
	},
	// XMCL removal only normalizes data; keep the legacy column as a rollback tombstone.
	20261004130000: {},
	20261002120000: { tables: ['official_login_proof'] },
	// settings.close_behavior
	20260903120000: { columns: [{ table: 'settings', column: 'close_behavior' }] },
	// instances: the direct link columns
	20260904120000: {
		columns: [
			{ table: 'instances', column: 'linked_launcher' },
			{ table: 'instances', column: 'linked_launcher_root' },
			{ table: 'instances', column: 'linked_dot_minecraft' },
			{ table: 'instances', column: 'linked_version_id' },
			{ table: 'instances', column: 'linked_version_json_path' },
		],
	},
	// telemetry samples; the tables stay, so nothing to drop
	20260905000000: {},
	// settings.mc_maximize_window
	20260908000000: { columns: [{ table: 'settings', column: 'mc_maximize_window' }] },
	// instances.linked_game_dir_mode
	20260908010000: { columns: [{ table: 'instances', column: 'linked_game_dir_mode' }] },
	// crash_analysis_ai_settings.ai_source
	20260911120000: {
		columns: [{ table: 'crash_analysis_ai_settings', column: 'ai_source' }],
	},
	// settings.log_level
	20260912120000: { columns: [{ table: 'settings', column: 'log_level' }] },
	// log level default normalization; data only
	20260913170000: {},
	// instance synchronization and screenshot center
	20260914090000: {
		columns: [
			{ table: 'settings', column: 'sync_features_across_devices' },
			{ table: 'settings', column: 'show_files_tab_in_instances' },
			{ table: 'settings', column: 'show_worlds_tab_in_instances' },
			{ table: 'settings', column: 'show_screenshots_tab_in_instances' },
			{ table: 'settings', column: 'show_skin_selector_in_sidebar' },
		],
		// Reverse creation order keeps dependent tables ahead of their parents.
		tables: [
			'synced_pack_instances',
			'synced_pack_catalog',
			'screenshot_group_memberships',
			'screenshot_groups',
			'screenshots',
			'instance_server_pack_state',
			'instance_server_projection_entries',
			'instance_servers',
			'synced_servers',
			'synced_server_state',
			'synced_hotbar_state',
			'instance_sync_checkpoints',
			'game_option_locale_origins',
			'instance_game_option_update_state',
			'instance_game_option_pack_bases',
			'synced_game_option_preferences',
			'synced_game_option_values',
			'synced_game_option_state',
			'instance_sync_preferences',
			'sync_feature_settings',
		],
	},
	// linked server project sync source; data only
	20260915090000: {},
	// minecraft_users_with_identity table; trigger drops are not reversible
	20260916090000: {
		tables: ['minecraft_users_with_identity'],
	},
	// settings.ignore_ssl_errors
	20260917120000: {
		columns: [{ table: 'settings', column: 'ignore_ssl_errors' }],
	},
	// settings.home_widget_background_opacity
	20260917130000: {
		columns: [{ table: 'settings', column: 'home_widget_background_opacity' }],
	},
	// settings.hidden_nav_items
	20260917131000: {
		columns: [{ table: 'settings', column: 'hidden_nav_items' }],
	},
	// settings.custom_window_title_enabled / default_window_title
	20260917140000: {
		columns: [
			{ table: 'settings', column: 'custom_window_title_enabled' },
			{ table: 'settings', column: 'default_window_title' },
		],
	},
	// settings.custom_background_component_opacity
	20260917150000: {
		columns: [{ table: 'settings', column: 'custom_background_component_opacity' }],
	},
	// settings.backup_repository_path
	20260920130000: {
		columns: [{ table: 'settings', column: 'backup_repository_path' }],
	},
	// old backup repository paths awaiting cleanup
	20260920160000: {
		tables: ['pending_backup_repository_cleanups'],
	},
	// synced pack selection state
	20260922110000: {
		columns: [
			{ table: 'synced_pack_catalog', column: 'selected' },
			{ table: 'synced_pack_catalog', column: 'selection_order' },
		],
	},
	// settings.allow_external_scheme
	20260922120000: {
		columns: [{ table: 'settings', column: 'allow_external_scheme' }],
	},
	// settings.ui_font / mono_font
	20260919120000: {
		columns: [
			{ table: 'settings', column: 'ui_font' },
			{ table: 'settings', column: 'mono_font' },
		],
	},
	// settings.allow_privileged_scheme
	20261004120000: {
		columns: [{ table: 'settings', column: 'allow_privileged_scheme' }],
	},
}

function fail(message) {
	console.error(`error: ${message}`)
	process.exit(1)
}

function note(message) {
	console.log(message)
}

function parseArgs(argv) {
	const args = {}
	const takeValue = (index, token) => {
		const value = argv[index + 1]
		if (value === undefined || value.startsWith('--')) fail(`${token} needs a value`)
		return value
	}

	for (let index = 0; index < argv.length; index += 1) {
		const token = argv[index]
		switch (token) {
			case '--to': {
				const value = takeValue(index, token)
				if (!/^\d{14}$/.test(value)) {
					fail(`--to expects a 14 digit migration version, got ${value}`)
				}
				args.to = Number(value)
				index += 1
				break
			}
			case '--db':
				args.db = takeValue(index, token)
				index += 1
				break
			case '--channel':
				args.channel = takeValue(index, token)
				index += 1
				break
			case '--suffix':
				args.suffix = takeValue(index, token)
				index += 1
				break
			case '--apply':
				args.apply = true
				break
			case '--allow-unmapped':
				args.allowUnmapped = true
				break
			case '--list':
				args.list = true
				break
			default:
				fail(`unknown argument: ${token}`)
		}
	}

	return args
}

function printUsage() {
	console.log(
		[
			'Downgrade the launcher app database so an older build can open it.',
			'',
			'  node scripts/axolotl/downgrade-app-db.mjs --list',
			'  node scripts/axolotl/downgrade-app-db.mjs --to <version> [--apply]',
			'',
			'  --to <version>    remove every applied migration at or after this version',
			'  --db <path>       target app.db, when it is not in the default location',
			'  --channel <name>  release or beta; defaults to the active channel',
			'  --suffix <name>   settings directory of a suffixed build',
			'  --apply           perform the downgrade; without it nothing is written',
			'  --allow-unmapped  continue although a migration has no known schema here',
			'',
			'Without --apply the script only reports what it would change.',
			'Undo an accidental change with the backup it writes.',
		].join('\n'),
	)
}

// Mirrors the sanitizing in packages/app-lib/src/brand.rs so a suffix typed here
// names the same directory the build wrote to. The order matters: that function
// trims the dots and dashes off both ends *before* the length cap, so a run of
// them cannot spend the allowance and hide the characters after it.
function sanitizeSuffix(suffix) {
	return suffix
		.replace(/[^A-Za-z0-9._-]/g, '')
		.replace(/^[.-]+|[.-]+$/g, '')
		.slice(0, 32)
		.replace(/^[.-]+|[.-]+$/g, '')
}

function baseSettingsDir() {
	const override = process.env.THESEUS_CONFIG_DIR
	if (override) return override

	if (process.platform !== 'win32') {
		const supportDir =
			process.platform === 'darwin' ? '~/Library/Application Support' : '~/.local/share'
		fail(
			`resolving the default settings directory needs Windows; pass --db with the path under ${supportDir}/${SETTINGS_DIR_NAME}`,
		)
	}

	const appData = process.env.APPDATA
	if (!appData) fail('APPDATA is not set; pass --db explicitly')
	return join(appData, SETTINGS_DIR_NAME)
}

function settingsBaseDir(args) {
	const base = baseSettingsDir()
	if (!args.suffix) return base

	const suffix = sanitizeSuffix(args.suffix)
	if (suffix === '') fail(`--suffix ${args.suffix} leaves no usable directory name`)
	return `${base}-${suffix}`
}

// The launcher records the channel it uses next to its settings; a build with a
// suffixed directory keeps its own copy there. When there is none - a build that
// never switched channels - whichever channel already holds a database is the
// one it used. Both or neither would mean guessing, and a guess here writes to
// the wrong database, so it returns null and the caller explains.
function resolveChannel(args, settingsDir) {
	if (args.channel) {
		if (!CHANNELS.includes(args.channel)) {
			fail(`--channel must be one of ${CHANNELS.join(', ')}, got ${args.channel}`)
		}
		return args.channel
	}

	const statePath = join(settingsDir, 'update-channel.json')
	if (existsSync(statePath)) {
		let channel
		try {
			channel = JSON.parse(readFileSync(statePath, 'utf8')).active_channel
		} catch (error) {
			fail(`could not read ${statePath}: ${error.message}`)
		}

		if (!CHANNELS.includes(channel)) {
			fail(`${statePath} does not name a usable channel (${channel}); pass --channel`)
		}
		return channel
	}

	const present = CHANNELS.filter((channel) => existsSync(join(settingsDir, channel, APP_DB)))
	return present.length === 1 ? present[0] : null
}

// Other builds - suffixed ones in particular - keep their own settings directory
// next to the default one. Pointing at them is much friendlier than reporting a
// missing file when the database that needs downgrading lives there. The prefix
// follows the directory actually in use, so a custom THESEUS_CONFIG_DIR (portable
// mode included) finds its own siblings.
function otherDatabases(base) {
	const parent = dirname(base)
	if (!existsSync(parent)) return []

	let entries
	try {
		entries = readdirSync(parent, { withFileTypes: true })
	} catch {
		return []
	}

	return entries
		.filter(
			(entry) =>
				entry.isDirectory() &&
				entry.name !== basename(base) &&
				entry.name.startsWith(`${basename(base)}-`),
		)
		.flatMap((entry) => CHANNELS.map((channel) => join(parent, entry.name, channel, APP_DB)))
		.filter((candidate) => existsSync(candidate))
}

function elsewhereHint(args) {
	if (args.suffix) return ''

	const others = otherDatabases(baseSettingsDir())
	if (others.length === 0) return ''

	return `\n\nOther builds keep their own directories, and these databases exist:\n  ${others.join(
		'\n  ',
	)}\nSelect one with --db, or --suffix for a suffixed build. Only the database named here is ever written to.`
}

function resolveDatabase(args) {
	if (args.db) return args.db

	const settingsDir = settingsBaseDir(args)
	const channel = resolveChannel(args, settingsDir)

	if (channel === null) {
		fail(
			`could not tell which update channel ${settingsDir} uses; pass --channel release or --channel beta${elsewhereHint(args)}`,
		)
	}

	const db = join(settingsDir, channel, APP_DB)
	if (!existsSync(db)) fail(`no database at ${db}${elsewhereHint(args)}`)

	return db
}

function openReadOnly(db) {
	try {
		return new DatabaseSync(db, { readOnly: true })
	} catch (error) {
		fail(`could not open ${db}: ${error.message}`)
	}
}

// Guards against pointing the script at something that is not a launcher
// database at all, which the missing-table errors of the queries below would
// otherwise report much less clearly.
function assertLauncherDatabase(database, db) {
	const tables = database
		.prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
		.all()
		.map((row) => row.name)

	for (const table of ['_sqlx_migrations', 'settings']) {
		if (!tables.includes(table)) {
			fail(`${db} has no ${table} table, so it is not a launcher database`)
		}
	}
}

function listMigrations(db) {
	const database = openReadOnly(db)
	assertLauncherDatabase(database, db)
	const rows = database
		.prepare('SELECT version, description, success FROM _sqlx_migrations ORDER BY version')
		.all()
	database.close()

	note(`database: ${db}`)
	note(`${rows.length} applied migrations`)
	for (const row of rows) {
		const state = row.success ? '' : '  FAILED'
		note(`  ${row.version}  ${row.description}${state}`)
	}
}

function columnsPresent(db, columns) {
	if (columns.length === 0) return []

	const database = openReadOnly(db)
	const present = columns.map(({ table, column }) => {
		const names = database
			.prepare('SELECT name FROM pragma_table_info(?)')
			.all(table)
			.map((row) => row.name)
		return { table, column, present: names.includes(column) }
	})
	database.close()
	return present
}

function tablesPresent(db, tables) {
	if (tables.length === 0) return []

	const database = openReadOnly(db)
	const present = tables.map((table) => ({
		table,
		present:
			database
				.prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?")
				.get(table) !== undefined,
	}))
	database.close()
	return present
}

function objectsPresent(db, objects) {
    if (objects.length === 0) return []
    const database = openReadOnly(db)
    try {
        return objects.map(({ type, name }) => {
            if (!['view', 'trigger', 'index'].includes(type) || !/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(name)) {
                fail('invalid schema object in REVERTIBLE_SCHEMA')
            }
            return { type, name, present: database.prepare('SELECT 1 FROM sqlite_master WHERE type = ? AND name = ?').get(type, name) !== undefined }
        })
    } finally {
        database.close()
    }
}

// Names a running launcher can appear under. Installed builds use the configured
// main binary name and a `tauri dev` binary keeps the crate name; a fork may
// rename it again, so AXOLOTL_LAUNCHER_IMAGES replaces the list.
const LAUNCHER_IMAGES = ['Axolotl Launcher.exe', 'theseus_gui.exe']

function launcherImages() {
	const configured = process.env.AXOLOTL_LAUNCHER_IMAGES
	if (configured === undefined) return LAUNCHER_IMAGES

	const images = configured
		.split(',')
		.map((image) => image.trim())
		.filter((image) => image !== '')
	if (images.length === 0) fail('AXOLOTL_LAUNCHER_IMAGES names no process')

	return images
}

// The name a running launcher answers to, or null when none is running. Its
// database must not be touched: the running process would carry on with the
// schema it read at startup.
function runningLauncher() {
	if (process.platform !== 'win32') {
		note('warning: cannot check whether the launcher is running on this platform')
		return null
	}

	// tasklist takes a single image name per filter, so every candidate needs its
	// own query. One filter hides all the other names, and a launcher running
	// under one of them would be missed while its database is rewritten.
	return (
		launcherImages().find((image) => {
			const result = spawnSync('tasklist', ['/NH', '/FI', `IMAGENAME eq ${image}`], {
				encoding: 'utf8',
			})

			if (result.error) {
				fail(`could not run tasklist to check for a running launcher: ${result.error.message}`)
			}

			const output = `${result.stdout ?? ''}${result.stderr ?? ''}`
			return output.includes(image)
		}) ?? null
	)
}

// A file copy of a live database can capture the main file and its -wal at
// different points, so the copy is not a database as of any single moment.
// VACUUM INTO writes a complete, self-consistent database instead - the same
// thing the launcher itself does before risky operations.
function writeBackup(db, backup) {
	const database = openReadOnly(db)
	try {
		database.prepare('VACUUM INTO ?').run(backup)
	} catch (error) {
		database.close()
		fail(`could not write the backup to ${backup}: ${error.message}`)
	}
	database.close()

	const verification = openReadOnly(backup)
	const integrity = verification.prepare('PRAGMA integrity_check').get().integrity_check
	const count = verification.prepare('SELECT COUNT(*) AS count FROM _sqlx_migrations').get().count
	verification.close()

	if (integrity !== 'ok') {
		fail(`the backup at ${backup} failed its integrity check (${integrity})`)
	}

	return count
}

function backupPath(db) {
	const stamp = new Date().toISOString().replace(/[:.]/g, '-')
	return `${db}.before-downgrade-${stamp}`
}

function planDowngrade(db, target) {
	const database = openReadOnly(db)
	assertLauncherDatabase(database, db)

	const rows = database
		.prepare('SELECT version, success FROM _sqlx_migrations WHERE version >= ? ORDER BY version')
		.all(target)
	database.close()

	const applied = rows.filter((row) => row.success).map((row) => Number(row.version))
	const failed = rows.filter((row) => !row.success).map((row) => Number(row.version))

	const plan = applied.map((version) => ({
		version,
		mapped: REVERTIBLE_SCHEMA[version] !== undefined,
		columns: columnsPresent(db, REVERTIBLE_SCHEMA[version]?.columns ?? []),
		tables: tablesPresent(db, REVERTIBLE_SCHEMA[version]?.tables ?? []),
        objects: objectsPresent(db, REVERTIBLE_SCHEMA[version]?.objects ?? []),
	}))

	return { applied, failed, plan }
}

function reportPlan({ applied, failed, plan }) {
	note(`migrations to remove: ${applied.join(', ')}`)
    for (const { version, mapped, columns, tables, objects } of plan) {
		if (!mapped) {
			note(`  ${version}: no known schema for this migration, removing its record only`)
			continue
		}
		for (const { table, column, present } of columns) {
			note(
				present
					? `  ${version}: drop ${table}.${column}`
					: `  ${version}: ${table}.${column} is NOT in the database`,
			)
		}
		for (const { table, present } of tables) {
			note(
				present ? `  ${version}: drop table ${table}` : `  ${version}: table ${table} is MISSING`,
			)
		}
        for (const { type, name, present } of objects) {
            note(present ? `  ${version}: drop ${type} ${name}` : `  ${version}: ${type} ${name} is MISSING`)
        }
	}

	if (failed.length > 0) {
		note(
			`\nwarning: ${failed.join(', ')} are recorded as FAILED migrations. Removing their\n` +
				'records lets a build that carries them try again, but a half-applied migration\n' +
				'may have left the schema in a state neither build expects.',
		)
	}
}

// A mapped migration whose column is absent means this script's idea of the
// schema and the database disagree - the version was reused, the column renamed,
// or the mapping is simply wrong. Deleting the record anyway would leave a
// database that no build can open, which is the exact state this script exists
// to repair, so stop instead.
function checkMappings(plan) {
	const mismatched = plan
		.filter(
            ({ mapped, columns, tables, objects }) =>
				mapped &&
                (columns.some((column) => !column.present) || tables.some((table) => !table.present) || objects.some((object) => !object.present)),
		)
		.map(({ version }) => version)

	if (mismatched.length > 0) {
		fail(
			`the schema recorded here for ${mismatched.join(', ')} does not match this database.\nRefusing to remove the records: reinstalling that build would then fail on a duplicate\nschema object, and neither build could open the database. Check REVERTIBLE_SCHEMA against\npackages/app-lib/migrations, and use --list to inspect the database.`,
		)
	}
}

function unmappedVersions(plan) {
	return plan.filter(({ mapped }) => !mapped).map(({ version }) => version)
}

function main() {
	const args = parseArgs(process.argv.slice(2))
	if (!args.list && args.to === undefined) {
		printUsage()
		return
	}

	const db = resolveDatabase(args)
	if (!existsSync(db)) fail(`no database at ${db}`)

	if (args.list) {
		listMigrations(db)
		return
	}

	const state = planDowngrade(db, args.to)
	note(`database: ${db}`)

	if (state.applied.length === 0) {
		note(`nothing to do: no applied migration at or after ${args.to}`)
		if (state.failed.length > 0) reportPlan(state)
		return
	}

	reportPlan(state)
	checkMappings(state.plan)

	const unmapped = unmappedVersions(state.plan)
	if (unmapped.length > 0) {
		const message =
			`${unmapped.join(', ')} have no known schema in this script, so any columns or\n` +
			'tables they added stay in place. Reinstalling a build that carries them will then\n' +
			'fail on the duplicate object.'
		if (!args.allowUnmapped) {
			fail(`${message}\n\nPass --allow-unmapped to remove their records anyway.`)
		}
		note(`\nwarning: ${message}`)
	}

	if (!args.apply) {
		note('\ndry run: rerun with --apply to perform the downgrade')
		return
	}

	const running = runningLauncher()
	if (running) {
		fail(`${running} is running; close the launcher before downgrading the database`)
	}

	const backup = backupPath(db)
	const backedUp = writeBackup(db, backup)
	note(`\nbackup: ${backup} (${backedUp} migration records)`)

	const writable = new DatabaseSync(db)
	try {
		// The connection defaults to enforcing foreign keys, and the pragma is a
		// no-op inside a transaction. Nothing being dropped here is referenced by
		// a foreign key, but the checks below run regardless.
		writable.exec('PRAGMA foreign_keys = OFF')
		writable.exec('BEGIN')
		try {
            for (const { columns, tables, objects } of [...state.plan].reverse()) {
                for (const { type, name, present } of objects) {
                    if (present) writable.exec(`DROP ${type.toUpperCase()} ${name}`)
                }
				for (const { table, present } of tables) {
					if (present) writable.exec(`DROP TABLE ${table}`)
				}
				for (const { table, column, present } of columns) {
					if (present) writable.exec(`ALTER TABLE ${table} DROP COLUMN ${column}`)
				}
			}
			writable
				.prepare(`DELETE FROM _sqlx_migrations WHERE version IN (${state.applied.join(', ')})`)
				.run()
			writable.exec('COMMIT')
		} catch (error) {
			writable.exec('ROLLBACK')
			fail(
				`downgrade failed and was rolled back, so the database is unchanged (backup: ${backup}): ${error.message}`,
			)
		}

		const remaining = writable.prepare('SELECT COUNT(*) AS count FROM _sqlx_migrations').get().count
		const integrity = writable.prepare('PRAGMA integrity_check').get().integrity_check
		const brokenReferences = writable.prepare('PRAGMA foreign_key_check').all()

		if (integrity !== 'ok') fail(`integrity check failed after the downgrade: ${integrity}`)
		if (brokenReferences.length > 0) {
			fail(`the downgrade left ${brokenReferences.length} broken foreign key references`)
		}

		note(`migration records remaining: ${remaining}`)
	} finally {
		writable.close()
	}

	note(
		'\nDone. An older build can open this database again. Reinstalling a build that\n' +
			`carries the removed migrations applies them anew and restores what was dropped.\n` +
			`Restore the backup by deleting ${APP_DB} and its -wal/-shm sidecars, then renaming\n` +
			`${backup} back to ${APP_DB}.`,
	)
}

main()
