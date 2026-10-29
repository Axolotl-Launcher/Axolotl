import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { findSuccessfulBaseline, scopeProtocol } from './ci-baseline.mjs'
import { detectChanges } from './ci-changes.mjs'
import { validateCiResults } from './ci-result.mjs'

const sha = (letter) => letter.repeat(40)
const env = {
    CI_SCOPE_PROTOCOL: scopeProtocol,
    GITHUB_EVENT_NAME: 'push',
    GITHUB_REPOSITORY: 'Example/Repo',
    GITHUB_REF_NAME: 'main',
    GITHUB_RUN_ID: '999',
    CHANGE_BASE: sha('b'),
}
const success = (head = sha('a'), overrides = {}) => ({
    id: 123,
    head_sha: head,
    event: 'push',
    status: 'completed',
    conclusion: 'success',
    head_branch: 'main',
    repository: { full_name: 'Example/Repo' },
    ...overrides,
})
const log = () => {}

test('baseline lookup rejects old strategy, failed, cancelled, foreign and non-ancestor runs', () => {
    const candidates = [
        success(sha('b'), { conclusion: 'failure' }),
        success(sha('b'), { conclusion: 'cancelled' }),
        success(sha('b'), { status: 'in_progress' }),
        success(sha('b'), { event: 'pull_request' }),
        success(sha('b'), { head_branch: 'other' }),
        success(sha('b'), { repository: { full_name: 'Other/Repo' } }),
        success(sha('b'), { id: 999 }),
        success(sha('c')),
        success(sha('d')),
        success(),
    ]
    const visited = []
    const run = (program, args) => {
        if (program === 'gh') return JSON.stringify({ workflow_runs: candidates })
        visited.push(args)
        if (args[0] === 'merge-base' && args[2] === sha('c')) throw new Error('not ancestor')
        if (args[0] === 'show')
            return args[1].startsWith(sha('d'))
                ? 'old workflow'
                : `env:\n  CI_SCOPE_PROTOCOL: ${scopeProtocol}\n`
        return ''
    }
    assert.equal(
        findSuccessfulBaseline({
            repository: env.GITHUB_REPOSITORY,
            branch: 'main',
            runId: '999',
            run,
            log,
        }),
        sha('a'),
    )
    assert(!visited.some((args) => args.includes(sha('b'))))
})

test('lookup paginates and falls back safely on API errors or missing trusted history', () => {
    let calls = 0
    const run = (program, args) => {
        if (program === 'gh') {
            calls++
            return JSON.stringify({
                workflow_runs:
                    calls === 1
                        ? Array.from({ length: 100 }, () => success(sha('d')))
                        : [success()],
            })
        }
        return args[0] === 'show' && args[1].startsWith(sha('a'))
            ? `  CI_SCOPE_PROTOCOL: ${scopeProtocol}`
            : ''
    }
    const options = { repository: env.GITHUB_REPOSITORY, branch: 'main', runId: '999', log }
    assert.equal(findSuccessfulBaseline({ ...options, run }), sha('a'))
    assert.equal(calls, 2)
    assert.equal(
        findSuccessfulBaseline({
            ...options,
            run: () => {
                throw new Error('403')
            },
        }),
        undefined,
    )
    assert.equal(
        findSuccessfulBaseline({ ...options, run: () => JSON.stringify({ workflow_runs: [] }) }),
        undefined,
    )
})

test('cancelled desktop push followed by docs retains desktop scope; successful baseline advances', (t) => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-ci-scope-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    const git = (args) =>
        execFileSync('git', args, {
            cwd: directory,
            encoding: 'utf8',
            stdio: ['ignore', 'pipe', 'pipe'],
        }).trim()
    git(['init'])
    git(['config', 'user.name', 'CI test'])
    git(['config', 'user.email', 'ci@example.invalid'])
    const commit = (name, contents) => {
        const file = path.join(directory, name)
        fs.mkdirSync(path.dirname(file), { recursive: true })
        fs.writeFileSync(file, contents)
        git(['add', '--all'])
        git(['commit', '-m', 'test: scope fixture'])
        return git(['rev-parse', 'HEAD'])
    }
    const initial = commit(
        '.github/workflows/axolotl-ci.yml',
        `env:\n  CI_SCOPE_PROTOCOL: ${scopeProtocol}\n`,
    )
    const desktop = commit('apps/app/src/main.rs', 'desktop')
    commit('README.md', 'docs')
    let baseline = initial
    const run = (program, args) =>
        program === 'gh' ? JSON.stringify({ workflow_runs: [success(baseline)] }) : git(args)
    const pushEnv = { ...env, CHANGE_BASE: desktop }
    assert.deepEqual(detectChanges({ env: pushEnv, run, log }), {
        desktop: true,
        website: false,
        frontend: true,
    })
    baseline = git(['rev-parse', 'HEAD'])
    commit('README.md', 'more docs')
    assert.deepEqual(detectChanges({ env: pushEnv, run, log }), {
        desktop: false,
        website: false,
        frontend: false,
    })
    commit('apps/website/page.vue', 'website')
    assert.deepEqual(detectChanges({ env: pushEnv, run, log }), {
        desktop: false,
        website: true,
        frontend: true,
    })
    // PRs retain the cumulative base and must not query Actions.
    const prRun = (program, args) => {
        assert.equal(program, 'git')
        return git(args)
    }
    assert.deepEqual(
        detectChanges({
            env: { ...env, GITHUB_EVENT_NAME: 'pull_request', CHANGE_BASE: initial },
            run: prRun,
            log,
        }),
        { desktop: true, website: true, frontend: true },
    )
    const neverRun = () => assert.fail('No commands expected')
    assert.deepEqual(
        detectChanges({
            env: { ...env, GITHUB_EVENT_NAME: 'workflow_dispatch' },
            run: neverRun,
            log,
        }),
        { desktop: true, website: true, frontend: true },
    )
    // Repo checks share the detector but keep their existing per-push behavior.
    assert.deepEqual(detectChanges({ env: { CHANGE_BASE: baseline }, run: prRun, log }), {
        desktop: false,
        website: true,
        frontend: true,
    })
    // Renaming a Rust file into docs must include its removed source path.
    baseline = git(['rev-parse', 'HEAD'])
    git(['mv', 'apps/app/src/main.rs', 'desktop-example.md'])
    git(['commit', '-m', 'test: rename fixture'])
    assert.equal(detectChanges({ env: pushEnv, run, log }).desktop, true)
})

test('lookup/diff failures force all scopes rather than using the last push as fallback', () => {
    const run = () => {
        throw new Error('unavailable')
    }
    const all = { desktop: true, website: true, frontend: true }
    assert.deepEqual(detectChanges({ env, run, log }), all)
    assert.deepEqual(
        detectChanges({ env, run: () => JSON.stringify({ workflow_runs: [] }), log }),
        all,
    )
    assert.deepEqual(
        detectChanges({ env: { ...env, GITHUB_EVENT_NAME: 'pull_request' }, run, log }),
        all,
    )
})

test('CI result rejects skipped selected jobs and cancelled runs before establishing a baseline', () => {
    const jobs = {
        changes: {
            result: 'success',
            outputs: { desktop: 'true', frontend: 'true', website: 'false' },
        },
        guardrails: { result: 'success' },
        frontend: { result: 'success' },
        resources: { result: 'success' },
        desktop: { result: 'success' },
        website: { result: 'skipped' },
    }
    assert.doesNotThrow(() => validateCiResults(jobs))
    for (const result of ['skipped', 'cancelled', 'failure']) {
        assert.throws(
            () => validateCiResults({ ...jobs, desktop: { result } }),
            /desktop: expected success/,
        )
    }
    assert.throws(
        () => validateCiResults({ ...jobs, guardrails: { result: 'skipped' } }),
        /guardrails/,
    )
    assert.throws(
        () => validateCiResults({ ...jobs, changes: { result: 'success', outputs: {} } }),
        /Invalid desktop scope/,
    )
})
