import fs from 'node:fs'
import path from 'node:path'
import { parseArgs } from 'node:util'

import { root } from './desktop-build-utils.mjs'

const { values } = parseArgs({
    options: {
        input: { type: 'string' },
        output: { type: 'string' },
        platform: { type: 'string' },
    },
})
if (!values.input || !values.output || !['linux', 'macos'].includes(values.platform)) {
    throw new Error('Expected --input, --output and --platform linux|macos')
}
const destination = path.resolve(root, values.output)
fs.mkdirSync(destination, { recursive: true })
const suffixes =
    values.platform === 'linux'
        ? ['.deb', '.rpm', '.AppImage', '.AppImage.tar.gz']
        : ['.dmg', '.app.tar.gz']
const input = path.resolve(root, values.input)
const selected = fs
    .readdirSync(input, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .flatMap((entry) => {
        const directory = path.join(input, entry.name)
        return fs
            .readdirSync(directory, { withFileTypes: true })
            .filter(
                (file) => file.isFile() && suffixes.some((suffix) => file.name.endsWith(suffix)),
            )
            .map((file) => path.join(directory, file.name))
    })
for (const suffix of suffixes) {
    if (selected.filter((file) => file.endsWith(suffix)).length !== 1) {
        throw new Error(`Expected exactly one ${suffix} artifact`)
    }
}
for (const file of selected) {
    const name =
        values.platform === 'macos' && file.endsWith('.app.tar.gz')
            ? 'Axolotl.Launcher_universal.app.tar.gz'
            : path.basename(file).replaceAll(' ', '.')
    fs.copyFileSync(file, path.join(destination, name))
    if (fs.existsSync(`${file}.sig`))
        fs.copyFileSync(`${file}.sig`, path.join(destination, `${name}.sig`))
    if (
        (file.endsWith('.AppImage.tar.gz') || file.endsWith('.app.tar.gz')) &&
        !fs.existsSync(`${file}.sig`)
    ) {
        throw new Error(`Missing updater signature for ${name}`)
    }
}
