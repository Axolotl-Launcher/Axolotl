// The new-task page, its first-run tour and the guide: plain-language starting
// points for people who have never used an agent.
import { append, button, element, h, icon } from './dom.js'
import { axolotl, setMood } from './mascot.js'
import { text } from './messages.js'
import { animate } from './motion.js'
import { session } from './session.js'
import { announce } from './view-overlays.js'

const SUGGESTIONS = [
    ['wrench', 'suggestCrash', 'promptCrash', 'suggestCrashHint'],
    ['zap', 'suggestSmooth', 'promptSmooth', 'suggestSmoothHint'],
    ['archive', 'suggestBackup', 'promptBackup', 'suggestBackupHint'],
    ['layers', 'suggestPack', 'promptPack', 'suggestPackHint'],
]

const STEPS = [
    ['message', 'introDescribeTitle', 'introDescribeBody'],
    ['list', 'introReviewTitle', 'introReviewBody'],
    ['shield', 'introApproveTitle', 'introApproveBody'],
]

function steps(className) {
    return h(
        'ol',
        { class: className },
        STEPS.map(([name, title, body], index) =>
            h('li', { style: `--index: ${index}` }, [
                h('span', { class: 'intro-icon' }, [icon(name)]),
                h('span', { class: 'intro-copy' }, [
                    h('strong', { text: text(title) }),
                    h('span', { text: text(body) }),
                ]),
            ]),
        ),
    )
}

export function renderHome() {
    const mark = element('hero-mark')
    if (!mark.firstChild) mark.append(axolotl({ mood: 'idle', className: 'hero-axo' }))
    watchHero(mark)
    element('suggestions').replaceChildren(
        ...SUGGESTIONS.map(([name, label, prompt, hint], index) =>
            button({ class: 'suggestion', data: { prompt }, style: `--index: ${index}` }, [
                h('span', { class: 'suggestion-icon' }, [icon(name)]),
                h('span', { class: 'suggestion-label' }, [
                    h('span', { text: text(label) }),
                    h('small', { text: text(hint) }),
                ]),
                icon('arrow-right', 'suggestion-go'),
            ]),
        ),
    )
    const intro = element('intro')
    intro.hidden = session.store.introDismissed
    if (!intro.hidden)
        intro.replaceChildren(
            h('div', { class: 'intro-heading' }, [
                h('span', { text: text('introTitle') }),
                button({
                    class: 'text-button',
                    text: text('introDismiss'),
                    data: { action: 'dismiss-intro' },
                }),
            ]),
            steps('intro-steps'),
        )
}

// The mascot on the new task page waves when the page appears, looks around
// now and then while nobody types, perks up while a request is being written
// or while the pointer rests on it, and hops when clicked. It does not follow
// the pointer around the page: that read as being watched. It only moves
// while it can be seen.
let heroVisible = false
let lookTimer = 0
const heroAxo = () => element('hero-mark').querySelector('.axo')

function scheduleLook() {
    clearTimeout(lookTimer)
    lookTimer = setTimeout(
        () => {
            const axo = heroAxo()
            if (heroVisible && !document.hidden && axo?.dataset.mood === 'idle')
                setMood(axo, 'look', { then: 'idle' })
            if (heroVisible) scheduleLook()
        },
        9000 + Math.random() * 7000,
    )
}

function watchHero(mark) {
    if (mark.dataset.watched) return
    mark.dataset.watched = 'true'
    new IntersectionObserver(([entry]) => {
        const visible = entry.isIntersecting
        if (visible === heroVisible) return
        heroVisible = visible
        if (!visible) {
            clearTimeout(lookTimer)
            return
        }
        const typing = !!element('message-input').value.trim()
        setMood(heroAxo(), typing ? 'attend' : 'hello', typing ? {} : { then: 'idle' })
        scheduleLook()
    }).observe(mark)
    mark.addEventListener('click', () => setMood(heroAxo(), 'happy', { then: 'idle' }))
    // Resting the pointer on it is a hello of its own: it perks up, and
    // settles again when the pointer moves on, unless a request is being typed.
    mark.addEventListener('pointerenter', (event) => {
        const axo = heroAxo()
        if (event.pointerType !== 'touch' && axo?.dataset.mood === 'idle') setMood(axo, 'attend')
    })
    mark.addEventListener('pointerleave', () => {
        const axo = heroAxo()
        if (axo?.dataset.mood === 'attend' && !element('message-input').value.trim())
            setMood(axo, 'idle')
    })
}

// Called as the request is typed on the new task page.
export function heroListens(typing) {
    const axo = heroAxo()
    if (!heroVisible || !axo) return
    if (typing) setMood(axo, 'attend')
    else if (axo.dataset.mood === 'attend') setMood(axo, 'idle')
}

// ---------- first-run tour ----------

// Three stops on the new task page: where to write, what the agent uses and
// where it works, and the promise that nothing changes without approval.
// Only the card takes the pointer, so the page stays usable underneath, and
// anything done outside the card ends the tour.
const TOUR = [
    {
        target: () => element('composer'),
        pad: 6,
        radius: 26,
        title: 'tourComposeTitle',
        body: 'tourComposeBody',
        notes: [
            { key: '/', text: 'tourSlash' },
            { key: '@', text: 'tourMention' },
        ],
    },
    {
        target: () => element('composer').querySelector('.composer-toolbar'),
        pad: 5,
        radius: 15,
        title: 'tourPickersTitle',
        notes: [
            { icon: 'cube', text: 'tourInstance' },
            { icon: 'sparkle', text: 'tourModel' },
        ],
    },
    {
        target: () => element('send-button'),
        pad: 5,
        radius: 17,
        title: 'tourSafetyTitle',
        body: 'tourSafetyBody',
        notes: [{ key: 'Esc', text: 'tourStop' }],
    },
]
// Room kept between the card, its target and the edges of the window.
const GAP = 12
const MARGIN = 12
const NS = 'http://www.w3.org/2000/svg'
let tour = null
// A tour still fading out when another opens goes at once: both use the same ids.
let fading = null

export function tourOpen() {
    return !!tour
}

// Settles once the new task page has risen into place, so a tour opened on it
// is cut where the composer comes to rest rather than where it passes.
export function homeSettled() {
    const composer = element('composer')
    const running = document
        .getAnimations()
        .filter(
            (animation) =>
                animation.effect?.target?.contains?.(composer) &&
                Number.isFinite(animation.effect.getComputedTiming().endTime),
        )
    return Promise.allSettled(running.map((animation) => animation.finished))
}

function svg(tag, attributes = {}) {
    const node = document.createElementNS(NS, tag)
    for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, String(value))
    return node
}

// The window in the dim is a rounded rectangle drawn as two crossed bars and
// four corner circles. Each piece only moves and scales, so the window glides
// from one target to the next with transforms alone and its corners stay round
// on the way.
function dimLayer() {
    const pieces = [
        svg('rect', { width: 1, height: 1, fill: '#000' }),
        svg('rect', { width: 1, height: 1, fill: '#000' }),
        ...[0, 1, 2, 3].map(() => svg('circle', { r: 1, fill: '#000' })),
    ]
    const mask = svg('mask', { id: 'tour-hole', class: 'tour-hole' })
    mask.append(svg('rect', { width: '100%', height: '100%', fill: '#fff' }), ...pieces)
    const defs = svg('defs')
    defs.append(mask)
    const layer = svg('svg', { class: 'tour-dim', 'aria-hidden': 'true' })
    layer.append(defs, svg('rect', { width: '100%', height: '100%', mask: 'url(#tour-hole)' }))
    return { layer, pieces }
}

function cutOut([across, down, ...corners], x, y, width, height, radius) {
    const move = (node, left, top, scale) => {
        node.style.transform = `translate(${left}px, ${top}px) scale(${scale})`
    }
    move(across, x, y + radius, `${width}, ${height - 2 * radius}`)
    move(down, x + radius, y, `${width - 2 * radius}, ${height}`)
    const right = x + width - radius
    const bottom = y + height - radius
    ;[
        [x + radius, y + radius],
        [right, y + radius],
        [x + radius, bottom],
        [right, bottom],
    ].forEach(([left, top], index) => move(corners[index], left, top, radius))
}

// Opens on the first stop with the focus in the card. `onEnd` runs once,
// however the tour ends: finished, skipped or left for the page.
export function openTour(onEnd) {
    if (tour) return
    const { layer, pieces } = dimLayer()
    const action = (name, className) => button({ class: className, data: { tour: name } })
    const card = h(
        'div',
        {
            id: 'tour-card',
            class: 'tour-card',
            role: 'dialog',
            tabindex: '-1',
            'aria-describedby': 'tour-step tour-title tour-copy',
        },
        [
            axolotl({ mood: 'idle', className: 'axo-sm tour-axo' }),
            h('h2', { id: 'tour-title', class: 'tour-title' }),
            h('div', { id: 'tour-copy', class: 'tour-copy' }),
            h('div', { class: 'tour-foot' }, [
                h(
                    'span',
                    { class: 'tour-dots', 'aria-hidden': 'true' },
                    TOUR.map(() => h('i')),
                ),
                h('span', { id: 'tour-step', class: 'sr-only' }),
                h('span', { class: 'tour-buttons' }, [
                    action('skip', 'text-button tour-skip'),
                    action('back', 'secondary-button small'),
                    action('next', 'primary-button small'),
                ]),
            ]),
        ],
    )
    const root = h('div', { class: 'tour' }, [layer, card])
    const listeners = new AbortController()
    fading?.remove()
    tour = { root, layer, card, pieces, step: 0, placed: '', onEnd, listeners }
    document.body.append(root)
    renderStop()
    reveal(TOUR[0])
    place(true)
    if (!tour) return
    card.focus({ preventScroll: true })
    animate(layer, [{ opacity: 0 }, { opacity: 1 }], { duration: 260, easing: 'ease' })
    animate(
        card,
        [
            { opacity: 0, scale: 0.97 },
            { opacity: 1, scale: 1 },
        ],
        { duration: 320 },
    )

    const { signal } = listeners
    card.addEventListener('keydown', tourKeydown, { signal })
    card.addEventListener(
        'click',
        (event) => {
            const name = event.target.closest('[data-tour]')?.dataset.tour
            if (name === 'skip') endTour('skip')
            else if (name) go(name === 'back' ? -1 : 1)
        },
        { signal },
    )
    // A click or the focus anywhere else means the page is wanted instead.
    document.addEventListener(
        'pointerdown',
        (event) => {
            if (!card.contains(event.target)) endTour('dismiss')
        },
        { capture: true, signal },
    )
    document.addEventListener(
        'focusin',
        (event) => {
            if (!card.contains(event.target)) endTour('dismiss')
        },
        { signal },
    )
    // The window follows its target as the page scrolls, resizes or reflows.
    const follow = () => tour && place(true)
    window.addEventListener('resize', follow, { signal })
    element('message-scroll').addEventListener('scroll', follow, { passive: true, signal })
    const observer = new ResizeObserver(follow)
    observer.observe(element('conversation'))
    observer.observe(element('composer'))
    signal.addEventListener('abort', () => observer.disconnect())
}

export function closeTour() {
    endTour('dismiss')
}

// Redraws the open stop, for a new language or a page that redrew around it.
export function refreshTour() {
    if (!tour) return
    renderStop()
    place(true)
}

function tourKeydown(event) {
    if (event.ctrlKey || event.metaKey || event.altKey) return
    const last = tour.step === TOUR.length - 1
    if (event.key === 'Escape') {
        // Escape belongs to the tour here, never to the page behind it.
        event.stopPropagation()
        endTour('skip')
    } else if (event.key === 'ArrowRight' && !last) go(1)
    else if (event.key === 'ArrowLeft' && tour.step > 0) go(-1)
    // Enter on a button presses it; on the card itself it goes on.
    else if (event.key === 'Enter' && event.target === tour.card) go(1)
    else return
    event.preventDefault()
}

function renderStop() {
    const { card, step } = tour
    const stop = TOUR[step]
    const last = step === TOUR.length - 1
    card.setAttribute('aria-label', text('tourName'))
    card.dataset.step = String(step)
    card.querySelector('.tour-title').textContent = text(stop.title)
    const copy = card.querySelector('.tour-copy')
    copy.replaceChildren()
    append(copy, [
        stop.body && h('p', { text: text(stop.body) }),
        h(
            'ul',
            { class: 'tour-notes' },
            stop.notes.map((note) =>
                h('li', {}, [
                    note.key ? h('kbd', { text: note.key }) : icon(note.icon),
                    h('span', { text: text(note.text) }),
                ]),
            ),
        ),
    ])
    card.querySelector('#tour-step').textContent = text('tourStep', {
        current: step + 1,
        total: TOUR.length,
    })
    card.querySelectorAll('.tour-dots i').forEach((dot, index) => {
        if (index === step) dot.dataset.current = ''
        else delete dot.dataset.current
    })
    const [skip, back, next] = card.querySelectorAll('[data-tour]')
    // A button about to be hidden hands the focus on first, so it never drops
    // to the page and ends the tour.
    if (
        (step === 0 && back === document.activeElement) ||
        (last && skip === document.activeElement)
    )
        next.focus()
    skip.hidden = last
    back.hidden = step === 0
    skip.textContent = text('tourSkip')
    back.textContent = text('tourBack')
    next.textContent = text(last ? 'tourFinish' : 'tourNext')
}

function go(delta) {
    const step = tour.step + delta
    if (step < 0) return
    if (step >= TOUR.length) {
        endTour('finish')
        return
    }
    tour.step = step
    const stop = TOUR[step]
    renderStop()
    reveal(stop)
    place()
    setMood(tour.card.querySelector('.axo'), 'nod', { then: 'idle' })
    const copy = tour.card.querySelector('.tour-copy')
    animate(
        copy,
        [
            { opacity: 0, transform: 'translateY(3px)' },
            { opacity: 1, transform: 'none' },
        ],
        { duration: 280 },
    )
    // The focus stays on the button that was pressed, so the new stop is read
    // out instead.
    announce(
        tour.card.querySelector('#tour-step').textContent,
        text(stop.title),
        stop.body && text(stop.body),
        ...stop.notes.map((note) => `${note.key ? `${note.key} ` : ''}${text(note.text)}`),
    )
}

// Brings a target into view before the window is cut round it; the scroll
// this causes then finds the window already in place.
function reveal(stop) {
    stop.target()?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' })
}

// Cuts the window round the stop's target and puts the card below it when it
// fits, else above, inside the window and clear of the title bar. Moves glide
// unless `instant`, as when following a resize or a scroll.
function place(instant = false) {
    const stop = TOUR[tour.step]
    const target = stop.target()
    if (!target?.getClientRects().length) {
        endTour('dismiss')
        return
    }
    const box = target.getBoundingClientRect()
    const x = Math.round(box.left - stop.pad)
    const y = Math.round(box.top - stop.pad)
    const width = Math.round(box.width + stop.pad * 2)
    const height = Math.round(box.height + stop.pad * 2)
    const radius = Math.min(stop.radius, width / 2, height / 2)
    const bounds = element('workspace').getBoundingClientRect()
    const { offsetWidth: cardWidth, offsetHeight: cardHeight } = tour.card
    const room = {
        below: bounds.bottom - MARGIN - (y + height + GAP),
        above: y - GAP - bounds.top - MARGIN,
    }
    const below = room.below >= cardHeight || (room.above < cardHeight && room.below >= room.above)
    const top = Math.round(
        Math.max(
            bounds.top + MARGIN,
            Math.min(
                bounds.bottom - MARGIN - cardHeight,
                below ? y + height + GAP : y - GAP - cardHeight,
            ),
        ),
    )
    // Centred on the target, but kept over the composer's column so a card for
    // the send button lines up with the composer instead of the window's edge.
    const column = element('composer-wrap').getBoundingClientRect()
    const centred = Math.max(
        column.left,
        Math.min(column.right - cardWidth, x + width / 2 - cardWidth / 2),
    )
    const left = Math.round(
        Math.max(MARGIN, Math.min(window.innerWidth - MARGIN - cardWidth, centred)),
    )
    // Scrolls and observers report far more often than anything moves.
    const placed = [x, y, width, height, radius, left, top].join()
    if (placed === tour.placed) return
    tour.placed = placed
    if (instant) tour.root.classList.add('is-instant')
    cutOut(tour.pieces, x, y, width, height, radius)
    tour.card.style.translate = `${left}px ${top}px`
    tour.card.dataset.side = below ? 'below' : 'above'
    if (!instant) return
    // Applies the new place with transitions off, so nothing glides to it.
    void tour.root.offsetWidth
    tour.root.classList.remove('is-instant')
}

function endTour(reason) {
    if (!tour) return
    const { root, layer, card, listeners, onEnd } = tour
    tour = null
    listeners.abort()
    root.classList.add('is-leaving')
    const finished = reason === 'finish'
    if (finished) setMood(card.querySelector('.axo'), 'happy')
    // Finished or skipped, the next thing to do is to write, so the focus goes
    // to the composer; a dismissal leaves it wherever the user put it.
    if (reason !== 'dismiss') {
        element('message-input').focus({ preventScroll: true })
        card.inert = true
    }
    // The page comes back at once; the card stays a moment longer to hop.
    const leaving = [
        animate(layer, [{ opacity: 1 }, { opacity: 0 }], {
            duration: 220,
            easing: 'ease',
            fill: 'forwards',
        }),
        animate(card, [{ opacity: 1 }, { opacity: 0, scale: 0.97 }], {
            duration: 200,
            delay: finished ? 360 : 0,
            fill: 'forwards',
        }),
    ].filter(Boolean)
    if (leaving.length) {
        fading = root
        Promise.allSettled(leaving.map((animation) => animation.finished)).then(() => {
            root.remove()
            if (fading === root) fading = null
        })
    } else root.remove()
    onEnd?.(reason)
}

export function renderHelp() {
    const facts = ['helpFactModel', 'helpFactScope', 'helpFactScreen', 'localHistory']
    const shortcuts = [
        ['Enter', 'shortcutSend'],
        ['Shift Enter', 'shortcutNewline'],
        ['Esc', 'shortcutStop'],
        ['Ctrl N', 'shortcutNew'],
        ['Ctrl K', 'shortcutPalette'],
        ['/', 'shortcutSlash'],
        ['@', 'shortcutMention'],
        ['Ctrl B', 'shortcutSidebar'],
        ['Ctrl [ ]', 'shortcutSwitch'],
        ['F2', 'rename'],
        ['Ctrl ,', 'shortcutSettings'],
    ]
    element('help-body').replaceChildren(
        h('p', { class: 'help-lead', text: text('helpLead') }),
        steps('help-steps'),
        h('section', { class: 'help-section' }, [
            h('h3', { text: text('helpFactsTitle') }),
            h(
                'ul',
                { class: 'help-facts' },
                facts.map((key) => h('li', {}, [icon('check'), h('span', { text: text(key) })])),
            ),
        ]),
        h('section', { class: 'help-section' }, [
            h('h3', { text: text('shortcutsTitle') }),
            h(
                'dl',
                { class: 'shortcut-list' },
                shortcuts.flatMap(([keys, label]) => [
                    h(
                        'dt',
                        {},
                        keys.split(' ').map((key) => h('kbd', { text: key })),
                    ),
                    h('dd', { text: text(label) }),
                ]),
            ),
        ]),
    )
}
