import fs from 'node:fs'

import { isExcludedRelease } from './release-policy.mjs'
import { assetSha256, releaseVersion, verifyRelease } from './release-utils.mjs'

// 从 GitHub release 元数据生成网站下载元数据（tag + 每个 asset 的名字与 SHA-256）。
// 该文件提交到仓库 main 分支后由 CNB 镜像同步，网站从 CNB 拉取，
// 使国内用户无需访问 GitHub API 即可获取下载链接。
const [releasePath, outputPath] = process.argv.slice(2)

if (!releasePath || !outputPath) {
    throw new Error('Usage: node create-website-release-metadata.mjs <release.json> <output.json>')
}

const release = JSON.parse(fs.readFileSync(releasePath, 'utf8'))

verifyRelease(release, release.tag_name, { published: true })

if (releaseVersion(release.tag_name).includes('-')) {
    throw new Error('Website stable downloads cannot use a prerelease')
}

if (fs.existsSync(outputPath)) {
    const current = JSON.parse(fs.readFileSync(outputPath, 'utf8'))
    const nextParts = releaseVersion(release.tag_name).split('.').map(Number)
    // Excluded test releases are not a baseline for stable downloads.
    const currentVersion = isExcludedRelease(current.tag_name) ? null : releaseVersion(current.tag_name)
    if (currentVersion && !currentVersion.includes('-') && !release.tag_name.includes('-')) {
        const currentParts = currentVersion.split('.').map(Number)
        const difference = nextParts
            .map((value, index) => value - currentParts[index])
            .find((value) => value !== 0)
        if (difference < 0) {
            console.log(
                `Preserved newer website release ${current.tag_name}; requested ${release.tag_name}`,
            )
            process.exit(0)
        }
    }
}

const metadata = {
    tag_name: release.tag_name,
    assets: release.assets
        .filter((asset) => typeof asset?.name === 'string' && asset.name.length > 0)
        .map((asset) => ({ name: asset.name, sha256: assetSha256(asset) })),
}

fs.writeFileSync(outputPath, `${JSON.stringify(metadata, null, 2)}\n`)
console.log(`Wrote ${outputPath} for ${metadata.tag_name} (${metadata.assets.length} assets)`)
