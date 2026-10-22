import fs from 'node:fs'

import { output } from './desktop-build-utils.mjs'

let names
try {
    const base = process.env.CHANGE_BASE
    names =
        !base || /^0+$/.test(base)
            ? undefined
            : output('git', ['diff', '--name-only', `${base}...HEAD`]).split('\n')
} catch {
    names = undefined
}
const shared = (name) =>
    /^(packages\/|scripts\/|\.github\/|standards\/)/.test(name) ||
    /^(Cargo\.(toml|lock)|rust-toolchain\.toml|pnpm-lock\.yaml|pnpm-workspace\.yaml|package\.json|turbo\.jsonc|\.nvmrc|\.cargo\/)/.test(
        name,
    )
const desktop =
    !names ||
    names.some(
        (name) =>
            shared(name) ||
            /^(apps\/(app|app-frontend|installer-ui)\/|third-party\/|\.gitmodules$)/.test(name),
    )
const website = !names || names.some((name) => shared(name) || name.startsWith('apps/website/'))
const flags = { desktop, website, frontend: desktop || website }
for (const [name, value] of Object.entries(flags)) {
    console.log(`${name}=${value}`)
    fs.appendFileSync(process.env.GITHUB_OUTPUT, `${name}=${value}\n`)
}
