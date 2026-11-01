// The new-task page and the guide: plain-language starting points for people who
// have never used an agent.
import { button, element, h, icon } from './dom.js'
import { axolotl, gaze, setMood } from './mascot.js'
import { text } from './messages.js'
import { session } from './session.js'

export const SUGGESTIONS = [
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
// and hops when clicked. It only moves while it can be seen.
let heroVisible = false
let lookTimer = 0
const heroAxo = () => element('hero-mark').querySelector('.axo')

function scheduleLook() {
    clearTimeout(lookTimer)
    lookTimer = setTimeout(
        () => {
            const axo = heroAxo()
            if (heroVisible && !document.hidden && axo?.dataset.mood === 'idle' && !following)
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
    const page = element('conversation')
    page.addEventListener('pointermove', followPointer, { passive: true })
    page.addEventListener('pointerleave', letGo)
}

// While idle it turns its head after the pointer, easing towards it so the
// turn has weight, and lets go once the pointer rests or leaves the page.
let following = false
let target = 0
let current = 0
let frame = 0
let resting = 0

function followPointer(event) {
    const axo = heroAxo()
    if (!heroVisible || !axo || event.pointerType === 'touch') return
    if (axo.dataset.mood !== 'idle') {
        if (following) letGo()
        return
    }
    const box = axo.getBoundingClientRect()
    target = Math.max(-1, Math.min(1, (event.clientX - box.left - box.width / 2) / 420))
    following = true
    clearTimeout(resting)
    resting = setTimeout(letGo, 2400)
    if (!frame) frame = requestAnimationFrame(turn)
}

function turn() {
    frame = 0
    const axo = heroAxo()
    if (!following || !axo || axo.dataset.mood !== 'idle') return
    current += (target - current) * 0.14
    gaze(axo, Math.abs(current) < 0.05 ? 0 : current)
    if (Math.abs(target - current) > 0.01) frame = requestAnimationFrame(turn)
}

function letGo() {
    clearTimeout(resting)
    cancelAnimationFrame(frame)
    frame = 0
    following = false
    current = 0
    gaze(heroAxo(), null)
}

// Called as the request is typed on the new task page.
export function heroListens(typing) {
    const axo = heroAxo()
    if (!heroVisible || !axo) return
    if (typing) setMood(axo, 'attend')
    else if (axo.dataset.mood === 'attend') setMood(axo, 'idle')
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
