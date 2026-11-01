// The command menu (Ctrl K): type to run an action or jump to a task. Arrow
// keys move, Enter runs, Escape closes.
import { button, element, h, icon } from './dom.js'
import { pixelLoader } from './mascot.js'
import { text } from './messages.js'

let entries = []
let active = 0

function matches(query, ...values) {
    return values.join(' ').toLocaleLowerCase().includes(query)
}

export function openPalette({ actions, threads }) {
    const dialog = element('palette')
    const input = element('palette-input')
    input.value = ''
    dialog.paletteSource = { actions, threads }
    renderPalette()
    if (!dialog.open) dialog.showModal()
    input.focus()
}

export function renderPalette() {
    const dialog = element('palette')
    const { actions, threads } = dialog.paletteSource || { actions: [], threads: [] }
    const query = element('palette-input').value.trim().toLocaleLowerCase()
    const shownActions = actions.filter((action) => matches(query, action.label))
    const shownThreads = threads
        .filter((thread) => matches(query, thread.label, thread.detail))
        .slice(0, query ? 12 : 6)
    entries = [...shownActions, ...shownThreads]
    active = Math.min(active, Math.max(0, entries.length - 1))
    if (!query) active = 0
    const row = (entry, index) => {
        const node = button(
            {
                class: 'palette-row',
                role: 'option',
                'aria-selected': String(index === active),
                data: { paletteIndex: String(index) },
            },
            [
                h('span', { class: 'palette-icon' }, [
                    entry.status === 'active' ? pixelLoader() : icon(entry.icon),
                ]),
                h('span', { class: 'palette-label', text: entry.label }),
                entry.detail ? h('span', { class: 'palette-detail', text: entry.detail }) : null,
                entry.keys
                    ? h(
                          'span',
                          { class: 'palette-keys' },
                          entry.keys.split(' ').map((key) => h('kbd', { text: key })),
                      )
                    : null,
            ],
        )
        return node
    }
    const groups = []
    if (shownActions.length)
        groups.push(
            h('div', { class: 'palette-group', text: text('paletteActions') }),
            ...shownActions.map((entry, index) => row(entry, index)),
        )
    if (shownThreads.length)
        groups.push(
            h('div', { class: 'palette-group', text: text('paletteTasks') }),
            ...shownThreads.map((entry, index) => row(entry, shownActions.length + index)),
        )
    if (!entries.length) groups.push(h('p', { class: 'palette-empty', text: text('noResults') }))
    element('palette-list').replaceChildren(...groups)
}

function setActive(index) {
    if (!entries.length) return
    active = (index + entries.length) % entries.length
    for (const node of element('palette-list').querySelectorAll('.palette-row'))
        node.setAttribute('aria-selected', String(Number(node.dataset.paletteIndex) === active))
    element('palette-list')
        .querySelector(`[data-palette-index="${active}"]`)
        ?.scrollIntoView({ block: 'nearest' })
}

function run(index) {
    const entry = entries[index]
    if (!entry) return
    element('palette').close()
    entry.run()
}

export function bindPalette() {
    const input = element('palette-input')
    input.addEventListener('input', () => {
        active = 0
        renderPalette()
    })
    input.addEventListener('keydown', (event) => {
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault()
            setActive(active + (event.key === 'ArrowDown' ? 1 : -1))
        } else if (event.key === 'Enter' && !event.isComposing) {
            event.preventDefault()
            run(active)
        }
    })
    const list = element('palette-list')
    list.addEventListener('pointermove', (event) => {
        const node = event.target.closest('.palette-row')
        if (node && Number(node.dataset.paletteIndex) !== active)
            setActive(Number(node.dataset.paletteIndex))
    })
    list.addEventListener('click', (event) => {
        const node = event.target.closest('.palette-row')
        if (node) run(Number(node.dataset.paletteIndex))
    })
}
