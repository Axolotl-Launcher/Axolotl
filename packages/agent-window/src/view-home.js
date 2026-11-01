// The new-task page and the guide: plain-language starting points for people who
// have never used an agent.
import { button, element, h, icon } from './dom.js'
import { axolotl } from './mascot.js'
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

export function renderHelp() {
    const facts = ['helpFactModel', 'helpFactScope', 'helpFactScreen', 'localHistory']
    const shortcuts = [
        ['Enter', 'shortcutSend'],
        ['Shift Enter', 'shortcutNewline'],
        ['Esc', 'shortcutStop'],
        ['Ctrl N', 'shortcutNew'],
        ['Ctrl K', 'shortcutPalette'],
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
