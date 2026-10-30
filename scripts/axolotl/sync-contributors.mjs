import fs from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const REPOSITORY = 'Mystic-Stars/Axolotl'
const PER_PAGE = 100
const MAX_PAGES = 10
const OUTPUT_PATH = fileURLToPath(
    new URL('../../apps/app-frontend/src/data/about/contributors.json', import.meta.url),
)

function requestHeaders() {
    const headers = {
        Accept: 'application/vnd.github+json',
        'User-Agent': 'Axolotl-Launcher-Contributors-Sync',
    }
    const token = process.env.AXOLOTL_GITHUB_TOKEN || process.env.GITHUB_TOKEN
    if (token) {
        headers.Authorization = `Bearer ${token}`
    }
    return headers
}

async function fetchPage(page) {
    const url = new URL(`https://api.github.com/repos/${REPOSITORY}/contributors`)
    url.searchParams.set('per_page', String(PER_PAGE))
    url.searchParams.set('page', String(page))

    let failure
    for (let attempt = 1; attempt <= 3; attempt++) {
        try {
            const response = await fetch(url, {
                headers: requestHeaders(),
                signal: AbortSignal.timeout(30_000),
            })
            if (!response.ok) throw new Error(`HTTP ${response.status}`)
            return await response.json()
        } catch (error) {
            failure = error
            if (attempt < 3) await new Promise((resolve) => setTimeout(resolve, attempt * 1_000))
        }
    }
    throw new Error(`Unable to fetch contributors from ${url}`, { cause: failure })
}

function normalizeContributor(contributor) {
    if (!contributor || typeof contributor !== 'object' || Array.isArray(contributor))
        return undefined
    if (typeof contributor.login !== 'string' || !contributor.login) return undefined
    if (typeof contributor.html_url !== 'string' || !contributor.html_url) return undefined
    if (typeof contributor.avatar_url !== 'string' || !contributor.avatar_url) return undefined
    if (!Number.isInteger(contributor.contributions) || contributor.contributions < 1)
        return undefined

    const avatarUrl = new URL(contributor.avatar_url)
    avatarUrl.searchParams.set('s', '96')

    return {
        name: contributor.login,
        avatarUrl: avatarUrl.toString(),
        url: contributor.html_url,
        contributions: contributor.contributions,
    }
}

async function fetchContributors() {
    const pages = []
    for (let page = 1; page <= MAX_PAGES; page++) {
        const contributors = await fetchPage(page)
        if (!Array.isArray(contributors)) throw new Error(`Page ${page} did not contain an array`)
        pages.push(contributors)
        if (contributors.length < PER_PAGE) break
    }

    const contributors = pages
        .flat()
        .map(normalizeContributor)
        .filter((contributor) => contributor !== undefined)
        .sort(
            (left, right) =>
                right.contributions - left.contributions || left.name.localeCompare(right.name),
        )

    if (contributors.length === 0)
        throw new Error('The contributors response did not contain any people')
    return contributors
}

async function main() {
    let contributors
    try {
        contributors = await fetchContributors()
    } catch (error) {
        if (existsSync(OUTPUT_PATH)) {
            console.warn(
                `Unable to refresh contributors, keeping the existing snapshot: ${error.message}`,
            )
            return
        }
        throw error
    }

    const nextText = `${JSON.stringify(contributors, null, 2)}\n`
    const currentText = existsSync(OUTPUT_PATH) ? await fs.readFile(OUTPUT_PATH, 'utf8') : ''

    if (currentText === nextText) {
        console.log(`Contributors are up to date (${contributors.length} people).`)
        return
    }

    await fs.writeFile(OUTPUT_PATH, nextText)
    console.log(`Synchronized ${contributors.length} contributors from ${REPOSITORY}.`)
}
await main()
