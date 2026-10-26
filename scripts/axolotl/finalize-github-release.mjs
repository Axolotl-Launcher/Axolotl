import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'

import { createUpdateManifest } from './create-update-manifest.mjs'
import { githubRepository, isMain, releaseVersion, verifyRelease } from './release-utils.mjs'
import { verifyUpdateManifest } from './verify-update-manifest.mjs'

function gh(args) {
    return execFileSync('gh', args, { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 })
}

function assertPublishedManifest(published, expected) {
    const mismatch = () =>
        new Error('Published manifest does not match release assets and signatures')
    const platforms = published.platforms
    if (!platforms || typeof platforms !== 'object') throw mismatch()
    const names = Object.keys(expected.platforms)
    if (Object.keys(platforms).sort().join() !== [...names].sort().join()) throw mismatch()
    for (const platform of names) {
        const actual = platforms[platform]
        const wanted = expected.platforms[platform]
        if (
            !actual ||
            typeof actual !== 'object' ||
            actual.signature !== wanted.signature ||
            actual.url !== wanted.url
        ) {
            throw mismatch()
        }
        if (actual.sha256 === undefined) {
            console.log(`Preserved published manifest without SHA-256 for ${platform}`)
        } else if (actual.sha256 !== wanted.sha256) {
            throw mismatch()
        }
    }
}

export function finalizeGithubRelease(tag, directory, { publishedOnly = false, run = gh } = {}) {
    releaseVersion(tag)
    const repository = githubRepository()
    fs.mkdirSync(path.join(directory, 'signatures'), { recursive: true })
    let release
    // The release-by-tag API can return 404 for drafts; list authenticated releases instead.
    for (let page = 1; ; page++) {
        const releases = JSON.parse(
            run(['api', `repos/${repository}/releases?per_page=100&page=${page}`]),
        )
        const match = releases.find((candidate) => candidate.tag_name === tag)
        if (match) {
            release = JSON.parse(run(['api', `repos/${repository}/releases/${match.id}`]))
            break
        }
        if (releases.length < 100) throw new Error(`GitHub release ${tag} was not found`)
    }
    verifyRelease(release, tag, { published: publishedOnly })
    const releaseArgs = [tag, '--repo', repository]
    run([
        'release',
        'download',
        ...releaseArgs,
        '--pattern',
        '*.sig',
        '--dir',
        path.join(directory, 'signatures'),
        '--clobber',
    ])
    const expected = createUpdateManifest(release, path.join(directory, 'signatures'), tag)
    verifyUpdateManifest(expected, tag)
    const manifestPath = path.join(directory, 'latest.json')
    if (!release.draft && release.assets.some((asset) => asset.name === 'latest.json')) {
        run([
            'release',
            'download',
            ...releaseArgs,
            '--pattern',
            'latest.json',
            '--dir',
            directory,
            '--clobber',
        ])
        const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'))
        verifyUpdateManifest(manifest, tag)
        assertPublishedManifest(manifest, expected)
        console.log(`Preserved published updater manifest for ${tag}`)
    } else {
        if (publishedOnly) throw new Error(`Published release ${tag} is missing latest.json`)
        fs.writeFileSync(manifestPath, `${JSON.stringify(expected, null, 2)}\n`)
        run(['release', 'upload', ...releaseArgs, manifestPath, '--clobber'])
    }
    if (release.draft) {
        run([
            'release',
            'edit',
            ...releaseArgs,
            '--draft=false',
            ...(tag.includes('-') ? ['--prerelease', '--latest=false'] : ['--latest']),
        ])
    }
    // Catalogs and downstream jobs must only see the final public asset URLs.
    release = JSON.parse(run(['api', `repos/${repository}/releases/${release.id}`]))
    verifyRelease(release, tag, { published: true })
    fs.writeFileSync(path.join(directory, 'release.json'), `${JSON.stringify(release, null, 2)}\n`)
    return release
}

if (isMain(import.meta.url)) {
    const [tag, directory, mode] = process.argv.slice(2)
    if (!tag || !directory || (mode && mode !== '--published-only')) {
        throw new Error(
            'Usage: node finalize-github-release.mjs <tag> <output-dir> [--published-only]',
        )
    }
    finalizeGithubRelease(tag, directory, { publishedOnly: mode === '--published-only' })
}
