import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'

import { root } from './desktop-build-utils.mjs'
import { releaseVersion, webDemoAssetName } from './release-utils.mjs'

/**
 * Builds the web demo (apps/app-frontend/demo) and zips it as the release asset the
 * website loads. The archive holds the build output as is: its entry is demo/index.html.
 */
const [tag, output] = process.argv.slice(2)
if (!tag || !output) throw new Error('Usage: node package-web-demo.mjs <version-tag> <output-dir>')

const frontend = path.join(root, 'apps/app-frontend')
// The demo reports the version in this package.json, as the desktop build does.
const { version } = JSON.parse(fs.readFileSync(path.join(frontend, 'package.json'), 'utf8'))
if (version !== releaseVersion(tag)) {
    throw new Error(`apps/app-frontend/package.json is ${version}, expected ${releaseVersion(tag)}`)
}

execFileSync(
    process.execPath,
    [
        path.join(frontend, 'node_modules/vite/bin/vite.js'),
        'build',
        '--config',
        'demo/vite.config.ts',
    ],
    { cwd: frontend, stdio: 'inherit' },
)

const destination = path.resolve(root, output)
fs.mkdirSync(destination, { recursive: true })
const archive = path.join(destination, webDemoAssetName(tag))
fs.rmSync(archive, { force: true })
execFileSync('zip', ['-q', '-r', '-X', archive, '.'], {
    cwd: path.join(frontend, 'demo/dist'),
    stdio: 'inherit',
})
console.log(`Packaged ${path.relative(root, archive)} (${fs.statSync(archive).size} bytes)`)
