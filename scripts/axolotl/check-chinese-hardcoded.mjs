import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

/**
 *   node scripts/axolotl/check-chinese-hardcoded.mjs           # 汇总
 *   node scripts/axolotl/check-chinese-hardcoded.mjs --detail  # 逐行
 *   node scripts/axolotl/check-chinese-hardcoded.mjs --ci      # 发现即退出 1
 */

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(scriptDir, '..', '..')

const CJK = /[\u4e00-\u9fff\u3400-\u4dbf\uf900-\ufaff\uff00-\uffef\u3000-\u303f]/

const SKIP_DIRS = new Set([
	'node_modules',
	'target',
	'dist',
	'.git',
	'.turbo',
	'.pnpm-store',
	'.nuxt',
	'.output',
	'.vite',
	'coverage',
	'__pycache__',
	'third-party',
	'vendor',
	'.cargo',
	'.codegraph',
	'.omo',
	'.opencode',
	'.agents',
	'.idea',
	'.vscode',
])

const SKIP_FILE = [
	/\.mdx?$/i,
	/(^|\/)locales\//,
	/src\/locales\/site\.ts$/,
	/announcements\/catalog\.ts$/,
	/-zh\.(ts|json)$/,
	/\/daily-challenges\.ts$/,
	/create-release-notes\.mjs$/,
	/\/installer-ui\//,
	/lightweight_mode\.rs$/,
	/oauth_utils\/auth_code_reply\.rs$/,
	/packages\/ui\/src\/composables\/i18n\.ts$/,
	/game-settings-modal\/languages\.ts$/,
	/^apps\/website\//,
	/\/mod_translation(\.rs|\/)/,
	/\/AIIcon\.vue$/,
	/\/helpers\/translation\.ts$/,
	/\.test\.[tj]s$/,
	/\.spec\.[tj]sx?$/,
	/(^|\/)generated\//,
	/\/storageData\.ts$/,
	/\/recipe-layouts\.ts$/,
	/\/launcher\/direct_link\.rs$/,
	/\/logs\/crash_analysis\.rs$/,
	/(^|\/)public\//,
	/\/releases\/catalog\.json$/,
	/\/item-name-index\.json$/,
	/\/block-name-index\.json$/,
	/\/lobehub-provider-descriptions\//,
	/\/curseforge-category-map\.ts$/,
	/monaco\//,
	/\.svg$/,
	/\.lock$/,
	/pnpm-lock\.yaml$/,
	/Cargo\.lock$/,
	/\.min\.(js|css)$/,
	/web-types\.json$/,
	/\.d\.ts$/,
	/\/check-chinese-hardcoded\.mjs$/,
]
const SKIP_LINE = [/locale\.value\.startsWith\(/]

const EXTS = new Set([
	'.vue',
	'.ts',
	'.tsx',
	'.js',
	'.jsx',
	'.mjs',
	'.cjs',
	'.rs',
	'.html',
	'.css',
	'.scss',
	'.json',
	'.md',
	'.toml',
	'.yaml',
	'.yml',
])

async function* walk(dir) {
	let entries
	try {
		entries = await readdir(dir, { withFileTypes: true })
	} catch {
		return
	}
	for (const entry of entries) {
		if (entry.isDirectory()) {
			if (SKIP_DIRS.has(entry.name) || entry.name.startsWith('.')) continue
			yield* walk(path.join(dir, entry.name))
		} else if (entry.isFile()) {
			yield path.join(dir, entry.name)
		}
	}
}

function classifyLine(line) {
	const trimmed = line.trimStart()
	if (
		trimmed.startsWith('//') ||
		trimmed.startsWith('/*') ||
		trimmed.startsWith('*') ||
		trimmed.startsWith('<!--') ||
		trimmed.startsWith('#!') ||
		(trimmed.startsWith('#') && !trimmed.startsWith('#[')) ||
		trimmed.startsWith('///')
	) {
		return 'comment'
	}
	return 'string'
}

const results = []
const seen = new Set()

for await (const file of walk(ROOT)) {
	const rel = path.relative(ROOT, file).replace(/\\/g, '/')
	if (!EXTS.has(path.extname(file))) continue
	if (SKIP_FILE.some((re) => re.test(rel))) continue

	let text
	try {
		text = await readFile(file, 'utf8')
	} catch {
		continue
	}
	if (file.endsWith('.rs')) {
		const testAt = text.search(/#\[cfg\((all\()?test/)
		if (testAt !== -1) text = text.slice(0, testAt)
	}

	if (!CJK.test(text)) continue

	const lines = text.split(/\r?\n/)
	let inBlockComment = false
	lines.forEach((line, i) => {
		if (!CJK.test(line)) return
		if (SKIP_LINE.some((re) => re.test(line))) return

		let kind = classifyLine(line)

		// 追踪块注释
		if (inBlockComment) {
			kind = 'comment'
			if (line.includes('*/')) inBlockComment = false
		} else if (!line.includes('*/') && /\/\*/.test(line)) {
			inBlockComment = true
		}

		// 行内注释（代码后跟 // 中文）
		if (kind === 'string') {
			const idx = line.search(/(^|[^:'"\/\/])\/\/\s/)
			if (idx !== -1) kind = 'comment'
		}

		const matched = line.match(
			/[\u4e00-\u9fff\u3400-\u4dbf\uf900-\ufaff\uff00-\uffef\u3000-\u303f]+/g,
		)
		results.push({
			file: rel,
			line: i + 1,
			kind,
			snippet: line.trim().slice(0, 160),
			chars: matched ? matched.join('') : '',
		})
		seen.add(rel)
	})
}

const strings = results.filter((r) => r.kind === 'string')
const comments = results.filter((r) => r.kind === 'comment')

const byFile = new Map()
for (const r of strings) {
	if (!byFile.has(r.file)) byFile.set(r.file, [])
	byFile.get(r.file).push(r)
}
const sortedFiles = [...byFile.entries()].sort((a, b) => b[1].length - a[1].length)

const detail = process.argv.includes('--detail')
const ci = process.argv.includes('--ci')

console.log(`扫描根目录: ${ROOT}`)
console.log(
	`命中总行数: ${results.length}（疑似可见 ${strings.length} 行，注释 ${comments.length} 行）`,
)
console.log(`涉及文件: ${seen.size} 个\n`)

console.log(`文件共 ${sortedFiles.length} 个`)
for (const [file, hits] of sortedFiles) {
	console.log(`  ${String(hits.length).padStart(4)}  ${file}`)
}

if (detail) {
	console.log('\n明细')
	for (const [file, hits] of sortedFiles) {
		for (const h of hits) {
			console.log(`${file}:${h.line}: ${h.snippet}`)
		}
	}
}

if (ci && strings.length > 0) {
	console.error(`\n发现 ${strings.length} 行`)
	process.exit(1)
}
