import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import LogAgentActivity from './LogAgentActivity.vue'

vi.mock('@modrinth/ui', () => ({
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (
            message: { defaultMessage: string },
            values: Record<string, unknown> = {},
        ) =>
            message.defaultMessage.replace(/\{(\w+)(?:, number)?\}/g, (_, key) =>
                String(values[key]),
            ),
    }),
}))

it('shows readable search progress and keeps completed work available in a collapsed history', async () => {
    const wrapper = mount(LogAgentActivity, {
        props: {
            busy: true,
            elapsed: 12,
            entries: [
                { id: 'model', kind: 'model', state: 'ready' },
                {
                    id: 'grep',
                    kind: 'tool',
                    name: 'grep_log_file',
                    state: 'processing',
                    file: 'latest.log',
                    query: 'OutOfMemoryError',
                },
            ],
        },
    })
    expect(wrapper.text()).toContain('Searching logs for related clues')
    expect(wrapper.text()).toContain('latest.log')
    expect(wrapper.text()).toContain('OutOfMemoryError')
    expect(wrapper.text()).not.toContain('grep_log_file')
    expect(wrapper.find('details').attributes('open')).toBeDefined()
    await wrapper.setProps({ busy: false })
    expect(wrapper.find('details').attributes('open')).toBeUndefined()
    expect(wrapper.findAll('li')).toHaveLength(2)
    wrapper.unmount()
})
