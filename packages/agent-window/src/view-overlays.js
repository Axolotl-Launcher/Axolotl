// Popovers (pickers, menus, previews and tooltips), dialogs, settings and toasts.
import { button, element, focusable, focusPlace, h, icon, logLines, restoreFocus } from './dom.js'
import { pixelLoader } from './mascot.js'
import { text, whenLabel } from './messages.js'
import { currentThread, modelsAvailable, permissionsAvailable, session } from './session.js'
import { changeTitle } from './tools.js'
import { PASTE_ICONS } from './view-composer.js'
import { KINDS } from './view-transcript.js'

// ---------- openers ----------

// The control that opened a popover shows it as expanded until it closes, and
// gets the focus back if a redraw replaced it in the meantime.
const openers = new WeakMap()

function opened(overlay, anchor) {
    anchor?.setAttribute('aria-expanded', 'true')
    openers.set(overlay, focusPlace(anchor) || { node: anchor, key: null, scopes: [] })
}

export function closePopover(popover) {
    if (popover.matches(':popover-open')) popover.hidePopover()
}

export function bindOverlays(fallback) {
    for (const popover of document.querySelectorAll('.popover')) {
        // beforetoggle comes as the popover closes, toggle after it has.
        popover.addEventListener('beforetoggle', (event) => {
            if (event.newState === 'closed')
                openers.get(popover)?.node?.setAttribute('aria-expanded', 'false')
        })
        popover.addEventListener('toggle', (event) => {
            const active = document.activeElement
            if (
                event.newState === 'closed' &&
                (popover.contains(active) || active === document.body)
            )
                restoreFocus(openers.get(popover), fallback())
        })
        // Tabbing out closes it, so no popover stays open away from the focus.
        popover.addEventListener('focusout', (event) => {
            if (
                event.relatedTarget &&
                !popover.contains(event.relatedTarget) &&
                popover.matches(':popover-open')
            )
                popover.hidePopover()
        })
    }
    for (const dialog of document.querySelectorAll('dialog'))
        dialog.addEventListener('close', () => {
            const active = document.activeElement
            if (dialog.contains(active) || active === document.body)
                restoreFocus(openers.get(dialog), fallback())
        })
    element('menu').addEventListener('keydown', menuKeydown)
    // Pressing the picker's own button moves the focus out of the picker,
    // which closes it before the click arrives; so whether it was open is
    // read here, as the pointer goes down.
    document.addEventListener(
        'pointerdown',
        (event) => {
            pressedOpen = pickerOpen() && pickerAnchor?.contains(event.target) ? pickerAnchor : null
        },
        true,
    )
}

// ---------- pickers ----------

let pickerKind = null
let pickerAnchor = null
let pressedOpen = null

export function pickerOpen() {
    return element('picker').matches(':popover-open')
}

// The model and instance buttons open their picker and close it again, like
// any button that opens a menu; the other button switches pickers. A click
// from the pointer counts what the picker was when the pointer went down, a
// key press what it is now. With no event (a shortcut, the command menu) the
// picker just opens.
export function togglePicker(kind, anchor, event = null) {
    const wasOpen =
        event?.detail > 0
            ? pressedOpen === anchor
            : !!event && pickerOpen() && pickerKind === kind && pickerAnchor === anchor
    pressedOpen = null
    if (!wasOpen) {
        openPicker(kind, anchor)
        return true
    }
    closePicker()
    anchor.focus()
    return false
}

export function openPicker(kind, anchor) {
    const picker = element('picker')
    closePopover(picker)
    pickerKind = kind
    pickerAnchor = anchor
    picker.setAttribute('aria-label', text(kind))
    const search = element('picker-search')
    search.value = ''
    search.placeholder = text(kind === 'model' ? 'modelSearch' : 'instanceSearch')
    search.setAttribute('aria-label', search.placeholder)
    renderPicker()
    const rect = anchor.getBoundingClientRect()
    const width = Math.min(kind === 'model' ? 340 : 320, window.innerWidth - 24)
    const left = kind === 'model' ? rect.right - width : rect.left
    const above = rect.top > window.innerHeight - rect.bottom
    Object.assign(picker.style, {
        width: `${width}px`,
        left: `${Math.max(12, Math.min(left, window.innerWidth - width - 12))}px`,
        top: above ? 'auto' : `${rect.bottom + 8}px`,
        bottom: above ? `${window.innerHeight - rect.top + 8}px` : 'auto',
        maxHeight: `${Math.max(180, (above ? rect.top : window.innerHeight - rect.bottom) - 24)}px`,
        transformOrigin: `${kind === 'model' ? 'right' : 'left'} ${above ? 'bottom' : 'top'}`,
    })
    opened(picker, anchor)
    // With its opener as source, Tab moves on from the popover to what follows
    // the opener.
    picker.showPopover({ source: anchor })
    if (focusable(search)) search.focus()
    else picker.querySelector('button')?.focus()
}

export function renderPicker() {
    if (!pickerKind) return
    const query = element('picker-search').value.trim().toLocaleLowerCase()
    const thread = currentThread()
    const options = []
    const footer = element('picker-footer')
    footer.hidden = true
    if (pickerKind === 'model') {
        element('picker-search').hidden = !modelsAvailable()
        if (!modelsAvailable()) {
            options.push(
                h('div', { class: 'picker-setup' }, [
                    h('span', { class: 'setup-icon' }, [icon('sparkle')]),
                    h('strong', {
                        text: text(session.context.ai_enabled ? 'noModels' : 'aiDisabled'),
                    }),
                    h('p', { text: text('setupModelHint') }),
                    button(
                        { class: 'secondary-button small', data: { action: 'refresh-context' } },
                        [icon('refresh'), h('span', { text: text('checkAgain') })],
                    ),
                ]),
            )
        } else {
            session.context.providers.forEach((provider, index) => {
                const models = provider.models.filter((model) =>
                    `${provider.name} ${model.name} ${model.id}`
                        .toLocaleLowerCase()
                        .includes(query),
                )
                if (!models.length) return
                // A listbox holds only options, so the provider heading is
                // hidden and read as each option's description instead.
                const group = `picker-group-${index}`
                options.push(
                    h('div', {
                        id: group,
                        class: 'picker-group',
                        'aria-hidden': 'true',
                        text: provider.name,
                    }),
                )
                for (const model of models) {
                    const selected =
                        thread.selection?.provider_id === provider.provider_id &&
                        thread.selection?.model_id === model.id
                    options.push(
                        button(
                            {
                                class: 'picker-option',
                                role: 'option',
                                'aria-selected': String(selected),
                                'aria-describedby': group,
                                data: { provider: provider.provider_id, model: model.id },
                            },
                            [
                                h('span', { class: 'option-copy' }, [
                                    h('strong', { text: model.name }),
                                    h('small', { text: model.id }),
                                ]),
                                selected ? icon('check', 'option-check') : null,
                            ],
                        ),
                    )
                }
            })
            if (!options.length)
                options.push(h('p', { class: 'picker-empty', text: text('noModelMatches') }))
            footer.hidden = false
            footer.replaceChildren(h('span', { text: text('byok') }))
        }
    } else {
        element('picker-search').hidden = false
        if (!query || text('allLauncher').toLocaleLowerCase().includes(query))
            options.push(
                button(
                    {
                        class: 'picker-option',
                        role: 'option',
                        'aria-selected': String(!thread.instanceId),
                        data: { instance: '' },
                    },
                    [
                        h('span', { class: 'option-icon' }, [icon('globe')]),
                        h('span', { class: 'option-copy' }, [
                            h('strong', { text: text('allLauncher') }),
                            h('small', { text: text('allLauncherHint') }),
                        ]),
                        !thread.instanceId ? icon('check', 'option-check') : null,
                    ],
                ),
            )
        const instances = session.context.instances.filter((instance) =>
            `${instance.name} ${instance.game_version} ${instance.loader}`
                .toLocaleLowerCase()
                .includes(query),
        )
        if (instances.length)
            options.push(
                h('div', { class: 'picker-group', 'aria-hidden': 'true', text: text('instances') }),
            )
        for (const instance of instances) {
            const selected = thread.instanceId === instance.id
            options.push(
                button(
                    {
                        class: 'picker-option',
                        role: 'option',
                        'aria-selected': String(selected),
                        data: { instance: instance.id },
                    },
                    [
                        h('span', { class: 'option-icon' }, [icon('cube')]),
                        h('span', { class: 'option-copy' }, [
                            h('strong', { text: instance.name }),
                            h('small', {
                                text: [instance.loader, instance.game_version]
                                    .filter(Boolean)
                                    .join(' · '),
                            }),
                        ]),
                        selected ? icon('check', 'option-check') : null,
                    ],
                ),
            )
        }
        if (!instances.length)
            options.push(
                h('p', {
                    class: 'picker-empty',
                    text: text(query ? 'noInstanceMatches' : 'noInstances'),
                }),
            )
    }
    const list = element('picker-options')
    list.replaceChildren(...options)
    // The list is one tab stop, on the current choice; the setup hint and
    // "no matches" are not a list at all.
    const choices = [...list.querySelectorAll('.picker-option')]
    const current =
        choices.find((choice) => choice.getAttribute('aria-selected') === 'true') || choices[0]
    for (const choice of choices) choice.tabIndex = choice === current ? 0 : -1
    if (choices.length) {
        list.setAttribute('role', 'listbox')
        list.setAttribute(
            'aria-label',
            text(pickerKind === 'model' ? 'modelLabel' : 'instanceLabel'),
        )
    } else {
        list.removeAttribute('role')
        list.removeAttribute('aria-label')
    }
}

export function closePicker() {
    closePopover(element('picker'))
}

// Arrow keys move between options as in a native list; up from the first one
// returns to the search field.
export function pickerKeydown(event) {
    const options = [...element('picker-options').querySelectorAll('.picker-option')]
    const index = options.indexOf(document.activeElement)
    if (index < 0) return
    const next = { ArrowDown: index + 1, ArrowUp: index - 1, Home: 0, End: options.length - 1 }[
        event.key
    ]
    if (next === undefined) return
    event.preventDefault()
    const search = element('picker-search')
    if (next < 0) {
        if (focusable(search)) search.focus()
    } else focusPickerOption(next)
}

export function focusPickerOption(index) {
    const options = [...element('picker-options').querySelectorAll('.picker-option')]
    const target = options[Math.min(index, options.length - 1)]
    for (const option of options) option.tabIndex = option === target ? 0 : -1
    target?.focus()
}

// ---------- menus ----------

// A menu of actions, or, given a label, a short note such as the preview
// explanation, which is a small dialog rather than a menu.
export function openMenu(anchor, items, align = 'end', label = '') {
    const menu = element('menu')
    closePopover(menu)
    menu.replaceChildren(...items)
    const note = menu.querySelector('p')
    menu.setAttribute('role', label ? 'dialog' : 'menu')
    if (label) {
        menu.tabIndex = -1
        menu.setAttribute('aria-label', label)
        if (note) note.id = 'menu-note-text'
        menu.setAttribute('aria-describedby', 'menu-note-text')
    } else {
        for (const name of ['tabindex', 'aria-label', 'aria-describedby'])
            menu.removeAttribute(name)
    }
    const rect = anchor.getBoundingClientRect()
    menu.style.minWidth = '200px'
    menu.style.top = `${rect.bottom + 6}px`
    menu.style.left = align === 'end' ? 'auto' : `${rect.left}px`
    menu.style.right =
        align === 'end' ? `${Math.max(12, window.innerWidth - rect.right)}px` : 'auto'
    menu.style.transformOrigin = `${align === 'end' ? 'right' : 'left'} top`
    opened(menu, anchor)
    menu.showPopover({ source: anchor })
    ;(menu.querySelector('[role="menuitem"]') || menu).focus()
}

// Menu items are reached with the arrow keys, not Tab: Tab leaves the menu.
export function menuItem(name, label, data, danger = false) {
    return button(
        {
            class: `menu-item${danger ? ' is-danger' : ''}`,
            role: 'menuitem',
            tabindex: '-1',
            data,
        },
        [icon(name), h('span', { text: label })],
    )
}

function menuKeydown(event) {
    if (event.key === 'Tab') {
        // Closing hands the focus back to the opener first, so Tab carries on
        // from there.
        closeMenu()
        return
    }
    const entries = [...element('menu').querySelectorAll('[role="menuitem"]')]
    const index = entries.indexOf(document.activeElement)
    const next = { ArrowDown: index + 1, ArrowUp: index - 1, Home: 0, End: entries.length - 1 }[
        event.key
    ]
    if (!entries.length || next === undefined) return
    event.preventDefault()
    entries[(next + entries.length) % entries.length].focus()
}

export function closeMenu() {
    closePopover(element('menu'))
}

// ---------- dialogs ----------

export function openDialog(id) {
    const dialog = element(id)
    const error = dialog.querySelector('[data-dialog-error]')
    if (error) {
        error.textContent = ''
        error.hidden = true
    }
    if (dialog.open) return
    openers.set(dialog, focusPlace())
    dialog.showModal()
}

let confirmState = null

// One dialog for every confirmation. `accept` may be async; the dialog stays
// open while it runs and shows its failure inside instead of closing.
export function confirm({
    title,
    body,
    accept,
    icon: name = 'warning',
    danger = false,
    rename = null,
    onAccept,
}) {
    confirmState = { onAccept, busy: false }
    element('confirm-title').textContent = title
    element('confirm-text').textContent = body || ''
    element('confirm-text').hidden = !body
    element('confirm-icon').replaceChildren(icon(name))
    element('confirm-icon').dataset.tone = danger
        ? 'danger'
        : name === 'shield'
          ? 'accent'
          : 'warning'
    element('confirm-icon').hidden = !!rename
    const field = element('rename-field')
    field.hidden = rename === null
    const input = element('rename-input')
    input.required = rename !== null
    input.value = rename ?? ''
    const acceptButton = element('confirm-accept')
    acceptButton.textContent = accept
    acceptButton.classList.toggle('is-danger', danger)
    setConfirmBusy(false)
    openDialog('confirm-dialog')
    if (rename !== null) {
        input.focus()
        input.select()
    } else element('confirm-cancel').focus()
}

function setConfirmBusy(busy) {
    if (confirmState) confirmState.busy = busy
    element('confirm-accept').disabled = busy
    element('confirm-cancel').disabled = busy
    element('confirm-accept').setAttribute('aria-busy', String(busy))
}

export function confirmBusy() {
    return confirmState?.busy === true
}

export async function acceptConfirm() {
    if (!confirmState || confirmState.busy) return
    const state = confirmState
    setConfirmBusy(true)
    let closed = false
    try {
        closed = (await state.onAccept(element('rename-input').value.trim())) !== false
    } finally {
        if (confirmState === state) {
            setConfirmBusy(false)
            if (closed) {
                confirmState = null
                element('confirm-dialog').close()
            }
        }
    }
}

export function dialogError(message) {
    const dialog = [...document.querySelectorAll('dialog[open]')].at(-1)
    const error = dialog?.querySelector('[data-dialog-error]')
    if (!error) return false
    error.textContent = message
    error.hidden = false
    return true
}

// ---------- settings ----------

export function renderSettings() {
    const available = permissionsAvailable()
    const busy = session.permissionsLoading || session.permissionsSaving
    const full = element('permission-full-access')
    const computer = element('permission-computer-use')
    full.checked = session.permissions.full_access
    full.disabled = !available || busy || session.permissionReadError
    computer.checked = session.permissions.computer_use
    computer.disabled =
        !available || busy || session.permissionReadError || !session.permissions.full_access
    element('permission-status').textContent = text(
        !available
            ? 'permissionUnavailable'
            : session.permissionReadError
              ? 'permissionError'
              : session.permissionsLoading
                ? 'permissionPending'
                : 'permissionFuture',
    )
    // A radio group is one tab stop, on the checked choice.
    for (const choice of element('detail-choice').querySelectorAll('[data-detail]')) {
        const checked = choice.dataset.detail === session.store.detail
        choice.setAttribute('aria-checked', String(checked))
        choice.tabIndex = checked ? 0 : -1
    }
    renderDetailPreview()
    const instructions = element('instructions-input')
    if (document.activeElement !== instructions) instructions.value = session.store.instructions
    element('settings-dialog')
        .querySelectorAll('[data-action="close-dialog"]')
        .forEach((close) => {
            close.disabled = session.permissionsSaving
        })
}

// A miniature of a task part-way through, drawn in the step groups' own
// marks, under the steps-shown choice: a finished group, then one at work on
// its third step. Concise folds both, the working one to the step at hand;
// balanced opens the working one; detailed opens both and names each tool.
// The picture is built once per catalog and only its level changes, so CSS
// can move it from one level to the next.
const PREVIEW_DONE = [
    ['detailPreviewJava', 'java_list'],
    ['detailPreviewMemory', 'system_get_memory'],
]
const PREVIEW_WORKING = [
    ['detailPreviewPick', 'instance_list'],
    ['detailPreviewRead', 'instance_get'],
    ['detailPreviewLog', 'logs_read'],
]
const PREVIEW_LEVELS = {
    concise: 'detailCaptionConcise',
    balanced: 'detailCaptionBalanced',
    detailed: 'detailCaptionDetailed',
}

function previewGroup(working, labels, steps) {
    return h('div', { class: 'dp-group', data: { state: working ? 'running' : 'done' } }, [
        h('div', { class: 'dp-head' }, [
            h('span', { class: 'dp-mark' }, [working ? pixelLoader() : icon('check-circle')]),
            h(
                'span',
                { class: 'dp-label' },
                labels.map((label) => h('span', { text: label })),
            ),
            icon('right', 'dp-chevron'),
        ]),
        h('div', { class: 'dp-body' }, [
            h('div', { class: 'dp-clip' }, [
                h(
                    'ol',
                    { class: 'dp-steps' },
                    steps.map(([title, tool], index) => {
                        const now = working && index === steps.length - 1
                        return h(
                            'li',
                            { class: 'dp-step', data: { state: now ? 'running' : 'done' } },
                            [
                                h('span', { class: 'dp-icon' }, [
                                    now ? pixelLoader() : icon('check'),
                                ]),
                                h('span', { class: 'dp-title', text: title }),
                                h('code', { class: 'dp-tool', text: tool }),
                            ],
                        )
                    }),
                ),
            ]),
        ]),
    ])
}

function renderDetailPreview() {
    const preview = element('detail-preview')
    if (preview.dataset.level !== session.store.detail) preview.dataset.level = session.store.detail
    const done = PREVIEW_DONE.map(([key, tool]) => [text(key), tool])
    const working = PREVIEW_WORKING.map(([key, tool]) => [text(key), tool])
    const labels = [
        text('stepsDoneNamed', { title: done[0][0], count: done.length }),
        text('detailPreviewNow'),
        text('stepsRunning', { count: working.length }),
    ]
    const captions = Object.entries(PREVIEW_LEVELS).map(([level, key]) => [level, text(key)])
    const signature = JSON.stringify([done, working, labels, captions])
    if (preview.dataset.signature === signature) return
    preview.dataset.signature = signature
    preview.replaceChildren(
        h('div', { class: 'dp-well' }, [
            previewGroup(false, [labels[0]], done),
            // Folded, the working group names the step at hand; open, it counts.
            previewGroup(true, [labels[1], labels[2]], working),
        ]),
        h(
            'p',
            { class: 'dp-caption' },
            captions.map(([level, caption]) => h('span', { data: { for: level }, text: caption })),
        ),
    )
}

// ---------- toasts ----------

export function toast(message, action = null, tone = 'info') {
    const holder = element('toasts')
    const node = h('div', { class: 'toast', data: { tone } }, [
        icon(tone === 'error' ? 'alert' : tone === 'success' ? 'check-circle' : 'info'),
        h('span', { text: message }),
    ])
    if (action) {
        const run = button({ class: 'text-button', text: action.label }, [])
        run.addEventListener('click', () => {
            action.run()
            dismiss()
        })
        node.append(run)
    }
    const dismiss = () => {
        if (!node.isConnected || node.dataset.leaving) return
        node.dataset.leaving = 'true'
        node.addEventListener('animationend', () => node.remove(), { once: true })
        setTimeout(() => node.remove(), 400)
    }
    holder.append(node)
    while (holder.childElementCount > 3) holder.firstElementChild.remove()
    let timer = setTimeout(dismiss, action ? 7000 : 4500)
    // A toast waits while the pointer or the focus is on it, so its action can
    // still be reached by keyboard or read at leisure.
    const hold = () => clearTimeout(timer)
    const release = (held) => {
        clearTimeout(timer)
        if (!held) timer = setTimeout(dismiss, 2000)
    }
    node.addEventListener('pointerenter', hold)
    node.addEventListener('focusin', hold)
    node.addEventListener('pointerleave', () => release(node.contains(document.activeElement)))
    node.addEventListener('focusout', (event) => {
        if (!node.contains(event.relatedTarget)) release(node.matches(':hover'))
    })
    return node
}

// One polite announcement for screen readers. Each line is a new node, so the
// same words are read again when they come round again.
export function announce(...lines) {
    element('announcer').replaceChildren(
        ...lines.filter(Boolean).map((line) => h('p', { text: line })),
    )
}

// ---------- tooltips ----------

// Buttons that only show an icon name themselves after a short rest, with their
// shortcut as keycaps. Once one tooltip has shown, the next shows at once, so
// running the pointer along a toolbar reads every button.
const MAC = /Mac|iPhone|iPad/.test(navigator.platform)
const keyLabel = (key) => (key === 'Ctrl' && MAC ? '⌘' : key)
let tipTimer = 0
let tipPending = null
let tipShown = null
let warmUntil = 0

function showTip(target) {
    const tip = element('tip')
    tipShown = target
    tip.replaceChildren(
        h('span', { text: target.dataset.tip }),
        ...(target.dataset.keys || '')
            .split(' ')
            .filter(Boolean)
            .map((key) => h('kbd', { text: keyLabel(key) })),
    )
    if (!tip.matches(':popover-open')) tip.showPopover()
    const rect = target.getBoundingClientRect()
    const below = rect.bottom + 8 + tip.offsetHeight < window.innerHeight
    const x = rect.left + rect.width / 2 - tip.offsetWidth / 2
    tip.style.setProperty(
        '--x',
        `${Math.round(Math.min(Math.max(x, 8), window.innerWidth - tip.offsetWidth - 8))}px`,
    )
    tip.style.setProperty(
        '--y',
        `${Math.round(below ? rect.bottom + 8 : rect.top - 8 - tip.offsetHeight)}px`,
    )
    tip.dataset.side = below ? 'below' : 'above'
    tip.classList.add('is-shown')
}

function hideTip() {
    clearTimeout(tipTimer)
    tipPending = null
    if (!tipShown) return
    tipShown = null
    warmUntil = Date.now() + 500
    element('tip').classList.remove('is-shown')
}

function queueTip(target, delay) {
    if (target && (target === tipPending || target === tipShown)) return
    hideTip()
    if (!target || target.disabled) return
    tipPending = target
    tipTimer = setTimeout(
        () => {
            tipPending = null
            if (target.isConnected) showTip(target)
        },
        Date.now() < warmUntil ? 0 : delay,
    )
}

export function bindTips() {
    // A button that was just pressed stays quiet until the pointer leaves it:
    // what it opened can redraw under the pointer and look like a new hover.
    let pressed = null
    document.addEventListener('pointerover', (event) => {
        if (event.pointerType === 'touch') return
        const target = event.target.closest?.('[data-tip]') || null
        if (target && target === pressed) return
        pressed = null
        queueTip(target, 520)
    })
    // Focus shows a tooltip only when the keyboard moved it, not when a click
    // opened something that focused its first button.
    let keyboard = false
    document.addEventListener(
        'pointerdown',
        (event) => {
            keyboard = false
            pressed = event.target.closest?.('[data-tip]') || null
        },
        true,
    )
    document.addEventListener('keydown', () => (keyboard = true), true)
    document.addEventListener('focusin', (event) => {
        const target = event.target.closest?.('[data-tip]')
        if (keyboard && target?.matches(':focus-visible')) queueTip(target, 280)
    })
    for (const name of ['pointerdown', 'keydown', 'scroll', 'focusout'])
        document.addEventListener(name, hideTip, true)
    window.addEventListener('blur', hideTip)
}

// ---------- pasted text ----------

// A pasted crash report or log, shown above its chip with the same colours as
// a log in an answer. Long pastes show their start; the copy has all of it.
const PEEK_LINES = 300
export function openFilePeek(file, anchor) {
    const peek = element('file-peek')
    closePopover(peek)
    const lines = file.text.split('\n')
    peek.replaceChildren(
        h('div', { class: 'file-peek-head' }, [
            icon(PASTE_ICONS[file.kind] || 'file'),
            h('strong', { text: file.name }),
            h('span', { text: text('lineCount', { count: file.lines }) }),
            button(
                {
                    class: 'icon-button tiny',
                    label: text('copyText'),
                    data: { copyPaste: file.id },
                },
                icon('copy'),
            ),
        ]),
        h('pre', {}, [
            h('code', { class: 'is-log' }, logLines(lines.slice(0, PEEK_LINES).join('\n'))),
        ]),
        ...(lines.length > PEEK_LINES
            ? [
                  h('p', {
                      class: 'file-peek-more',
                      text: text('moreLines', { count: lines.length - PEEK_LINES }),
                  }),
              ]
            : []),
    )
    peek.setAttribute('aria-label', text('previewFile', { name: file.name }))
    const rect = showPeek(peek, anchor)
    // It goes on the side with room for it, or the roomier side, and is never
    // taller than that room, so it never runs off the window.
    const roomAbove = rect.top - 20
    const roomBelow = window.innerHeight - rect.bottom - 20
    peek.style.maxHeight = ''
    const natural = peek.offsetHeight
    const above = natural <= roomAbove || (natural > roomBelow && roomAbove > roomBelow)
    const room = Math.max(120, above ? roomAbove : roomBelow)
    if (natural > room) peek.style.maxHeight = `${Math.round(room)}px`
    peek.style.top = `${Math.round(above ? rect.top - 8 - peek.offsetHeight : rect.bottom + 8)}px`
    peek.dataset.side = above ? 'above' : 'below'
    // The preview takes the focus, so it is read out and Escape closes it.
    peek.focus()
}

// Shows a preview by the control that opened it, starting at the control's
// left edge but kept inside the window. Placing needs the preview's size, so
// it shows first; the caller places it vertically.
function showPeek(peek, anchor) {
    opened(peek, anchor)
    peek.showPopover({ source: anchor })
    const rect = anchor.getBoundingClientRect()
    peek.style.left = `${Math.round(Math.max(12, Math.min(rect.left, window.innerWidth - peek.offsetWidth - 12)))}px`
    return rect
}

// ---------- changes ----------

// What the open task has changed, newest last, with one way to ask for all of
// it to be undone. Undoing goes through the agent, so it is planned and
// approved like any other change.
export function openChanges(entries, anchor) {
    const peek = element('changes-peek')
    closePopover(peek)
    peek.replaceChildren(
        h('div', { class: 'changes-head' }, [
            h('strong', { text: text('changesTitle') }),
            h('span', { text: String(entries.length) }),
        ]),
        h(
            'ul',
            { class: 'changes-list' },
            entries.map(({ change, turn }) => {
                const kind = KINDS[change.kind]
                return h('li', {}, [
                    h('span', { class: 'changes-icon' }, [
                        icon(kind?.[0] || change.icon || 'cube'),
                    ]),
                    h('span', { class: 'changes-copy' }, [
                        h('strong', {
                            text: changeTitle(change),
                        }),
                        h('span', {
                            text: [kind ? text(kind[1]) : '', whenLabel(turn.createdAt)]
                                .filter(Boolean)
                                .join(' · '),
                        }),
                    ]),
                    change.to
                        ? h('code', {
                              class: 'changes-diff',
                              text: change.from ? `${change.from} → ${change.to}` : change.to,
                          })
                        : null,
                ])
            }),
        ),
        h('div', { class: 'changes-foot' }, [
            button({ class: 'secondary-button small', data: { followup: text('undoAllPrompt') } }, [
                icon('refresh'),
                h('span', { text: text('changesUndo') }),
            ]),
        ]),
    )
    const rect = showPeek(peek, anchor)
    peek.style.top = `${Math.round(Math.max(12, rect.top - 8 - peek.offsetHeight))}px`
    // The list takes the focus rather than its undo button, so a second Enter
    // does not ask for an undo by accident.
    peek.focus()
}
