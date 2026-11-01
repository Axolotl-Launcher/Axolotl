// The message navigator replaces the scrollbar: one tick per request the user
// sent, the one in view lit and the marked ones flagged with a pixel. Ticks
// swell under the pointer like a dock while a card previews the request they
// lead to; a click scrolls there and flashes it. Arrow keys walk the ticks.
import { button, element, h, icon } from './dom.js'
import { text, whenLabel } from './messages.js'
import { animate, reducedMotion } from './motion.js'
import { currentThread } from './session.js'
import { stateLabel } from './view-panel.js'
import { revealTurn } from './view-transcript.js'

const scroller = () => element('message-scroll')
const rail = () => element('turn-rail')
const SLOT = 12 // px per tick while they fit
const MIN_SLOT = 4
const REACH = 3 // ticks on each side that swell with the one under the pointer
const TONES = { failed: 'danger', awaiting: 'accent', stopped: 'muted', declined: 'muted' }

let turns = new Map()
let frame = 0
let swollen = []
let peeked = null

// A request that is only an attachment is named by it.
const preview = (turn) =>
    turn.text.replace(/\s+/g, ' ').trim() || turn.attachments.map((file) => file.name).join(' · ')

export function renderRail(thread) {
    const nav = rail()
    nav.hidden = thread.turns.length < 2
    turns = new Map(thread.turns.map((turn) => [turn.id, turn]))
    const signature = thread.turns
        .map((turn) => `${turn.id}${turn.marked ? '*' : ''}${TONES[turn.run.status] || ''}`)
        .join()
    if (nav.dataset.signature !== signature) {
        nav.dataset.signature = signature
        nav.replaceChildren(
            h(
                'ol',
                { class: 'rail-list' },
                thread.turns.map((turn) => {
                    const data = { jump: turn.id }
                    if (turn.marked) data.marked = ''
                    if (TONES[turn.run.status]) data.tone = TONES[turn.run.status]
                    return h('li', {}, [
                        button(
                            {
                                class: 'rail-item',
                                tabindex: '-1',
                                'aria-label': text('jumpTo', { text: preview(turn).slice(0, 80) }),
                                data,
                            },
                            h('i', { class: 'rail-tick' }),
                        ),
                    ])
                }),
            ),
        )
        fit()
    }
    markRail()
    if (peeked && !nav.hidden) peek(peeked)
}

// Ticks get a smaller slot when a long task would not fit the rail.
function fit() {
    const nav = rail()
    const count = nav.querySelectorAll('.rail-item').length
    if (!count) return
    const room = nav.clientHeight - 12
    nav.style.setProperty(
        '--slot',
        `${Math.max(MIN_SLOT, Math.min(SLOT, Math.floor(room / count)))}px`,
    )
}

// Lights the request whose turn holds the reading line, a third of the way down.
export function markRail() {
    const nav = rail()
    if (nav.hidden) return
    const view = scroller()
    const line = view.getBoundingClientRect().top + view.clientHeight / 3
    let current = null
    for (const node of element('messages').querySelectorAll(':scope > [data-turn]')) {
        if (node.getBoundingClientRect().top <= line) current = node.dataset.turn
        else break
    }
    current ||= element('messages').firstElementChild?.dataset.turn
    const roving = !nav.contains(document.activeElement)
    for (const item of nav.querySelectorAll('.rail-item')) {
        const on = item.dataset.jump === current
        item.toggleAttribute('data-current', on)
        if (roving) item.tabIndex = on ? 0 : -1
    }
}

function items() {
    return [...rail().querySelectorAll('.rail-item')]
}

// The dock: the tick under the pointer grows most, its neighbours less.
function swell(list, index, exact) {
    for (const item of swollen) item.style.removeProperty('--m')
    swollen = []
    if (index < 0) return
    for (
        let k = Math.max(0, Math.floor(index) - REACH);
        k <= Math.min(list.length - 1, Math.ceil(index) + REACH);
        k++
    ) {
        const distance = Math.abs(k - index)
        const m = Math.exp(-(distance * distance) / (exact ? 2.2 : 1.6))
        if (m < 0.04) continue
        list[k].style.setProperty('--m', m.toFixed(3))
        swollen.push(list[k])
    }
}

function peek(turnId, item = rail().querySelector(`[data-jump="${CSS.escape(turnId)}"]`)) {
    const card = element('rail-peek')
    const turn = turns.get(turnId)
    if (!turn || !item) {
        unpeek()
        return
    }
    const key = `${turnId}:${turn.run.status}:${!!turn.marked}`
    if (card.dataset.key !== key) {
        card.dataset.key = key
        const tone = TONES[turn.run.status] || (turn.run.status === 'done' ? 'success' : 'live')
        card.replaceChildren(
            h('p', { class: 'peek-text', text: preview(turn) }),
            h('div', { class: 'peek-meta' }, [
                h('span', {
                    class: 'peek-state',
                    data: { tone },
                    text: stateLabel(turn.run.status),
                }),
                h('time', { text: whenLabel(turn.createdAt) }),
                turn.attachments.length
                    ? h('span', { class: 'peek-files' }, [
                          icon('attach'),
                          String(turn.attachments.length),
                      ])
                    : null,
                turn.marked
                    ? h('span', { class: 'peek-mark' }, [icon('bookmark'), text('marked')])
                    : null,
            ]),
        )
    }
    peeked = turnId
    const area = card.offsetParent || card.parentElement
    const box = area.getBoundingClientRect()
    const at = item.getBoundingClientRect()
    const half = card.offsetHeight / 2
    const y = Math.min(
        Math.max(at.top + at.height / 2 - box.top - half, 8),
        box.height - card.offsetHeight - 8,
    )
    card.style.setProperty('--y', `${Math.round(y)}px`)
    // Clear of the widest tick and a marked tick's pixel.
    card.style.setProperty('--right', `${Math.round(box.right - at.right + 46)}px`)
    if (!card.classList.contains('is-shown')) {
        // Appear in place rather than gliding in from the previous spot.
        card.classList.add('is-placing')
        void card.offsetWidth
        card.classList.remove('is-placing')
    }
    card.classList.add('is-shown')
}

function unpeek() {
    peeked = null
    element('rail-peek').classList.remove('is-shown')
}

function jump(item) {
    // Older turns are built as they are scrolled to; a jump builds down to its own.
    const turn = revealTurn(currentThread(), item.dataset.jump)
    if (!turn) return
    scroller().scrollTo({
        top: turn.offsetTop - 20,
        behavior: reducedMotion.matches ? 'instant' : 'smooth',
    })
    animate(
        turn.querySelector('.user-bubble'),
        [
            { boxShadow: '0 0 0 0 var(--accent-soft)' },
            { boxShadow: '0 0 0 6px var(--accent-soft)' },
            { boxShadow: '0 0 0 0 transparent' },
        ],
        { duration: 900, delay: 280 },
    )
}

// The tick under the pointer, from the slot maths rather than hit testing, so
// the thin gaps between ticks still count.
function pointed(list, event) {
    const first = list[0].getBoundingClientRect()
    const slot = first.height || SLOT
    const exact = (event.clientY - first.top) / slot - 0.5
    return { exact, index: Math.min(list.length - 1, Math.max(0, Math.round(exact))) }
}

export function bindRail() {
    scroller().addEventListener(
        'scroll',
        () => {
            if (frame) return
            frame = requestAnimationFrame(() => {
                frame = 0
                markRail()
            })
        },
        { passive: true },
    )
    new ResizeObserver(() => fit()).observe(rail())
    const area = rail().parentElement
    area.append(h('div', { id: 'rail-peek', class: 'rail-peek', 'aria-hidden': 'true' }))

    const nav = rail()
    let move = 0
    let last = null
    // Keyboard focus keeps the card open; a tick focused by a click does not.
    let keyed = false
    nav.addEventListener('pointermove', (event) => {
        // The browser repeats the last pointer position after layout changes;
        // only a real move takes over from the keyboard.
        if (last && event.clientX === last.clientX && event.clientY === last.clientY) return
        last = event
        keyed = false
        if (move) return
        move = requestAnimationFrame(() => {
            move = 0
            const list = items()
            if (!list.length || !last) return
            const { exact, index } = pointed(list, last)
            nav.classList.add('is-swelling')
            swell(list, Math.min(list.length - 1, Math.max(0, exact)), true)
            peek(list[index].dataset.jump, list[index])
        })
    })
    const show = (item) => {
        const list = items()
        swell(list, list.indexOf(item), false)
        peek(item.dataset.jump, item)
    }
    nav.addEventListener('pointerdown', () => {
        keyed = false
    })
    nav.addEventListener('pointerleave', () => {
        last = null
        nav.classList.remove('is-swelling')
        if (keyed && nav.contains(document.activeElement)) return
        swell([], -1)
        unpeek()
    })
    nav.addEventListener('click', (event) => {
        const list = items()
        if (!list.length) return
        // A pointer click picks the tick under it even in the gaps; a keyboard
        // click (detail 0) uses the focused tick.
        const item = event.detail
            ? list[pointed(list, event).index]
            : event.target.closest('[data-jump]')
        if (item) jump(item)
    })
    nav.addEventListener('focusin', (event) => {
        const item = event.target.closest('[data-jump]')
        if (!item || !(keyed || item.matches(':focus-visible'))) return
        keyed = true
        show(item)
    })
    nav.addEventListener('focusout', (event) => {
        if (nav.contains(event.relatedTarget)) return
        swell([], -1)
        unpeek()
        markRail()
    })
    nav.addEventListener('keydown', (event) => {
        const list = items()
        const index = list.indexOf(document.activeElement)
        if (index < 0) return
        const next = {
            ArrowUp: index - 1,
            ArrowDown: index + 1,
            Home: 0,
            End: list.length - 1,
        }[event.key]
        if (next === undefined) return
        event.preventDefault()
        const target = list[Math.min(list.length - 1, Math.max(0, next))]
        for (const item of list) item.tabIndex = item === target ? 0 : -1
        keyed = true
        target.focus()
        show(target)
    })
}
