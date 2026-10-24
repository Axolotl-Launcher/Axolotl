import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'

test('generate stable tagged URLs from draft assets and verify the repository and tag', () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-manifest-'))
    const tag = 'v1.10.0-beta.5'
    const releasePath = path.join(directory, 'release.json')
    const outputPath = path.join(directory, 'latest.json')
    const signature = `${'signed-update-'.repeat(8)}\n`
    const names = [
        'Axolotl Launcher_universal.app.tar.gz',
        'Axolotl.Launcher_aarch64.AppImage.tar.gz',
        'Axolotl.Launcher_amd64.AppImage.tar.gz',
        'Axolotl.Launcher_x64-setup.nsis.zip',
    ]
    const platformNames = {
        'darwin-aarch64': names[0],
        'darwin-x86_64': names[0],
        'linux-aarch64': names[1],
        'linux-x86_64': names[2],
        'windows-x86_64': names[3],
    }
    const sha256 = (name) => createHash('sha256').update(name).digest('hex')
    const release = {
        tag_name: tag,
        draft: true,
        body: 'Existing release notes',
        assets: names
            .flatMap((name) => [name, `${name}.sig`])
            .map((name) => ({
                name,
                // GitHub reports lowercase hex; one asset stays uppercase to prove normalization.
                digest: `sha256:${name === names[1] ? sha256(name).toUpperCase() : sha256(name)}`,
                browser_download_url: `https://github.com/Axolotl-Launcher/Axolotl/releases/download/untagged-example/${name}`,
            })),
    }
    const run = (script, args, repository = 'Axolotl-Launcher/Axolotl') =>
        spawnSync(process.execPath, [path.join(import.meta.dirname, script), ...args], {
            encoding: 'utf8',
            env: { ...process.env, GITHUB_REPOSITORY: repository },
        })
    const generate = (repository) =>
        run('create-update-manifest.mjs', [releasePath, directory, tag, outputPath], repository)
    const verify = (repository, source = 'github') =>
        run('verify-update-manifest.mjs', [outputPath, tag, source], repository)

    try {
        fs.writeFileSync(releasePath, JSON.stringify(release))
        for (const name of names) fs.writeFileSync(path.join(directory, `${name}.sig`), signature)

        for (const repository of ['Axolotl-Launcher/Axolotl', 'Example/Fork']) {
            const result = generate(repository)
            assert.equal(result.status, 0, result.stderr)
            const manifest = JSON.parse(fs.readFileSync(outputPath, 'utf8'))
            assert.equal(manifest.version, '1.10.0-beta.5')
            assert.equal(manifest.notes, release.body)
            assert.equal(Object.keys(manifest.platforms).length, 5)
            assert.equal(
                manifest.platforms['darwin-aarch64'].url,
                `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(names[0])}`,
            )
            assert.deepEqual(
                manifest.platforms['darwin-aarch64'],
                manifest.platforms['darwin-x86_64'],
            )
            for (const update of Object.values(manifest.platforms))
                assert.equal(update.signature, signature)
            for (const [platform, name] of Object.entries(platformNames))
                assert.equal(manifest.platforms[platform].sha256, sha256(name))
            const verified = verify(repository)
            assert.equal(verified.status, 0, verified.stderr)
        }

        assert.equal(generate().status, 0)
        const original = JSON.parse(fs.readFileSync(outputPath, 'utf8'))
        for (const url of [
            `https://github.com/Mystic-Stars/Axolotl/releases/download/${tag}/asset.tar.gz`,
            'https://github.com/Axolotl-Launcher/Axolotl/releases/download/untagged-example/asset.tar.gz',
            'https://github.com/Axolotl-Launcher/Axolotl/releases/download/v1.10.0-beta.4/asset.tar.gz',
            `https://github.com/Axolotl-Launcher/Axolotl/releases/download/${tag}/Axolotl.Launcher_universal.app.tar.gz?download=1`,
            `https://github.com:443/Axolotl-Launcher/Axolotl/releases/download/${tag}/Axolotl.Launcher_universal.app.tar.gz`,
            `https://user@github.com/Axolotl-Launcher/Axolotl/releases/download/${tag}/Axolotl.Launcher_universal.app.tar.gz`,
            `http://github.com/Axolotl-Launcher/Axolotl/releases/download/${tag}/Axolotl.Launcher_universal.app.tar.gz`,
            `https://github.com/Axolotl-Launcher/Axolotl/releases/download/${tag}/Axolotl.Launcher_universal.app.tar.gz#fragment`,
            `https://github.com/extra/Axolotl-Launcher/Axolotl/releases/download/${tag}/asset.tar.gz`,
            `https://github.com/Axolotl-Launcher/Axolotl/releases/download/${tag}/wrong-platform_x64-setup.nsis.zip`,
        ]) {
            const manifest = structuredClone(original)
            manifest.platforms['darwin-aarch64'].url = url
            fs.writeFileSync(outputPath, JSON.stringify(manifest))
            assert.notEqual(verify().status, 0, `Accepted unexpected URL: ${url}`)
        }

        const invalidDigest = structuredClone(original)
        invalidDigest.platforms['darwin-aarch64'].sha256 = sha256(names[0]).slice(0, 32)
        fs.writeFileSync(outputPath, JSON.stringify(invalidDigest))
        const invalidDigestResult = verify()
        assert.notEqual(invalidDigestResult.status, 0)
        assert.match(invalidDigestResult.stderr, /Invalid SHA-256 digest for darwin-aarch64/)

        const legacyManifest = structuredClone(original)
        for (const update of Object.values(legacyManifest.platforms)) delete update.sha256
        fs.writeFileSync(outputPath, JSON.stringify(legacyManifest))
        const legacyResult = verify()
        assert.equal(legacyResult.status, 0, legacyResult.stderr)

        const missingDigest = structuredClone(release)
        for (const asset of missingDigest.assets) if (asset.name === names[1]) delete asset.digest
        fs.writeFileSync(releasePath, JSON.stringify(missingDigest))
        const missingDigestResult = generate()
        assert.notEqual(missingDigestResult.status, 0)
        assert.match(missingDigestResult.stderr, /is missing a SHA-256 digest/)
        assert(missingDigestResult.stderr.includes(names[1]))
        fs.writeFileSync(releasePath, JSON.stringify(release))

        const cnbManifest = structuredClone(original)
        for (const update of Object.values(cnbManifest.platforms)) {
            update.url = update.url.replace(
                'https://github.com/Axolotl-Launcher/Axolotl/releases/download/',
                'https://cnb.cool/axlmc/Axolotl/-/releases/download/',
            )
        }
        fs.writeFileSync(outputPath, JSON.stringify(cnbManifest))
        const cnbResult = verify(undefined, 'cnb')
        assert.equal(cnbResult.status, 0, cnbResult.stderr)

        const missingAttachment = structuredClone(release)
        missingAttachment.assets = missingAttachment.assets.filter(
            (asset) => asset.name !== `${names[0]}.sig`,
        )
        fs.writeFileSync(releasePath, JSON.stringify(missingAttachment))
        assert.match(generate().stderr, /Missing release signature attachment/)
        fs.writeFileSync(releasePath, JSON.stringify(release))
        fs.unlinkSync(path.join(directory, `${names[0]}.sig`))
        assert.match(generate().stderr, /Missing updater signature/)
        fs.writeFileSync(releasePath, JSON.stringify({ ...release, tag_name: 'v1.10.0-beta.4' }))
        assert.match(generate().stderr, /does not match/)
    } finally {
        fs.rmSync(directory, { recursive: true, force: true })
    }
})
