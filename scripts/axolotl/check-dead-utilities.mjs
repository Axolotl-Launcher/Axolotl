#!/usr/bin/env node
/**
 * Finds `/NN` alpha utilities that emit no CSS at all.
 *
 * The Tailwind preset maps its colours (surface, brand, red, …) to bare `var()`
 * values, so Tailwind cannot inject an alpha and silently generates no rule:
 * `bg-surface-3/80` reads like a background in review and paints nothing. Lint,
 * type-check and build all stay green, which is why these survive.
 *
 * The built stylesheet is the oracle, because that is exactly what the browser
 * sees. Build the app and the website first, then run this.
 *
 * Entries in ALLOWED name a file, a class and the reason it may stay.
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'

const ROOTS = ['packages/ui/src', 'apps/app-frontend/src', 'apps/website/src']
const CSS_DIRS = ['apps/app-frontend/dist/assets', 'apps/website/dist']
const ALLOWED = []

const UTILITY =
	/(?:^|[\s"'`=:[({])((?:bg|text|border|ring|from|via|to|divide|outline|shadow|fill|stroke|decoration|accent|caret|placeholder)-[a-z0-9-]+(?:\/\d+|\[[^\]]+\]))/g

function walk(dir, files = []) {
	let entries
	try {
		entries = readdirSync(dir)
	} catch {
		return files
	}
	for (const entry of entries) {
		const path = join(dir, entry)
		const stat = statSync(path)
		if (stat.isDirectory()) walk(path, files)
		else if (/\.(vue|ts|js|scss|css)$/.test(entry) && !entry.includes('.test.')) files.push(path)
	}
	return files
}

function builtCss() {
	const chunks = []
	const missing = []
	const collect = (dir) => {
		let entries
		try {
			entries = readdirSync(dir, { withFileTypes: true })
		} catch {
			missing.push(dir)
			return
		}
		for (const entry of entries) {
			const path = join(dir, entry.name)
			if (entry.isDirectory()) collect(path)
			else if (entry.name.endsWith('.css')) chunks.push(readFileSync(path, 'utf8'))
		}
	}
	for (const dir of CSS_DIRS) collect(dir)
	if (missing.length > 0) {
		console.warn(
			`  note: built CSS is not in this checkout, so the scan is narrower: ${missing.join(', ')}`,
		)
	}
	return chunks.join('\n')
}

// Tailwind escapes every character that is not valid in a CSS identifier.
const escape = (name) => name.replace(/[.:/[\]%!#(),]/g, (char) => `\\${char}`)

const css = builtCss()
if (!css) {
	console.error('No built CSS found. Build the app (and website) first.')
	process.exit(1)
}

const dead = []
const allowed = new Set(ALLOWED.map((entry) => `${entry.file}|${entry.className}`))

for (const file of ROOTS.flatMap((root) => walk(root))) {
	const source = readFileSync(file, 'utf8')
	for (const match of source.matchAll(UTILITY)) {
		const className = match[1]
		if (allowed.has(`${file}|${className}`)) continue
		if (css.includes(`.${escape(className)}`)) continue
		const line = source.slice(0, match.index).split('\n').length
		dead.push({ file, line, className })
	}
}

if (dead.length === 0) {
	console.log('No dead alpha utilities: every /NN utility in the scanned roots emits CSS.')
	process.exit(0)
}

console.error(`Dead alpha utilities (${dead.length}) — these paint nothing:`)
for (const { file, line, className } of dead) console.error(`  ${file}:${line}  ${className}`)
console.error(
	'\nRoute them through the opacity model (see packages/assets/styles/opacity.scss) or drop the alpha.',
)
process.exit(1)
