import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'

test('CNB mirrors the configured GitHub repository and verifies the final tagged manifest', (t) => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-cnb-test-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    const tag = 'v1.10.0-beta.5'
    const names = [
        'Axolotl_universal.app.tar.gz',
        'Axolotl_aarch64.AppImage.tar.gz',
        'Axolotl_amd64.AppImage.tar.gz',
        'Axolotl_x64-setup.nsis.zip',
    ]
    const platforms = Object.fromEntries(
        ['darwin-aarch64', 'darwin-x86_64', 'linux-aarch64', 'linux-x86_64', 'windows-x86_64'].map(
            (platform, index) => [
                platform,
                {
                    signature: 'signature'.repeat(12),
                    url: `https://github.com/Example/Fork/releases/download/${tag}/${names[Math.max(0, index - 1)]}`,
                },
            ],
        ),
    )
    const manifest = { version: tag.slice(1), platforms }
    const release = {
        id: 123,
        tag_name: tag,
        draft: false,
        published_at: '2026-10-22T12:00:00Z',
        assets: names.map((name) => ({
            name,
            size: 4,
            browser_download_url: `https://github.com/Example/Fork/releases/download/${tag}/${name}`,
        })),
    }
    const preloader = `
        import assert from 'node:assert/strict'
        const tag = ${JSON.stringify(tag)}
        const release = ${JSON.stringify(release)}
        const manifest = ${JSON.stringify(manifest)}
        const json = (value, status = 200) => new Response(JSON.stringify(value), { status })
        globalThis.fetch = async (url, options = {}) => {
            const value = String(url)
            if (value === 'https://github.com/Example/Fork/releases/download/' + tag + '/latest.json') return json(manifest)
            if (value === 'https://api.github.com/repos/Example/Fork/releases/tags/' + tag) return json(release)
            if (value === 'https://api.github.com/repos/Example/Fork/releases/latest') return json({ tag_name: 'v1.9.8', draft: false, prerelease: false })
            if (release.assets.some(asset => asset.browser_download_url === value)) return new Response('data')
            if (value === 'https://api.cnb.cool/axlmc/Axolotl/-/releases/tags/' + tag) return json({}, 404)
            if (value === 'https://api.cnb.cool/axlmc/Axolotl/-/releases' && options.method === 'POST') return json({ id: 456 })
            if (value.endsWith('/asset-upload-url')) return json({ upload_url: 'https://upload.example/file', verify_url: '/verify' })
            if (value === 'https://upload.example/file') {
                for await (const chunk of options.body) assert(chunk.length > 0)
                return new Response('')
            }
            if (value === 'https://api.cnb.cool/verify?ttl=0') return new Response('')
            if (value === 'https://api.cnb.cool/axlmc/Axolotl/-/releases/456' && options.method === 'PATCH') {
                const body = JSON.parse(options.body)
                assert.equal(body.prerelease, true)
                assert.equal(body.make_latest, 'false')
                return json({})
            }
            throw new Error('Unexpected request: ' + value)
        }
    `
    const result = spawnSync(
        process.execPath,
        [
            '--import',
            `data:text/javascript,${encodeURIComponent(preloader)}`,
            path.join(import.meta.dirname, 'cnb-release.mjs'),
            'finalize',
            tag,
            directory,
        ],
        {
            encoding: 'utf8',
            timeout: 15000,
            env: {
                ...process.env,
                CNB_TOKEN: 'test-token',
                GITHUB_REPOSITORY: 'Example/Fork',
                CNB_API_ENDPOINT: 'https://api.cnb.cool',
                CNB_REPO_SLUG: 'axlmc/Axolotl',
                GITHUB_API_BASE_URL: '',
                GITHUB_RELEASE_BASE_URL: '',
            },
        },
    )
    assert.equal(result.status, 0, result.stderr)
    const output = JSON.parse(fs.readFileSync(path.join(directory, 'latest.json'), 'utf8'))
    assert.equal(output.version, '1.10.0-beta.5')
    assert.equal(
        output.platforms['darwin-aarch64'].url,
        `https://cnb.cool/axlmc/Axolotl/-/releases/download/${tag}/${names[0]}`,
    )
})
