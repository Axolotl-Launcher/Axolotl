// The composer: what to do, where (instance) and with which model. While the
// agent works it turns into a stop button and queues follow-ups.
import { pendingPlan } from './agent.js'
import { button, element, h, icon } from './dom.js'
import { sizeLabel, text } from './messages.js'
import {
    currentThread,
    instanceChoice,
    isPreview,
    modelChoice,
    modelsAvailable,
    session,
} from './session.js'
import { ACTIVE_STATUSES, lastTurn } from './state.js'

export function placeComposer() {
    const thread = currentThread()
    const home = thread.turns.length ? element('composer-dock') : element('composer-home')
    const wrap = element('composer-wrap')
    if (wrap.parentElement !== home) home.append(wrap)
    element('conversation').classList.toggle('is-empty', !thread.turns.length)
}

export function renderComposer() {
    const thread = currentThread()
    const last = lastTurn(thread)
    const active = !!last && ACTIVE_STATUSES.includes(last.run.status)
    const awaiting = !!pendingPlan(last?.run)

    const instance = instanceChoice(thread.instanceId)
    element('instance-label').textContent =
        instance?.name || (thread.instanceId ? text('contextRemoved') : text('allLauncher'))
    element('instance-button').classList.toggle('is-warning', !!thread.instanceId && !instance)

    const model = modelChoice(thread.selection)
    const ready = modelsAvailable()
    let label = model?.model.name
    if (!label)
        label = !session.contextLoaded
            ? text('noModel')
            : !ready
              ? text('setupModel')
              : thread.selection
                ? text('modelRemoved')
                : text('noModel')
    element('model-label').textContent = label
    element('model-button').title = model
        ? `${model.provider.name} · ${model.model.name}`
        : text('model')
    element('model-dot').hidden = !session.contextLoaded || (!!model && ready)

    const input = element('message-input')
    if (input.value !== thread.draft) input.value = thread.draft
    input.placeholder = text(
        !thread.turns.length
            ? 'prompt'
            : active
              ? 'promptQueued'
              : awaiting
                ? 'promptAwaiting'
                : 'promptFollowup',
    )
    resizeInput()

    const send = element('send-button')
    const stopMode = active && !thread.draft.trim() && !thread.attachments.length
    send.classList.toggle('is-stop', stopMode)
    send.dataset.action = stopMode ? 'stop' : ''
    send.type = stopMode ? 'button' : 'submit'
    const sendLabel = text(stopMode ? 'stop' : active ? 'queueMessage' : 'send')
    send.setAttribute('aria-label', sendLabel)
    send.dataset.tip = sendLabel
    send.dataset.keys = stopMode ? 'Esc' : 'Enter'
    send.disabled = !stopMode && !thread.draft.trim() && !thread.attachments.length

    renderAttachments(thread)
    renderQueue(thread, active)
    renderNote(thread)
}

export function resizeInput() {
    const input = element('message-input')
    input.style.height = 'auto'
    const empty = !currentThread().turns.length
    input.style.height = `${Math.min(220, Math.max(empty ? 56 : 24, input.scrollHeight))}px`
    input.classList.toggle('is-scrolling', input.scrollHeight > 220)
}

const PASTE_ICONS = { crash: 'alert', log: 'list', paste: 'file' }

// One attached file or long paste. A paste shows its line count and, on
// hover, how it starts.
export function fileChip(file, removable = false) {
    const pasted = !!file.kind
    return h(
        'span',
        {
            class: `file-chip${removable ? ' is-removable' : ''}`,
            data: pasted ? { kind: file.kind } : {},
        },
        [
            ...chipLabel(file, pasted),
            removable
                ? button(
                      {
                          class: 'file-remove',
                          label: text('removeAttachment', { name: file.name }),
                          data: { removeAttachment: file.id },
                      },
                      icon('close'),
                  )
                : null,
        ],
    )
}

// A paste opens a preview of its text; a file only shows its name and size.
function chipLabel(file, pasted) {
    const parts = [
        icon(PASTE_ICONS[file.kind] || 'file'),
        h('span', { class: 'file-name', text: file.name }),
        h('span', {
            class: 'file-size',
            text: pasted ? text('lineCount', { count: file.lines }) : sizeLabel(file.size),
        }),
    ]
    if (!pasted) return parts
    return [
        button(
            {
                class: 'file-open',
                label: text('previewFile', { name: file.name }),
                data: { filePreview: file.id },
            },
            parts,
        ),
    ]
}

function renderAttachments(thread) {
    const list = element('attachment-list')
    list.hidden = !thread.attachments.length
    list.replaceChildren(...thread.attachments.map((file) => fileChip(file, true)))
}

function renderQueue(thread, active) {
    const list = element('queue-list')
    list.hidden = !thread.queue.length
    list.replaceChildren(
        ...thread.queue.map((entry) =>
            h('div', { class: 'queued', data: { queued: entry.id } }, [
                icon('clock'),
                h('span', { class: 'queued-text', text: entry.text }),
                active
                    ? h('span', { class: 'queued-tag', text: text('queuedTag') })
                    : button({
                          class: 'text-button',
                          text: text('sendNow'),
                          data: { sendQueued: entry.id },
                      }),
                button(
                    {
                        class: 'icon-button tiny',
                        label: text('removeQueued'),
                        data: { removeQueued: entry.id },
                    },
                    icon('close'),
                ),
            ]),
        ),
    )
}

function renderNote(thread) {
    const note = element('composer-note')
    const show = !thread.turns.length && isPreview()
    note.hidden = !show
    if (show) note.replaceChildren(icon('info'), h('span', { text: text('previewNote') }))
}

// A compact approval prompt above the composer, shown while the plan card
// itself is scrolled out of view.
export function renderApprovalBar(visible) {
    const thread = currentThread()
    const last = lastTurn(thread)
    const plan = pendingPlan(last?.run)
    const bar = element('approval-bar')
    const show = !!plan && visible
    if (show && bar.dataset.turn !== last.id) {
        bar.dataset.turn = last.id
        bar.replaceChildren(
            button({ class: 'approval-copy', data: { action: 'review-plan', turn: last.id } }, [
                icon('shield'),
                h('span', {}, [
                    h('strong', { text: text('approvalWaiting') }),
                    h('span', { text: plan.plan.approval || plan.plan.title }),
                ]),
            ]),
            h('div', { class: 'approval-buttons' }, [
                button({
                    class: 'secondary-button small',
                    text: text('decline'),
                    data: { action: 'decline', turn: last.id },
                }),
                button({
                    class: 'primary-button small',
                    text: text('approve'),
                    data: { action: 'approve', turn: last.id },
                }),
            ]),
        )
    }
    if (!show) delete bar.dataset.turn
    bar.hidden = !show
}
