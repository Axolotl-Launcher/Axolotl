// The command menu (Ctrl K): type to run an action or jump to a task. What the
// open task offers comes first. Arrow keys move, Enter runs, Escape closes.
// The focus stays in the search field, which points screen readers at the
// highlighted row with aria-activedescendant.
import { button, element, h, icon } from './dom.js'
import { pixelLoader } from './mascot.js'
import { text } from './messages.js'
import { openDialog } from './view-overlays.js'

let entries = []
let active = 0

function matches(query, ...values) {
    return values.join(' ').toLocaleLowerCase().includes(query)
}

export function openPalette({ current = null, actions, threads }) {
    const dialog = element('palette')
    const input = element('palette-input')
    input.value = ''
    dialog.paletteSource = { current, actions, threads }
    renderPalette()
    openDialog('palette')
    input.focus()
}

export function renderPalette() {
    const dialog = element('palette')
    const { current, actions, threads } = dialog.paletteSource || { actions: [], threads: [] }
    const query = element('palette-input').value.trim().toLocaleLowerCase()
    const sections = [
        current?.entries.length && {
            title: current.title,
            entries: current.entries.filter((entry) => matches(query, entry.label, current.title)),
        },
        {
            title: text('paletteActions'),
            entries: actions.filter((action) => matches(query, action.label)),
        },
        {
            title: text('paletteTasks'),
            entries: threads
                .filter((thread) => matches(query, thread.label, thread.detail))
                .slice(0, query ? 12 : 6),
        },
    ].filter((section) => section && section.entries.length)
    entries = sections.flatMap((section) => section.entries)
    active = Math.min(active, Math.max(0, entries.length - 1))
    if (!query) active = 0
    const row = (entry, index, group) => {
        const node = button(
            {
                id: `palette-option-${index}`,
                class: 'palette-row',
                role: 'option',
                tabindex: '-1',
                'aria-selected': String(index === active),
                'aria-describedby': group,
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
    let index = 0
    // A listbox holds only options: each heading is hidden and read as the
    // description of the rows under it.
    sections.forEach((section, number) => {
        const group = `palette-group-${number}`
        groups.push(
            h('div', {
                id: group,
                class: 'palette-group',
                'aria-hidden': 'true',
                text: section.title,
            }),
            ...section.entries.map((entry) => row(entry, index++, group)),
        )
    })
    if (!entries.length) groups.push(h('p', { class: 'palette-empty', text: text('noResults') }))
    element('palette-list').replaceChildren(...groups)
    pointAt(active)
}

// Tells screen readers which row Enter runs.
function pointAt(index) {
    const input = element('palette-input')
    if (entries.length) input.setAttribute('aria-activedescendant', `palette-option-${index}`)
    else input.removeAttribute('aria-activedescendant')
}

function setActive(index) {
    if (!entries.length) return
    active = (index + entries.length) % entries.length
    for (const node of element('palette-list').querySelectorAll('.palette-row'))
        node.setAttribute('aria-selected', String(Number(node.dataset.paletteIndex) === active))
    pointAt(active)
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
