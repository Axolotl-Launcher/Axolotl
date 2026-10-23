import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { isExcludedRelease } from './release-policy.mjs'
import { githubRepository } from './release-utils.mjs'

const migrationsDirectory = 'packages/app-lib/migrations'
const repository = githubRepository()
// v1.7.5 is the canonical snapshot immediately before the v1.7.6 incident.
const canonicalBootstrap = {
	ref: '7ddbfb8e57db4b0044a04cf28f25fb29e08c3279',
	tag: 'v1.7.5',
}
// Published divergences remain evidence of an incident and never replace the canonical bytes.
const knownPublishedDivergences = new Set([
	[
		'v1.7.6',
		'packages/app-lib/migrations/20260714120000_translation.sql',
		'c6fdf52790db7e67905003216ee7c099ec9ac29df1ee1b62602eb791881f321470f7e2b965c39fc8733b10ad114eace5',
	].join('\0'),
])

function git(args, encoding = 'utf8') {
	return execFileSync('git', args, {
		encoding,
		maxBuffer: 16 * 1024 * 1024,
		stdio: ['ignore', 'pipe', 'pipe'],
	})
}

function checksum(contents) {
	return createHash('sha384').update(contents).digest('hex')
}

function migrationChecksum(migration) {
	migration.checksum ??= checksum(git(['cat-file', 'blob', migration.blob], null))
	return migration.checksum
}

function migrationVersion(file) {
	const name = file.slice(file.lastIndexOf('/') + 1)
	const match = /^(\d+)_([a-z0-9][a-z0-9_-]*)\.sql$/.exec(name)
	return match ? Number(match[1]) : null
}

function migrationMapAt(ref) {
	const migrations = new Map()
	const output = git(['ls-tree', '-r', '-z', ref, '--', migrationsDirectory])
	for (const record of output.split('\0')) {
		if (!record) continue
		const tabIndex = record.indexOf('\t')
		const metadata = record.slice(0, tabIndex).split(' ')
		const file = record.slice(tabIndex + 1)
		if (!file.endsWith('.sql')) continue
		migrations.set(file, {
			blob: metadata[2],
			checksum: null,
			version: migrationVersion(file),
		})
	}
	return new Map([...migrations].sort(([left], [right]) => left.localeCompare(right)))
}

function validateMigrationSet(migrations, failures) {
	const versions = new Map()

	for (const [file, migration] of migrations) {
		if (migration.version === null) {
			failures.push(`INVALID NAME ${file}`)
			continue
		}

		const existing = versions.get(migration.version)
		if (existing) {
			failures.push(`DUPLICATE VERSION ${migration.version}: ${existing}, ${file}`)
		} else {
			versions.set(migration.version, file)
		}
	}
}

function compareCurrentWithCanonical(canonical, currentRef, failures) {
	const current = migrationMapAt(currentRef)
	validateMigrationSet(current, failures)

	for (const [file, expected] of canonical) {
		const actual = current.get(file)
		if (!actual) {
			failures.push(`DELETED ${file}`)
			continue
		}
		if (actual.blob !== expected.blob) {
			failures.push(
				`MODIFIED ${file}\n  Expected SHA-384: ${migrationChecksum(expected)}\n  Actual SHA-384:   ${migrationChecksum(actual)}`,
			)
		}
	}

	const maximumCanonicalVersion = Math.max(
		...Array.from(canonical.values(), (migration) => migration.version ?? 0),
	)
	for (const [file, migration] of current) {
		if (
			!canonical.has(file) &&
			migration.version !== null &&
			migration.version <= maximumCanonicalVersion
		) {
			failures.push(
				`OUT-OF-ORDER ${file}: new migration version must be greater than ${maximumCanonicalVersion}`,
			)
		}
	}
}

function parseVersion(tag) {
	const match = /^v(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/.exec(tag)
	if (!match) return null
	return {
		major: Number(match[1]),
		minor: Number(match[2]),
		patch: Number(match[3]),
		prerelease: match[4] ?? null,
	}
}

function compareVersions(left, right) {
	for (const key of ['major', 'minor', 'patch']) {
		if (left[key] !== right[key]) return left[key] - right[key]
	}
	if (left.prerelease === right.prerelease) return 0
	if (left.prerelease === null) return 1
	if (right.prerelease === null) return -1
	return left.prerelease.localeCompare(right.prerelease, 'en', { numeric: true })
}

function publishedReleases() {
    const args = [
        'release',
        'list',
        '--limit',
        '1000',
        '--json',
        'tagName,isDraft,publishedAt',
        '--repo',
        repository,
    ]
    const releases = JSON.parse(execFileSync('gh', args, { encoding: 'utf8' }))
    for (const release of releases) {
        if (isExcludedRelease(release.tagName)) {
            console.warn(`Migration guard notice: ignoring temporary test release ${release.tagName}`)
        }
    }
    const bootstrapVersion = parseVersion(canonicalBootstrap.tag)

    return releases
        .filter((release) => !release.isDraft && !isExcludedRelease(release.tagName))
        .map((release) => ({ ...release, version: parseVersion(release.tagName) }))
        .filter((release) => release.version && compareVersions(release.version, bootstrapVersion) >= 0)
        .sort((left, right) => Date.parse(left.publishedAt) - Date.parse(right.publishedAt))
}

function auditPublishedReleases(currentRef) {
	const failures = []
	const warnings = []
	const canonical = migrationMapAt(canonicalBootstrap.ref)
	validateMigrationSet(canonical, failures)

	for (const release of publishedReleases()) {
		if (release.tagName === canonicalBootstrap.tag) continue

		const released = migrationMapAt(release.tagName)
		for (const file of canonical.keys()) {
			if (!released.has(file)) {
				failures.push(`PUBLISHED DELETE ${release.tagName}: ${file}`)
			}
		}

		for (const [file, migration] of released) {
			const expected = canonical.get(file)
			if (!expected) {
				canonical.set(file, migration)
				continue
			}
			if (migration.blob === expected.blob) continue

			const releasedChecksum = migrationChecksum(migration)
			const divergence = [release.tagName, file, releasedChecksum].join('\0')
			if (knownPublishedDivergences.has(divergence)) {
				warnings.push(
					`${release.tagName} contains the known historical migration divergence in ${file}`,
				)
				continue
			}

			failures.push(
				`UNRECOGNIZED PUBLISHED DIVERGENCE ${release.tagName}: ${file}\n` +
					`  Canonical SHA-384: ${migrationChecksum(expected)}\n` +
					`  Released SHA-384:  ${releasedChecksum}`,
			)
		}
	}

	compareCurrentWithCanonical(canonical, currentRef, failures)
	finish(failures, warnings, `published release history from ${canonicalBootstrap.tag}`)
}

function resolveBaseRef(baseRef) {
	const isAvailable = () => {
		try {
			git(['rev-parse', '--verify', `${baseRef}^{commit}`])
			return true
		} catch {
			return false
		}
	}

	if (isAvailable()) return baseRef

	for (const remote of ['origin', 'AXL']) {
		try {
			git(['fetch', remote, baseRef])
			if (isAvailable()) return baseRef
		} catch {
			// Try the next remote.
		}
	}

	try {
		const upstream = git([
			'rev-parse',
			'--abbrev-ref',
			'--symbolic-full-name',
			'@{upstream}',
		]).trim()
		const upstreamCommit = git(['rev-parse', `${upstream}^{commit}`]).trim()
		const headCommit = git(['rev-parse', 'HEAD^{commit}']).trim()
		if (upstream && upstreamCommit !== headCommit) return upstream
	} catch {
		// Fall through to HEAD^.
	}

	console.warn(
		`Migration guard: base ref ${baseRef} is not available locally; falling back to HEAD^`,
	)
	return 'HEAD^'
}

function compareWithBase(baseRef, currentRef) {
	const failures = []
	const canonical = migrationMapAt(baseRef)
	validateMigrationSet(canonical, failures)
	compareCurrentWithCanonical(canonical, currentRef, failures)
	finish(failures, [], baseRef)
}

function finish(failures, warnings, baseline) {
	for (const warning of warnings) console.warn(`Migration guard notice: ${warning}`)

	if (failures.length > 0) {
		console.error(
			`Migration guard failed against ${baseline}:\n\n${failures.join('\n\n')}\n\n` +
				'Historical migrations are immutable. Add a new forward migration instead.',
		)
		process.exit(1)
	}

	console.log(`Migration guard passed against ${baseline}.`)
}

const args = process.argv.slice(2)
const currentIndex = args.indexOf('--current')
const currentRef = currentIndex === -1 ? 'HEAD' : args[currentIndex + 1]
const baseIndex = args.indexOf('--base')

if (args.includes('--release')) {
	auditPublishedReleases(currentRef)
} else if (baseIndex !== -1 && args[baseIndex + 1]) {
	compareWithBase(resolveBaseRef(args[baseIndex + 1]), currentRef)
} else {
	console.error(
		'Usage: node check-migrations.mjs (--release | --base <git-ref>) [--current <git-ref>]',
	)
	process.exit(2)
}
