// This tag was published only for temporary testing and is not a release baseline.
export function isExcludedRelease(tag) {
    return tag === 'v1.9.9'
}

export function selectLatestStableRelease(releases) {
    return releases.find(
        (release) => !release.draft && !release.prerelease && !isExcludedRelease(release.tag_name),
    )
}
