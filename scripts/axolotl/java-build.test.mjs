import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs/promises'
import http from 'node:http'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../..', import.meta.url))
const project = path.join(root, 'packages/app-lib/java')

test(
    'pinned Java agent cache, offline mode and checksum enforcement',
    { timeout: 300_000 },
    async (t) => {
        await fs.mkdir(path.join(root, 'target'), { recursive: true })
        const directory = await fs.mkdtemp(path.join(root, 'target/java-build-test-'))
        const artifact = Buffer.from('pinned Java agent test artifact')
        const checksum = createHash('sha256').update(artifact).digest('hex')
        let requests = 0
        let response = artifact
        const server = http.createServer((_request, result) => {
            requests++
            result.end(response)
        })
        await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
        t.after(async () => {
            await new Promise((resolve) => server.close(resolve))
            await fs.rm(directory, { recursive: true, force: true })
        })
        const source = (await fs.readFile(path.join(project, 'build.gradle.kts'), 'utf8'))
            .replace(
                'https://authlib-injector.yushi.moe/artifact/56/authlib-injector-1.2.8.jar',
                `http://127.0.0.1:${server.address().port}/agent.jar`,
            )
            .replace('9c7f4343e6c82034958ffb48c14a2cb0c85928be7283103ce17da00c6d5a7b10', checksum)
        await fs.writeFile(
            path.join(directory, 'build.gradle.kts'),
            `${source}\ntasks.named<DownloadPinnedAgent>("authlibInjector") {\n    cacheFile.set(layout.projectDirectory.file("cache/agent.jar"))\n}\n`,
        )
        for (const file of [
            'settings.gradle.kts',
            'gradle.properties',
            'gradle/libs.versions.toml',
        ]) {
            await fs.mkdir(path.dirname(path.join(directory, file)), { recursive: true })
            await fs.copyFile(path.join(project, file), path.join(directory, file))
        }
        const target = path.join(directory, 'build/libs/authlib-injector.jar')
        const cached = path.join(directory, 'cache/agent.jar')
        const java = process.env.JAVA_HOME
            ? path.join(
                  process.env.JAVA_HOME,
                  'bin',
                  process.platform === 'win32' ? 'java.exe' : 'java',
              )
            : 'java'
        async function run({ offline = true, success = true, extra = [] } = {}) {
            const args = [
                '-jar',
                path.join(project, 'gradle/wrapper/gradle-wrapper.jar'),
                '-p',
                directory,
                'authlibInjector',
                '--no-daemon',
                '--no-build-cache',
                '--console=plain',
                ...(offline ? ['--offline'] : []),
                ...extra,
            ]
            const result = await new Promise((resolve, reject) => {
                const child = spawn(java, args, { cwd: root, windowsHide: true })
                let output = ''
                child.stdout.on('data', (data) => {
                    output += data
                })
                child.stderr.on('data', (data) => {
                    output += data
                })
                child.on('error', reject)
                child.on('close', (code) => resolve({ code, output }))
            })
            assert.equal(result.code === 0, success, result.output)
            assert.doesNotMatch(
                result.output,
                /configuration cache problems|cannot serialize|configuration cache entry discarded/i,
            )
            return result.output
        }
        await t.test(
            'preseeded output works offline without task history and seeds the shared cache',
            async () => {
                await fs.mkdir(path.dirname(target), { recursive: true })
                await fs.writeFile(target, artifact)
                await run()
                assert.deepEqual(await fs.readFile(cached), artifact)
                assert.equal(requests, 0)
            },
        )
        await t.test('a new output directory reuses the shared cache offline', async () => {
            const alternate = path.join(directory, 'alternate-output')
            await run({ extra: [`-PaxolotlBuildDir=${alternate}`] })
            assert.deepEqual(
                await fs.readFile(path.join(alternate, 'libs/authlib-injector.jar')),
                artifact,
            )
            assert.equal(requests, 0)
        })
        await t.test('corrupt caches fail offline without making a request', async () => {
            await fs.writeFile(target, 'corrupt output')
            await fs.writeFile(cached, 'corrupt cache')
            const output = await run({ success: false })
            assert.match(output, /missing or invalid in offline mode/)
            assert.equal(requests, 0)
        })
        await t.test(
            'online recovery downloads once and forced execution reuses the valid artifact',
            async () => {
                await run({ offline: false })
                assert.deepEqual(await fs.readFile(target), artifact)
                assert.deepEqual(await fs.readFile(cached), artifact)
                assert.equal(requests, 1)
                await run({ extra: ['--rerun-tasks'] })
                assert.equal(requests, 1)
            },
        )
        await t.test(
            'output corruption is repaired from cache even with configuration cache reuse',
            async () => {
                await fs.writeFile(target, 'corrupt output')
                const output = await run()
                assert.match(output, /Reusing configuration cache/)
                assert.deepEqual(await fs.readFile(target), artifact)
                assert.equal(requests, 1)
            },
        )
        await t.test(
            'invalid downloads do not overwrite the old output and can be retried',
            async () => {
                await fs.writeFile(target, 'old invalid output')
                await fs.rm(cached)
                response = Buffer.from('wrong download')
                const output = await run({ offline: false, success: false })
                assert.match(output, /checksum mismatch/)
                assert.equal(await fs.readFile(target, 'utf8'), 'old invalid output')
                response = artifact
                await run({ offline: false })
                assert.deepEqual(await fs.readFile(target), artifact)
                assert.equal(requests, 3)
            },
        )
        await t.test('updating the pinned checksum rejects the previous artifact', async () => {
            const script = path.join(directory, 'build.gradle.kts')
            await fs.writeFile(
                script,
                (await fs.readFile(script, 'utf8')).replace(checksum, '0'.repeat(64)),
            )
            const output = await run({ success: false })
            assert.match(output, /missing or invalid in offline mode/)
            assert.equal(requests, 3)
        })
    },
)
