import { execFileSync } from 'node:child_process'

export const scopeProtocol = 'cumulative-v1'
const workflowPath = '.github/workflows/axolotl-ci.yml'

function command(program, args) {
    return execFileSync(program, args, {
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'pipe'],
        maxBuffer: 16 * 1024 * 1024,
        timeout: 30000,
    }).trim()
}

export function findSuccessfulBaseline({
    repository = process.env.GITHUB_REPOSITORY,
    branch = process.env.GITHUB_REF_NAME,
    runId = process.env.GITHUB_RUN_ID,
    run = command,
    log = console.log,
} = {}) {
    try {
        if (!/^[\w.-]+\/[\w.-]+$/.test(repository || '') || !branch) {
            throw new Error('Missing repository or branch for CI baseline lookup')
        }
        // Bound API work. Falling back to a full check is safe if history is too old.
        for (let page = 1; page <= 10; page++) {
            const response = JSON.parse(
                run('gh', [
                    'api',
                    `repos/${repository}/actions/workflows/axolotl-ci.yml/runs?branch=${encodeURIComponent(branch)}&event=push&status=success&per_page=100&page=${page}`,
                ]),
            )
            if (!Array.isArray(response.workflow_runs))
                throw new Error('Invalid Actions run response')
            for (const candidate of response.workflow_runs) {
                if (
                    String(candidate.id) === String(runId) ||
                    candidate.event !== 'push' ||
                    candidate.status !== 'completed' ||
                    candidate.conclusion !== 'success' ||
                    candidate.head_branch !== branch ||
                    candidate.repository?.full_name?.toLowerCase() !== repository.toLowerCase() ||
                    !/^[a-f0-9]{40}$/i.test(candidate.head_sha || '')
                )
                    continue
                try {
                    run('git', ['merge-base', '--is-ancestor', candidate.head_sha, 'HEAD'])
                    const source = run('git', ['show', `${candidate.head_sha}:${workflowPath}`])
                    // Old successful workflows did not establish a cumulative baseline.
                    if (!source.split(/\r?\n/).includes(`  CI_SCOPE_PROTOCOL: ${scopeProtocol}`))
                        continue
                } catch {
                    // Missing commits and successful runs from rewritten history are not baselines.
                    continue
                }
                log(
                    `Using successful CI run ${candidate.id} at ${candidate.head_sha} as the change baseline`,
                )
                return candidate.head_sha
            }
            if (response.workflow_runs.length < 100) break
        }
        log('No trusted successful CI ancestor found; selecting all checks')
    } catch (error) {
        log(`CI baseline lookup failed; selecting all checks: ${error.message}`)
    }
    return undefined
}
