// The composer: what to do, where (instance) and with which model. While the
// agent works it turns into a stop button and queues follow-ups.
import { includedChanges, pendingPlan } from './agent.js'
import { button, element, h, icon } from './dom.js'
import { locale, sizeLabel, text } from './messages.js'
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

    const taskComputer = element('task-computer-use')
    taskComputer.hidden = !(session.permissions.full_access && session.permissions.computer_use)
    const taskControl = element('task-computer-use-input')
    taskControl.disabled = active || awaiting
    taskControl.checked =
        (active || awaiting
            ? (last?.run.nativeComputerUse ?? session.computerUseTasks.has(thread.id))
            : session.computerUseTasks.has(thread.id)) && !taskComputer.hidden

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
    const modelButton = element('model-button')
    modelButton.title = model ? `${model.provider.name} · ${model.model.name}` : text('model')
    // Named "Choose a model" plus what it shows, unless it shows just that.
    modelButton.setAttribute(
        'aria-labelledby',
        label === text('model') ? 'model-label' : 'model-button model-label',
    )
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
    renderChangesPill(thread, active || awaiting)
}

// Every change the task has written, with the request it came from.
export function appliedChanges(thread) {
    return thread.turns.flatMap((turn) =>
        turn.run.items
            .filter((item) => item.type === 'plan' && item.state === 'applied')
            .flatMap((item) => includedChanges(item).map((change) => ({ change, turn }))),
    )
}

// Once a task has written changes, a pill above the composer counts them and
// opens the list; it steps aside while the agent works or waits for approval.
function renderChangesPill(thread, busy) {
    const pill = element('changes-pill')
    // Only the latest turn's plans can still change. Older turns change only
    // when turns are added or dropped, which moves the count or the last id.
    const last = thread.turns.at(-1)
    const signature = [
        thread.id,
        thread.turns.length,
        last?.id,
        last?.run.items
            .filter((item) => item.type === 'plan')
            .map((item) => item.state)
            .join(),
        busy,
    ].join('|')
    if (pill.dataset.signature === signature) return
    pill.dataset.signature = signature
    const count = appliedChanges(thread).length
    pill.hidden = !count || busy
    if (!pill.hidden) pill.querySelector('.changes-count').textContent = String(count)
}

export function resizeInput() {
    const input = element('message-input')
    input.style.height = 'auto'
    const empty = !currentThread().turns.length
    input.style.height = `${Math.min(220, Math.max(empty ? 56 : 24, input.scrollHeight))}px`
    input.classList.toggle('is-scrolling', input.scrollHeight > 220)
}

export const PASTE_ICONS = { crash: 'alert', log: 'list', paste: 'file' }

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
                'aria-haspopup': 'dialog',
                'aria-expanded': 'false',
                'aria-controls': 'file-peek',
                data: { filePreview: file.id },
            },
            parts,
        ),
    ]
}

// The composer renders on every keystroke, so the chips and the queue are
// rebuilt only when what they show changes. Names, sizes and queued entries
// never change; a text file gains its kind and line count once it is read.
function renderAttachments(thread) {
    const list = element('attachment-list')
    const signature = `${locale()}|${thread.attachments
        .map((file) => `${file.id}:${file.kind || ''}:${file.lines ?? ''}`)
        .join()}`
    if (list.dataset.signature === signature) return
    list.dataset.signature = signature
    list.hidden = !thread.attachments.length
    list.replaceChildren(...thread.attachments.map((file) => fileChip(file, true)))
}

function renderQueue(thread, active) {
    const list = element('queue-list')
    const signature = `${locale()}|${active}|${thread.queue.map((entry) => entry.id).join()}`
    if (list.dataset.signature === signature) return
    list.dataset.signature = signature
    list.hidden = !thread.queue.length
    list.replaceChildren(
        ...thread.queue.map((entry) =>
            h('div', { class: 'queued' }, [
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
