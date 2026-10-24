import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { stripTypeScriptTypes } from 'node:module'
import { setTimeout } from 'node:timers/promises'

import { renderReleaseNotice, validateReleaseNotices } from './release-notices.mjs'
import { githubRepository, isMain, releaseVersion, verifyRelease } from './release-utils.mjs'

const MiB = 1024 * 1024
const permissionLabels = new Map([
    [1024n, 'View Channel (查看频道)'],
    [2048n, 'Send Messages (发送消息)'],
    [16384n, 'Embed Links (嵌入链接)'],
    [32768n, 'Attach Files (附加文件)'],
    [65536n, 'Read Message History (读取消息历史)'],
])
const requiredPermissions = [...permissionLabels.keys()].reduce((value, bit) => value | bit, 0n)
const maxRequestFileBytes = 24 * MiB

export function uploadLimit(value = '20') {
    const limit = Number(value)
    // Leave room for multipart metadata within Discord's 25 MiB request limit.
    if (!Number.isFinite(limit) || limit <= 0 || limit > 24) {
        throw new Error('DISCORD_MAX_UPLOAD_MIB must be greater than 0 and at most 24')
    }
    return Math.floor(limit * MiB)
}

export async function loadAnnouncement(source, tag) {
    const version = releaseVersion(tag)
    const javascript = stripTypeScriptTypes(source)
    const catalog = await import(
        `data:text/javascript;base64,${Buffer.from(javascript).toString('base64')}`
    )
    const announcement = catalog.getAnnouncementByVersion(version)
    if (!announcement) throw new Error(`No bundled announcement found for release ${version}`)
    return announcement
}

export function updateServerDownloadUrl(tag, filename) {
    return `https://update.axlmc.org/dist/${encodeURIComponent(releaseVersion(tag))}/${encodeURIComponent(filename)}`
}

function isDiscordInstaller(asset) {
    return /\.(dmg|deb)$/i.test(asset.name) || /_modern-setup\.exe$/i.test(asset.name)
}

function installerLabel(filename) {
    const arch = /universal/i.test(filename)
        ? 'Universal'
        : /arm64|aarch64/i.test(filename)
          ? 'ARM64'
          : /x64|amd64|x86_64/i.test(filename)
            ? 'x64'
            : ''
    if (/\.dmg$/i.test(filename)) return `macOS ${arch}`.trim()
    if (/\.deb$/i.test(filename)) return `Linux ${arch} · DEB`.replace(/ +/g, ' ')
    const variant = /modern/i.test(filename) ? 'Modern' : /nsis/i.test(filename) ? 'NSIS' : 'EXE'
    return `Windows ${arch} · ${variant}`.replace(/ +/g, ' ')
}

export function releaseImage(release) {
    const body = release.body ?? ''
    const matches = [
        ...body.matchAll(/!\[[^\]]*\]\(\s*<?(https:\/\/[^\s)>]+)>?(?:\s+"[^"]*")?\s*\)/gi),
        ...body.matchAll(/<img\b[^>]*\bsrc\s*=\s*["'](https:\/\/[^"']+)["'][^>]*>/gi),
    ].sort((a, b) => a.index - b.index)
    const urls = matches.map((match) => match[1].replaceAll('&amp;', '&'))
    urls.push(
        ...release.assets
            .filter((asset) => /\.(png|jpe?g|gif|webp)$/i.test(asset.name))
            .map((asset) => asset.browser_download_url),
    )
    for (const value of urls) {
        try {
            const url = new URL(value)
            if (url.protocol === 'https:' && !url.username && !url.password && value.length <= 2048)
                return value
        } catch {
            // Ignore malformed image markup and try the next release image.
        }
    }
    return undefined
}

export function buildReleaseNotesText(announcement, { notice = null } = {}) {
    const pair = (text) => {
        if (!text?.['zh-CN']?.trim() || !text?.['en-US']?.trim())
            throw new Error('Announcement text requires both zh-CN and en-US')
        return `${text['zh-CN'].trim()}\n${text['en-US'].trim()}`
    }
    const labels = {
        added: '新增 / Added',
        changed: '变更 / Changed',
        deprecated: '弃用 / Deprecated',
        removed: '移除 / Removed',
        fixed: 'Bug 修复 / Fixed',
        security: '安全修复 / Security',
    }
    const sections = []
    if (notice) {
        pair(notice)
        sections.push(renderReleaseNotice(notice).join('\n').trim())
    }
    for (const [category, label] of Object.entries(labels)) {
        const changes = announcement.changes[category]
        if (changes?.length)
            sections.push(
                `**${label}**\n${changes.map((change) => `- ${pair(change)}`).join('\n\n')}`,
            )
    }
    if (announcement.notes) sections.push(`**说明 / Notes**\n${pair(announcement.notes)}`)
    return sections.join('\n\n')
}

function splitDescriptions(text) {
    const chunks = []
    while (text.length > 4096) {
        let end = text.lastIndexOf('\n', 4096)
        if (end < 2048) end = 4096
        // Avoid splitting a UTF-16 surrogate pair between embeds.
        if (/[\uD800-\uDBFF]/.test(text[end - 1])) end--
        chunks.push(text.slice(0, end))
        text = text.slice(end).replace(/^\n+/, '')
    }
    if (text) chunks.push(text)
    return chunks
}

export function buildReleaseNotification(
    release,
    announcement,
    { repository, notice = null } = {},
) {
    const tag = release.tag_name
    verifyRelease(release, tag, { published: true })
    if (announcement.version !== releaseVersion(tag))
        throw new Error('Announcement version mismatch')
    if (!Number.isSafeInteger(release.id) || release.id <= 0) throw new Error('Invalid release ID')
    const url = `https://github.com/${repository}/releases/tag/${encodeURIComponent(tag)}`
    const title = `Axolotl Launcher ${tag}`
    if (title.length > 220 || url.length > 512)
        throw new Error('Release tag is too long for Discord')
    const installers = release.assets.filter(isDiscordInstaller)
    for (const extension of ['exe', 'dmg', 'deb']) {
        if (!installers.some((asset) => asset.name.toLowerCase().endsWith(`.${extension}`))) {
            throw new Error(
                `Release is missing a ${extension === 'exe' ? 'Modern Windows .exe' : `.${extension}`} installer`,
            )
        }
    }
    const fields = installers.map((asset) => {
        if (
            !Number.isSafeInteger(asset.id) ||
            !Number.isSafeInteger(asset.size) ||
            asset.size <= 0
        ) {
            throw new Error(`Invalid installer metadata: ${asset.name}`)
        }
        const value = `[更新服务器](${updateServerDownloadUrl(tag, asset.name)}) · [GitHub](${asset.browser_download_url})`
        if (value.length > 1024) throw new Error('Installer filename is too long for Discord')
        return {
            name: `${installerLabel(asset.name)} · ${(asset.size / MiB).toFixed(1)} MiB`,
            value,
            inline: true,
        }
    })
    const notes = buildReleaseNotesText(announcement, { notice })
    const footer = `Axolotl Launcher · 更新日志 / Release notes · #${release.id}`
    const textLength =
        title.length +
        footer.length +
        fields.reduce((total, field) => total + field.name.length + field.value.length, 0)
    if (fields.length > 25 || textLength > 6000) {
        throw new Error('Installer links exceed the single-message Discord embed limits')
    }
    const textFiles = []
    let description = notes
    if (notes.length + textLength > 6000) {
        // Discord caps text across all embeds in one message at 6000 characters.
        // Preserve the full bilingual text as Markdown without posting extra messages.
        description =
            '完整中英更新日志见附带的 Markdown 文本或 GitHub Releases。\nFull bilingual release notes are attached as Markdown and available on GitHub Releases.'
        textFiles.push({
            name: `axolotl-release-${release.id}.md`,
            bytes: Buffer.from(`# ${title}\n\n${notes}\n`),
            contentType: 'text/markdown; charset=utf-8',
        })
    }
    if (description.length + textLength > 6000)
        throw new Error('Installer links exceed the single-message Discord embed limits')
    const embeds = splitDescriptions(description).map((part) => ({
        color: 0xf28db2,
        description: part,
    }))
    if (!embeds.length) embeds.push({ color: 0xf28db2 })
    Object.assign(embeds[0], { title, url, footer: { text: footer } })
    const image = releaseImage(release)
    if (image) embeds[0].image = { url: image }
    embeds.at(-1).fields = fields
    return {
        textFiles,
        footer,
        marker: `axolotl-release:${repository}:${release.id}`,
        installers,
        payload: {
            allowed_mentions: { parse: [] },
            embeds,
            components: [
                { type: 1, components: [{ type: 2, style: 5, label: 'GitHub Releases', url }] },
            ],
        },
    }
}

export class DiscordError extends Error {
    constructor(status, code) {
        super(`Discord API request failed (HTTP ${status}, code ${code ?? 'unknown'})`)
        this.status = status
        this.code = code
    }
}

export function discordClient(token, { fetchImpl = fetch, sleep = setTimeout } = {}) {
    return async (endpoint, { method = 'GET', payload, files = [] } = {}) => {
        for (let attempt = 0; attempt < 5; attempt++) {
            const headers = {
                Authorization: `Bot ${token}`,
                'User-Agent': 'DiscordBot (https://github.com/Axolotl-Launcher/Axolotl, 1.0)',
            }
            let body
            if (files.length) {
                body = new FormData()
                body.set('payload_json', JSON.stringify(payload))
                files.forEach((file, index) =>
                    body.set(
                        `files[${index}]`,
                        new Blob([file.bytes], {
                            type: file.contentType || 'application/octet-stream',
                        }),
                        file.name,
                    ),
                )
            } else if (payload) {
                headers['Content-Type'] = 'application/json'
                body = JSON.stringify(payload)
            }
            let response
            try {
                response = await fetchImpl(`https://discord.com/api/v10${endpoint}`, {
                    method,
                    headers,
                    body,
                    signal: AbortSignal.timeout(120000),
                })
            } catch {
                // POSTs use enforce_nonce, including when a response is lost after delivery.
                if (attempt === 4) throw new Error('Discord API network request failed')
                await sleep(1000 * 2 ** attempt)
                continue
            }
            const data = await response.json().catch(() => ({}))
            if (response.ok) return data
            if ((response.status === 429 || response.status >= 500) && attempt < 4) {
                const seconds = response.status === 429 ? Number(data.retry_after) : 2 ** attempt
                if (!Number.isFinite(seconds) || seconds < 0 || seconds > 120) {
                    throw new Error('Discord retry delay is invalid or exceeds 120 seconds')
                }
                await sleep(Math.ceil(seconds * 1000) + 100)
                continue
            }
            throw new DiscordError(response.status, data.code)
        }
    }
}

export async function downloadInstaller(asset, maxBytes, fetchImpl = fetch) {
    const response = await fetchImpl(asset.browser_download_url, {
        signal: AbortSignal.timeout(120000),
    })
    if (!response.ok) throw new Error(`Installer download failed (HTTP ${response.status})`)
    const chunks = []
    let size = 0
    for await (const chunk of response.body) {
        size += chunk.byteLength
        if (size > maxBytes || size > asset.size) throw new Error('Installer exceeds expected size')
        chunks.push(chunk)
    }
    if (size !== asset.size) throw new Error('Installer download size mismatch')
    const bytes = Buffer.concat(chunks)
    if (asset.digest) {
        const digest = `sha256:${createHash('sha256').update(bytes).digest('hex')}`
        if (asset.digest !== digest) throw new Error('Installer download digest mismatch')
    }
    return bytes
}

export function channelPermissions(channel, roles, member, botId) {
    const roleIds = new Set([channel.guild_id, ...member.roles])
    let permissions = roles
        .filter((role) => roleIds.has(role.id))
        .reduce((value, role) => value | BigInt(role.permissions), 0n)
    if (permissions & 8n) return permissions | requiredPermissions
    const overwrites = channel.permission_overwrites ?? []
    const apply = (allow, deny) => {
        permissions = (permissions & ~deny) | allow
    }
    const everyone = overwrites.find((entry) => entry.id === channel.guild_id)
    if (everyone) apply(BigInt(everyone.allow), BigInt(everyone.deny))
    let allow = 0n
    let deny = 0n
    for (const entry of overwrites) {
        if (entry.type === 0 && entry.id !== channel.guild_id && roleIds.has(entry.id)) {
            allow |= BigInt(entry.allow)
            deny |= BigInt(entry.deny)
        }
    }
    apply(allow, deny)
    const own = overwrites.find((entry) => entry.type === 1 && entry.id === botId)
    if (own) apply(BigInt(own.allow), BigInt(own.deny))
    return permissions
}

export async function publishDiscordRelease({
    release,
    notification,
    channelId,
    api,
    maxUploadBytes = uploadLimit(),
    download = downloadInstaller,
    beforeSend = async () => {},
    notifyRoleId,
    notifyRoleName = 'update-notify',
}) {
    if (!/^\d{17,20}$/.test(channelId)) throw new Error('Invalid DISCORD_CHANNEL_ID')
    const bot = await api('/users/@me')
    const channel = await api(`/channels/${channelId}`)
    if (![0, 5].includes(channel.type)) {
        throw new Error('DISCORD_CHANNEL_ID must identify a text or announcement channel')
    }
    const roles = await api(`/guilds/${channel.guild_id}/roles`)
    const member = await api(`/guilds/${channel.guild_id}/members/${bot.id}`)
    // Read Message History is required: without it Discord returns an empty history,
    // which would silently defeat deduplication on retries.
    const permissions = channelPermissions(channel, roles, member, bot.id)
    const missing = [...permissionLabels]
        .filter(([bit]) => (permissions & bit) !== bit)
        .map(([, label]) => label)
    if (missing.length) {
        throw new Error(
            `Bot ${bot.id} is missing channel permissions: ${missing.join(', ')}. ` +
                `Server ${channel.guild_id}, channel ${channelId}. ` +
                'In Discord, open Edit Channel > Permissions, add this bot as a member override, ' +
                'and explicitly allow the missing permissions. Server invite permissions can be overridden by channel permissions.',
        )
    }
    const notifyRoles = roles.filter((role) =>
        notifyRoleId ? role.id === notifyRoleId : role.name === notifyRoleName,
    )
    if (notifyRoles.length !== 1) {
        throw new Error(
            `Expected one ${notifyRoleName} role; found ${notifyRoles.length}. Set DISCORD_NOTIFY_ROLE_ID to the intended role ID.`,
        )
    }
    const notifyRole = notifyRoles[0]
    if (!notifyRole.mentionable && !(permissions & (8n | 131072n))) {
        throw new Error(
            `Role ${notifyRole.name} must allow mentions, or the bot needs Mention Everyone permission in channel ${channelId}`,
        )
    }
    let legacyMessage
    let before = ''
    do {
        const history = await api(`/channels/${channelId}/messages?limit=100${before}`)
        for (const entry of history) {
            if (entry.author?.id !== bot.id) continue
            if (entry.embeds?.some((embed) => embed.footer?.text === notification.footer))
                return { sent: 0, skipped: 1 }
            if (
                entry.attachments?.some(
                    (attachment) => attachment.filename === `axolotl-release-${release.id}.png`,
                ) ||
                entry.embeds?.some(
                    (embed) => embed.footer?.text === `${notification.marker}:notes-1`,
                )
            )
                legacyMessage = entry.id
        }
        const oldest = history.at(-1)
        if (history.length < 100 || Date.parse(oldest.timestamp) < Date.parse(release.published_at))
            break
        before = `&before=${oldest.id}`
    } while (true)

    await beforeSend()
    const files = [...notification.textFiles]
    let totalBytes = files.reduce((total, file) => total + file.bytes.length, 0)
    if (
        files.some((file) => file.bytes.length > maxUploadBytes) ||
        totalBytes > maxRequestFileBytes
    ) {
        throw new Error('Release notes text exceeds the Discord attachment limit')
    }
    for (const asset of notification.installers) {
        if (
            asset.size > maxUploadBytes ||
            totalBytes + asset.size > maxRequestFileBytes ||
            files.length >= 10
        )
            continue
        files.push({ name: asset.name, bytes: await download(asset, maxUploadBytes) })
        totalBytes += asset.size
    }
    const content = structuredClone(notification.payload)
    content.content = `<@&${notifyRole.id}>`
    content.allowed_mentions = { parse: [], roles: [notifyRole.id] }
    if (!legacyMessage) {
        content.nonce = createHash('sha256')
            .update(`${notification.marker}:summary`)
            .digest('hex')
            .slice(0, 24)
        content.enforce_nonce = true
    }
    const endpoint = `/channels/${channelId}/messages${legacyMessage ? `/${legacyMessage}` : ''}`
    const method = legacyMessage ? 'PATCH' : 'POST'
    content.attachments = files.map((file, id) => ({ id, filename: file.name }))
    try {
        await api(endpoint, { method, payload: content, files })
    } catch (error) {
        if (
            files.length === notification.textFiles.length ||
            !(error.status === 413 || error.code === 40005)
        )
            throw error
        // Retain all text and download links, dropping only installer uploads.
        content.attachments = notification.textFiles.map((file, id) => ({
            id,
            filename: file.name,
        }))
        await api(endpoint, { method, payload: content, files: notification.textFiles })
    }
    return { sent: 1, skipped: 0 }
}

export async function waitForUpdateServer(
    release,
    { fetchImpl = fetch, sleep = setTimeout, attempts = 24 } = {},
) {
    const version = releaseVersion(release.tag_name)
    const installers = release.assets.filter(isDiscordInstaller)
    for (let attempt = 0; attempt < attempts; attempt++) {
        try {
            const response = await fetchImpl(
                `https://update.axlmc.org/api/versions/${encodeURIComponent(version)}`,
                { signal: AbortSignal.timeout(15000) },
            )
            if (response.ok) {
                const published = await response.json()
                if (
                    published.status === 'published' &&
                    installers.every((asset) =>
                        published.artifacts?.some(
                            (file) => file.filename === asset.name && file.size === asset.size,
                        ),
                    )
                )
                    return
            }
        } catch {
            // Publication can be accepted before the mirror finishes downloading all files.
        }
        if (attempt < attempts - 1) await sleep(10000)
    }
    throw new Error(
        `Update Server installers for ${version} are not ready; retry the Discord notification after mirroring completes`,
    )
}

if (isMain(import.meta.url)) {
    const [tag, mode] = process.argv.slice(2)
    if (!tag || (mode && mode !== '--dry-run')) {
        throw new Error('Usage: node publish-discord-release.mjs <tag> [--dry-run]')
    }
    releaseVersion(tag)
    const repository = githubRepository()
    const git = (...args) => execFileSync('git', args, { encoding: 'utf8', maxBuffer: 8 * MiB })
    const ref = `refs/tags/${tag}`
    const announcement = await loadAnnouncement(
        git('show', `${ref}:apps/app-frontend/src/announcements/catalog.ts`),
        tag,
    )
    const noticesPath = '.github/release-notices.json'
    const notice = git('ls-tree', '--name-only', ref, '--', noticesPath).trim()
        ? validateReleaseNotices(JSON.parse(git('show', `${ref}:${noticesPath}`)))[tag]
        : null
    const gh = (endpoint) =>
        JSON.parse(execFileSync('gh', ['api', endpoint], { encoding: 'utf8', maxBuffer: 8 * MiB }))
    const release = gh(`repos/${repository}/releases/tags/${encodeURIComponent(tag)}`)
    release.assets = []
    for (let page = 1; ; page++) {
        const assets = gh(
            `repos/${repository}/releases/${release.id}/assets?per_page=100&page=${page}`,
        )
        release.assets.push(...assets)
        if (assets.length < 100) break
    }
    const notification = buildReleaseNotification(release, announcement, { repository, notice })
    const maxUploadBytes = uploadLimit(process.env.DISCORD_MAX_UPLOAD_MIB)
    if (mode === '--dry-run') {
        console.log(
            JSON.stringify(
                {
                    payload: notification.payload,
                    textAttachments: notification.textFiles.map((file) => ({
                        name: file.name,
                        text: file.bytes.toString('utf8'),
                    })),
                    installers: notification.installers,
                    maxUploadBytes,
                },
                null,
                2,
            ),
        )
    } else {
        if (!process.env.DISCORD_BOT_TOKEN) throw new Error('DISCORD_BOT_TOKEN is required')
        const result = await publishDiscordRelease({
            release,
            notification,
            channelId: process.env.DISCORD_CHANNEL_ID,
            api: discordClient(process.env.DISCORD_BOT_TOKEN),
            maxUploadBytes,
            beforeSend: () => waitForUpdateServer(release),
            notifyRoleId: process.env.DISCORD_NOTIFY_ROLE_ID || undefined,
        })
        console.log(
            `Discord release ${tag}: ${result.sent} sent, ${result.skipped} already delivered`,
        )
    }
}
