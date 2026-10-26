import { afterEach, expect, it } from 'vitest'
import { nextTick, ref } from 'vue'

import BigOptionButton from '../components/base/BigOptionButton.vue'
import ConfirmDropTypeModal from '../components/flows/drop/ConfirmDropTypeModal.vue'
import { I18N_INJECTION_KEY } from '../providers/i18n'
import { mountThemed } from './visual-harness'

const cleanups: (() => void)[] = []
afterEach(() =>
    cleanups
        .splice(0)
        .reverse()
        .forEach((cleanup) => cleanup()),
)
for (const classification of [
    null,
    { item_type: 'server' as const, file_path: '/fixture/server.zip' },
]) {
    it(`offers server import in ${classification ? 'detected' : 'manual'} mode`, async () => {
        const wrapper = await mountThemed(
            ConfirmDropTypeModal,
            { classification, fileName: 'server.zip' },
            'dark',
            {
                global: {
                    provide: {
                        [I18N_INJECTION_KEY as symbol]: {
                            locale: ref('en-US'),
                            t: (key: string) => key,
                            setLocale: () => undefined,
                        },
                    },
                },
            },
        )
        cleanups.push(() => wrapper.unmount())
        ;(wrapper.vm as unknown as { show: () => void }).show()
        await nextTick()
        const options = wrapper.findAllComponents(BigOptionButton)
        const server = options.find((option) => option.props('title') === 'Server')
        expect(server).toBeDefined()
        if (classification) expect(options).toHaveLength(1)
        else expect(options).toHaveLength(9)
        await server!.trigger('click')
        expect(wrapper.emitted('confirm')?.[0]?.[0]).toBe('server')
    })
}
