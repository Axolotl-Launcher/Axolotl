// The message navigator replaces the scrollbar: one tick per request the user
// sent, the one in view lit. Hovering lists the requests; a click scrolls to
// that request and flashes it.
import { button, element, h } from './dom.js'
import { text } from './messages.js'
import { animate, reducedMotion } from './motion.js'

const scroller = () => element('message-scroll')
let frame = 0

export function renderRail(thread) {
    const rail = element('turn-rail')
    const signature = thread.turns.map((turn) => turn.id).join()
    rail.hidden = thread.turns.length < 2
    if (rail.dataset.signature !== signature) {
        rail.dataset.signature = signature
        rail.replaceChildren(
            h(
                'ol',
                { class: 'rail-list' },
                thread.turns.map((turn) => {
                    const preview = turn.text.replace(/\s+/g, ' ').trim()
                    return h('li', {}, [
                        button(
                            {
                                class: 'rail-item',
                                label: text('jumpTo', { text: preview.slice(0, 80) }),
                                data: { jump: turn.id },
                            },
                            [
                                h('span', { class: 'rail-text', text: preview }),
                                h('i', { class: 'rail-tick' }),
                            ],
                        ),
                    ])
                }),
            ),
        )
    }
    markRail()
}

// Lights the request whose turn holds the reading line, a third of the way down.
export function markRail() {
    const rail = element('turn-rail')
    if (rail.hidden) return
    const view = scroller()
    const line = view.getBoundingClientRect().top + view.clientHeight / 3
    let current = null
    for (const node of element('messages').querySelectorAll(':scope > [data-turn]')) {
        if (node.getBoundingClientRect().top <= line) current = node.dataset.turn
        else break
    }
    current ||= element('messages').firstElementChild?.dataset.turn
    for (const item of rail.querySelectorAll('.rail-item'))
        item.toggleAttribute('data-current', item.dataset.jump === current)
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
    element('turn-rail').addEventListener('click', (event) => {
        const item = event.target.closest('[data-jump]')
        if (!item) return
        const turn = element('messages').querySelector(
            `[data-turn="${CSS.escape(item.dataset.jump)}"]`,
        )
        if (!turn) return
        const view = scroller()
        view.scrollTo({
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
    })
}
