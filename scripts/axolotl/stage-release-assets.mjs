import fs from 'node:fs'
import path from 'node:path'
import { parseArgs } from 'node:util'

import { app, root, run } from './desktop-build-utils.mjs'
import { isMain } from './release-utils.mjs'

export function stageReleaseAssets(
    { input, output, platform, signPackages = false },
    signPackage = (file) => run('pnpm', ['tauri', 'signer', 'sign', file], { cwd: app }),
) {
    if (!input || !output || !['linux', 'macos'].includes(platform)) {
        throw new Error('Expected --input, --output and --platform linux|macos')
    }
    const destination = path.resolve(root, output)
    const suffixes =
        platform === 'linux'
            ? ['.deb', '.rpm', '.AppImage', '.AppImage.tar.gz']
            : ['.dmg', '.app.tar.gz']
    const inputDirectory = path.resolve(root, input)
    const selected = fs
        .readdirSync(inputDirectory, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .flatMap((entry) => {
            const directory = path.join(inputDirectory, entry.name)
            return fs
                .readdirSync(directory, { withFileTypes: true })
                .filter(
                    (file) =>
                        file.isFile() && suffixes.some((suffix) => file.name.endsWith(suffix)),
                )
                .map((file) => path.join(directory, file.name))
        })
    for (const suffix of suffixes) {
        if (selected.filter((file) => file.endsWith(suffix)).length !== 1) {
            throw new Error(`Expected exactly one ${suffix} artifact`)
        }
    }
    for (const file of selected) {
        const packageFile = file.endsWith('.deb') || file.endsWith('.rpm')
        if (packageFile && signPackages) signPackage(file)
        if (
            (packageFile || file.endsWith('.AppImage.tar.gz') || file.endsWith('.app.tar.gz')) &&
            !fs.existsSync(`${file}.sig`)
        ) {
            throw new Error(`Missing updater signature for ${path.basename(file)}`)
        }
    }
    fs.mkdirSync(destination, { recursive: true })
    for (const file of selected) {
        const name =
            platform === 'macos' && file.endsWith('.app.tar.gz')
                ? 'Axolotl.Launcher_universal.app.tar.gz'
                : path.basename(file).replaceAll(' ', '.')
        fs.copyFileSync(file, path.join(destination, name))
        if (fs.existsSync(`${file}.sig`))
            fs.copyFileSync(`${file}.sig`, path.join(destination, `${name}.sig`))
    }
}

if (isMain(import.meta.url)) {
    const { values } = parseArgs({
        options: {
            input: { type: 'string' },
            output: { type: 'string' },
            platform: { type: 'string' },
            'sign-packages': { type: 'boolean', default: false },
        },
    })
    stageReleaseAssets({ ...values, signPackages: values['sign-packages'] })
}
