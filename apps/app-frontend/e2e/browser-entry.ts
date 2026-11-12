import { installDemoBackend } from '../demo/backend'
import { installDemoNetwork, showAnnouncements } from '../demo/backend/network'
import { settingsHandlers, useDemoSettings } from '../demo/backend/settings'
import { OOBE_STEP_IDS } from '../src/components/ui/oobe/step-ids'
import type { AppSettings } from '../src/helpers/settings'

const savedSettingsKey = 'axolotl-e2e-settings'
const readSettings = () => settingsHandlers['plugin:settings|settings_get']({}) as AppSettings

useDemoSettings({ locale: 'en-US', defaultPage: 'Home', theme: 'dark' })
settingsHandlers['plugin:settings|settings_patch']({
    patch: { oobe_completed_steps: [...OOBE_STEP_IDS] },
})
const saved = localStorage.getItem(savedSettingsKey)
if (saved) settingsHandlers['plugin:settings|settings_set']({ settings: JSON.parse(saved) })
showAnnouncements([])
installDemoNetwork()
installDemoBackend()

const testWindow = window as unknown as {
    __demoUnhandled?: string[]
    __TAURI_INTERNALS__: {
        invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>
    }
}
const invoke = testWindow.__TAURI_INTERNALS__.invoke
testWindow.__TAURI_INTERNALS__.invoke = async (command, args) => {
    if (command === 'plugin:system-accent|system_accent_color') return { hex: null }
    const result = await invoke(command, args)
    if (testWindow.__demoUnhandled?.includes(command)) {
        throw new Error(`Missing desktop E2E fixture for command: ${command}`)
    }
    if (
        command === 'plugin:settings|settings_set' ||
        command === 'plugin:settings|settings_patch'
    ) {
        localStorage.setItem(savedSettingsKey, JSON.stringify(readSettings()))
    }
    return result
}

// The native shell also supplies its bridge before the frontend loads.
void import('../src/main.js')
