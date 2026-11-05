// Task rows can be picked up and carried. Dropped at the top of the list or
// among the pinned tasks, a task is pinned there; pinned tasks reorder among
// themselves, and one dropped back below them is unpinned. Carried out of the
// sidebar, a task is fed to the axolotl, which deletes it. If the task left
// backups behind (an old mod version kept by an update, a world ZIP), the
// axolotl tilts its head and asks whether those go too; otherwise it does not
// ask.
import { button, element, h, icon } from './dom.js'
import { axolotl, setMood } from './mascot.js'
import { text } from './messages.js'
import { reducedMotion } from './motion.js'
import { session } from './session.js'
import { toast } from './view-overlays.js'
import { holdSidebar, renderSidebar } from './view-sidebar.js'

const NS = 'http://www.w3.org/2000/svg'
// How far the pointer travels before a press becomes a drag, so clicks and
// small jitters still open the task.
const LIFT = 5
// Without pinned tasks, the top of the first row already counts as the top.
const TOP_REACH = 0.4
const EASE = 'cubic-bezier(0.22, 1, 0.36, 1)'

let settling = false
let drag = null
let hooks = null

/**
 * @typedef {{ name: string, note: string }} Leftover
 * @param {{
 *   pin: (id: string, index: number) => boolean | Promise<boolean>,
 *   unpin: (id: string) => boolean | Promise<boolean>,
 *   feed: (id: string) => Promise<null | { leftovers: Leftover[], undo: null | (() => Promise<void>) }>,
 *   discard: (leftovers: Leftover[]) => void,
 * }} options
 */
export function bindTaskDrag(options) {
    hooks = options
    const list = element('conversation-list')
    list.addEventListener('pointerdown', press)
    // The browser's own drag (of selected text, say) would cancel this one.
    list.addEventListener('dragstart', (event) => event.preventDefault())
}

function press(event) {
    if (settling) return
    const link = event.target.closest('.thread-link')
    if (!link || drag || event.button !== 0 || event.pointerType === 'touch') return
    // While searching, the list is not in its own order.
    if (session.search.trim()) return
    const id = link.dataset.thread
    const start = { x: event.clientX, y: event.clientY }
    const move = (next) => {
        if (!drag) {
            if (Math.hypot(next.clientX - start.x, next.clientY - start.y) < LIFT) return
            if (!lift(id, start)) return stop()
        }
        drag.x = next.clientX
        drag.y = next.clientY
        if (!drag.frame) drag.frame = requestAnimationFrame(step)
    }
    const end = (next) => {
        stop()
        if (!drag) return
        // A quick flick can end before the next frame has placed the card, so
        // the drop is worked out from where the pointer let go.
        if (next.type === 'pointerup') {
            cancelAnimationFrame(drag.frame)
            drag.x = next.clientX
            drag.y = next.clientY
            step()
        }
        release(next.type !== 'pointerup')
    }
    const key = (next) => {
        if (next.key !== 'Escape' || !drag) return
        next.preventDefault()
        next.stopPropagation()
        stop()
        release(true)
    }
    function stop() {
        window.removeEventListener('pointermove', move)
        window.removeEventListener('pointerup', end)
        window.removeEventListener('pointercancel', end)
        window.removeEventListener('keydown', key, true)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', end)
    window.addEventListener('pointercancel', end)
    window.addEventListener('keydown', key, true)
}

// The click that ends a drag must not also open the task.
function swallow(event) {
    event.preventDefault()
    event.stopPropagation()
}

const rowOf = (id) =>
    element('conversation-list').querySelector(`.thread-row[data-row="${CSS.escape(id)}"]`)

function lift(id, start) {
    const row = rowOf(id)
    if (!row) return false
    const list = element('conversation-list')
    const box = row.getBoundingClientRect()
    const top = list.getBoundingClientRect().top - list.scrollTop
    // Where every row was when the drag began. Slots are found on this map, so
    // rows making room under the pointer never move the slot it points at.
    const rows = [...list.querySelectorAll('.thread-row')].map((node) => {
        const place = node.getBoundingClientRect()
        return {
            node,
            id: node.dataset.row,
            pinned: 'pinned' in node.dataset,
            top: place.top - top,
            height: place.height,
        }
    })
    const card = row.querySelector('.thread-link').cloneNode(true)
    for (const node of [card, ...card.querySelectorAll('[id]')]) node.removeAttribute('id')
    // The card says what letting go would do, since it covers the slot it
    // hovers over.
    const intent = h('span', { class: 'drag-intent' }, [icon('pin'), h('span')])
    const ghost = h('div', { class: 'drag-ghost', 'aria-hidden': 'true' }, [
        h('div', { class: 'drag-card' }, [card, intent]),
    ])
    ghost.style.width = `${box.width}px`
    ghost.style.transform = `translate(${box.left}px, ${box.top}px)`
    document.body.append(ghost)
    row.style.setProperty('--row-height', `${box.height}px`)
    row.classList.add('is-drag-source')
    holdSidebar(true)
    document.documentElement.classList.add('is-dragging-task')
    window.addEventListener('click', swallow, true)
    drag = {
        id,
        row,
        ghost,
        rows,
        pinned: 'pinned' in row.dataset,
        grab: { x: start.x - box.left, y: start.y - box.top },
        x: start.x,
        y: start.y,
        last: start.x,
        frame: 0,
        still: 0,
        place: null,
        slot: null,
    }
    showZone()
    return true
}

function step() {
    drag.frame = 0
    scrollList()
    const place = placeAt(drag.x, drag.y)
    if (place.key !== drag.place?.key) settle(place)
    // The card leans a little into fast sideways moves and rights itself
    // once the pointer rests.
    const lean = Math.max(-6, Math.min(6, (drag.x - drag.last) * 0.45))
    drag.last = drag.x
    drag.ghost.style.transform = `translate(${drag.x - drag.grab.x}px, ${drag.y - drag.grab.y}px)`
    drag.ghost.style.setProperty('--tilt', `${lean.toFixed(2)}deg`)
    clearTimeout(drag.still)
    drag.still = setTimeout(() => drag?.ghost.style.setProperty('--tilt', '0deg'), 90)
}

// Near the list's top or bottom edge, the list scrolls to bring more rows in.
function scrollList() {
    const list = element('conversation-list')
    const box = list.getBoundingClientRect()
    if (drag.x > box.right + 8) return
    const edge = drag.y < box.top + 28 ? -1 : drag.y > box.bottom - 28 ? 1 : 0
    if (!edge) return
    const before = list.scrollTop
    list.scrollTop += edge * 6
    if (list.scrollTop !== before) drag.frame ||= requestAnimationFrame(step)
}

function placeAt(x, y) {
    if (x > element('sidebar').getBoundingClientRect().right + 8)
        return { kind: 'feed', key: 'feed' }
    const list = element('conversation-list')
    const at = y - list.getBoundingClientRect().top + list.scrollTop
    const others = drag.rows.filter((row) => row.id !== drag.id)
    const pinned = others.filter((row) => row.pinned)
    const anchor = pinned.at(-1)
    const self = drag.rows.find((row) => row.id === drag.id)
    const reach = anchor
        ? anchor.top + anchor.height * 1.4
        : (others[0] || self).top + (others[0] || self).height * TOP_REACH
    if (at < reach) {
        const index = pinned.filter((row) => row.top + row.height / 2 < at).length
        const home = drag.pinned && index === pinned.filter((row) => row.top < self.top).length
        return home ? { kind: 'home', key: 'home' } : { kind: 'pin', index, key: `pin-${index}` }
    }
    return drag.pinned ? { kind: 'unpin', key: 'unpin' } : { kind: 'home', key: 'home' }
}

// Shows where the task would land: its own row folds away when it would move,
// and a slot opens where it would go.
function settle(place) {
    drag.place = place
    document.documentElement.dataset.dragPlace = place.kind
    drag.row.classList.toggle('is-away', place.kind !== 'home')
    const intent = drag.ghost.querySelector('.drag-intent')
    const says =
        place.kind === 'unpin' ? 'unpinChat' : place.kind === 'pin' && !drag.pinned ? 'pinChat' : ''
    intent.dataset.kind = says ? place.kind : ''
    if (says) intent.lastChild.textContent = text(says)
    if (drag.slot) closeSlot(drag.slot)
    drag.slot = null
    zoneState(place.kind === 'feed' ? 'ready' : 'peek')
    if (place.kind !== 'pin' && place.kind !== 'unpin') return
    const list = element('conversation-list')
    const pinning = place.kind === 'pin' && !drag.pinned
    const slot = h('div', { class: 'drop-slot', data: { kind: place.kind } }, [
        icon('pin'),
        pinning || place.kind === 'unpin'
            ? h('span', { text: text(pinning ? 'dropToPin' : 'dropToUnpin') })
            : null,
    ])
    const pinnedRows = drag.rows.filter((row) => row.pinned)
    let before
    if (place.kind === 'unpin') before = pinnedRows.at(-1).node.nextSibling
    else {
        const others = pinnedRows.filter((row) => row.id !== drag.id)
        before =
            others[place.index]?.node ||
            others.at(-1)?.node.nextSibling ||
            [...list.children].find((node) => !node.classList.contains('thread-highlight'))
    }
    list.insertBefore(slot, before || null)
    drag.slot = slot
}

function closeSlot(slot) {
    slot.classList.add('is-leaving')
    const done = () => slot.remove()
    slot.addEventListener('animationend', done, { once: true })
    setTimeout(done, 320)
}

async function release(cancelled) {
    settling = true
    try {
        const current = drag
        drag = null
        cancelAnimationFrame(current.frame)
        clearTimeout(current.still)
        setTimeout(() => window.removeEventListener('click', swallow, true))
        document.documentElement.classList.remove('is-dragging-task')
        delete document.documentElement.dataset.dragPlace
        const place = (!cancelled && current.place) || { kind: 'home' }
        holdSidebar(false)
        if (place.kind === 'feed') return await feed(current)
        hideZone()
        if (place.kind === 'pin') await hooks.pin(current.id, place.index)
        else if (place.kind === 'unpin') await hooks.unpin(current.id)
        renderSidebar()
        land(current.ghost, current.id)
    } finally {
        settling = false
    }
}

// The carried card settles into the row it became.
function land(ghost, id) {
    const row = rowOf(id)
    if (!row || reducedMotion.matches) return ghost.remove()
    const to = row.getBoundingClientRect()
    row.classList.add('is-landing')
    ghost.style.setProperty('--tilt', '0deg')
    ghost.classList.add('is-landing')
    const flight = ghost.animate(
        [
            { transform: ghost.style.transform },
            { transform: `translate(${to.left}px, ${to.top}px)` },
        ],
        { duration: 260, easing: EASE, fill: 'forwards' },
    )
    flight.finished
        .catch(() => {})
        .finally(() => {
            ghost.remove()
            row.classList.remove('is-landing')
        })
}

// ---------- feeding the axolotl ----------

function feedZone() {
    let zone = element('feed-zone')
    if (zone) return zone
    zone = h(
        'div',
        {
            id: 'feed-zone',
            class: 'feed-zone',
            role: 'dialog',
            'aria-labelledby': 'feed-title',
            'aria-hidden': 'true',
            hidden: true,
        },
        [
            h('div', { class: 'feed-stage' }, [
                h('div', { class: 'feed-mascot' }, [
                    axolotl({ mood: 'idle' }),
                    h('span', { class: 'feed-ask', 'aria-hidden': 'true' }, [questionMark()]),
                ]),
                h('p', { id: 'feed-title', class: 'feed-title' }),
                h('p', { class: 'feed-hint' }),
                h('div', { class: 'feed-question' }),
            ]),
        ],
    )
    // A press on the backdrop while it asks keeps the backups.
    zone.addEventListener('click', (event) => {
        if (event.target === zone && zone.dataset.state === 'asking') zone.answer?.(false)
    })
    document.body.append(zone)
    return zone
}

const mascotOf = (zone) => zone.querySelector('.axo')

function showZone() {
    const zone = feedZone()
    const box = element('conversation').getBoundingClientRect()
    Object.assign(zone.style, {
        left: `${box.left}px`,
        top: `${box.top}px`,
        width: `${box.width}px`,
        height: `${box.height}px`,
    })
    zone.hidden = false
    zoneState('peek')
}

function zoneState(state) {
    const zone = feedZone()
    if (zone.dataset.state === state) return
    const was = zone.dataset.state
    zone.dataset.state = state
    const title = { peek: 'feedPeek', ready: 'feedReady', asking: 'feedAsk' }[state]
    if (title) zone.querySelector('.feed-title').textContent = text(title)
    zone.querySelector('.feed-hint').textContent =
        state === 'ready' ? text('feedReadyHint') : state === 'asking' ? text('feedAskBody') : ''
    const axo = mascotOf(zone)
    if (state === 'ready') setMood(axo, 'attend')
    else if (state === 'peek' && was === 'ready') setMood(axo, 'idle')
}

function hideZone(delay = 0) {
    const zone = element('feed-zone')
    if (!zone || zone.hidden) return
    setTimeout(() => {
        zone.dataset.state = 'leaving'
        zone.setAttribute('aria-hidden', 'true')
        zone.answer = null
        setTimeout(
            () => {
                if (zone.dataset.state !== 'leaving') return
                zone.hidden = true
                delete zone.dataset.state
                zone.querySelector('.feed-question').replaceChildren()
                setMood(mascotOf(zone), 'idle')
            },
            reducedMotion.matches ? 0 : 240,
        )
    }, delay)
}

async function feed(current) {
    const zone = feedZone()
    const axo = mascotOf(zone)
    const result = await hooks.feed(current.id)
    if (!result) {
        // Nothing was deleted (history could not be saved): the card goes home.
        hideZone()
        renderSidebar()
        land(current.ghost, current.id)
        return
    }
    zone.dataset.state = 'eating'
    const mouth = axo.getBoundingClientRect()
    const card = current.ghost.getBoundingClientRect()
    const gulp = reducedMotion.matches
        ? null
        : current.ghost.animate(
              [
                  { transform: current.ghost.style.transform, opacity: 1 },
                  {
                      transform: `translate(${mouth.left + mouth.width / 2 - card.width / 2}px, ${
                          mouth.top + mouth.height * 0.58 - card.height / 2
                      }px) scale(0.1) rotate(-14deg)`,
                      opacity: 0,
                  },
              ],
              { duration: 340, easing: 'cubic-bezier(0.5, 0, 0.75, 0)', fill: 'forwards' },
          )
    const swallowed = () => {
        current.ghost.remove()
        zone.classList.add('is-munching')
        setTimeout(() => zone.classList.remove('is-munching'), 520)
        if (result.leftovers.length) {
            setMood(axo, 'happy', { then: 'attend' })
            setTimeout(() => ask(zone, result), reducedMotion.matches ? 0 : 560)
        } else {
            setMood(axo, 'happy', { then: 'idle' })
            toast(text('taskFed'), result.undo ? { label: text('undo'), run: result.undo } : null)
            hideZone(reducedMotion.matches ? 0 : 760)
        }
    }
    if (gulp) gulp.finished.catch(() => {}).finally(swallowed)
    else swallowed()
}

// The axolotl tilts its head, a question mark pops over it, and it lists what
// the task left behind. Keeping them is the default.
function ask(zone, result) {
    if (zone.dataset.state !== 'eating') return
    zoneState('asking')
    zone.setAttribute('aria-hidden', 'false')
    const keep = button({ class: 'secondary-button', text: text('keepBackups') })
    const drop = button({ class: 'primary-button is-danger', text: text('discardBackups') })
    zone.querySelector('.feed-question').replaceChildren(
        h(
            'ul',
            { class: 'feed-leftovers' },
            result.leftovers.map((entry) =>
                h('li', {}, [
                    icon('archive'),
                    h('span', { class: 'feed-leftover-name', text: entry.name }),
                    h('span', { class: 'feed-leftover-note', text: entry.note }),
                ]),
            ),
        ),
        h('div', { class: 'feed-actions' }, [keep, drop]),
    )
    const escape = (event) => {
        if (event.key !== 'Escape') return
        event.preventDefault()
        event.stopPropagation()
        answer(false)
    }
    const answer = (discard) => {
        window.removeEventListener('keydown', escape, true)
        zone.answer = null
        if (discard) hooks.discard(result.leftovers)
        else toast(text('taskFed'), result.undo ? { label: text('undo'), run: result.undo } : null)
        zone.dataset.state = 'eating'
        setMood(mascotOf(zone), discard ? 'happy' : 'idle', { then: 'idle' })
        element('message-input').focus({ preventScroll: true })
        hideZone(reducedMotion.matches ? 0 : 420)
    }
    zone.answer = answer
    keep.addEventListener('click', () => answer(false))
    drop.addEventListener('click', () => answer(true))
    window.addEventListener('keydown', escape, true)
    keep.focus({ preventScroll: true })
}

// A pixel question mark in the axolotl's own pixel scale.
function questionMark() {
    const svg = document.createElementNS(NS, 'svg')
    svg.setAttribute('viewBox', '0 0 5 7')
    svg.setAttribute('shape-rendering', 'crispEdges')
    const cells = ['.###.', '#...#', '....#', '..##.', '..#..', '.....', '..#..']
    cells.forEach((line, y) =>
        [...line].forEach((cell, x) => {
            if (cell !== '#') return
            const rect = document.createElementNS(NS, 'rect')
            for (const [name, value] of Object.entries({ x, y, width: 1, height: 1 }))
                rect.setAttribute(name, value)
            svg.append(rect)
        }),
    )
    return svg
}
