import { createHash } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import { parseArgs } from 'node:util'

import { app, files, output, root, run, sha256 } from './desktop-build-utils.mjs'

const { values } = parseArgs({
    options: {
        output: { type: 'string', default: path.join(root, 'target/build-resources') },
        verify: { type: 'boolean' },
        consume: { type: 'boolean' },
    },
})
const directory = path.resolve(values.output)
const blockbench = path.join(root, 'third-party/blockbench')
const inputs = [
    'packages/app-lib/java/src',
    'packages/app-lib/java/gradle',
    'packages/app-lib/java/build.gradle.kts',
    'packages/app-lib/java/settings.gradle.kts',
    'packages/app-lib/java/gradle.properties',
    'packages/app-lib/java/gradlew',
    'packages/app-lib/java/gradlew.bat',
    'scripts/axolotl/prepare-build-resources.mjs',
    'scripts/axolotl/desktop-build-utils.mjs',
    'scripts/axolotl/sync-blockbench-skin-editor.mjs',
    ...[
        'package.json',
        'package-lock.json',
        'build.js',
        'js',
        'css',
        'assets',
        'font',
        'index.html',
    ].map((file) => `third-party/blockbench/${file}`),
]
const inputHash = createHash('sha256')
const repositoryInputs = inputs.filter((input) => !input.startsWith('third-party/'))
const blockbenchInputs = inputs
    .filter((input) => input.startsWith('third-party/'))
    .map((input) => input.replace('third-party/blockbench/', ''))
for (const [cwd, paths] of [
    [root, repositoryInputs],
    [blockbench, blockbenchInputs],
]) {
    output('git', ['diff', '--exit-code', 'HEAD', '--', ...paths], cwd)
    inputHash.update(output('git', ['ls-tree', '-r', 'HEAD', '--', ...paths], cwd))
}
const source = inputHash.digest('hex')
const manifestPath = path.join(directory, 'manifest.json')

function verify() {
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'))
    if (manifest.schema !== 1 || manifest.source !== source) {
        throw new Error('Prepared build resources do not match the checked-out sources')
    }
    if (!manifest.files || typeof manifest.files !== 'object')
        throw new Error('Invalid resource manifest')
    const actualFiles = ['java', 'blockbench']
        .flatMap((name) => files(path.join(directory, name)))
        .concat(path.join(directory, 'contributors.json'))
        .map((file) => path.relative(directory, file).replaceAll('\\', '/'))
    if (
        actualFiles.length !== Object.keys(manifest.files).length ||
        actualFiles.some((file) => !Object.hasOwn(manifest.files, file))
    ) {
        throw new Error('Prepared resources contain unexpected files')
    }
    for (const required of [
        'java/theseus.jar',
        'java/authlib-injector.jar',
        'blockbench/dist/skin.bundle.js.gz',
        'contributors.json',
    ]) {
        if (!manifest.files[required]) throw new Error(`Missing prepared resource: ${required}`)
    }
    for (const [relative, checksum] of Object.entries(manifest.files)) {
        const file = path.resolve(directory, relative)
        if (!file.startsWith(`${directory}${path.sep}`) || sha256(file) !== checksum) {
            throw new Error(`Prepared resource checksum mismatch: ${relative}`)
        }
    }
}

if (values.verify || values.consume) {
    verify()
    if (values.consume) {
        const destination = path.join(app, 'resources/blockbench-skin')
        fs.mkdirSync(destination, { recursive: true })
        for (const file of files(path.join(directory, 'blockbench'))) {
            const target = path.join(
                destination,
                path.relative(path.join(directory, 'blockbench'), file),
            )
            if (!fs.existsSync(target) || sha256(file) !== sha256(target)) {
                fs.mkdirSync(path.dirname(target), { recursive: true })
                fs.copyFileSync(file, target)
            }
        }
        const contributors = path.join(root, 'apps/app-frontend/src/data/about/contributors.json')
        if (sha256(contributors) !== sha256(path.join(directory, 'contributors.json'))) {
            fs.copyFileSync(path.join(directory, 'contributors.json'), contributors)
        }
        console.log(`Using verified build resources from ${directory}`)
    }
} else {
    fs.mkdirSync(directory, { recursive: true })
    const java = path.join(root, 'packages/app-lib/java')
    const javaBuild = path.join(root, 'target/java-resource-build')
    run(
        process.platform === 'win32' ? 'gradlew.bat' : './gradlew',
        [`-PaxolotlBuildDir=${javaBuild}`, 'build', '--no-daemon', '--console=plain'],
        { cwd: java },
    )
    fs.mkdirSync(path.join(directory, 'java'), { recursive: true })
    for (const name of ['theseus.jar', 'authlib-injector.jar']) {
        fs.copyFileSync(path.join(javaBuild, 'libs', name), path.join(directory, 'java', name))
    }
    run('npm', ['ci', '--no-audit', '--no-fund'], { cwd: blockbench })
    run('npm', ['run', 'build-skin'], { cwd: blockbench })
    run(process.execPath, ['scripts/axolotl/sync-blockbench-skin-editor.mjs'])
    fs.cpSync(path.join(app, 'resources/blockbench-skin'), path.join(directory, 'blockbench'), {
        recursive: true,
    })
    fs.copyFileSync(
        path.join(root, 'apps/app-frontend/src/data/about/contributors.json'),
        path.join(directory, 'contributors.json'),
    )
    const checksums = Object.fromEntries(
        ['java', 'blockbench']
            .flatMap((name) => files(path.join(directory, name)))
            .concat(path.join(directory, 'contributors.json'))
            .map((file) => [path.relative(directory, file).replaceAll('\\', '/'), sha256(file)]),
    )
    fs.writeFileSync(
        manifestPath,
        `${JSON.stringify({ schema: 1, source, files: checksums }, null, 2)}\n`,
    )
    verify()
    console.log(`Prepared platform-independent resources (${source})`)
}
