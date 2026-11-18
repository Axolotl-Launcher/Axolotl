import { ProgressBar, provideI18n, provideLoadingState } from '@modrinth/ui'
import { IntlMessageFormat } from 'intl-messageformat'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, ref } from 'vue'

import type { StartupLoadingProgress } from '@/helpers/startup-progress'
import zhCN from '@/locales/zh-CN/index.json'
import { mountThemed } from '@/test/visual-harness'

import SplashScreen from './SplashScreen.vue'

const cleanup: (() => void)[] = []
afterEach(() => {
    cleanup.splice(0).forEach((dispose) => dispose())
    vi.useRealTimers()
})

async function mountSplash(initialProgress: StartupLoadingProgress | null, pendingValue = true) {
    const progress = ref(initialProgress)
    const pending = ref(pendingValue)
    const setEnabled = vi.fn()
    const Harness = defineComponent({
        setup() {
            provideI18n({
                locale: ref('zh-CN'),
                t: (key, values) => {
                    const message = (zhCN as Record<string, { message: string }>)[key]?.message
                    return message
                        ? (new IntlMessageFormat(message, 'zh-CN').format(values) as string)
                        : key
                },
                setLocale: () => {},
            })
            provideLoadingState({
                pending,
                barEnabled: ref(false),
                begin: () => Symbol(),
                end: () => {},
                beginManual: () => {},
                setEnabled,
            })
            return () => h(SplashScreen, { startupProgress: progress.value })
        },
    })
    const wrapper = await mountThemed(Harness, {}, 'dark')
    cleanup.push(() => wrapper.unmount())
    return { wrapper, progress, pending, setEnabled }
}

const migrating = (fraction: number | null): StartupLoadingProgress => ({
    loader_uuid: 'migration',
    event: { type: 'database_migration', stage: 'recovery', completed: 169, total: 423 },
    fraction,
})

it('shows localized migration stage and counts, retaining real progress past 15 seconds', async () => {
    vi.useFakeTimers()
    const { wrapper } = await mountSplash(migrating(0.4), false)
    expect(wrapper.text()).toContain('正在迁移启动器数据……')
    expect(wrapper.text()).toContain('正在迁移恢复记录……')
    expect(wrapper.text()).toContain('169 / 423')
    expect(wrapper.getComponent(ProgressBar).props('progress')).toBe(40)
    await vi.advanceTimersByTimeAsync(60_000)
    await nextTick()
    expect(wrapper.getComponent(ProgressBar).props('progress')).toBe(40)
    expect(wrapper.find('img').exists()).toBe(true)
})

it('waits for successful migration completion even if ordinary loading is already idle', async () => {
    vi.useFakeTimers()
    const { wrapper, progress } = await mountSplash(migrating(0.4), false)
    await vi.advanceTimersByTimeAsync(20_000)
    expect(wrapper.find('img').exists()).toBe(true)
    progress.value = migrating(null)
    await nextTick()
    await vi.advanceTimersByTimeAsync(1)
    await nextTick()
    expect(wrapper.find('img').exists()).toBe(false)
})

it('keeps directory relocation progress and clears migration-specific details', async () => {
    const { wrapper, progress } = await mountSplash(migrating(0.4))
    progress.value = { event: { type: 'directory_move' }, loader_uuid: 'move', fraction: 0.2 }
    await nextTick()
    expect(wrapper.text()).toContain('正在更新应用目录……')
    expect(wrapper.text()).not.toContain('169 / 423')
    expect(wrapper.text()).not.toContain('正在迁移恢复记录……')
    expect(wrapper.getComponent(ProgressBar).props('progress')).toBe(20)
})

it('does not mark an interrupted migration complete on unmount', async () => {
    vi.useFakeTimers()
    const { wrapper, setEnabled } = await mountSplash(migrating(0.4))
    wrapper.unmount()
    cleanup.pop()
    await vi.advanceTimersByTimeAsync(60_000)
    expect(setEnabled).not.toHaveBeenCalled()
})
