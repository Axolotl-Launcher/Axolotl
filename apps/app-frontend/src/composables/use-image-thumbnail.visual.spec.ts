import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { ref } from 'vue'

import { useImageThumbnail } from './use-image-thumbnail'
const backend = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({
    invoke: backend.invoke,
    convertFileSrc: (path: string) => path,
}))
it('unmounted queued consumers never invoke native decoding and shared requests remain usable', async () => {
    const finish: Array<(value: ArrayBuffer) => void> = []
    backend.invoke.mockImplementation(
        () =>
            new Promise((resolve) => {
                finish.push(resolve)
            }),
    )
    const card = (id: string) =>
        mount({
            setup() {
                useImageThumbnail({ instanceId: 'test', filePath: id, path: id })
                return () => null
            },
        })
    const active = [card('1'), card('2'), card('3'), card('4')]
    const obsolete = card('old')
    const shared = card('new')
    const another = card('new')
    obsolete.unmount()
    shared.unmount()
    await flushPromises()
    expect(backend.invoke).toHaveBeenCalledTimes(4)
    finish[0](new ArrayBuffer(1))
    await flushPromises()
    expect(backend.invoke).toHaveBeenCalledTimes(5)
    expect(backend.invoke.mock.calls[4][1].filePath).toBe('new')
    for (const resolve of finish.slice(1)) resolve(new ArrayBuffer(1))
    await flushPromises()
    for (const wrapper of active) wrapper.unmount()
    another.unmount()
})

it('only acquires visible sources and never falls back to decoding a large original', async () => {
    backend.invoke.mockClear()
    backend.invoke.mockRejectedValue(new Error('decode failed'))
    const source = ref<{ instanceId: string; filePath: string; path: string } | null>(null)
    let thumbnail!: ReturnType<typeof useImageThumbnail>
    const wrapper = mount({
        setup() {
            thumbnail = useImageThumbnail(source)
            return () => null
        },
    })
    await flushPromises()
    expect(backend.invoke).not.toHaveBeenCalled()
    source.value = { instanceId: 'test', filePath: 'huge.png', path: 'huge-original.png' }
    await flushPromises()
    expect(backend.invoke).toHaveBeenCalledOnce()
    expect(thumbnail.value).toBe('')
    source.value = null
    await flushPromises()
    wrapper.unmount()
})
