import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'

test('generate stable tagged URLs from draft assets and verify the repository and tag', () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-manifest-'))
    // At or above firstTagWithArtifacts, so the portable artifact is required.
    const tag = 'v1.11.0-beta.1'
    const releasePath = path.join(directory, 'release.json')
    const outputPath = path.join(directory, 'latest.json')
    const signature = `${'signed-update-'.repeat(8)}\n`
    const names = [
        'Axolotl Launcher_universal.app.tar.gz',
        'Axolotl.Launcher_aarch64.AppImage.tar.gz',
        'Axolotl.Launcher_amd64.AppImage.tar.gz',
        'Axolotl.Launcher_x64-setup.nsis.zip',
        'Axolotl_Launcher_1.11.0-beta.1_x64_portable.zip',
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
                size: 1234,
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
            assert.equal(manifest.version, '1.11.0-beta.1')
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
            assert.deepEqual(manifest.artifacts, {
                portable: {
                    platform: 'windows-x86_64',
                    name: names[4],
                    sha256: sha256(names[4]),
                    signature,
                    size: 1234,
                    url: `https://github.com/${repository}/releases/download/${tag}/${names[4]}`,
                },
            })
            const verified = verify(repository)
            assert.equal(verified.status, 0, verified.stderr)
        }

        assert.equal(generate().status, 0)
        const original = JSON.parse(fs.readFileSync(outputPath, 'utf8'))
        for (const mutation of [
            { signature: '' },
            { signature: undefined },
            { size: 0 },
            { size: undefined },
        ]) {
            const damaged = structuredClone(original)
            Object.assign(damaged.artifacts.portable, mutation)
            fs.writeFileSync(outputPath, JSON.stringify(damaged))
            assert.notEqual(
                verify().status,
                0,
                `Accepted portable metadata ${JSON.stringify(mutation)}`,
            )
        }
        const unsignedPortable = structuredClone(original)
        delete unsignedPortable.artifacts.portable.signature
        delete unsignedPortable.artifacts.portable.size
        fs.writeFileSync(outputPath, JSON.stringify(unsignedPortable))
        assert.equal(verify().status, 0, 'Historical unsigned portable metadata remains verifiable')
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

        // A stale zip from another version must not be described as this release's artifact.
        const staleZip = structuredClone(release)
        staleZip.assets = staleZip.assets.map((asset) =>
            asset.name === names[4]
                ? { ...asset, name: 'Axolotl_Launcher_1.10.0-beta.5_x64_portable.zip' }
                : asset,
        )
        fs.writeFileSync(releasePath, JSON.stringify(staleZip))
        const staleResult = generate()
        assert.notEqual(staleResult.status, 0)
        assert.match(staleResult.stderr, /x64_portable\.zip/)
        fs.writeFileSync(releasePath, JSON.stringify(release))

        // Above the threshold the portable artifact is required, not optional.
        const noPortable = structuredClone(release)
        noPortable.assets = noPortable.assets.filter((asset) => !asset.name.includes('portable'))
        fs.writeFileSync(releasePath, JSON.stringify(noPortable))
        const noPortableResult = generate()
        assert.notEqual(noPortableResult.status, 0)
        assert.match(noPortableResult.stderr, /Expected one release asset named/)
        fs.writeFileSync(releasePath, JSON.stringify(release))

        const cnbManifest = structuredClone(original)
        for (const update of Object.values(cnbManifest.platforms)) {
            update.url = update.url.replace(
                'https://github.com/Axolotl-Launcher/Axolotl/releases/download/',
                'https://cnb.cool/axlmc/Axolotl/-/releases/download/',
            )
        }
        cnbManifest.artifacts.portable.url = cnbManifest.artifacts.portable.url.replace(
            'https://github.com/Axolotl-Launcher/Axolotl/releases/download/',
            'https://cnb.cool/axlmc/Axolotl/-/releases/download/',
        )
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
        fs.writeFileSync(releasePath, JSON.stringify({ ...release, tag_name: 'v1.10.4-beta.1' }))
        assert.match(generate().stderr, /does not match/)
    } finally {
        fs.rmSync(directory, { recursive: true, force: true })
    }
})

test('Windows ARM64 metadata is complete, signed and optional for historical releases', async (t) => {
    const { createUpdateManifest, updaterTargets, portableArtifactName, portableTargets } =
        await import('./create-update-manifest.mjs')
    const { verifyUpdateManifest } = await import('./verify-update-manifest.mjs')
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-arm64-manifest-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    const tag = 'v1.11.0-beta.1'
    const names = [
        ...updaterTargets.map((target) => `Axolotl${target.assetSuffix}`),
        ...portableTargets.map((target) => portableArtifactName(tag, target)),
    ]
    const signature = 'signature'.repeat(12)
    for (const name of names) fs.writeFileSync(path.join(directory, `${name}.sig`), signature)
    const release = {
        tag_name: tag,
        assets: names
            .flatMap((name) => [name, `${name}.sig`])
            .map((name) => ({
                name,
                size: 123,
                digest: `sha256:${'a'.repeat(64)}`,
            })),
    }
    const manifest = createUpdateManifest(release, directory, tag)
    assert(manifest.platforms['windows-aarch64'])
    assert.equal(
        manifest.artifacts['portable-arm64'].name,
        'Axolotl_Launcher_1.11.0-beta.1_arm64_portable.zip',
    )
    assert.doesNotThrow(() => verifyUpdateManifest(manifest, tag, 'github', release))
    for (const damage of [
        (value) => delete value.platforms['windows-aarch64'],
        (value) => delete value.artifacts['portable-arm64'],
        (value) => delete value.artifacts['portable-arm64'].signature,
        (value) => (value.artifacts['portable-arm64'].name = value.artifacts.portable.name),
    ]) {
        const broken = structuredClone(manifest)
        damage(broken)
        assert.throws(() => verifyUpdateManifest(broken, tag, 'github', release))
    }
    for (const missing of [
        '_arm64-setup.nsis.zip',
        '_arm64_portable.zip',
        '_arm64_portable.zip.sig',
    ]) {
        const broken = structuredClone(release)
        broken.assets = broken.assets.filter((asset) => !asset.name.endsWith(missing))
        assert.throws(() => createUpdateManifest(broken, directory, tag))
    }
    const historical = structuredClone(release)
    historical.assets = historical.assets.filter((asset) => !asset.name.includes('_arm64'))
    const oldManifest = createUpdateManifest(historical, directory, tag)
    assert.equal(oldManifest.platforms['windows-aarch64'], undefined)
    const previousRequirement = process.env.AXOLOTL_REQUIRE_WINDOWS_ARM64
    try {
        process.env.AXOLOTL_REQUIRE_WINDOWS_ARM64 = 'true'
        assert.throws(
            () => createUpdateManifest(historical, directory, tag),
            /missing Windows ARM64/,
        )
        assert.doesNotThrow(() => createUpdateManifest(release, directory, tag))
    } finally {
        if (previousRequirement === undefined) delete process.env.AXOLOTL_REQUIRE_WINDOWS_ARM64
        else process.env.AXOLOTL_REQUIRE_WINDOWS_ARM64 = previousRequirement
    }
    assert.doesNotThrow(() => verifyUpdateManifest(oldManifest, tag, 'github', historical))
})
