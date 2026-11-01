// Slash shortcuts and @ mentions in the composer. Typing "/" at the start
// lists starter requests and window actions; typing "@" lists the instances,
// and picking one makes it the task's context. Arrow keys move, Enter or Tab
// picks, Escape closes. Slash names stay in English so they are the same in
// every language.
import { button, element, h, icon } from './dom.js'
import { text } from './messages.js'
import { session } from './session.js'

const SLASH = [
    { name: 'crash', icon: 'wrench', label: 'suggestCrash', prompt: 'promptCrash' },
    { name: 'smooth', icon: 'zap', label: 'suggestSmooth', prompt: 'promptSmooth' },
    { name: 'backup', icon: 'archive', label: 'suggestBackup', prompt: 'promptBackup' },
    { name: 'pack', icon: 'layers', label: 'suggestPack', prompt: 'promptPack' },
    { name: 'model', icon: 'sparkle', label: 'model', action: 'model-picker' },
    { name: 'instance', icon: 'cube', label: 'instance', action: 'instance-picker' },
    { name: 'attach', icon: 'attach', label: 'attach', action: 'attach' },
    { name: 'new', icon: 'plus', label: 'newChat', action: 'new-chat' },
    { name: 'settings', icon: 'settings', label: 'settings', action: 'settings' },
    { name: 'help', icon: 'help', label: 'help', action: 'help' },
]

let shown = []
let active = 0
let onPick = () => {}

export function bindSlash(pick) {
    onPick = pick
    const menu = element('slash-menu')
    menu.addEventListener('pointerdown', (event) => event.preventDefault())
    menu.addEventListener('click', (event) => {
        const row = event.target.closest('[data-slash]')
        if (row) choose(Number(row.dataset.slash))
    })
}

function slashOpen() {
    return !element('slash-menu').hidden
}

export function closeSlash() {
    element('slash-menu').hidden = true
    shown = []
    pointAt(-1)
}

// The focus stays in the message box; aria-activedescendant tells screen
// readers which row Enter picks.
function pointAt(index) {
    const input = element('message-input')
    if (index < 0) {
        input.removeAttribute('aria-activedescendant')
        input.removeAttribute('aria-controls')
        return
    }
    input.setAttribute('aria-controls', 'slash-menu')
    input.setAttribute('aria-activedescendant', `slash-option-${index}`)
}

// Keeps the highlighted row in view inside the list, without scrolling the
// page behind it.
function reveal(index) {
    const menu = element('slash-menu')
    const row = element(`slash-option-${index}`)
    if (!row) return
    if (row.offsetTop < menu.scrollTop) menu.scrollTop = row.offsetTop
    else if (row.offsetTop + row.offsetHeight > menu.scrollTop + menu.clientHeight)
        menu.scrollTop = row.offsetTop + row.offsetHeight - menu.clientHeight
}

// An "@" and a partial name right before the caret, at the start or after a
// space, so e-mail addresses and the like do not open it.
function mentionAt(value) {
    const input = element('message-input')
    const before = value.slice(0, input.selectionEnd ?? value.length)
    const match = before.match(/(?:^|\s)@([^\s@]*)$/)
    return match
        ? { query: match[1].toLowerCase(), start: before.length - match[1].length - 1 }
        : null
}

// Shows the list while the box holds only "/" and a partial name, or while an
// "@" is being typed and there are instances to pick from.
export function syncSlash(value) {
    const mention = mentionAt(value)
    if (mention && session.context.instances.length) {
        const all = text('allLauncher')
        show(
            text('mentionTitle'),
            [
                { key: 'all', icon: 'all', name: `@${all}`, label: '', mention, instance: null },
                ...session.context.instances.map((instance) => ({
                    key: instance.id,
                    icon: 'cube',
                    name: `@${instance.name}`,
                    label: [instance.loader, instance.game_version].filter(Boolean).join(' · '),
                    mention,
                    instance,
                })),
            ].filter(
                (entry) =>
                    !mention.query ||
                    `${entry.name} ${entry.label}`.toLowerCase().includes(mention.query),
            ),
        )
        return
    }
    const match = value.match(/^\/([\w-]*)$/)
    if (!match) {
        closeSlash()
        return
    }
    const query = match[1].toLowerCase()
    show(
        text('slashTitle'),
        SLASH.filter(
            (entry) =>
                entry.name.startsWith(query) || text(entry.label).toLowerCase().includes(query),
        ).map((entry) => ({
            ...entry,
            key: entry.name,
            name: `/${entry.name}`,
            label: text(entry.label),
        })),
    )
}

function show(title, next) {
    if (next.map((entry) => entry.key).join() !== shown.map((entry) => entry.key).join()) active = 0
    shown = next
    const menu = element('slash-menu')
    menu.hidden = !shown.length
    menu.setAttribute('aria-label', title)
    menu.replaceChildren(
        // The list is named by this heading, so the heading itself is hidden.
        h('div', { class: 'slash-head', 'aria-hidden': 'true', text: title }),
        ...shown.map((entry, index) =>
            button(
                {
                    id: `slash-option-${index}`,
                    class: 'slash-row',
                    role: 'option',
                    tabindex: '-1',
                    'aria-selected': String(index === active),
                    data: { slash: String(index) },
                },
                [
                    h('span', { class: 'slash-icon' }, [icon(entry.icon)]),
                    h('span', { class: 'slash-name', text: entry.name }),
                    entry.label ? h('span', { class: 'slash-label', text: entry.label }) : null,
                ],
            ),
        ),
    )
    pointAt(shown.length ? active : -1)
}

function choose(index) {
    const entry = shown[index]
    if (!entry) return
    closeSlash()
    onPick(entry)
}

// Handles the keys the list owns; returns true when the key was used.
export function slashKeydown(event) {
    if (!slashOpen() || event.isComposing) return false
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault()
        active = (active + (event.key === 'ArrowDown' ? 1 : -1) + shown.length) % shown.length
        for (const row of element('slash-menu').querySelectorAll('.slash-row'))
            row.setAttribute('aria-selected', String(Number(row.dataset.slash) === active))
        pointAt(active)
        reveal(active)
        return true
    }
    if ((event.key === 'Enter' && !event.shiftKey) || event.key === 'Tab') {
        event.preventDefault()
        choose(active)
        return true
    }
    if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        closeSlash()
        return true
    }
    return false
}
