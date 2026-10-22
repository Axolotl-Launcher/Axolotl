import { readFile, readdir, stat } from 'node:fs/promises'
import path from 'node:path'

const roots = ['packages/ui/src', 'packages/assets/styles', 'apps/app-frontend/src']
const ignoredDirectories = new Set([
	'node_modules',
	'.vite',
	'dist',
	'.output',
	'__screenshots__',
	'public',
])
const sourceExtensions = new Set([
	'.vue',
	'.scss',
	'.css',
	'.sass',
	'.less',
	'.ts',
	'.js',
	'.mjs',
	'.tsx',
])

// A `var(--x)` whose name is cut short by a non-name character is built by
// string concatenation at runtime (e.g. `var(--color-${platform})`), so there is
// no static name to resolve. Those references are skipped instead of reported.
const terminatorCharacters = new Set([',', ')', ';', ':', ' ', '\t', '\n', '\r', '', '/'])

// Every entry here must name the exact site that provides the variable. An
// entry that cannot be traced to a setter, a declaration, or a documented
// legacy dangling reference belongs in the failure output instead.
const allowlist = new Map()
function allow(group, entries) {
	for (const [name, reason] of entries) allowlist.set(name, { group, reason })
}

// Tailwind's own utilities provide these. A coloured shadow is not a colour
// utility: `shadow-<colour>` only stores the colour, and the declaration that
// draws it has to switch `--tw-shadow` to the coloured variant, exactly as the
// `shadow-*` rule that emits the variable does.
allow('tailwind-internal', [
	[
		'tw-shadow-colored',
		'packages/tooling-config/tailwind emits it from every shadow-* utility; a hand-written coloured shadow in DependencyGraphModal.vue switches --tw-shadow to it',
	],
])

// Values applied through a Vue `:style` binding, `style="--x: …"`, or
// `element.style.setProperty('--x', …)` at runtime, so no stylesheet declares them.
allow('runtime-set', [
    [
        'reka-dropdown-menu-content-available-height',
        'reka-ui src/DropdownMenu/DropdownMenuContent.vue :style maps it to --reka-popper-available-height, measured and set by src/Popper/PopperContent.vue',
    ],
	[
		'_project-color',
		'packages/ui/src/components/project/card/ProjectCard.vue :style `--_project-color`',
	],
	[
		'biome-color',
		'apps/app-frontend/src/components/lab/seed-map/SeedMapBiomePicker.vue :style `--biome-color`',
	],
	[
		'button-color',
		'packages/ui/src/components/base/buttons/ButtonFrame.vue :style `--button-color`',
	],
	['current-value', 'packages/ui/src/components/base/Slider.vue :style `--current-value`'],
	['min-value', 'packages/ui/src/components/base/Slider.vue :style `--min-value`'],
	['max-value', 'packages/ui/src/components/base/Slider.vue :style `--max-value`'],
	[
		'custom-accent-light',
		'apps/app-frontend/src/store/theme.ts setProperty `--custom-accent-light`',
	],
	['custom-accent-dark', 'apps/app-frontend/src/store/theme.ts setProperty `--custom-accent-dark`'],
	[
		'custom-bg-page-alpha-setting',
		'apps/app-frontend/src/store/theme.ts setProperty `--custom-bg-page-alpha-setting`',
	],
	[
		'custom-bg-component-alpha-setting',
		'apps/app-frontend/src/store/theme.ts setProperty `--custom-bg-component-alpha-setting`',
	],
	[
		'floating-action-bar-left-offset',
		'packages/ui/src/components/base/FloatingActionBar.vue :style `--floating-action-bar-left-offset`',
	],
	[
		'floating-action-bar-right-offset',
		'packages/ui/src/components/base/FloatingActionBar.vue :style `--floating-action-bar-right-offset`',
	],
	[
		'home-free-grid-column-pitch',
		'apps/app-frontend/src/components/home/HomeDashboard.vue :style `--home-free-grid-column-pitch`',
	],
	[
		'home-free-grid-row-pitch',
		'apps/app-frontend/src/components/home/HomeDashboard.vue :style `--home-free-grid-row-pitch`',
	],
	[
		'home-greeting-font-family',
		'apps/app-frontend/src/components/home/HomeGreeting.vue :style `--home-greeting-font-family`',
	],
	[
		'home-greeting-font-size',
		'apps/app-frontend/src/components/home/HomeGreeting.vue :style `--home-greeting-font-size`',
	],
	[
		'home-widget-bg-opacity',
		'apps/app-frontend/src/store/theme.ts setProperty `--home-widget-bg-opacity`',
	],
	[
		'minecraft-text-color',
		'apps/app-frontend/src/pages/LabGradientText.vue :style `--minecraft-text-color`',
	],
	[
		'minecraft-text-shadow-color',
		'apps/app-frontend/src/pages/LabGradientText.vue :style `--minecraft-text-shadow-color`',
	],
	[
		'onboarding-dialogue-reserved-space',
		'apps/app-frontend/src/components/ui/onboarding/useOnboardingTour.ts setProperty `--onboarding-dialogue-reserved-space`',
	],
	[
		'property-progress',
		'packages/ui/src/components/image-viewer-editor/controls.vue :style `--property-progress`',
	],
	[
		'recipe-palette-max-height',
		'apps/app-frontend/src/pages/LabRecipeGenerator.vue :style `--recipe-palette-max-height`',
	],
	[
		'reka-select-trigger-width',
		'reka-ui SelectPopperPosition.vue setProperty `--reka-select-trigger-width`',
	],
	[
		'scroll-distance',
		'apps/app-frontend/src/components/ui/Breadcrumbs.vue and packages/ui/src/layouts/shared/files-tab/components/FileNavbar.vue :style `--scroll-distance`',
	],
	['search-stagger', 'apps/app-frontend/src/pages/Settings.vue :style `--search-stagger`'],
	[
		'transparent-window-alpha',
		'apps/app-frontend/src/store/theme.ts setProperty `--transparent-window-alpha`',
	],
])

// Declared in a stylesheet the desktop guard does not scan, but consumed by
// shared packages/ui components on the surface that does declare them.
// (`--color-text`, `--color-text-dark`, `--color-heading` and
// `--color-brand-inverted` used to be listed here; their in-scope references
// were rewritten to the shared aliases, so the entries are gone.)
allow('declared-outside-scan-roots', [
	[
		'size-mobile-navbar-height',
		'apps/website/src/assets/styles/global.scss declares `--size-mobile-navbar-height`',
	],
])

// References with no declaration. These are harmless: an inline fallback, or a
// binding whose live value is provided elsewhere.
allow('known-dangling-reference', [
	[
		'_mouse-x',
		'packages/ui/src/components/modal/NewModal.vue references it only inside a commented-out transform; the live value binds via v-bind(mouseXOffset)',
	],
	[
		'_mouse-y',
		'packages/ui/src/components/modal/NewModal.vue references it only inside a commented-out transform; the live value binds via v-bind(mouseYOffset)',
	],
	[
		'window-controls-width',
		'apps/app-frontend/src/App.vue reads it with an inline `0px` fallback; its setter was removed from WindowControls.vue',
	],
])

async function* files(directory) {
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		if (ignoredDirectories.has(entry.name)) continue
		const entryPath = path.join(directory, entry.name)
		if (entry.isDirectory()) yield* files(entryPath)
		else if (sourceExtensions.has(path.extname(entry.name))) yield entryPath
	}
}

for (const root of roots) {
	try {
		const rootStat = await stat(root)
		if (!rootStat.isDirectory()) throw new Error('not a directory')
	} catch {
		console.error(`Axolotl CSS variable check failed:\nscanned root is missing: ${root}`)
		process.exit(1)
	}
}

const declared = new Set()
const references = new Map()
let dynamicReferences = 0

for (const root of roots) {
	for await (const file of files(root)) {
		const contents = await readFile(file, 'utf8')
		const lines = contents.split(/\r?\n/)
		for (let index = 0; index < lines.length; index++) {
			const line = lines[index]
			for (const match of line.matchAll(/(?<![\w-])--([a-zA-Z_][\w-]*)\s*:/g)) {
				declared.add(match[1])
			}
			for (const match of line.matchAll(/var\(\s*--([a-zA-Z_][\w-]*)/g)) {
				const name = match[1]
				const following = line[match.index + match[0].length] ?? ''
				if (!terminatorCharacters.has(following)) {
					dynamicReferences++
					continue
				}
				if (!references.has(name)) references.set(name, [])
				references.get(name).push(`${file}:${index + 1}`)
			}
		}
	}
}

const failures = []
const seen = new Set()
for (const name of [...references.keys()].sort()) {
	if (declared.has(name)) continue
	seen.add(name)
	if (allowlist.has(name)) continue
	const sites = references.get(name)
	failures.push(`--${name} (${sites.length} reference${sites.length === 1 ? '' : 's'})`)
	for (const site of sites) failures.push(`  ${site}`)
}

const stale = [...allowlist.keys()].filter((name) => !seen.has(name))
if (stale.length > 0) {
	console.warn(
		`Axolotl CSS variable allowlist has ${stale.length} unused entr${stale.length === 1 ? 'y' : 'ies'} (the variable is gone or now declared); remove ${stale.map((name) => `--${name}`).join(', ')}.`,
	)
}

if (failures.length > 0) {
	console.error(
		`Axolotl CSS variable check failed: ${failures.filter((line) => line.startsWith('--')).length} undefined variable(s).\n` +
			'Declare the variable in a stylesheet or add a justified entry to the allowlist in scripts/axolotl/check-css-vars.mjs.\n' +
			failures.join('\n'),
	)
	process.exit(1)
}

console.log(
	`Axolotl CSS variable check passed (${allowlist.size} allowlisted, ${dynamicReferences} dynamic reference(s) skipped).`,
)
