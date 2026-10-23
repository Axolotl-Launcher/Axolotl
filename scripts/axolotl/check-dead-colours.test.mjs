/**
 * Fixture coverage for the parts the inline contract cannot reach: the walk, the
 * allowlist, the reporting and the missing-root notice. Every root is a fresh
 * temporary directory, so the repository is never scanned from here and nothing
 * is written inside a scanned root.
 */
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import { scan } from './check-dead-colours.mjs'

const fixture = (files) => {
	const root = mkdtempSync(join(tmpdir(), 'dead-colours-'))
	for (const [name, contents] of Object.entries(files)) {
		const path = join(root, name)
		mkdirSync(dirname(path), { recursive: true })
		writeFileSync(path, contents)
	}
	return root
}

test('reports the full token and leaves the converged spellings alone', () => {
	const root = fixture({
		'src/a.vue': '<i class="hover:!text-secondary text-[var(--color-text-primary)]" />',
		'src/skip.test.ts': 'const className = "text-secondary"',
		'src/other.test.mjs': 'export const name = "text-primary"',
		'src/more.spec.mjs': 'export const name = "text-contrast"',
	})

	const { violations } = scan([root])

	assert.deepEqual(
		violations.map((violation) => violation.utility),
		['hover:!text-secondary'],
	)
	assert.equal(violations[0].reason, 'use text-[var(--color-text-tertiary)]')
})

test('reports the file and the line of a bare utility', () => {
	const root = fixture({ 'src/b.ts': 'export const one = 1\nconst two = "bg-secondary"\n' })

	const { violations } = scan([root])

	assert.equal(violations.length, 1)
	assert.equal(violations[0].line, 2)
	assert.equal(violations[0].utility, 'bg-secondary')
	assert.equal(violations[0].reason, 'use bg-[var(--color-text-tertiary)]')
})

test('skips the directories the scan is not meant to read', () => {
	const root = fixture({
		'node_modules/pkg/c.vue': '<i class="text-primary" />',
		'dist/assets/d.css': '.text-primary { color: red }',
		'__screenshots__/f.vue': '<i class="text-secondary" />',
		'src/nested/e.vue': '<i class="text-contrast" />',
	})

	const { violations } = scan([root])

	assert.deepEqual(
		violations.map((violation) => violation.file.replace(root, '').replaceAll('\\', '/')),
		['/src/nested/e.vue'],
	)
})

test('names a root that is not in the checkout instead of skipping it silently', () => {
	const absent = join(tmpdir(), 'dead-colours-absent-root')
	const present = fixture({ 'src/a.vue': '<i class="text-primary" />' })

	const { missing, unreadable, violations } = scan([absent, present])

	assert.deepEqual(missing, [absent])
	assert.deepEqual(unreadable, [])
	assert.deepEqual(
		violations.map((violation) => violation.utility),
		['text-primary'],
	)
})

test('fails from the command line when no root can be read', () => {
	const cwd = mkdtempSync(join(tmpdir(), 'dead-colours-empty-'))
	const guard = fileURLToPath(new URL('./check-dead-colours.mjs', import.meta.url))

	assert.throws(
		() => execFileSync(process.execPath, [guard], { cwd, stdio: 'pipe' }),
		/scanned no files/,
	)
})
