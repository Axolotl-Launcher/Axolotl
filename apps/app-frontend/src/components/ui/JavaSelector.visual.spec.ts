import '@/test/visual-styles.scss'

import { provideI18n, provideNotificationManager } from '@modrinth/ui'
import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, reactive, ref } from 'vue'

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

function mountSelector(props: InstanceType<typeof JavaSelector>['$props']) {
    return mount(
        defineComponent({
            setup() {
                provideI18n({ locale: ref('en-US'), t: (key) => key, setLocale: () => {} })
                provideNotificationManager({ handleError: vi.fn() } as Parameters<
                    typeof provideNotificationManager
                >[0])
                return () =>
                    h('div', { class: 'flex flex-col items-start' }, [h(JavaSelector, props)])
            },
        }),
        { attachTo: document.body, global: { directives: { tooltip: {} } } },
    )
}

it('keeps Java actions after the status icon when they fit and wraps them in narrow settings', () => {
    const wrapper = mountSelector({
        id: 'java-selector-layout',
        modelValue: { path: 'C:/Game Software/Minecraft/Java/jdk-21/bin/java.exe' },
        version: 21,
    })
    try {
        const host = wrapper.get<HTMLElement>('div').element
        const selector = wrapper.get<HTMLElement>('#java-selector-layout').element
        const status = wrapper.get('button[aria-label="Test Java installation"]').element
        const actions = [
            wrapper.get('button[aria-label="Install recommended"]').element,
            wrapper.get('button[aria-label="Detect"]').element,
            wrapper.get('button[aria-label="Browse for Java executable"]').element,
        ]

        host.style.width = '900px'
        expect(selector.getBoundingClientRect().width).toBe(900)
        const statusBounds = status.getBoundingClientRect()
        for (const action of actions) {
            const bounds = action.getBoundingClientRect()
            expect(bounds.top).toBeCloseTo(statusBounds.top)
            expect(bounds.left).toBeGreaterThan(statusBounds.right)
        }

        host.style.width = '300px'
        const narrowStatusBounds = status.getBoundingClientRect()
        const selectorBounds = selector.getBoundingClientRect()
        for (const action of actions) {
            const bounds = action.getBoundingClientRect()
            expect(bounds.top).toBeGreaterThan(narrowStatusBounds.bottom)
            expect(bounds.right).toBeLessThanOrEqual(selectorBounds.right)
        }
    } finally {
        wrapper.unmount()
    }
})

it('rechecks an existing Java path when the recommended version arrives asynchronously', async () => {
    const props = reactive({
        modelValue: { path: 'custom/java.exe' },
        version: undefined as number | undefined,
        selectAllVersions: true,
    })
    const wrapper = mountSelector(props)
    try {
        tests.debounced.mockClear()
        props.version = 21
        await nextTick()
        expect(tests.debounced).toHaveBeenCalledWith('custom/java.exe', 21)
    } finally {
        wrapper.unmount()
    }
})
