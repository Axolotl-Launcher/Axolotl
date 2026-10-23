import assert from 'node:assert/strict'
import crypto from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'

const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-update-catalog-'))
const releasePath = path.join(directory, 'release.json')
const output = path.join(directory, 'catalog.json')
const files = [
	'Axolotl.Launcher-1.9.5-beta.1-1.aarch64.rpm',
	'Axolotl.Launcher-1.9.5-beta.1-1.aarch64.rpm.sig',
	'Axolotl.Launcher-1.9.5-beta.1-1.x86_64.rpm',
	'Axolotl.Launcher-1.9.5-beta.1-1.x86_64.rpm.sig',
	'Axolotl.Launcher_1.9.5-beta.1_aarch64.AppImage',
	'Axolotl.Launcher_1.9.5-beta.1_aarch64.AppImage.sig',
	'Axolotl.Launcher_1.9.5-beta.1_aarch64.AppImage.tar.gz',
	'Axolotl.Launcher_1.9.5-beta.1_aarch64.AppImage.tar.gz.sig',
	'Axolotl.Launcher_1.9.5-beta.1_amd64.AppImage',
	'Axolotl.Launcher_1.9.5-beta.1_amd64.AppImage.sig',
	'Axolotl.Launcher_1.9.5-beta.1_amd64.AppImage.tar.gz',
	'Axolotl.Launcher_1.9.5-beta.1_amd64.AppImage.tar.gz.sig',
	'Axolotl.Launcher_1.9.5-beta.1_amd64.deb',
	'Axolotl.Launcher_1.9.5-beta.1_amd64.deb.sig',
	'Axolotl.Launcher_1.9.5-beta.1_arm64.deb',
	'Axolotl.Launcher_1.9.5-beta.1_arm64.deb.sig',
	'Axolotl.Launcher_1.9.5-beta.1_universal.dmg',
	'Axolotl.Launcher_1.9.5-beta.1_x64-setup.exe',
	'Axolotl.Launcher_1.9.5-beta.1_x64-setup.exe.sig',
	'Axolotl.Launcher_1.9.5-beta.1_x64-setup.nsis.zip',
	'Axolotl.Launcher_1.9.5-beta.1_x64-setup.nsis.zip.sig',
	'Axolotl.Launcher_universal.app.tar.gz',
	'Axolotl.Launcher_universal.app.tar.gz.sig',
	'Axolotl_Launcher_1.9.5-beta.1_x64_modern-setup.exe',
	'Axolotl_Launcher_1.9.5-beta.1_x64_modern-setup.exe.sig',
	'Axolotl_Launcher_1.9.5-beta.1_x64_nsis-setup.exe',
	'Axolotl_Launcher_1.9.5-beta.1_x64_nsis-setup.exe.sig',
	'Axolotl_Launcher_1.9.5-beta.1_x64_portable.zip',
]

try {
	fs.writeFileSync(
		releasePath,
		JSON.stringify({
            tag_name: 'v1.9.5-beta.1',
            draft: false,
            published_at: '2026-10-22T00:00:00Z',
			assets: files.map((name, index) => ({
				name,
				size: index + 1,
				digest: `sha256:${crypto.createHash('sha256').update(name).digest('hex')}`,
                browser_download_url: `https://github.com/Axolotl-Launcher/Axolotl/releases/download/v1.9.5-beta.1/${name}`,
			})),
		}),
	)
	const result = spawnSync(
		process.execPath,
		['scripts/axolotl/create-update-server-catalog.mjs', releasePath, 'v1.9.5-beta.1', output],
        {
            cwd: path.resolve(import.meta.dirname, '..', '..'),
            encoding: 'utf8',
            env: { ...process.env, GITHUB_REPOSITORY: 'Axolotl-Launcher/Axolotl' },
        },
	)
	assert.equal(result.status, 0, result.stderr)
	const catalog = JSON.parse(fs.readFileSync(output, 'utf8'))
	assert.equal(catalog.version, '1.9.5-beta.1')
	assert.equal(catalog.files.length, files.length)
	assert.deepEqual(
		catalog.artifacts.find((artifact) => artifact.filename.endsWith('_amd64.AppImage.tar.gz'))
			.targetPlatforms,
		['linux-x86_64'],
	)
	assert.equal(
        catalog.artifacts.find((artifact) => artifact.filename.endsWith('modern-setup.exe'))
            .variant,
		'modern',
	)
	assert.equal(
		catalog.artifacts.find((artifact) => artifact.filename.endsWith('portable.zip')).kind,
		'portable',
	)
	assert.deepEqual(
		catalog.artifacts.find((artifact) => artifact.filename.endsWith('_universal.app.tar.gz'))
			.targetPlatforms,
		['darwin-aarch64', 'darwin-x86_64'],
	)
    const release = JSON.parse(fs.readFileSync(releasePath, 'utf8'))
    const run = () =>
        spawnSync(
            process.execPath,
            [
                'scripts/axolotl/create-update-server-catalog.mjs',
                releasePath,
                'v1.9.5-beta.1',
                output,
            ],
            {
                cwd: path.resolve(import.meta.dirname, '..', '..'),
                encoding: 'utf8',
                env: { ...process.env, GITHUB_REPOSITORY: 'Axolotl-Launcher/Axolotl' },
            },
        )
    release.assets.reverse()
    fs.writeFileSync(releasePath, JSON.stringify(release))
    assert.equal(run().status, 0)
    assert.deepEqual(JSON.parse(fs.readFileSync(output, 'utf8')), catalog)
    for (const change of [
        (value) => {
            value.draft = true
        },
        (value) => {
            value.tag_name = 'v1.9.5-beta.2'
        },
        (value) => {
            value.assets[0].browser_download_url = value.assets[0].browser_download_url.replace(
                'v1.9.5-beta.1',
                'untagged-example',
            )
        },
        (value) => {
            value.assets[0].digest = 'sha256:invalid'
        },
        (value) => {
            value.assets.push(value.assets[0])
        },
    ]) {
        const invalid = structuredClone(release)
        change(invalid)
        fs.writeFileSync(releasePath, JSON.stringify(invalid))
        const failure = run()
        assert.notEqual(failure.status, 0)
        assert.deepEqual(JSON.parse(fs.readFileSync(output, 'utf8')), catalog)
    }
} finally {
	fs.rmSync(directory, { recursive: true, force: true })
}
