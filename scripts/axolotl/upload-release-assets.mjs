import fs from 'node:fs'
import path from 'node:path'
import { setTimeout } from 'node:timers/promises'

import { root, run } from './desktop-build-utils.mjs'
import { githubRepository, releaseVersion } from './release-utils.mjs'

const [tag, directory] = process.argv.slice(2)
releaseVersion(tag)
if (!directory) {
    throw new Error('Expected a version tag and release asset directory')
}
const absolute = path.resolve(root, directory)
const assets = fs.readdirSync(absolute, { withFileTypes: true })
if (!assets.length || assets.some((entry) => !entry.isFile()))
    throw new Error('Expected release asset files only')
for (const entry of assets) {
    for (let attempt = 1; ; attempt++) {
        try {
            run('gh', [
                'release',
                'upload',
                tag,
                '--repo',
                githubRepository(),
                path.join(absolute, entry.name),
                '--clobber',
            ])
            break
        } catch (error) {
            if (attempt === 3) throw error
            await setTimeout(attempt * 2000)
        }
    }
}
