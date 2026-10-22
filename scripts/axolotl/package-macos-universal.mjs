import fs from 'node:fs'
import path from 'node:path'
import { parseArgs } from 'node:util'

import {
    app,
    output,
    requireFile,
    requireProfile,
    root,
    run,
    sha256,
    version,
} from './desktop-build-utils.mjs'

const { values } = parseArgs({
    options: { input: { type: 'string', default: 'target/macos-input' } },
})
if (process.platform !== 'darwin') throw new Error('Universal packaging requires macOS')
requireProfile('release')
const input = path.resolve(root, values.input)
const targets = ['aarch64-apple-darwin', 'x86_64-apple-darwin']
const manifests = targets.map((target) => {
    const directory = path.join(input, target)
    const manifest = JSON.parse(fs.readFileSync(path.join(directory, 'axolotl-build.json'), 'utf8'))
    const binary = requireFile(path.join(directory, 'Axolotl Launcher'))
    run('lipo', [binary, '-verify_arch', target.startsWith('aarch64') ? 'arm64' : 'x86_64'])
    if (
        manifest.schema !== 1 ||
        manifest.target !== target ||
        manifest.profile !== 'release' ||
        manifest.commit !== output('git', ['rev-parse', 'HEAD']) ||
        manifest.version !== version() ||
        manifest.binarySha256 !== sha256(binary)
    )
        throw new Error(`Invalid ${target} build artifact`)
    return manifest
})
const resources = process.env.AXOLOTL_BUILD_RESOURCES
if (!resources) throw new Error('Prepared resources are required for packaging')
run(process.execPath, [
    'scripts/axolotl/prepare-build-resources.mjs',
    '--consume',
    '--output',
    resources,
])
const resourceId = JSON.parse(fs.readFileSync(path.join(resources, 'manifest.json'), 'utf8')).source
if (manifests.some((manifest) => manifest.resources !== resourceId)) {
    throw new Error('macOS architecture builds used different resources')
}
const directory = path.join(root, 'target/universal-apple-darwin/release')
fs.mkdirSync(directory, { recursive: true })
const binary = path.join(directory, 'Axolotl Launcher')
run('lipo', [
    '-create',
    ...targets.map((target) => path.join(input, target, 'Axolotl Launcher')),
    '-output',
    binary,
])
run('lipo', [binary, '-verify_arch', 'arm64', 'x86_64'])
fs.chmodSync(binary, 0o755)
run(
    'pnpm',
    [
        'tauri',
        'bundle',
        '--ci',
        '--target',
        'universal-apple-darwin',
        '--config',
        'tauri-release.conf.json',
        '--config',
        'tauri-updater.conf.json',
    ],
    { cwd: app },
)
