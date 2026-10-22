import fs from 'node:fs'
import path from 'node:path'
import { parseArgs } from 'node:util'

import {
    app,
    files,
    requireFile,
    requireProfile,
    root,
    run,
    sha256,
    version,
} from './desktop-build-utils.mjs'

const { values } = parseArgs({
    options: {
        profile: { type: 'string', default: 'release' },
        output: { type: 'string', default: path.join(root, 'target/release-assets/windows') },
    },
})
if (process.platform !== 'win32') throw new Error('Windows packaging requires Windows')
const profile = values.profile
requireProfile(profile)
const source = path.join(root, 'target', profile)
const binary = requireFile(path.join(source, 'Axolotl Launcher.exe'))
const initialHash = sha256(binary)
process.env.AXOLOTL_INSTALLER_UI_PATH = requireFile(path.join(source, 'axolotl-installer-ui.exe'))
const destination = path.resolve(values.output)
fs.mkdirSync(destination, { recursive: true })
const target =
    profile === 'release' ? path.join(root, 'target') : path.join(root, 'target/ci-bundle')
process.env.CARGO_TARGET_DIR = target
const releaseDirectory = path.join(target, 'release')
fs.mkdirSync(releaseDirectory, { recursive: true })
if (profile !== 'release')
    fs.copyFileSync(binary, path.join(releaseDirectory, 'Axolotl Launcher.exe'))
const bundleDirectory = path.join(releaseDirectory, 'bundle/nsis')

for (const variant of ['modern', 'native']) {
    if (fs.existsSync(bundleDirectory)) {
        for (const file of fs.readdirSync(bundleDirectory)) {
            if (/(-setup\.exe|\.sig|\.nsis\.zip)$/.test(file))
                fs.unlinkSync(path.join(bundleDirectory, file))
        }
    }
    const args = [
        'tauri',
        'bundle',
        '--ci',
        '--bundles',
        'nsis',
        '--config',
        'tauri-release.conf.json',
        '--config',
        `tauri-${variant}.conf.json`,
    ]
    if (variant === 'modern' && profile === 'release')
        args.push('--config', 'tauri-updater.conf.json')
    if (profile === 'ci') args.push('--no-sign')
    run('pnpm', args, { cwd: app })
    const installers = fs.readdirSync(bundleDirectory).filter((file) => file.endsWith('-setup.exe'))
    if (installers.length !== 1) throw new Error(`Expected one ${variant} installer`)
    const installer = path.join(bundleDirectory, installers[0])
    const name = `Axolotl_Launcher_${version()}_x64_${variant === 'modern' ? 'modern' : 'nsis'}-setup.exe`
    fs.copyFileSync(installer, path.join(destination, name))
    if (variant === 'native' && fs.existsSync(`${installer}.sig`)) {
        fs.copyFileSync(`${installer}.sig`, path.join(destination, `${name}.sig`))
    }
    if (profile === 'release' && variant === 'modern') {
        requireFile(`${installer}.sig`)
        fs.copyFileSync(`${installer}.sig`, path.join(destination, `${name}.sig`))
        const archives = fs
            .readdirSync(bundleDirectory)
            .filter((file) => file.endsWith('-setup.nsis.zip'))
        if (archives.length !== 1) throw new Error('Missing modern updater archive')
        for (const suffix of ['', '.sig']) {
            const file = requireFile(path.join(bundleDirectory, `${archives[0]}${suffix}`))
            fs.copyFileSync(file, path.join(destination, path.basename(file)))
        }
    }
}
if (sha256(binary) !== initialHash) throw new Error('Packaging changed the compiled application')
const portable = path.join(root, 'target', `portable-${profile}`, 'Axolotl')
fs.mkdirSync(path.join(portable, '.Axolotl'), { recursive: true })
fs.copyFileSync(binary, path.join(portable, 'Axolotl Launcher.exe'))
requireFile(path.join(app, 'resources/seed-map/axolotl-cubiomes.dll'))
fs.cpSync(path.join(app, 'resources/seed-map'), path.join(portable, 'resources/seed-map'), {
    recursive: true,
})
fs.cpSync(
    path.join(app, 'resources/blockbench-skin'),
    path.join(portable, 'resources/blockbench-skin'),
    {
        recursive: true,
    },
)
run(
    'powershell.exe',
    [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        'Compress-Archive -LiteralPath $env:AXOLOTL_PORTABLE_SOURCE -DestinationPath $env:AXOLOTL_PORTABLE_ZIP -Force',
    ],
    {
        env: {
            ...process.env,
            AXOLOTL_PORTABLE_SOURCE: portable,
            AXOLOTL_PORTABLE_ZIP: path.join(
                destination,
                `Axolotl_Launcher_${version()}_x64_portable.zip`,
            ),
        },
    },
)
for (const file of files(destination).filter((file) => path.dirname(file) === destination))
    console.log(file)
