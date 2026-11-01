// The conversation: user requests on the right, the agent's work under each one.
// Turns are keyed, and a running turn is patched in place so streaming text,
// step spinners and expanded groups keep their state between events.
import { button, element, h, hue, icon, markdown, spinner } from './dom.js'
import { dateLabel, durationLabel, text } from './messages.js'
import { animate } from './motion.js'
import { instanceChoice } from './session.js'
import { ACTIVE_STATUSES } from './state.js'

const root = () => element('messages')

export function renderTranscript(thread) {
    root().replaceChildren(...thread.turns.map((turn) => createTurn(thread, turn, false)))
}

export function turnNode(turnId) {
    return root().querySelector(`[data-turn="${CSS.escape(turnId)}"]`)
}

function createTurn(thread, turn, fresh) {
    const node = h('article', { class: 'turn', data: { turn: turn.id } }, [
        userRow(turn),
        h('div', { class: 'agent' }, [
            h('div', { class: 'agent-items' }),
            h('div', { class: 'run-status', hidden: true }),
            h('div', { class: 'run-notice', hidden: true }),
            h('div', { class: 'turn-actions' }),
        ]),
    ])
    if (fresh) node.classList.add('is-arriving')
    syncTurn(thread, turn, node, false)
    return node
}

function userRow(turn) {
    const bubble = h('div', { class: 'user-bubble' }, [h('p', { text: turn.text })])
    if (turn.attachments.length)
        bubble.append(
            h(
                'div',
                { class: 'bubble-files' },
                turn.attachments.map((file) =>
                    h('span', { class: 'file-chip' }, [
                        icon('file'),
                        h('span', { text: file.name }),
                    ]),
                ),
            ),
        )
    return h('div', { class: 'user-row' }, [
        bubble,
        h('div', { class: 'user-meta' }, [
            h('time', { text: dateLabel(turn.createdAt) }),
            button(
                {
                    class: 'icon-button tiny',
                    label: text('copy'),
                    data: { copyTurn: turn.id, copyKind: 'user' },
                },
                icon('copy'),
            ),
        ]),
    ])
}

export function syncTurn(thread, turn, node = turnNode(turn.id), fresh = true) {
    if (!node) {
        node = createTurn(thread, turn, fresh)
        root().append(node)
        return node
    }
    const run = turn.run
    const items = node.querySelector('.agent-items')
    const active = ACTIVE_STATUSES.includes(run.status)
    const lastIndex = run.items.length - 1
    run.items.forEach((item, index) => {
        let child = items.querySelector(`:scope > [data-item="${CSS.escape(item.id)}"]`)
        if (!child) {
            child = createItem(item)
            if (fresh) child.classList.add('is-arriving')
            items.append(child)
        }
        updateItem(child, item, { thread, turn, active, last: index === lastIndex })
    })
    syncStatus(node.querySelector('.run-status'), turn, active)
    syncNotice(node.querySelector('.run-notice'), thread, turn)
    syncActions(node.querySelector('.turn-actions'), turn, active)
    node.dataset.status = run.status
    return node
}

function createItem(item) {
    const props = { data: { item: item.id } }
    switch (item.type) {
        case 'activity':
            return h('div', { ...props, class: 'activity' }, [
                button({ class: 'activity-summary', data: { action: 'toggle-activity' } }, [
                    h('span', { class: 'activity-icon' }),
                    h('span', { class: 'activity-label' }),
                    h('span', { class: 'activity-meta' }),
                    icon('right', 'activity-chevron'),
                ]),
                h('div', { class: 'activity-body' }, [
                    h('div', { class: 'activity-clip' }, [h('ol', { class: 'steps' })]),
                ]),
            ])
        case 'text':
            return h('div', { ...props, class: 'agent-text' })
        case 'plan':
            return h('section', { ...props, class: 'plan-card' })
        case 'result':
        case 'error':
            return h('div', {
                ...props,
                class: `result-card ${item.type === 'error' ? 'is-error' : 'is-success'}`,
            })
        default:
            return h('div', props)
    }
}

function updateItem(node, item, view) {
    if (item.type === 'activity') updateActivity(node, item, view)
    else if (item.type === 'text') updateText(node, item, view)
    else if (item.type === 'plan') updatePlan(node, item, view)
    else if (!node.childElementCount) fillResult(node, item, view)
    else if (item.type === 'error') syncRetry(node, item, view)
}

function stepIcon(status) {
    if (status === 'running') return spinner()
    return icon(status === 'done' ? 'check' : status === 'failed' ? 'close' : 'stop')
}

function updateActivity(node, item, { active }) {
    const running =
        item.steps.some((step) => step.status === 'running') || (!item.endedAt && active)
    const failed = item.steps.some((step) => step.status === 'failed')
    const stopped = item.steps.some((step) => step.status === 'stopped')
    const state = running ? 'running' : failed ? 'failed' : stopped ? 'stopped' : 'done'
    if (node.dataset.state !== state) {
        node.dataset.state = state
        const holder = node.querySelector('.activity-icon')
        holder.replaceChildren(
            running
                ? spinner()
                : icon(state === 'done' ? 'check-circle' : state === 'failed' ? 'alert' : 'stop'),
        )
        if (!running) holder.firstChild.classList.add('pop')
    }
    // The group stays open while it works and folds away when finished, unless
    // the user opened or closed it themselves.
    const open = node.dataset.user ? node.dataset.user === 'open' : running
    node.toggleAttribute('data-open', open)
    node.querySelector('.activity-summary').setAttribute('aria-expanded', String(open))
    const current = item.steps.findLast((step) => step.status === 'running')
    node.querySelector('.activity-label').textContent = running
        ? current?.title || text('working')
        : item.steps.length === 1
          ? item.steps[0].title
          : text('stepsDone', { count: item.steps.length })
    const end = item.endedAt || item.steps.at(-1)?.endedAt
    node.querySelector('.activity-meta').textContent =
        !running && item.startedAt && end ? durationLabel(end - item.startedAt) : ''

    const list = node.querySelector('.steps')
    for (const step of item.steps) {
        let row = list.querySelector(`:scope > [data-step="${CSS.escape(step.id)}"]`)
        if (!row) {
            row = h('li', { class: 'step is-arriving', data: { step: step.id } }, [
                h('span', { class: 'step-icon' }),
                h('span', { class: 'step-copy' }, [
                    h('span', { class: 'step-title' }),
                    h('span', { class: 'step-detail' }),
                ]),
                h('code', { class: 'step-tool' }),
            ])
            list.append(row)
        }
        if (row.dataset.state !== step.status) {
            row.querySelector('.step-icon').replaceChildren(stepIcon(step.status))
            if (row.dataset.state) row.querySelector('.step-icon').firstChild.classList.add('pop')
            row.dataset.state = step.status
        }
        row.querySelector('.step-title').textContent = step.title
        const detail = row.querySelector('.step-detail')
        detail.textContent = step.detail
        detail.hidden = !step.detail
        const tool = row.querySelector('.step-tool')
        tool.textContent = step.tool || ''
        tool.hidden = !step.tool
    }
}

function updateText(node, item, { active, last }) {
    if (node.dataset.length !== String(item.text.length)) {
        node.replaceChildren(markdown(item.text))
        node.dataset.length = String(item.text.length)
    }
    node.classList.toggle('is-streaming', active && last)
}

function changeRow(change) {
    const mark = change.glyph
        ? h('span', {
              class: 'change-mark glyph',
              text: change.glyph,
              style: `--mark-hue: ${hue(change.name)}`,
          })
        : h('span', { class: 'change-mark' }, [icon(change.icon || 'cube')])
    let after
    if (change.manual) after = h('span', { class: 'manual-tag', text: text('manualChange') })
    else
        after = h('span', { class: 'change-diff' }, [
            change.from ? h('span', { class: 'version is-from', text: change.from }) : null,
            change.from ? icon('arrow-right', 'diff-arrow') : null,
            change.to ? h('span', { class: 'version is-to', text: change.to }) : null,
        ])
    return h('li', { class: `change${change.manual ? ' is-manual' : ''}` }, [
        mark,
        h('span', { class: 'change-copy' }, [
            h('strong', { text: change.name }),
            change.note ? h('span', { text: change.note }) : null,
        ]),
        after,
    ])
}

function planFooter(item, turn) {
    const total = item.plan.changes.filter((change) => !change.manual).length
    const turnId = turn.id
    switch (item.state) {
        case 'pending':
            return h('div', { class: 'plan-actions' }, [
                h('p', { class: 'plan-promise' }, [icon('shield'), text('approvalPromise')]),
                h('div', { class: 'plan-buttons' }, [
                    button({
                        class: 'secondary-button',
                        text: text('decline'),
                        data: { action: 'decline', turn: turnId },
                    }),
                    button(
                        {
                            class: 'primary-button',
                            data: { action: 'approve', turn: turnId },
                            'data-testid': 'approve-plan',
                        },
                        [icon('check'), h('span', { text: text('approve') })],
                    ),
                ]),
            ])
        case 'applying': {
            const done = Math.min(item.done, total)
            return h('div', { class: 'plan-actions is-progress' }, [
                h('div', { class: 'plan-progress' }, [
                    h('span', {
                        class: 'plan-progress-bar',
                        style: `--progress: ${total ? done / total : 0}`,
                    }),
                ]),
                h('p', { class: 'plan-state' }, [
                    spinner(),
                    text('applyingProgress', { done, total }),
                ]),
            ])
        }
        case 'applied':
            return h('p', { class: 'plan-state is-applied' }, [
                icon('check-circle'),
                text('planApplied'),
            ])
        case 'declined':
            return h('p', { class: 'plan-state' }, [icon('close'), text('planDeclined')])
        case 'superseded':
            return h('p', { class: 'plan-state' }, [icon('refresh'), text('planSuperseded')])
        default:
            return h('p', { class: 'plan-state is-unknown' }, [
                icon('warning'),
                text('planUnknown'),
            ])
    }
}

function updatePlan(node, item, { turn }) {
    const signature = `${item.state}:${item.done}`
    if (node.dataset.signature === signature) return
    const first = !node.dataset.signature
    node.dataset.signature = signature
    node.dataset.state = item.state
    if (first) {
        const plan = item.plan
        node.append(
            h('header', { class: 'plan-header' }, [
                h('div', {}, [
                    h('span', { class: 'plan-kicker', text: text('planKicker') }),
                    h('h3', { text: plan.title }),
                ]),
                plan.summary ? h('span', { class: 'plan-count', text: plan.summary }) : null,
            ]),
            h('ul', { class: 'change-list' }, plan.changes.map(changeRow)),
            plan.notes.length
                ? h(
                      'ul',
                      { class: 'plan-notes' },
                      plan.notes.map((note) =>
                          h('li', {}, [icon('check'), h('span', { text: note })]),
                      ),
                  )
                : null,
            h('div', { class: 'plan-footer' }),
        )
    }
    const footer = node.querySelector('.plan-footer')
    footer.replaceChildren(planFooter(item, turn))
    if (!first)
        animate(footer.firstChild, [{ opacity: 0, transform: 'translateY(4px)' }, { opacity: 1 }])
}

function fillResult(node, item, view) {
    node.append(
        icon(item.type === 'error' ? 'alert' : 'check-circle', 'result-icon'),
        h('div', { class: 'result-copy' }, [
            h('strong', { text: item.title }),
            item.text ? h('p', { text: item.text }) : null,
            item.type === 'result' && view.turn.run.preview
                ? h('span', { class: 'preview-note', text: text('previewResult') })
                : null,
        ]),
    )
    if (item.type === 'error') syncRetry(node, item, view)
}

function syncRetry(node, item, { thread, turn, active }) {
    const show = item.retryable && !active && thread.turns.at(-1)?.id === turn.id
    let retry = node.querySelector('[data-action="retry"]')
    if (show && !retry) {
        retry = button({ class: 'secondary-button', data: { action: 'retry', turn: turn.id } }, [
            icon('refresh'),
            h('span', { text: text('retry') }),
        ])
        node.append(retry)
    } else if (!show) retry?.remove()
}

function syncStatus(node, turn, active) {
    node.hidden = !active
    if (!active) return
    const label = turn.run.label || text(turn.run.status === 'applying' ? 'applying' : 'thinking')
    if (!node.childElementCount)
        node.append(
            h('span', { class: 'thinking-mark' }, [icon('sparkle')]),
            h('span', { class: 'shimmer' }),
            h('span', { class: 'elapsed' }),
            h('span', { class: 'status-hint' }, [h('kbd', { text: 'Esc' }), text('escToStop')]),
        )
    node.querySelector('.shimmer').textContent = label
    updateElapsed(node, turn)
}

export function updateElapsed(node, turn) {
    const started = turn.run.phaseAt || turn.run.startedAt
    node.querySelector('.elapsed').textContent = started ? durationLabel(Date.now() - started) : ''
}

const NOTICES = {
    stopped: ['stop', 'noticeStopped', 'retry'],
    interrupted: ['clock', 'noticeInterrupted', 'retry'],
    detached: ['warning', 'noticeDetached', 'check-status'],
}

function syncNotice(node, thread, turn) {
    const notice = NOTICES[turn.run.status]
    const latest = thread.turns.at(-1)?.id === turn.id
    const signature = notice ? `${turn.run.status}:${latest}` : ''
    if (node.dataset.signature === signature) return
    node.dataset.signature = signature
    node.hidden = !notice
    if (!notice) {
        node.replaceChildren()
        return
    }
    const [name, key, action] = notice
    node.dataset.tone = turn.run.status
    node.replaceChildren(
        icon(name),
        h('span', { text: text(key) }),
        action && latest
            ? button({
                  class: 'text-button',
                  text: text(action === 'retry' ? 'retry' : 'checkStatus'),
                  data: { action, turn: turn.id },
              })
            : null,
    )
}

function syncActions(node, turn, active) {
    const hasText = turn.run.items.some((item) => item.type === 'text')
    const show = !active && hasText
    if (node.dataset.shown === String(show)) return
    node.dataset.shown = String(show)
    node.replaceChildren(
        ...(show
            ? [
                  button(
                      {
                          class: 'icon-button tiny',
                          label: text('copyAnswer'),
                          data: { copyTurn: turn.id, copyKind: 'agent' },
                      },
                      icon('copy'),
                  ),
              ]
            : []),
    )
}

export function toggleActivity(summary) {
    const node = summary.closest('.activity')
    const open = !node.hasAttribute('data-open')
    node.dataset.user = open ? 'open' : 'closed'
    node.toggleAttribute('data-open', open)
    summary.setAttribute('aria-expanded', String(open))
}

// The context a turn ran with, for the copy action and the progress panel.
export function turnInstanceName(turn) {
    return (
        instanceChoice(turn.instanceId)?.name ||
        (turn.instanceId ? text('contextRemoved') : text('allLauncher'))
    )
}
