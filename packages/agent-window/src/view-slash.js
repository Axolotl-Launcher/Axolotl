// Slash shortcuts: typing "/" at the start of the composer lists starter
// requests and window actions. Arrow keys move, Enter or Tab picks, Escape
// closes. Names stay in English so they are the same in every language.
import { button, element, h, icon } from './dom.js'
import { text } from './messages.js'

export const SLASH = [
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

export function slashOpen() {
    return !element('slash-menu').hidden
}

export function closeSlash() {
    element('slash-menu').hidden = true
    shown = []
}

// Shows the list while the box holds only "/" and a partial name.
export function syncSlash(value) {
    const match = value.match(/^\/([\w-]*)$/)
    if (!match) {
        closeSlash()
        return
    }
    const query = match[1].toLowerCase()
    const next = SLASH.filter(
        (entry) => entry.name.startsWith(query) || text(entry.label).toLowerCase().includes(query),
    )
    if (next.map((entry) => entry.name).join() !== shown.map((entry) => entry.name).join())
        active = 0
    shown = next
    const menu = element('slash-menu')
    menu.hidden = !shown.length
    menu.replaceChildren(
        h('div', { class: 'slash-head', text: text('slashTitle') }),
        ...shown.map((entry, index) =>
            button(
                {
                    class: 'slash-row',
                    role: 'option',
                    'aria-selected': String(index === active),
                    data: { slash: String(index) },
                },
                [
                    h('span', { class: 'slash-icon' }, [icon(entry.icon)]),
                    h('span', { class: 'slash-name', text: `/${entry.name}` }),
                    h('span', { class: 'slash-label', text: text(entry.label) }),
                ],
            ),
        ),
    )
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
