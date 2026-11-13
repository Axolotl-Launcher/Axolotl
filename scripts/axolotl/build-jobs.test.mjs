import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import { runWithBuildJobs } from './build-jobs.mjs'

test('selects two jobs only for macOS ARM64 and forwards command, arguments and working directory', () => {
    for (const [platform, arch, jobs] of [
        ['darwin', 'arm64', '2'],
        ['darwin', 'x64', '4'],
        ['linux', 'arm64', '4'],
        ['linux', 'x64', '4'],
        ['win32', 'x64', '4'],
    ]) {
        const env = { PATH: 'toolchain' }
        const args = ['clippy', '--all-targets', '--features', 'updater']
        const result = runWithBuildJobs(
            'cargo',
            args,
            { cwd: 'app directory' },
            {
                platform,
                arch,
                env,
                runCommand(command, forwarded, options) {
                    assert.equal(command, 'cargo')
                    assert.equal(forwarded, args)
                    assert.equal(options.cwd, 'app directory')
                    assert.deepEqual(options.env, { ...env, CARGO_BUILD_JOBS: jobs })
                    return 'ran'
                },
            },
        )
        assert.equal(result, 'ran')
        assert.deepEqual(env, { PATH: 'toolchain' })
    }
})

test('preserves an explicit job override for Tauri and propagates command failure', () => {
    const failure = new Error('compiler failed')
    assert.throws(
        () =>
            runWithBuildJobs(
                'pnpm',
                ['exec', 'tauri', 'dev'],
                {},
                {
                    platform: 'darwin',
                    arch: 'arm64',
                    env: { CARGO_BUILD_JOBS: '6' },
                    runCommand(command, args, options) {
                        assert.equal(command, 'pnpm')
                        assert.deepEqual(args, ['exec', 'tauri', 'dev'])
                        assert.equal(options.env.CARGO_BUILD_JOBS, '6')
                        throw failure
                    },
                },
            ),
        (error) => error === failure,
    )
})

test('an empty override uses the platform default', () => {
    runWithBuildJobs(
        'cargo',
        ['--version'],
        {},
        {
            platform: 'darwin',
            arch: 'arm64',
            env: { CARGO_BUILD_JOBS: '' },
            runCommand(command, args, options) {
                assert.equal(options.env.CARGO_BUILD_JOBS, '2')
            },
        },
    )
})

test('the Cargo entry point preserves the invoking package directory', () => {
    const app = fileURLToPath(new URL('../../apps/app/', import.meta.url))
    const wrapper = fileURLToPath(new URL('./cargo.mjs', import.meta.url))
    const options = { cwd: app, encoding: 'utf8' }
    const expected = execFileSync('cargo', ['locate-project', '--message-format', 'plain'], options)
    const actual = execFileSync(
        process.execPath,
        [wrapper, 'locate-project', '--message-format', 'plain'],
        options,
    )
    assert.equal(actual, expected)
})

test('all app test, lint and fix Cargo invocations use the platform-aware wrapper', () => {
    const { scripts } = JSON.parse(
        fs.readFileSync(new URL('../../apps/app/package.json', import.meta.url)),
    )
    for (const name of ['test', 'lint', 'fix']) {
        for (const command of scripts[name].split(' && ')) {
            assert.match(command, /^node \.\.\/\.\.\/scripts\/axolotl\/cargo\.mjs /)
        }
    }
})
