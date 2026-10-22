import fs from 'node:fs/promises'
import { createRequire } from 'node:module'

import { readReleaseNotice, renderReleaseNotice } from './release-notices.mjs'

const require = createRequire(new URL('../../apps/app-frontend/package.json', import.meta.url))
const ts = require('typescript')

const [tag, outputPath] = process.argv.slice(2)
const version = tag?.replace(/^v/, '')

if (!version || !outputPath) {
    throw new Error(
        'Usage: node scripts/axolotl/create-release-notes.mjs <version-tag> <output-path>',
    )
}

const catalogSource = await fs.readFile('apps/app-frontend/src/announcements/catalog.ts', 'utf8')
const catalogModule = await import(
    `data:text/javascript;base64,${Buffer.from(
        ts.transpileModule(catalogSource, {
            compilerOptions: {
                module: ts.ModuleKind.ESNext,
                target: ts.ScriptTarget.ES2022,
            },
        }).outputText,
    ).toString('base64')}`
)

const announcement = catalogModule.getAnnouncementByVersion(version)
if (!announcement) {
    throw new Error(`No bundled announcement found for release ${version}`)
}

const notice = await readReleaseNotice(tag)

const categoryLabels = {
    added: { en: 'Added', zh: '新增' },
    changed: { en: 'Changed', zh: '变更' },
    deprecated: { en: 'Deprecated', zh: '弃用' },
    removed: { en: 'Removed', zh: '移除' },
    fixed: { en: 'Fixed', zh: 'Bug 修复' },
    security: { en: 'Security', zh: '安全修复' },
}

function renderLanguage(language) {
    const locale = language === 'zh' ? 'zh-CN' : 'en-US'
    const lines = [`## ${language === 'zh' ? '中文' : 'English'}`, '']

    for (const type of catalogModule.ANNOUNCEMENT_CHANGE_TYPES) {
        const changes = announcement.changes[type]
        if (!changes?.length) continue

        lines.push(`### ${categoryLabels[type][language]}`, '')
        for (const change of changes) {
            lines.push(`- ${change[locale]}`)
        }
        lines.push('')
    }

    if (announcement.notes) {
        lines.push(
            `### ${language === 'zh' ? '说明' : 'Notes'}`,
            '',
            announcement.notes[locale],
            '',
        )
    }

    return lines
}

const lines = [
    `# ${announcement.title['zh-CN']}`,
    '',
    `发布日期 / Published: ${announcement.publishedAt}`,
    '',
    ...renderReleaseNotice(notice),
    ...renderLanguage('zh'),
    ...renderLanguage('en'),
]

await fs.writeFile(outputPath, `${lines.join('\n').replace(/\n+$/, '')}\n`)
console.log(`Generated release notes for ${version} from the launcher announcement catalog.`)
