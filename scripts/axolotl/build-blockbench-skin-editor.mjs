import { createHash } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { root, run, sha256 } from './desktop-build-utils.mjs'

export const blockbenchBuildInputs = [
    'package.json',
    'package-lock.json',
    'build.js',
    'tsconfig.json',
    'js',
    'lang',
    'keymaps',
    'themes',
    'assets',
]

function hashTree(hash, directory, relative = '') {
    const file = path.join(directory, relative)
    const metadata = fs.lstatSync(file)
    if (metadata.isSymbolicLink()) throw new Error(`Unexpected symbolic link: ${file}`)
    hash.update(JSON.stringify([relative, metadata.isDirectory() ? 'directory' : 'file']))
    if (metadata.isDirectory()) {
        for (const name of fs.readdirSync(file).sort()) {
            hashTree(hash, directory, path.join(relative, name))
        }
    } else {
        hash.update(sha256(file))
    }
}

export function buildBlockbenchSkinEditor({ repositoryRoot = root, outDir, execute = run }) {
    const blockbench = path.join(repositoryRoot, 'third-party/blockbench')
    const bundle = path.join(blockbench, 'dist/skin.bundle.js')
    const receiptPath = path.join(outDir, 'blockbench-skin-inputs.json')
    const hash = createHash('sha256')
    for (const input of blockbenchBuildInputs) hashTree(hash, blockbench, input)
    for (const input of [
        'scripts/axolotl/build-blockbench-skin-editor.mjs',
        'scripts/axolotl/desktop-build-utils.mjs',
    ]) {
        hashTree(hash, repositoryRoot, input)
    }
    const source = hash.digest('hex')
    let previous
    try {
        previous = JSON.parse(fs.readFileSync(receiptPath, 'utf8'))
    } catch {}
    const fresh =
        previous?.schema === 1 &&
        previous.source === source &&
        fs.existsSync(bundle) &&
        previous.bundle === sha256(bundle)
    if (!fresh) {
        if (!fs.existsSync(path.join(blockbench, 'node_modules'))) {
            execute('npm', ['ci'], { cwd: blockbench })
        }
        execute('npm', ['run', 'build-skin'], { cwd: blockbench })
    }

    // The sync compares content and repairs every asset, even if only the
    // destination or sync script changed. No bundle rebuild is needed for that.
    execute(process.execPath, ['scripts/axolotl/sync-blockbench-skin-editor.mjs'], {
        cwd: repositoryRoot,
    })
    const receipt = `${JSON.stringify({ schema: 1, source, bundle: sha256(bundle) })}\n`
    if (fs.existsSync(receiptPath) && fs.readFileSync(receiptPath, 'utf8') === receipt) return
    fs.mkdirSync(outDir, { recursive: true })
    fs.writeFileSync(receiptPath, receipt)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    const outDir = process.argv[2]
    if (!outDir) throw new Error('Expected Cargo OUT_DIR as the first argument')
    buildBlockbenchSkinEditor({ outDir: path.resolve(outDir) })
}
