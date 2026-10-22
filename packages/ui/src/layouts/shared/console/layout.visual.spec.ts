import { expect, it, vi } from 'vitest'
import { ref } from 'vue'

import CollapsibleAdmonition from '../../../components/base/CollapsibleAdmonition.vue'
import { mountThemed, waitFor } from '../../../test/visual-harness'
import ConsolePageLayout from './layout.vue'
import type { LocalCrashAnalysis } from './providers/console-manager'
import type { LogLine } from './types'

for (const height of [440, 640]) {
    it(`scrolls long diagnostics while keeping the log visible in a ${height}px console`, async () => {
        const host = document.createElement('div')
        host.style.cssText = `display:flex;flex-direction:column;width:1100px;height:${height}px`
        document.body.append(host)
        const teleports = document.createElement('div')
        teleports.id = 'teleports'
        document.body.append(teleports)
        const onExportCrashContext = vi.fn(async () => {})
        const localCrashAnalysis = ref<LocalCrashAnalysis | null>({
            crashed: true,
            sources: [{ filename: 'latest.log', source_type: 'log', line_count: 100 }],
            findings: [
                {
                    id: 'missing_dependency',
                    confidence: 'high',
                    evidence: Array.from({ length: 80 }, (_, index) => ({
                        filename: 'latest.log',
                        line: index + 1,
                        text: `Missing dependency ${index + 1}`,
                    })),
                },
            ],
            mods: [],
            mod_changes: [],
            mod_change_counts: { added: 0, removed: 0, modified: 0 },
            windows_events: [],
        })
        const wrapper = await mountThemed(ConsolePageLayout, {}, 'dark', {
            attachTo: host,
            global: {
                provide: {
                    [Symbol.for('modrinth:consoleManagerContext')]: {
                        logLines: ref<LogLine[]>(
                            Array.from({ length: 100 }, (_, index) => ({
                                text: `Log line ${index + 1}`,
                                level: 'info',
                            })),
                        ),
                        localCrashAnalysis,
                        onExportCrashContext,
                    },
                    [Symbol.for('modrinth:modrinthClient')]: {},
                    [Symbol.for('modrinth:modalBehavior')]: { noblur: ref(false) },
                    [Symbol.for('modrinth:notificationManager')]: { addNotification: vi.fn() },
                },
                stubs: { NewModal: true, ShareModal: true },
            },
        })
        try {
            const mountContainer = host.firstElementChild as HTMLElement
            mountContainer.style.cssText = 'display:flex;min-height:0;flex:1;flex-direction:column'
            const admonition = wrapper.getComponent(CollapsibleAdmonition)
            const diagnostics = admonition.element.parentElement!
            const viewport = wrapper.get<HTMLElement>('.log-viewport').element
            const collapsedLogHeight = viewport.clientHeight
            await admonition.get('button[aria-expanded]').trigger('click')
            await waitFor(() => diagnostics.scrollHeight > height, { label: 'expanded diagnosis' })
            await waitFor(() => admonition.element.getAnimations({ subtree: true }).length === 0, {
                label: 'diagnosis expansion to finish',
            })
            expect(diagnostics.clientHeight).toBeLessThanOrEqual(height / 2)
            expect(viewport.clientHeight).toBeGreaterThan(0)
            expect(viewport.getBoundingClientRect().bottom).toBeLessThanOrEqual(
                host.getBoundingClientRect().bottom + 1,
            )

            const exportButton = diagnostics.querySelector<HTMLElement>(':scope > div > button')!
            diagnostics.scrollTop = diagnostics.scrollHeight
            expect(diagnostics.scrollTop).toBeGreaterThan(0)
            const lastEvidence = admonition
                .findAll('span')
                .find((span) => span.text().includes('Missing dependency 80'))!
            expect(lastEvidence.element.getBoundingClientRect().bottom).toBeLessThanOrEqual(
                diagnostics.getBoundingClientRect().bottom,
            )
            expect(exportButton.getBoundingClientRect().bottom).toBeLessThanOrEqual(
                diagnostics.getBoundingClientRect().bottom,
            )
            exportButton.click()
            expect(onExportCrashContext).toHaveBeenCalledOnce()

            viewport.scrollTop = 0
            expect(viewport.scrollTop).toBe(0)
            viewport.scrollTop = viewport.scrollHeight
            expect(viewport.scrollTop).toBeGreaterThan(0)

            await admonition.get('button[aria-expanded]').trigger('click')
            await waitFor(() => viewport.clientHeight >= collapsedLogHeight, {
                label: 'collapsed diagnosis',
            })
            localCrashAnalysis.value = null
            await wrapper.vm.$nextTick()
            expect(wrapper.findComponent(CollapsibleAdmonition).exists()).toBe(false)
            expect(viewport.clientHeight).toBeGreaterThan(collapsedLogHeight)
        } finally {
            wrapper.unmount()
            host.remove()
            teleports.remove()
        }
    })
}
