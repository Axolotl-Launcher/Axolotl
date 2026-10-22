import { Checkbox, I18N_INJECTION_KEY } from '@modrinth/ui'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { ref } from 'vue'

import {
    type ResourceBackupFile,
    resourceExecuteHistoryCleanup,
    resourcePrepareHistoryCleanup,
    resourceScanBackups,
} from '@/helpers/resources'
import { mountThemed, waitFor } from '@/test/visual-harness'

import ResourceBackupsModal from './ResourceBackupsModal.vue'

vi.mock('@/helpers/resources', () => ({
    resourceScanBackups: vi.fn(),
    resourcePrepareHistoryCleanup: vi.fn(),
    resourceExecuteHistoryCleanup: vi.fn(),
}))

const backups = (): ResourceBackupFile[] => [
    {
        id: 'recorded',
        historyId: 'recorded',
        originalFileName: 'Create-1.21.1-6.0.9.jar',
        relativePath: 'mods/.resource-46bd0629-90b1-4f92-8d68-74d8f172a33e-0.old',
        integrity: { size: 18400000, hashes: [] },
        rollbackAvailable: true,
        recognized: true,
        exists: true,
    },
    {
        id: 'orphan',
        historyId: null,
        relativePath: 'mods/another.jar_previous.jar.old',
        integrity: { size: 240000, hashes: [] },
        rollbackAvailable: false,
        recognized: true,
        exists: true,
    },
    {
        id: 'unknown',
        historyId: null,
        relativePath: 'mods/unknown.old',
        integrity: { size: 10000, hashes: [] },
        rollbackAvailable: false,
        recognized: false,
        exists: true,
    },
]

beforeEach(() => {
    const target = document.createElement('div')
    target.id = 'teleports'
    document.body.appendChild(target)
    vi.resetAllMocks()
    vi.mocked(resourceScanBackups).mockResolvedValue(backups())
    vi.mocked(resourcePrepareHistoryCleanup).mockImplementation(async (_, ids) => ({
        id: 'plan',
        revision: 1,
        instanceId: 'instance',
        instanceRevision: 1,
        entries: [],
        files: backups().filter((file) => ids.includes(file.id)),
        issues: [],
    }))
})
afterEach(() => document.getElementById('teleports')?.remove())

it.each([1000, 480])(
    'filters real backup names, retains selection and reviews cleanup at %i px',
    async (width) => {
        await page.viewport(width, 800)
        const wrapper = await mountThemed(ResourceBackupsModal, {}, 'dark', {
            global: {
                directives: { tooltip: {} },
                provide: {
                    [I18N_INJECTION_KEY as symbol]: {
                        locale: ref('en-US'),
                        t: (key: string) => key,
                        setLocale: () => {},
                    },
                },
            },
        })
        try {
            wrapper.vm.show('instance')
            await flushPromises()
            await waitFor(() => !!document.querySelector('[role="dialog"]'))
            const dialog = () => document.querySelector('[role="dialog"]')!
            expect(dialog().textContent).toContain('Create-1.21.1-6.0.9.jar')
            expect(
                wrapper
                    .findAllComponents(Checkbox)
                    .filter((checkbox) => checkbox.props('disabled')),
            ).toHaveLength(1)
            if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
                await page.screenshot({
                    path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/backups-list-${width}.png`,
                })
            const input = dialog().querySelector('input')!
            input.value = 'Create'
            input.dispatchEvent(new Event('input', { bubbles: true }))
            await flushPromises()
            expect(dialog().textContent).not.toContain('unknown.old')
            const selectAll = document.querySelector<HTMLButtonElement>('[role="checkbox"]')!
            selectAll.click()
            await flushPromises()
            const review = [...dialog().querySelectorAll('button')].find((button) =>
                button.textContent?.includes('Review selected cleanup'),
            )!
            review.click()
            await flushPromises()
            expect(resourcePrepareHistoryCleanup).toHaveBeenCalledExactlyOnceWith('instance', [
                'recorded',
            ])
            expect(dialog().textContent).toContain(
                'permanently removes 1 available rollback backups',
            )
            expect(resourceExecuteHistoryCleanup).not.toHaveBeenCalled()
            if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
                await page.screenshot({
                    path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/backups-review-${width}.png`,
                })
            const confirm = [...dialog().querySelectorAll('button')].find((button) =>
                button.textContent?.includes('Delete these backups'),
            )!
            vi.mocked(resourceExecuteHistoryCleanup).mockRejectedValueOnce(new Error('failed'))
            confirm.click()
            await flushPromises()
            expect(dialog().textContent).toContain('Confirm backup cleanup')
            expect(dialog().textContent).toContain('Could not complete the content operation')
            vi.mocked(resourceExecuteHistoryCleanup).mockResolvedValueOnce({
                removed: ['recorded'],
                failures: [],
            })
            ;[...dialog().querySelectorAll('button')]
                .find((button) => button.textContent?.includes('Delete these backups'))!
                .click()
            await flushPromises()
            expect(wrapper.emitted('cleaned')).toHaveLength(1)
        } finally {
            wrapper.unmount()
        }
    },
)
