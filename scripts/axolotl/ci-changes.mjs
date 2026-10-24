import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import { pathToFileURL } from 'node:url'

import { findSuccessfulBaseline, scopeProtocol } from './ci-baseline.mjs'

function command(program, args) {
    return execFileSync(program, args, {
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'pipe'],
        maxBuffer: 16 * 1024 * 1024,
        timeout: 30000,
    }).trim()
}

export function detectChanges({ env = process.env, run = command, log = console.log } = {}) {
    let names
    try {
        let base = env.CHANGE_BASE
        if (env.CI_SCOPE_PROTOCOL === scopeProtocol) {
            if (env.GITHUB_EVENT_NAME === 'push') {
                base = findSuccessfulBaseline({
                    repository: env.GITHUB_REPOSITORY,
                    branch: env.GITHUB_REF_NAME,
                    runId: env.GITHUB_RUN_ID,
                    run,
                    log,
                })
            } else if (env.GITHUB_EVENT_NAME !== 'pull_request') {
                base = undefined
                log('Manual CI run: selecting all checks')
            }
        }
        if (base && !/^0+$/.test(base)) {
            const baseline = run('git', ['rev-parse', '--verify', `${base}^{commit}`])
            if (env.CI_SCOPE_PROTOCOL === scopeProtocol && env.GITHUB_EVENT_NAME === 'push') {
                run('git', ['merge-base', '--is-ancestor', baseline, 'HEAD'])
            }
            names = run('git', ['diff', '--name-only', '-z', '--no-renames', `${baseline}...HEAD`])
                .split('\0')
                .filter(Boolean)
            log(`Checking changes from ${baseline} to HEAD`)
        }
    } catch (error) {
        log(`Change detection failed; selecting all checks: ${error.message}`)
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
    return { desktop, website, frontend: desktop || website }
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
    const flags = detectChanges()
    for (const [name, value] of Object.entries(flags)) {
        console.log(`${name}=${value}`)
        fs.appendFileSync(process.env.GITHUB_OUTPUT, `${name}=${value}\n`)
    }
}
