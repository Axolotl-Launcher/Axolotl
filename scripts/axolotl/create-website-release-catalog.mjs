import fs from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

// 从 app 前端的公告 catalog（apps/app-frontend/src/announcements/catalog.ts，
// 发布时人工维护的唯一数据源）导出网站 changelog 数据。
// 该文件提交到仓库 main 分支后由网站在构建期导入，使 changelog 页面
// 能够生成包含完整更新记录的静态 HTML。
// catalog.ts 只使用可擦除语法（无 enum/namespace），直接由 Node 原生类型剥离导入，
// 无需 typescript 依赖（verify-and-publish job 不安装依赖）。
const [outputPath, sourcePath] = process.argv.slice(2)

if (!outputPath) {
    throw new Error(
        'Usage: node create-website-release-catalog.mjs <output.json> [announcement-catalog.ts]',
    )
}

const catalogModule = await import(
    sourcePath
        ? pathToFileURL(path.resolve(sourcePath))
        : new URL('../../apps/app-frontend/src/announcements/catalog.ts', import.meta.url)
)

const announcements = catalogModule.launcherAnnouncements
if (!Array.isArray(announcements) || announcements.length === 0) {
    throw new Error('Announcement catalog does not contain any announcements')
}

const catalog = {
    updated_at: new Date().toISOString(),
    announcements: announcements.map(
        ({ id, version, publishedAt, title, changes, notes, externalUrl }) => ({
            id,
            version,
            publishedAt,
            title,
            changes,
            notes,
            externalUrl,
        }),
    ),
}

await fs.writeFile(outputPath, `${JSON.stringify(catalog, null, 2)}\n`)
console.log(`Wrote ${outputPath} with ${catalog.announcements.length} announcements`)
