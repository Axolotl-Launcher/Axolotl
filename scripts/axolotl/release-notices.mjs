import fs from 'node:fs/promises'

const noticeTypes = new Set(['note', 'tip', 'important', 'warning', 'caution'])
const releaseTag = /^v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/
const isObject = (value) => value !== null && typeof value === 'object' && !Array.isArray(value)
const isText = (value) => typeof value === 'string' && value.trim().length > 0

export function validateReleaseNotices(config) {
    if (!isObject(config) || !isObject(config.notices)) {
        throw new Error('Release notices must be an object with a notices object')
    }
    if (Object.keys(config).some((key) => !['$schema', 'notices'].includes(key))) {
        throw new Error('Unknown release notices configuration field')
    }
    if (Object.hasOwn(config, '$schema') && !isText(config.$schema)) {
        throw new Error('Release notices $schema must be a non-empty string')
    }
    for (const [tag, notice] of Object.entries(config.notices)) {
        if (!releaseTag.test(tag))
            throw new Error(`Invalid release notice tag: ${tag}; use v<version>`)
        if (
            !isObject(notice) ||
            Object.keys(notice).some((key) => !['type', 'zh-CN', 'en-US'].includes(key))
        ) {
            throw new Error(`Invalid release notice fields for ${tag}`)
        }
        if (!noticeTypes.has(notice.type)) throw new Error(`Invalid release notice type for ${tag}`)
        for (const locale of ['zh-CN', 'en-US']) {
            if (!isText(notice[locale]))
                throw new Error(`Release notice ${tag} requires non-empty ${locale} text`)
        }
    }
    return config.notices
}

export async function readReleaseNotice(tag, filename = '.github/release-notices.json') {
    let source
    try {
        source = await fs.readFile(filename, 'utf8')
    } catch (error) {
        if (error.code === 'ENOENT') return null
        throw error
    }
    let notices
    try {
        notices = validateReleaseNotices(JSON.parse(source))
    } catch (error) {
        throw new Error(`Invalid ${filename}: ${error.message}`, { cause: error })
    }
    const key = tag.startsWith('v') ? tag : `v${tag}`
    return Object.hasOwn(notices, key) ? notices[key] : null
}

export function renderReleaseNotice(notice) {
    if (!notice) return []
    const quote = (value) =>
        value
            .trim()
            .replace(/\r\n?/g, '\n')
            .split('\n')
            .map((line) => (line ? `> ${line}` : '>'))
    return [
        `> [!${notice.type.toUpperCase()}]`,
        ...quote(notice['zh-CN']),
        '>',
        ...quote(notice['en-US']),
        '',
    ]
}
