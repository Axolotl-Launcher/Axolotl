import assert from 'node:assert/strict'
import { test, type TestContext } from 'node:test'

import { computed, type Ref, ref } from 'vue'

import { type DownloadSource, normalizeDownloadCatalog } from '../utils/download-sources.ts'
import type { DownloadStatus } from '../utils/launcher-downloads.ts'
import { useLauncherDownloads } from './use-launcher-downloads.ts'

function setup(t: TestContext) {
    const source = ref<DownloadSource>('auto')
    const channel = ref<'release' | 'beta'>('release')
    const data = ref(
        normalizeDownloadCatalog({
            version: '1.10.0',
            release_tag: 'v1.10.0',
            downloads: [
                {
                    filename: 'Axolotl_x64-setup.exe',
                    relative_path: 'dist/1.10.0/Axolotl_x64-setup.exe',
                },
            ],
        }),
    )
    const status = ref<DownloadStatus>('success')
    const refresh = t.mock.fn(async () => {})
    const calls: Array<{
        url: Ref<string>
        options: { server: boolean; timeout: number; retry: number }
    }> = []
    const globals = {
        computed,
        useState: () => source,
        onMounted: () => {},
        useFetch: (
            url: Ref<string>,
            options: { server: boolean; timeout: number; retry: number },
        ) => {
            calls.push({ url, options })
            return { data, status, refresh }
        },
    }
    const previous = Object.keys(globals).map((key) => ({
        key,
        exists: Object.hasOwn(globalThis, key),
        value: Reflect.get(globalThis, key),
    }))
    Object.assign(globalThis, globals)
    t.after(() => {
        for (const { key, exists, value } of previous) {
            if (exists) Reflect.set(globalThis, key, value)
            else Reflect.deleteProperty(globalThis, key)
        }
    })
    return { source, channel, data, status, refresh, calls, result: useLauncherDownloads(channel) }
}

test('source changes reuse one authoritative catalog and only change the file host', (t) => {
    const f = setup(t)
    const release = f.result.release.value
    assert.equal(f.calls.length, 1)
    assert.equal(new URL(f.result.links.value.windows!).hostname, 'dist.axlmc.org')
    f.source.value = 'gitlink'
    assert.equal(new URL(f.result.links.value.windows!).hostname, 'www.gitlink.org.cn')
    f.source.value = 'github'
    assert.equal(new URL(f.result.links.value.windows!).hostname, 'github.com')
    assert.equal(f.result.release.value, release)
    assert.equal(f.result.release.value!.tag_name, 'v1.10.0')
    assert.equal(f.calls.length, 1)
})

test('metadata failures and loading never expose stale file links from any source', (t) => {
    const f = setup(t)
    for (const source of ['auto', 'update-server', 'github', 'gitlink'] as const) {
        f.source.value = source
        for (const status of ['idle', 'pending', 'error'] as const) {
            f.status.value = status
            assert.equal(f.result.release.value, null)
            assert(Object.values(f.result.links.value).every((url) => url === null))
        }
        f.status.value = 'success'
        assert(f.result.links.value.windows)
    }
})

test('channel changes and retries target only Update Server and remain browser-only', async (t) => {
    const f = setup(t)
    assert.equal(
        f.calls[0].url.value,
        'https://update.axlmc.org/api/downloads/latest?channel=release',
    )
    f.channel.value = 'beta'
    assert.equal(f.calls[0].url.value, 'https://update.axlmc.org/api/downloads/latest?channel=beta')
    assert.equal(f.calls[0].options.server, false)
    assert.equal(f.calls[0].options.timeout, 10000)
    assert.equal(f.calls[0].options.retry, 1)
    await f.result.refresh()
    assert.equal(f.refresh.mock.callCount(), 1)
    assert.equal(f.calls.length, 1)
})

test('HTTP 204 and successful empty catalogs have no download links', (t) => {
    const f = setup(t)
    for (const data of [null, normalizeDownloadCatalog({ version: '1.10.0', downloads: [] })]) {
        f.data.value = data
        assert(Object.values(f.result.links.value).every((url) => url === null))
        assert.equal(f.result.status.value, 'success')
    }
})
