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
    options: {
        profile: { type: 'string', default: process.env.AXOLOTL_BUILD_PROFILE || 'release' },
        target: { type: 'string' },
        'installer-only': { type: 'boolean' },
    },
})
const profile = values.profile
if (!values['installer-only'] || process.env.CI) requireProfile(profile)
if (values['installer-only']) {
    run('cargo', ['build', '--locked', '--profile', profile, '--package', 'axolotl-installer-ui'])
} else {
    const resources = process.env.AXOLOTL_BUILD_RESOURCES
    if (!resources)
        throw new Error('AXOLOTL_BUILD_RESOURCES must point to prepared build resources')
    run(process.execPath, [
        'scripts/axolotl/prepare-build-resources.mjs',
        '--consume',
        '--output',
        resources,
    ])
    const target = values.target
    if (target && !/^[a-z0-9_-]+$/.test(target)) throw new Error('Invalid target triple')
    const targetDirectory = path.join(root, 'target', ...(target ? [target] : []), profile)
    const binary = path.join(
        targetDirectory,
        `Axolotl Launcher${process.platform === 'win32' ? '.exe' : ''}`,
    )
    const args = ['tauri', 'build', '--ci', '--no-bundle', '--config', 'tauri-release.conf.json']
    if (target) args.push('--target', target)
    args.push('--', '--locked', '--profile', profile)
    run('pnpm', args, { cwd: app })
    requireFile(binary)
    fs.writeFileSync(
        path.join(targetDirectory, 'axolotl-build.json'),
        `${JSON.stringify(
            {
                schema: 1,
                commit: output('git', ['rev-parse', 'HEAD']),
                version: version(),
                profile,
                target: target || output('rustc', ['-vV']).match(/^host: (.+)$/m)[1],
                binarySha256: sha256(binary),
                resources: JSON.parse(
                    fs.readFileSync(path.join(resources, 'manifest.json'), 'utf8'),
                ).source,
            },
            null,
            2,
        )}\n`,
    )
}
