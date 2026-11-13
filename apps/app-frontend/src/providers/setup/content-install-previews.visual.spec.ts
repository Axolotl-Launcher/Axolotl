import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, ref } from 'vue'

import { setupContentInstallPreviews } from './content-install-previews'

it('rebinds cart and single-install previews when mounted template refs are replaced and clears them on disposal', async () => {
    const revision = ref(0)
    const calls: number[] = []
    const Preview = defineComponent({
        props: { revision: { type: Number, required: true } },
        setup(props, { expose }) {
            expose({ show: () => calls.push(props.revision) })
            return () => h('div', `Preview ${props.revision}`)
        },
    })
    let cartResource: { show: () => void } | null = null
    let cartContent: { show: () => void } | null = null
    let singleResource: { show: () => void } | null = null
    const contentSelection = {
        setPreviewModal: vi.fn((modal) => {
            cartContent = modal
        }),
        setResourceInstallPreview: vi.fn((modal) => {
            cartResource = modal
        }),
    }
    const contentInstall = {
        setResourceInstallPreview: vi.fn((modal) => {
            singleResource = modal
        }),
    }
    const Host = defineComponent({
        setup() {
            const resourcePreview = ref(null)
            const contentPreview = ref(null)
            setupContentInstallPreviews({
                resourcePreview,
                contentPreview,
                contentSelection,
                contentInstall,
            })
            return () =>
                h('div', [
                    h(Preview, {
                        key: `resource-${revision.value}`,
                        ref: resourcePreview,
                        revision: revision.value,
                    }),
                    h(Preview, {
                        key: `content-${revision.value}`,
                        ref: contentPreview,
                        revision: revision.value,
                    }),
                ])
        },
    })
    const wrapper = mount(Host)
    try {
        await nextTick()
        cartResource!.show()
        cartContent!.show()
        singleResource!.show()
        expect(calls).toEqual([0, 0, 0])
        revision.value = 1
        await nextTick()
        cartResource!.show()
        cartContent!.show()
        singleResource!.show()
        expect(calls).toEqual([0, 0, 0, 1, 1, 1])
    } finally {
        wrapper.unmount()
    }
    expect(contentSelection.setPreviewModal).toHaveBeenLastCalledWith(null)
    expect(contentSelection.setResourceInstallPreview).toHaveBeenLastCalledWith(null)
    expect(contentInstall.setResourceInstallPreview).toHaveBeenLastCalledWith(null)
})
