// Popovers, dialogs and toasts.
import { button, element, h, icon, logLines } from './dom.js'
import { text } from './messages.js'
import { currentThread, modelsAvailable, session } from './session.js'

// ---------- pickers ----------

let pickerKind = null

export function pickerOpen() {
    return element('picker').matches(':popover-open')
}

export function openPicker(kind, anchor) {
    const picker = element('picker')
    if (pickerOpen()) picker.hidePopover()
    pickerKind = kind
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
    picker.showPopover()
    search.focus()
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
            for (const provider of session.context.providers) {
                const models = provider.models.filter((model) =>
                    `${provider.name} ${model.name} ${model.id}`
                        .toLocaleLowerCase()
                        .includes(query),
                )
                if (!models.length) continue
                options.push(h('div', { class: 'picker-group', text: provider.name }))
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
            }
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
            options.push(h('div', { class: 'picker-group', text: text('instances') }))
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
    element('picker-options').replaceChildren(...options)
}

export function closePicker() {
    if (pickerOpen()) element('picker').hidePopover()
}

// Arrow keys move between options, as in a native list.
export function pickerKeydown(event) {
    if (!['ArrowDown', 'ArrowUp'].includes(event.key)) return
    const options = [...element('picker-options').querySelectorAll('.picker-option')]
    if (!options.length) return
    event.preventDefault()
    const index = options.indexOf(document.activeElement)
    const next =
        event.key === 'ArrowDown'
            ? options[(index + 1) % options.length]
            : options[(index - 1 + options.length) % options.length]
    next.focus()
}

// ---------- menus ----------

export function openMenu(anchor, items, align = 'end') {
    const menu = element('menu')
    if (menu.matches(':popover-open')) menu.hidePopover()
    menu.replaceChildren(...items)
    const rect = anchor.getBoundingClientRect()
    menu.style.minWidth = '200px'
    menu.style.top = `${rect.bottom + 6}px`
    menu.style.left = align === 'end' ? 'auto' : `${rect.left}px`
    menu.style.right =
        align === 'end' ? `${Math.max(12, window.innerWidth - rect.right)}px` : 'auto'
    menu.style.transformOrigin = `${align === 'end' ? 'right' : 'left'} top`
    menu.showPopover()
    menu.querySelector('button')?.focus()
}

export function menuItem(name, label, data, danger = false) {
    return button({ class: `menu-item${danger ? ' is-danger' : ''}`, role: 'menuitem', data }, [
        icon(name),
        h('span', { text: label }),
    ])
}

export function closeMenu() {
    const menu = element('menu')
    if (menu.matches(':popover-open')) menu.hidePopover()
}

// ---------- dialogs ----------

export function openDialog(id) {
    const dialog = element(id)
    const error = dialog.querySelector('[data-dialog-error]')
    if (error) {
        error.textContent = ''
        error.hidden = true
    }
    if (!dialog.open) dialog.showModal()
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
    const available =
        typeof window.AgentHost?.getPermissions === 'function' &&
        typeof window.AgentHost?.setPermissions === 'function'
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
    for (const choice of element('detail-choice').querySelectorAll('[data-detail]'))
        choice.setAttribute('aria-checked', String(choice.dataset.detail === session.store.detail))
    const instructions = element('instructions-input')
    if (document.activeElement !== instructions) instructions.value = session.store.instructions
    element('settings-dialog')
        .querySelectorAll('[data-action="close-dialog"]')
        .forEach((close) => {
            close.disabled = session.permissionsSaving
        })
}

// ---------- toasts ----------

export function toast(message, action = null, tone = 'info') {
    const holder = element('toasts')
    const node = h('div', { class: 'toast', data: { tone } }, [
        icon(tone === 'error' ? 'alert' : tone === 'success' ? 'check-circle' : 'info'),
        h('span', { text: message }),
    ])
    if (action) node.append(button({ class: 'text-button', text: action.label }, []))
    if (action)
        node.lastChild.addEventListener('click', () => {
            action.run()
            dismiss()
        })
    const dismiss = () => {
        if (!node.isConnected || node.dataset.leaving) return
        node.dataset.leaving = 'true'
        node.addEventListener('animationend', () => node.remove(), { once: true })
        setTimeout(() => node.remove(), 400)
    }
    holder.append(node)
    while (holder.childElementCount > 3) holder.firstElementChild.remove()
    setTimeout(dismiss, action ? 7000 : 4500)
    return node
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

export function hideTip() {
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
    document.addEventListener('pointerover', (event) => {
        if (event.pointerType === 'touch') return
        queueTip(event.target.closest?.('[data-tip]') || null, 520)
    })
    // Focus shows a tooltip only when the keyboard moved it, not when a click
    // opened something that focused its first button.
    let keyboard = false
    document.addEventListener('pointerdown', () => (keyboard = false), true)
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
    if (peek.matches(':popover-open')) peek.hidePopover()
    const lines = file.text.split('\n')
    peek.replaceChildren(
        h('div', { class: 'file-peek-head' }, [
            icon(file.kind === 'crash' ? 'alert' : file.kind === 'log' ? 'list' : 'file'),
            h('strong', { text: file.name }),
            h('span', { text: text('lineCount', { count: file.lines }) }),
            button(
                {
                    class: 'icon-button tiny',
                    label: text('copy'),
                    data: { copyPaste: file.id },
                },
                icon('copy'),
            ),
        ]),
        h('pre', {}, [
            h('code', { class: 'is-log' }, logLines(lines.slice(0, PEEK_LINES).join('\n'))),
        ]),
        lines.length > PEEK_LINES
            ? h('p', {
                  class: 'file-peek-more',
                  text: text('moreLines', { count: lines.length - PEEK_LINES }),
              })
            : null,
    )
    peek.showPopover()
    const rect = anchor.getBoundingClientRect()
    const above = rect.top - 12 > peek.offsetHeight
    peek.style.left = `${Math.round(Math.max(12, Math.min(rect.left, window.innerWidth - peek.offsetWidth - 12)))}px`
    peek.style.top = `${Math.round(above ? rect.top - 8 - peek.offsetHeight : rect.bottom + 8)}px`
    peek.dataset.side = above ? 'above' : 'below'
}
