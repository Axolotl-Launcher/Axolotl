import assert from 'node:assert/strict'
import { test } from 'node:test'

import { isExcludedRelease, selectLatestStableRelease } from './release-policy.mjs'

test('only the temporary test tag is excluded', () => {
    assert.equal(isExcludedRelease('v1.9.9'), true)
    for (const tag of ['v1.9.8', 'v1.9.9-beta.1', 'v1.9.10', 'v1.10.0-beta.3']) {
        assert.equal(isExcludedRelease(tag), false, tag)
    }
})

test('latest stable selection skips the test release, drafts and prereleases', () => {
    const releases = [
        { tag_name: 'v1.9.9', draft: false, prerelease: false },
        { tag_name: 'v1.10.0-beta.3', draft: false, prerelease: true },
        { tag_name: 'v1.10.0', draft: true, prerelease: false },
        { tag_name: 'v1.9.8', draft: false, prerelease: false },
    ]
    assert.equal(selectLatestStableRelease(releases), releases[3])
    assert.equal(selectLatestStableRelease(releases.slice(0, 3)), undefined)
})

test('a subsequent official stable release remains eligible', () => {
    const release = { tag_name: 'v1.9.10', draft: false, prerelease: false }
    assert.equal(selectLatestStableRelease([release]), release)
})
