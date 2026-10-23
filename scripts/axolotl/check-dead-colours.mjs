#!/usr/bin/env node
/**
 * Fails on Tailwind colour utilities whose key no longer exists.
 *
 * `primary`, `secondary` and `contrast` were dropped from the preset by the
 * explicit-token migration, but a class naming a removed key is invisible to
 * eslint, to `tsc` and to the build: Tailwind emits nothing at all and the
 * element silently keeps whatever colour it inherits. Every regression of this
 * class found in review looked exactly like that — `text-primary` in the skin
 * preview, `text-secondary`/`text-contrast` in the settings pages that came back
 * with an upstream merge, and the drop target and selection ring that had never
 * been visible.
 *
 * Matching uses boundaries rather than substrings: the utility may not be
 * preceded by a word character or a hyphen, and the key may not be followed by
 * one. That catches the variant, important and alpha forms
 * (`hover:text-secondary`, `!border-contrast`, `md:text-contrast`,
 * `bg-secondary/50`, `@apply text-primary;`, a token at the start of a line)
 * while leaving the converged spellings alone (`--color-text-primary`,
 * `text-[var(--color-text-primary)]`), because a hyphen always precedes the key
 * there. Test files are skipped: they assert on the strings rather than use the
 * classes, and `screenshot-thumbnail-contract.test.ts` deliberately names them.
 *
 * Usage: node scripts/axolotl/check-dead-colours.mjs
 * `scan(roots)` is exported so `check-dead-colours.test.mjs` can cover the walk,
 * the allowlist and the reporting against fixtures; the matcher itself is proven
 * by the contract below on every run, import included.
 */
import { readFileSync, readdirSync, realpathSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOTS = ['apps/app-frontend/src', 'packages/ui/src', 'apps/website/src']
const EXTENSIONS = ['.vue', '.ts', '.tsx', '.mjs', '.js', '.scss', '.css']
const IGNORED_DIRECTORIES = new Set(['node_modules', 'dist', '__screenshots__'])
const IGNORED_FILE = /\.(test|spec)\.(?:[cm]?js|[jt]sx?)$/

/** `${file}|${utility}` entries that may stay, mapped to the reason why. */
const ALLOWED = new Map([
	// ['apps/app-frontend/src/components/example.vue|bg-secondary', 'why it stays'],
])

const PREFIXES = [
	'bg',
	'text',
	'border',
	'ring',
	'ring-offset',
	'divide',
	'divide-x',
	'divide-y',
	'from',
	'via',
	'to',
	'fill',
	'stroke',
	'shadow',
	'outline',
	'accent',
	'caret',
	'placeholder',
	'decoration',
].join('|')
/** `border-t`, `divide-x`, … are colour utilities of their own. */
const DIRECTIONS = '(?:-[tblrxy])?'

/** Removed key → the token that replaces it, mirrored from `variables.scss`. */
const REMOVED = new Map([
	['primary', 'var(--color-text-default)'],
	['secondary', 'var(--color-text-tertiary)'],
	['contrast', 'var(--color-text-primary)'],
])

/** The variant chain (`hover:`, `md:`) and an important marker belong to the token. */
const VARIANT = '(?:[\\w-]+:)*!?'
const CANDIDATE = new RegExp(
	`(?<![\\w-])${VARIANT}((?:${PREFIXES})${DIRECTIONS})-(primary|secondary|contrast)(?![\\w-])!?`,
	'g',
)

/**
 * The matcher is the whole check, so it is proven against a fixed contract on
 * every run: the positive table must match (and report the full token, variant
 * included) and the negative table must stay silent, because those are the
 * spellings the migration converged on and the ones that only look like a key.
 * A weakened boundary fails here before it can pass the scan.
 */
const MATCHER_CONTRACT = [
	['hover:text-secondary', ['hover:text-secondary']],
	['!border-contrast', ['!border-contrast']],
	['text-secondary!', ['text-secondary!']],
	['md:text-contrast', ['md:text-contrast']],
	['bg-secondary/50', ['bg-secondary']],
	['@apply text-primary;', ['text-primary']],
	['border-t-secondary', ['border-t-secondary']],
	['from-primary', ['from-primary']],
	['text-primary', ['text-primary']],
	['--color-text-primary: #ffffff;', []],
	['text-[var(--color-text-primary)]', []],
	['bg-primary-500', []],
	['text-primaryForeground', []],
	['bg-secondaryish', []],
	['some-secondary', []],
]

const matchesOf = (line) => [...readdirMatches(line)].map((match) => match[0])

for (const [line, expected] of MATCHER_CONTRACT) {
	const actual = matchesOf(line)
	if (JSON.stringify(actual) !== JSON.stringify(expected)) {
		console.error('Axolotl dead-colour check self-test failed:')
		console.error(`  ${line}`)
		console.error(`  expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`)
		process.exit(1)
	}
}

function* readdirMatches(line) {
	for (const match of line.matchAll(CANDIDATE)) yield match
}

function collectFiles(directory, files = [], unreadable = []) {
	let entries
	try {
		entries = readdirSync(directory, { withFileTypes: true })
	} catch {
		// A directory that cannot be read is recorded and skipped: when it is a
		// root, the caller diagnoses it; when it is nested, the subtree is left out.
		unreadable.push(directory)
		return files
	}

	for (const entry of entries) {
		if (IGNORED_DIRECTORIES.has(entry.name) || entry.name.startsWith('.')) continue

		const path = join(directory, entry.name)
		if (entry.isDirectory()) {
			collectFiles(path, files, unreadable)
			continue
		}
		if (IGNORED_FILE.test(entry.name)) continue
		if (EXTENSIONS.some((extension) => entry.name.endsWith(extension))) files.push(path)
	}
	return files
}

/**
 * Scans the given roots and returns what the caller should act on: the
 * violations, the allowlist entries that no longer match anything, and the
 * roots that are not in this checkout (the website is optional in some).
 */
export function scan(roots = ROOTS) {
	const files = []
	const missing = []
	const unreadable = []

	for (const root of roots) {
		try {
			if (statSync(root).isDirectory()) {
				collectFiles(root, files, unreadable)
				continue
			}
		} catch {
			// Not in this checkout; reported below rather than silently skipped.
		}
		missing.push(root)
	}

	const seen = new Set()
	const violations = []

	for (const file of files) {
		let source
		try {
			source = readFileSync(file, 'utf8')
		} catch {
			// A file that vanished or cannot be read is skipped for the same reason
			// a directory is: the walk reports, it does not throw.
			unreadable.push(file)
			continue
		}
		const lines = source.split('\n')

		for (const [index, line] of lines.entries()) {
			CANDIDATE.lastIndex = 0
			for (const match of readdirMatches(line)) {
				const utility = match[0]
				const entry = `${file}|${utility}`

				if (ALLOWED.has(entry)) {
					seen.add(entry)
					continue
				}

				violations.push({
					file,
					line: index + 1,
					utility,
					reason: `use ${match[1]}-[${REMOVED.get(match[2])}]`,
				})
			}
		}
	}

	return {
		violations,
		unused: [...ALLOWED.keys()].filter((entry) => !seen.has(entry)),
		missing,
		unreadable,
		scanned: files.length,
	}
}

/**
 * True when this module is the process entry point. Both sides are resolved
 * through realpath so a package-manager shim or a symlinked checkout still runs
 * the scan instead of exiting 0 having done nothing.
 */
const isCli = () => {
	if (!process.argv[1]) return false

	try {
		return realpathSync(fileURLToPath(import.meta.url)) === realpathSync(process.argv[1])
	} catch {
		console.warn(`  note: could not resolve ${process.argv[1]}, so nothing was scanned`)
		return false
	}
}

if (isCli()) {
	const { violations, unused, missing, unreadable, scanned } = scan()

	for (const root of missing) console.warn(`  note: scanned root is not in this checkout: ${root}`)
	for (const directory of unreadable) {
		console.warn(`  note: could not read a scanned path: ${directory}`)
	}
	for (const entry of unused) console.warn(`  unused allowlist entry: ${entry}`)

	if (scanned === 0) {
		console.error(
			'Axolotl dead-colour check scanned no files: every root is missing or unreadable.' +
				' Run it from the repository root, like the sibling guards.',
		)
		process.exit(1)
	}

	if (violations.length > 0) {
		console.error(`Axolotl dead-colour check failed: ${violations.length} class(es) emit no CSS\n`)
		for (const violation of violations) {
			console.error(
				`  ${violation.file}:${violation.line}  ${violation.utility}  — ${violation.reason}`,
			)
		}
		console.error(
			'\nReplace them with the converged token, or add an entry to ALLOWED' +
				' naming the file, the utility and the reason it may stay.',
		)
		process.exit(1)
	}

	console.log(`Axolotl dead-colour check passed (${ALLOWED.size} allowlisted).`)
}
