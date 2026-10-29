import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { ref } from 'vue'

import JavaSelector from './JavaSelector.vue'

const tests = vi.hoisted(() => ({ run: vi.fn(), debounced: vi.fn() }))
vi.mock('@/composables/useJavaTest', () => ({
    default: () => ({
        testingJava: ref(false),
        javaTestResult: ref(null),
        testJavaInstallation: tests.run,
        testJavaInstallationDebounced: tests.debounced,
    }),
}))
vi.mock('@/components/ui/JavaDetectionModal.vue', () => ({ default: { render: () => null } }))
vi.mock('@/helpers/analytics', () => ({ trackEvent: vi.fn() }))
vi.mock('@modrinth/ui', () => ({
    Button: { render: () => null },
    IconButton: { render: () => null },
    StyledInput: { render: () => null },
    commonMessages: {},
    defineMessages: (value: unknown) => value,
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

it('rechecks an existing Java path when the recommended version arrives asynchronously', async () => {
    const wrapper = mount(JavaSelector, {
        props: { modelValue: { path: 'custom/java.exe' }, version: null, selectAllVersions: true },
        global: { directives: { tooltip: {} } },
    })
    try {
        tests.debounced.mockClear()
        await wrapper.setProps({ version: 21 })
        expect(tests.debounced).toHaveBeenCalledWith('custom/java.exe', 21)
    } finally {
        wrapper.unmount()
    }
})
