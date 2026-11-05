// Task history: pinned tasks first, in the order they were placed, then newest
// first, with a quiet marker for tasks that are working or waiting for the user.
// The new task page is not a task until something is sent from it, so its
// unsent draft never adds a row.
import { button, element, h, icon } from './dom.js'
import { axolotl, pixelLoader } from './mascot.js'
import { text } from './messages.js'
import { reducedMotion } from './motion.js'
import { currentThread, session } from './session.js'
import { ACTIVE_STATUSES, threadStatus } from './state.js'

const STATUS_LABELS = {
    active: 'statusActive',
    awaiting: 'statusAwaiting',
    attention: 'statusAttention',
}

// Pinned tasks keep the place they were dragged to; ones pinned before places
// existed, or from the menu, sort by recency among themselves.
export function byPlace(a, b) {
    const pinned = Number(!!b.pinned) - Number(!!a.pinned)
    if (pinned) return pinned
    const rank = a.pinned ? (a.rank ?? Infinity) - (b.rank ?? Infinity) : 0
    return (Number.isNaN(rank) ? 0 : rank) || b.updatedAt - a.updatedAt
}

// Whether a task has been started, and so has a row in the list.
export const listed = (thread) => thread.turns.length > 0 || !!thread.title

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

// How far a working task's checklist has got, or null when it is not working.
export function progressOf(thread) {
    const run = thread.turns.at(-1)?.run
    const steps = run?.progress || []
    if (!steps.length || !ACTIVE_STATUSES.includes(run.status)) return null
    const done = steps.filter((step) => step.status === 'done').length
    const current = steps.some((step) => step.status === 'current') ? 0.5 : 0
    return Math.min(1, (done + current) / steps.length)
}

// The first match of the search, marked.
function marked(value, query) {
    const at = query ? value.toLocaleLowerCase().indexOf(query) : -1
    if (at < 0) return [value]
    return [
        value.slice(0, at),
        h('mark', { text: value.slice(at, at + query.length) }),
        value.slice(at + query.length),
    ]
}

// When only a message matches, a few words around the match show why.
function snippet(thread, query) {
    for (const turn of thread.turns) {
        const value = turn.text.replace(/\s+/g, ' ')
        const at = value.toLocaleLowerCase().indexOf(query)
        if (at < 0) continue
        const start = Math.max(0, at - 12)
        const end = Math.min(value.length, at + query.length + 28)
        return `${start ? '…' : ''}${value.slice(start, end)}${end < value.length ? '…' : ''}`
    }
    return ''
}

// While a task is being dragged the list holds still; what changed meanwhile
// is drawn once it is dropped.
let held = false
let stale = false
export function holdSidebar(hold) {
    held = hold
    if (!hold && stale) renderSidebar()
}

export function renderSidebar() {
    if (held) {
        stale = true
        return
    }
    stale = false
    const query = session.search.trim().toLocaleLowerCase()
    const visible = session.store.threads
        .filter(listed)
        .filter(
            (thread) =>
                !query ||
                `${thread.title} ${thread.turns.map((turn) => turn.text).join(' ')}`
                    .toLocaleLowerCase()
                    .includes(query),
        )
        .sort(byPlace)
    const rows = []
    let previous
    for (const thread of visible) {
        const heading = thread.pinned ? 'pinned' : group(thread.updatedAt)
        if (heading !== previous) rows.push(h('div', { class: 'list-group', text: text(heading) }))
        previous = heading
        const selected = thread.id === session.store.activeId
        const status = threadStatus(thread)
        const name = thread.title || text('untitled')
        const hit = query && !name.toLocaleLowerCase().includes(query) ? snippet(thread, query) : ''
        const progress = progressOf(thread)
        const link = button(
            {
                class: 'thread-link',
                data: { thread: thread.id },
                'aria-current': selected ? 'page' : 'false',
            },
            [
                indicator(status, thread.unread),
                h('span', { class: 'thread-text' }, [
                    h(
                        'span',
                        { id: `thread-name-${thread.id}`, class: 'thread-name' },
                        marked(name, query),
                    ),
                    hit ? h('span', { class: 'thread-snippet' }, marked(hit, query)) : null,
                ]),
            ],
        )
        if (status !== 'idle') link.title = text(STATUS_LABELS[status])
        else if (thread.unread) link.title = text('statusUnread')
        rows.push(
            h(
                'div',
                {
                    class: `thread-row${selected ? ' is-selected' : ''}${thread.unread ? ' is-unread' : ''}${shown.has(thread.id) ? ' is-kept' : ''}`,
                    data: { state: status, row: thread.id, ...(thread.pinned && { pinned: '' }) },
                },
                [
                    link,
                    // Described by its task, so "More actions" says which one.
                    button(
                        {
                            class: 'icon-button thread-more',
                            label: text('more'),
                            'aria-describedby': `thread-name-${thread.id}`,
                            'aria-haspopup': 'menu',
                            'aria-expanded': 'false',
                            data: { threadMenu: thread.id },
                        },
                        icon('more'),
                    ),
                    progress === null
                        ? null
                        : h('span', {
                              class: 'thread-progress',
                              style: `--progress: ${progress.toFixed(3)}`,
                              'aria-hidden': 'true',
                          }),
                ],
            ),
        )
    }
    shown = new Set(visible.map((thread) => thread.id))
    // The highlight stays in the list so its glide is not reset by the re-render.
    const list = element('conversation-list')
    if (highlight.parentNode !== list) list.prepend(highlight)
    for (const child of [...list.children]) if (child !== highlight) child.remove()
    list.append(
        ...(rows.length
            ? rows
            : [
                  h('div', { class: 'list-empty' }, [
                      query ? null : axolotl({ mood: 'idle', className: 'axo-sm' }),
                      h('p', { text: text(query ? 'noResults' : 'noChats') }),
                  ]),
              ]),
    )
    placeHighlight(list)
    // Away from the new task page, its button shows when an unsent draft is
    // waiting there.
    const waiting =
        listed(currentThread()) &&
        session.store.threads.some(
            (thread) =>
                !listed(thread) &&
                thread.id !== session.store.activeId &&
                (thread.draft.trim() || thread.attachments.length),
        )
    const start = document.querySelector('.new-task')
    start.classList.toggle('has-draft', waiting)
    if (waiting) {
        start.dataset.tip = text('draftKept')
        start.dataset.draft = text('draftShort')
    } else {
        delete start.dataset.tip
        delete start.dataset.draft
    }
}

// One highlight lives behind the rows and glides to the selected task, so
// switching tasks reads as a single movement rather than two repaints.
const highlight = h('span', { class: 'thread-highlight', 'aria-hidden': 'true' })
// Tasks the list showed last time; their rows are redrawn without the entrance
// animation, so a list that re-renders while a task works does not flicker.
let shown = new Set()
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
