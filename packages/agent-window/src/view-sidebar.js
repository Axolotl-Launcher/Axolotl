// Task history: pinned tasks first, then newest first, with a quiet marker for
// tasks that are working or waiting for the user.
import { button, element, h, icon } from './dom.js'
import { pixelLoader } from './mascot.js'
import { text } from './messages.js'
import { reducedMotion } from './motion.js'
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

function indicator(status, unread) {
    if (status === 'active') return h('span', { class: 'thread-indicator' }, [pixelLoader()])
    if (status === 'awaiting' || status === 'attention' || unread)
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
        .sort((a, b) => Number(!!b.pinned) - Number(!!a.pinned) || b.updatedAt - a.updatedAt)
    const rows = []
    let previous
    for (const thread of visible) {
        const heading = thread.pinned ? 'pinned' : group(thread.updatedAt)
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
                indicator(status, thread.unread),
                h('span', { class: 'thread-name', text: thread.title || text('untitled') }),
            ],
        )
        if (status !== 'idle') link.title = text(STATUS_LABELS[status])
        else if (thread.unread) link.title = text('statusUnread')
        rows.push(
            h(
                'div',
                {
                    class: `thread-row${selected ? ' is-selected' : ''}${thread.unread ? ' is-unread' : ''}`,
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
    // The highlight stays in the list so its glide is not reset by the re-render.
    const list = element('conversation-list')
    if (highlight.parentNode !== list) list.prepend(highlight)
    for (const child of [...list.children]) if (child !== highlight) child.remove()
    list.append(
        ...(rows.length
            ? rows
            : [h('p', { class: 'list-empty', text: text(query ? 'noResults' : 'noChats') })]),
    )
    placeHighlight(list)
}

// One highlight lives behind the rows and glides to the selected task, so
// switching tasks reads as a single movement rather than two repaints.
const highlight = h('span', { class: 'thread-highlight', 'aria-hidden': 'true' })
let highlightPlaced = false

function placeHighlight(list) {
    const row = list.querySelector('.thread-row.is-selected')
    highlight.hidden = !row
    if (!row) {
        highlightPlaced = false
        return
    }
    const glide = highlightPlaced && !reducedMotion.matches
    highlight.classList.toggle('is-gliding', glide)
    highlight.style.transform = `translateY(${row.offsetTop}px)`
    highlight.style.height = `${row.offsetHeight}px`
    highlightPlaced = row.offsetHeight > 0
}
