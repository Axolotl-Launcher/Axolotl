// Task history, newest first, with a quiet marker for tasks that are working or
// waiting for the user.
import { button, element, h, icon, spinner } from './dom.js'
import { text } from './messages.js'
import { session } from './session.js'
import { threadStatus } from './state.js'

export const STATUS_LABELS = {
    active: 'statusActive',
    awaiting: 'statusAwaiting',
    attention: 'statusAttention',
}

function group(timestamp) {
    const day = new Date(timestamp)
    day.setHours(0, 0, 0, 0)
    const today = new Date()
    today.setHours(0, 0, 0, 0)
    const days = Math.round((today - day) / 86_400_000)
    return days <= 0 ? 'today' : days === 1 ? 'yesterday' : 'earlier'
}

function indicator(status) {
    if (status === 'active') return h('span', { class: 'thread-indicator' }, [spinner()])
    if (status === 'awaiting' || status === 'attention')
        return h('span', { class: 'thread-indicator' }, [h('span', { class: 'thread-dot' })])
    return null
}

export function renderSidebar() {
    const query = session.search.trim().toLocaleLowerCase()
    const visible = session.store.threads
        .filter((thread) => thread.turns.length || thread.draft || thread.title)
        .filter(
            (thread) =>
                !query ||
                `${thread.title} ${thread.turns.map((turn) => turn.text).join(' ')}`
                    .toLocaleLowerCase()
                    .includes(query),
        )
        .sort((a, b) => b.updatedAt - a.updatedAt)
    const rows = []
    let previous
    for (const thread of visible) {
        const heading = group(thread.updatedAt)
        if (heading !== previous) rows.push(h('div', { class: 'list-group', text: text(heading) }))
        previous = heading
        const selected = thread.id === session.store.activeId
        const status = threadStatus(thread)
        const link = button(
            {
                class: 'thread-link',
                data: { thread: thread.id },
                'aria-current': selected ? 'page' : 'false',
            },
            [
                indicator(status),
                h('span', { class: 'thread-name', text: thread.title || text('untitled') }),
            ],
        )
        if (status !== 'idle') link.title = text(STATUS_LABELS[status])
        rows.push(
            h(
                'div',
                {
                    class: `thread-row${selected ? ' is-selected' : ''}`,
                    data: { state: status, row: thread.id },
                },
                [
                    link,
                    button(
                        {
                            class: 'icon-button thread-more',
                            label: text('more'),
                            data: { threadMenu: thread.id },
                        },
                        icon('more'),
                    ),
                ],
            ),
        )
    }
    element('conversation-list').replaceChildren(
        ...(rows.length
            ? rows
            : [h('p', { class: 'list-empty', text: text(query ? 'noResults' : 'noChats') })]),
    )
}
