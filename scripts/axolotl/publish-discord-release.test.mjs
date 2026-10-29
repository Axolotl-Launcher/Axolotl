import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import fs from 'node:fs/promises'
import test from 'node:test'

import {
    buildReleaseNotification,
    buildReleaseNotesText,
    channelPermissions,
    discordClient,
    DiscordError,
    downloadInstaller,
    loadAnnouncement,
    publishDiscordRelease,
    releaseImage,
    updateServerDownloadUrl,
    uploadLimit,
    waitForUpdateServer,
} from './publish-discord-release.mjs'
import { assetDownloadUrl, githubRepository } from './release-utils.mjs'

const repository = githubRepository()
const tag = 'v2.3.4-beta.1'
const channelId = '123456789012345678'
const botId = '234567890123456789'
const guildId = '345678901234567890'
const roleId = '456789012345678901'
const required = 117760n
const MiB = 1024 * 1024
const announcement = {
    version: tag.slice(1),
    publishedAt: '2026-01-01',
    changes: { added: [{ 'zh-CN': '新增测试功能', 'en-US': 'Added a test feature' }] },
    notes: { 'zh-CN': '中文说明', 'en-US': 'English notes' },
}
const release = {
    id: 42,
    tag_name: tag,
    draft: false,
    published_at: '2026-01-01T00:00:00Z',
    assets: ['exe', 'dmg', 'deb', 'deb', 'exe.sig', 'rpm'].map((extension, index) => ({
        id: index + 100,
        name: index === 0 ? 'Axolotl_x64_modern-setup.exe' : `Axolotl-${index}.${extension}`,
        size: 4,
        browser_download_url: assetDownloadUrl(
            tag,
            index === 0 ? 'Axolotl_x64_modern-setup.exe' : `Axolotl-${index}.${extension}`,
        ),
    })),
}
const plan = (source = release, options = {}) =>
    buildReleaseNotification(source, announcement, { repository, ...options })

function mockDiscord({
    history = [],
    permission = required,
    channelType = 0,
    roles: extraRoles,
} = {}) {
    const sent = []
    const reads = []
    const api = async (endpoint, options = {}) => {
        if (endpoint === '/users/@me') return { id: botId }
        if (endpoint === `/channels/${channelId}`) return { type: channelType, guild_id: guildId }
        if (endpoint.endsWith('/roles'))
            return [
                { id: guildId, permissions: String(permission) },
                ...(extraRoles ?? [{ id: roleId, name: 'update-notify', mentionable: true }]),
            ]
        if (endpoint.endsWith(`/members/${botId}`)) return { roles: [] }
        if (['POST', 'PATCH'].includes(options.method)) {
            sent.push({ endpoint, ...options })
            return { id: 'sent' }
        }
        reads.push(endpoint)
        return history[reads.length - 1] ?? []
    }
    return { api, sent, reads }
}
const send = (mock, options = {}) =>
    publishDiscordRelease({
        release,
        notification: plan(),
        channelId,
        api: mock.api,
        download: async () => Buffer.from('test'),
        ...options,
    })
const historical = (overrides = {}) => ({
    id: '777',
    author: { id: botId },
    timestamp: '2026-01-02T00:00:00Z',
    ...overrides,
})

test('one notification contains bilingual text and both direct links for every installer architecture', () => {
    const notification = plan()
    assert.deepEqual(
        notification.installers.map((asset) => asset.id),
        [100, 101, 102, 103],
    )
    assert.equal(notification.payload.embeds.length, 1)
    const embed = notification.payload.embeds[0]
    assert.equal(embed.image, undefined)
    assert.equal(notification.imageHtml, undefined)
    assert.equal(notification.textFiles.length, 0)
    assert.match(embed.description, /新增测试功能\nAdded a test feature/)
    assert.match(embed.description, /中文说明\nEnglish notes/)
    assert.equal(embed.fields.length, 4)
    for (const [index, field] of embed.fields.entries()) {
        assert.ok(
            field.value.includes(updateServerDownloadUrl(tag, notification.installers[index].name)),
        )
        assert.ok(field.value.includes(notification.installers[index].browser_download_url))
    }
    assert.equal(
        notification.payload.components[0].components[0].url,
        `https://github.com/${repository}/releases/tag/${tag}`,
    )
    assert.doesNotMatch(embed.footer.text, /axolotl-release:/)
})

test('mirror links follow the website route, omit v and encode filenames', () => {
    assert.equal(
        updateServerDownloadUrl(tag, 'Axolotl x64.deb'),
        'https://update.axlmc.org/dist/2.3.4-beta.1/Axolotl%20x64.deb',
    )
})

test('only Modern Windows installers are uploaded or linked; NSIS and legacy EXEs are excluded', async () => {
    const excluded = ['Axolotl_x64_nsis-setup.exe', 'Axolotl_x64-setup.exe'].map((name, index) => ({
        id: 900 + index,
        name,
        size: 4,
        browser_download_url: assetDownloadUrl(tag, name),
    }))
    const notification = plan({ ...release, assets: [...release.assets, ...excluded] })
    assert.deepEqual(
        notification.installers.map((asset) => asset.id),
        [100, 101, 102, 103],
    )
    assert.doesNotMatch(JSON.stringify(notification.payload), /nsis-setup|x64-setup/)
    const mock = mockDiscord()
    await send(mock, { notification })
    assert.equal(mock.sent[0].files.filter((file) => file.name.endsWith('.exe')).length, 1)
    assert.ok(mock.sent[0].files.some((file) => file.name.endsWith('_modern-setup.exe')))
    assert.throws(
        () =>
            plan({
                ...release,
                assets: [
                    ...release.assets.filter((asset) => !asset.name.endsWith('_modern-setup.exe')),
                    ...excluded,
                ],
            }),
        /Modern Windows/,
    )
    await waitForUpdateServer(
        { ...release, assets: [...release.assets, ...excluded] },
        {
            attempts: 1,
            fetchImpl: async () =>
                Response.json({
                    status: 'published',
                    artifacts: notification.installers.map((asset) => ({
                        filename: asset.name,
                        size: asset.size,
                    })),
                }),
        },
    )
})

test('Markdown pairs Chinese and English and preserves categories, formatting and notices', () => {
    const text = buildReleaseNotesText(
        {
            ...announcement,
            notes: { 'zh-CN': '中文说明', 'en-US': 'Use **bold** and `code`' },
        },
        {
            notice: { type: 'warning', 'zh-CN': '警告', 'en-US': 'Warning' },
        },
    )
    assert.match(text, /新增 \/ Added/)
    assert.match(text, /新增测试功能\nAdded a test feature/)
    assert.match(text, /WARNING/)
    assert.match(text, /Use \*\*bold\*\* and `code`/)
    assert.throws(
        () => buildReleaseNotesText({ ...announcement, notes: { 'zh-CN': '中文' } }),
        /both zh-CN and en-US/,
    )
})

test('overlong changelogs retain every entry in a single Markdown attachment', async () => {
    const changes = Array.from({ length: 350 }, (_, index) => ({
        'zh-CN': `第${index}条修复`,
        'en-US': `Fix number ${index}`,
    }))
    const notification = buildReleaseNotification(
        release,
        { ...announcement, changes: { fixed: changes } },
        { repository },
    )
    assert.equal(notification.payload.embeds.length, 1)
    assert.equal(notification.textFiles.length, 1)
    assert.match(notification.textFiles[0].name, /\.md$/)
    const text = notification.textFiles[0].bytes.toString('utf8')
    for (const change of changes) {
        assert.ok(text.includes(change['zh-CN']))
        assert.ok(text.includes(change['en-US']))
    }
    const mock = mockDiscord()
    await send(mock, { notification })
    assert.equal(mock.sent.length, 1)
    assert.equal(mock.sent[0].files[0].contentType, 'text/markdown; charset=utf-8')
    await assert.rejects(send(mockDiscord(), { notification, maxUploadBytes: 5 }), /text exceeds/)
    const rejected = mockDiscord()
    await send(rejected, {
        notification,
        api: async (endpoint, options) => {
            if (options?.files?.length > 1) throw new DiscordError(413)
            return rejected.api(endpoint, options)
        },
    })
    assert.equal(rejected.sent.length, 1)
    assert.deepEqual(rejected.sent[0].files, notification.textFiles)
    assert.equal(rejected.sent[0].payload.attachments.length, 1)
})

test('text over 4096 characters is split into embeds within one message without losing text', () => {
    for (const chinese of ['中'.repeat(4090) + '🌟'.repeat(8), '第一行\n' + '中'.repeat(4100)]) {
        const source = {
            ...announcement,
            changes: { added: [{ 'zh-CN': chinese, 'en-US': 'English detail' }] },
        }
        const notification = buildReleaseNotification(release, source, { repository })
        assert.equal(notification.textFiles.length, 0)
        assert.equal(notification.payload.embeds.length, 2)
        const embeds = notification.payload.embeds
        assert.ok(embeds.every((embed) => embed.description.length <= 4096))
        const joined = embeds.map((embed) => embed.description).join('\n')
        assert.equal(
            joined.replaceAll('\n', ''),
            buildReleaseNotesText(source).replaceAll('\n', ''),
        )
        const length = embeds.reduce(
            (total, embed) =>
                total +
                (embed.title?.length ?? 0) +
                embed.description.length +
                (embed.footer?.text.length ?? 0) +
                (embed.fields ?? []).reduce(
                    (sum, field) => sum + field.name.length + field.value.length,
                    0,
                ),
            0,
        )
        assert.ok(length <= 6000)
    }
})

test('uses the first release image in the main image position with image asset fallback', () => {
    const source = {
        ...release,
        body: '<img alt="banner" src="https://example.com/first.png">\n![second](https://example.com/second.png)',
    }
    assert.equal(plan(source).payload.embeds[0].image.url, 'https://example.com/first.png')
    assert.equal(plan(source).payload.embeds[0].thumbnail, undefined)
    assert.equal(
        releaseImage({ ...release, body: '![pic](<https://example.com/a.png> "title")' }),
        'https://example.com/a.png',
    )
    assert.equal(
        releaseImage({ ...release, body: '![unsafe](https://user:secret@example.com/a.png)' }),
        undefined,
    )
    const asset = {
        id: 200,
        name: 'banner.png',
        size: 10,
        browser_download_url: assetDownloadUrl(tag, 'banner.png'),
    }
    assert.equal(
        plan({ ...release, assets: [...release.assets, asset] }).payload.embeds[0].image.url,
        asset.browser_download_url,
    )
    assert.equal(plan().payload.embeds[0].image, undefined)
})

test('loads exact TypeScript versions and rejects missing versions', async () => {
    const source = `const entry: { version: string } = ${JSON.stringify(announcement)}
        export function getAnnouncementByVersion(version: string) { return version === entry.version ? entry : undefined }`
    assert.deepEqual(await loadAnnouncement(source, tag), announcement)
    await assert.rejects(loadAnnouncement(source, 'v2.3.5'), /No bundled announcement/)
    const real = await fs.readFile(
        new URL('../../apps/app-frontend/src/announcements/catalog.ts', import.meta.url),
        'utf8',
    )
    assert.equal((await loadAnnouncement(real, 'v1.4.0')).version, '1.4.0')
})

test('rejects unpublished, mismatched, incomplete and foreign release metadata before sending', () => {
    for (const update of [
        { draft: true },
        { published_at: null },
        { tag_name: 'v2.3.5' },
        { assets: release.assets.filter((asset) => !asset.name.endsWith('.dmg')) },
        {
            assets: [
                { ...release.assets[0], browser_download_url: 'https://example.com/file.exe' },
            ],
        },
        { assets: release.assets.map((asset) => ({ ...asset, size: 0 })) },
    ])
        assert.throws(() => plan({ ...release, ...update }))
    assert.throws(() =>
        buildReleaseNotification(release, { ...announcement, version: '2.3.5' }, { repository }),
    )
})

test('sends one message with all attachments and mentions only update-notify at the beginning', async () => {
    const mock = mockDiscord()
    assert.deepEqual(await send(mock), { sent: 1, skipped: 0 })
    assert.equal(mock.sent.length, 1)
    const { payload, files } = mock.sent[0]
    assert.equal(files.length, 4)
    assert.ok(files.every((file) => !file.name.endsWith('.png')))
    assert.deepEqual(payload.allowed_mentions, { parse: [], roles: [roleId] })
    assert.equal(payload.content, `<@&${roleId}>`)
    assert.ok(payload.nonce.length <= 25)
    assert.equal(payload.enforce_nonce, true)
    assert.equal(payload.attachments.length, files.length)
})

test('large files use mirror and GitHub links in the same text message without downloading', async () => {
    const mock = mockDiscord()
    await send(mock, {
        maxUploadBytes: 5,
        download: () => assert.fail('Must not download'),
        notification: plan({
            ...release,
            assets: release.assets.map((asset) => ({ ...asset, size: 100 })),
        }),
    })
    assert.equal(mock.sent.length, 1)
    assert.equal(mock.sent[0].files.length, 0)
    assert.ok(
        mock.sent[0].payload.embeds[0].fields.every(
            (field) =>
                field.value.includes('update.axlmc.org') && field.value.includes('github.com'),
        ),
    )
})

test('caps aggregate upload bytes and attachment count within a single message', async () => {
    const large = {
        ...release,
        assets: release.assets.map((asset) => ({ ...asset, size: 10 * MiB })),
    }
    const mock = mockDiscord()
    let downloads = 0
    await send(mock, {
        notification: plan(large),
        download: async () => {
            downloads++
            return Buffer.alloc(10 * MiB)
        },
    })
    assert.equal(downloads, 2)
    assert.ok(mock.sent[0].files.reduce((total, file) => total + file.bytes.length, 0) <= 24 * MiB)
    const many = {
        ...release,
        assets: [
            ...release.assets,
            ...Array.from({ length: 12 }, (_, index) => ({
                id: 500 + index,
                size: 4,
                name: `extra-${index}.deb`,
                browser_download_url: assetDownloadUrl(tag, `extra-${index}.deb`),
            })),
        ],
    }
    const crowded = mockDiscord()
    await send(crowded, { notification: plan(many) })
    assert.equal(crowded.sent.length, 1)
    assert.equal(crowded.sent[0].files.length, 10)
})

test('Discord size rejection retains all text, the release image and both download links on retry', async () => {
    for (const error of [new DiscordError(413), new DiscordError(400, 40005)]) {
        const mock = mockDiscord()
        await send(mock, {
            notification: plan({ ...release, body: '![banner](https://example.com/banner.png)' }),
            api: async (endpoint, options) => {
                if (options?.files?.length > 0) throw error
                return mock.api(endpoint, options)
            },
        })
        assert.equal(mock.sent.length, 1)
        assert.equal(mock.sent[0].files.length, 0)
        assert.equal(mock.sent[0].payload.attachments.length, 0)
        assert.match(mock.sent[0].payload.embeds[0].description, /Added a test feature/)
        assert.equal(mock.sent[0].payload.embeds[0].image.url, 'https://example.com/banner.png')
        assert.equal(mock.sent[0].payload.embeds[0].fields.length, 4)
    }
    const mock = mockDiscord()
    await assert.rejects(
        send(mock, {
            api: async (endpoint, options) => {
                if (options?.method === 'POST') throw new DiscordError(403, 50013)
                return mock.api(endpoint, options)
            },
        }),
        /HTTP 403/,
    )
})

test('reruns paginate history and skip the delivered text before downloading', async () => {
    const filler = Array.from({ length: 100 }, (_, index) =>
        historical({
            id: String(index),
            author: { id: 'other-bot' },
            embeds: [{ footer: { text: plan().footer } }],
        }),
    )
    const mock = mockDiscord({
        history: [filler, [historical({ embeds: [{ footer: { text: plan().footer } }] })]],
    })
    assert.deepEqual(
        await send(mock, {
            download: () => assert.fail('Must not download'),
            beforeSend: () => assert.fail('Must not wait'),
        }),
        { sent: 0, skipped: 1 },
    )
    assert.match(mock.reads[1], /before=99/)
    assert.equal(mock.sent.length, 0)
})

test('upgrades old text and rendered changelogs in place instead of posting another notification', async () => {
    for (const history of [
        { embeds: [{ footer: { text: `${plan().marker}:notes-1` } }] },
        { attachments: [{ filename: `axolotl-release-${release.id}.png` }] },
    ]) {
        const mock = mockDiscord({
            history: [[historical(history)]],
        })
        await send(mock)
        assert.equal(mock.sent.length, 1)
        assert.equal(mock.sent[0].method, 'PATCH')
        assert.equal(mock.sent[0].endpoint, `/channels/${channelId}/messages/777`)
        assert.equal(mock.sent[0].payload.nonce, undefined)
        assert.ok(mock.sent[0].files.every((file) => !file.name.endsWith('.png')))
    }
})

test('reports the exact missing channel permission before sending', async () => {
    const mock = mockDiscord({ permission: required & ~65536n })
    await assert.rejects(send(mock), (error) => {
        assert.match(error.message, /missing channel permissions: Read Message History/)
        assert.ok(error.message.includes(`Bot ${botId}`))
        assert.ok(error.message.includes(`channel ${channelId}`))
        assert.doesNotMatch(error.message, /Send Messages/)
        return true
    })
    assert.equal(mock.sent.length, 0)
    await assert.rejects(send(mockDiscord({ channelType: 15 })), /text or announcement channel/)
    await assert.rejects(
        send(mockDiscord(), { channelId: 'invalid' }),
        /Invalid DISCORD_CHANNEL_ID/,
    )
})

test('resolves role names or explicit IDs and rejects ambiguous or unmentionable roles', async () => {
    await assert.rejects(send(mockDiscord({ roles: [] })), /found 0/)
    const role = { id: roleId, name: 'update-notify', mentionable: true }
    await assert.rejects(
        send(mockDiscord({ roles: [role, { ...role, id: 'other-role' }] })),
        /found 2/,
    )
    await assert.rejects(
        send(mockDiscord({ roles: [{ ...role, mentionable: false }] })),
        /must allow mentions/,
    )
    const explicit = mockDiscord({ roles: [{ ...role, name: 'renamed-role' }] })
    await send(explicit, { notifyRoleId: roleId })
    assert.equal(explicit.sent[0].payload.content, `<@&${roleId}>`)
})

test('channel role and member overwrites take precedence over invite permissions', () => {
    const channel = {
        guild_id: guildId,
        permission_overwrites: [
            { id: guildId, type: 0, allow: '0', deny: '65536' },
            { id: 'role', type: 0, allow: '65536', deny: '0' },
            { id: 'role2', type: 0, allow: '0', deny: '65536' },
        ],
    }
    const roles = [{ id: guildId, permissions: String(required) }]
    const member = { roles: ['role', 'role2'] }
    assert.equal(channelPermissions(channel, roles, member, botId), required)
    channel.permission_overwrites.push({ id: botId, type: 1, allow: '0', deny: '65536' })
    assert.equal(channelPermissions(channel, roles, member, botId), required & ~65536n)
    assert.equal(
        channelPermissions(channel, [{ id: guildId, permissions: '8' }], member, botId) & required,
        required,
    )
})

test('waits for matching published mirror artifacts and bounds incomplete-publication retries', async () => {
    const delays = []
    let calls = 0
    await waitForUpdateServer(release, {
        attempts: 3,
        sleep: async (delay) => delays.push(delay),
        fetchImpl: async (url) => {
            assert.equal(url, 'https://update.axlmc.org/api/versions/2.3.4-beta.1')
            calls++
            if (calls === 1) return new Response('', { status: 404 })
            if (calls === 2) return Response.json({ status: 'pending', artifacts: [] })
            return Response.json({
                status: 'published',
                artifacts: release.assets.map((asset) => ({
                    filename: asset.name,
                    size: asset.size,
                })),
            })
        },
    })
    assert.deepEqual(delays, [10000, 10000])
    await assert.rejects(
        waitForUpdateServer(release, {
            attempts: 2,
            sleep: async () => {},
            fetchImpl: async () => Response.json({ status: 'published', artifacts: [] }),
        }),
        /not ready/,
    )
})

test('verifies downloaded installer size and SHA-256', async () => {
    const bytes = Buffer.from('test')
    const asset = {
        ...release.assets[0],
        digest: `sha256:${createHash('sha256').update(bytes).digest('hex')}`,
    }
    const download = (body) => async () => new Response(body)
    assert.deepEqual(await downloadInstaller(asset, 10, download(bytes)), bytes)
    await assert.rejects(downloadInstaller(asset, 2, download(bytes)), /exceeds expected size/)
    await assert.rejects(downloadInstaller(asset, 10, download('tes')), /size mismatch/)
    await assert.rejects(
        downloadInstaller(asset, 10, download('test extra')),
        /exceeds expected size/,
    )
    await assert.rejects(downloadInstaller(asset, 10, download('evil')), /digest mismatch/)
})

test('multipart upload uses separate indexed fields for the image and installers', async () => {
    const api = discordClient('test-token', {
        fetchImpl: async (url, options) => {
            assert.equal(url, 'https://discord.com/api/v10/channels/123/messages')
            assert.equal(options.headers.Authorization, 'Bot test-token')
            assert.equal(options.headers['Content-Type'], undefined)
            assert.deepEqual(JSON.parse(options.body.get('payload_json')), { content: 'hello' })
            assert.equal(options.body.get('files[0]').type, 'image/png')
            assert.equal(await options.body.get('files[1]').text(), 'test')
            assert.equal(options.body.get('files[1]').name, 'Axolotl.exe')
            return Response.json({ id: 'sent' })
        },
    })
    await api('/channels/123/messages', {
        method: 'POST',
        payload: { content: 'hello' },
        files: [
            { name: 'notes.png', bytes: Buffer.from('image'), contentType: 'image/png' },
            { name: 'Axolotl.exe', bytes: Buffer.from('test') },
        ],
    })
})

test('rate limits and transient failures retry with an unchanged nonce', async () => {
    const delays = [],
        payloads = []
    let calls = 0
    const api = discordClient('test-token', {
        sleep: async (delay) => delays.push(delay),
        fetchImpl: async (_url, options) => {
            calls++
            payloads.push(options.body)
            if (calls === 1) return Response.json({ retry_after: 1.25 }, { status: 429 })
            if (calls === 2) throw new Error('Network error with sensitive details')
            if (calls === 3) return Response.json({}, { status: 503 })
            return Response.json({ id: 'sent' })
        },
    })
    const payload = { nonce: 'same', enforce_nonce: true }
    await api('/channels/123/messages', { method: 'POST', payload })
    assert.deepEqual(delays, [1350, 2000, 4100])
    assert.ok(payloads.every((body) => body === JSON.stringify(payload)))
})

test('does not retry permanent failures or expose raw API errors, and bounds transient retries', async () => {
    let calls = 0
    const api = discordClient('test-token', {
        sleep: async () => {},
        fetchImpl: async () => {
            calls++
            return Response.json({ code: 50013, message: 'sensitive data' }, { status: 403 })
        },
    })
    await assert.rejects(api('/users/@me'), {
        message: 'Discord API request failed (HTTP 403, code 50013)',
    })
    assert.equal(calls, 1)
    calls = 0
    const failing = discordClient('test-token', {
        sleep: async () => {},
        fetchImpl: async () => {
            calls++
            return Response.json({}, { status: 503 })
        },
    })
    await assert.rejects(failing('/users/@me'), /HTTP 503/)
    assert.equal(calls, 5)
})

test('per-file upload configuration reserves multipart request space', () => {
    assert.equal(uploadLimit(), 20 * MiB)
    assert.equal(uploadLimit('24'), 24 * MiB)
    for (const value of ['0', '-1', '25', 'NaN', 'Infinity', ''])
        assert.throws(() => uploadLimit(value), /DISCORD_MAX_UPLOAD_MIB/)
})
