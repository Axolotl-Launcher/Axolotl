// The Progress panel: the task's checklist, what it would change, what it runs
// with and every tool call so far. It only describes the latest request.
import { activitySteps, pendingPlan } from './agent.js'
import { element, h, icon, spinner } from './dom.js'
import { dateLabel, text } from './messages.js'
import { currentThread, instanceChoice, modelChoice, session } from './session.js'
import { lastTurn } from './state.js'

const STATE_KEYS = {
    thinking: 'stateWorking',
    running: 'stateWorking',
    applying: 'stateApplying',
    awaiting: 'stateAwaiting',
    done: 'stateDone',
    failed: 'stateFailed',
    stopped: 'stateStopped',
    detached: 'stateDetached',
    declined: 'stateDeclined',
    superseded: 'stateSuperseded',
    interrupted: 'stateInterrupted',
}

export function stateLabel(status) {
    return text(STATE_KEYS[status] || 'stateDone')
}

function section(title, children) {
    return h('section', { class: 'panel-section' }, [h('h3', { text: title }), ...children])
}

function checklistIcon(status) {
    if (status === 'current') return spinner()
    if (status === 'done') return icon('check')
    if (status === 'failed') return icon('close')
    return h('span', { class: 'check-empty' })
}

export function progressFraction(run) {
    if (!run?.progress.length) return null
    return run.progress.filter((step) => step.status === 'done').length / run.progress.length
}

export function renderPanel() {
    const thread = currentThread()
    const turn = lastTurn(thread)
    const content = element('task-content')
    if (!turn) {
        content.replaceChildren(
            h('div', { class: 'panel-empty' }, [
                icon('list'),
                h('p', { text: text('panelEmpty') }),
            ]),
        )
        return
    }
    const run = turn.run
    const plan = run.items.findLast((item) => item.type === 'plan')
    const parts = [
        h('div', { class: 'panel-summary', data: { status: run.status } }, [
            h('span', {
                class: 'state-pill',
                data: { status: run.status },
                text: stateLabel(run.status),
            }),
            h('p', { text: thread.title || turn.text }),
        ]),
    ]
    if (run.progress.length)
        parts.push(
            section(text('panelSteps'), [
                h(
                    'ol',
                    { class: 'checklist' },
                    run.progress.map((step) =>
                        h('li', { data: { state: step.status } }, [
                            h('span', { class: 'checklist-icon' }, [checklistIcon(step.status)]),
                            h('span', { class: 'checklist-copy' }, [
                                h('strong', { text: step.title }),
                                step.detail ? h('span', { text: step.detail }) : null,
                            ]),
                        ]),
                    ),
                ),
            ]),
        )
    if (plan?.plan.scope.length)
        parts.push(
            section(text('panelChanges'), [
                h(
                    'ul',
                    { class: 'scope-list' },
                    plan.plan.scope.map((entry) =>
                        h('li', { class: entry.count ? '' : 'is-zero' }, [
                            h('span', { text: entry.label }),
                            h('span', { class: 'scope-count', text: String(entry.count) }),
                        ]),
                    ),
                ),
                pendingPlan(run)
                    ? h('p', { class: 'panel-hint' }, [icon('shield'), text('approvalPromise')])
                    : null,
            ]),
        )
    const instance = instanceChoice(turn.instanceId)
    const model = modelChoice(turn.selection)
    parts.push(
        section(text('context'), [
            h('dl', { class: 'context-list' }, [
                h('dt', { text: text('instanceLabel') }),
                h('dd', {}, [
                    h('span', {
                        text:
                            instance?.name ||
                            (turn.instanceId ? text('contextRemoved') : text('allLauncher')),
                    }),
                    instance
                        ? h('small', {
                              text: [instance.loader, instance.game_version]
                                  .filter(Boolean)
                                  .join(' · '),
                          })
                        : null,
                ]),
                h('dt', { text: text('modelLabel') }),
                h('dd', {}, [
                    h('span', {
                        text:
                            model?.model.name ||
                            (turn.selection ? text('modelRemoved') : text('unselected')),
                    }),
                    model ? h('small', { text: model.provider.name }) : null,
                ]),
                h('dt', { text: text('permissionLabel') }),
                h('dd', {}, [
                    h('span', {
                        text: text(session.permissions.full_access ? 'fullAccess' : 'askFirst'),
                    }),
                ]),
            ]),
        ]),
    )
    const steps = activitySteps(run)
    if (steps.length)
        parts.push(
            section(text('panelLog'), [
                h(
                    'ol',
                    { class: 'log-list' },
                    steps.map((step) =>
                        h('li', { data: { state: step.status } }, [
                            h('time', { text: step.startedAt ? dateLabel(step.startedAt) : '' }),
                            h('span', { text: step.title }),
                            step.tool ? h('code', { text: step.tool }) : null,
                        ]),
                    ),
                ),
            ]),
        )
    content.replaceChildren(...parts)
}

// The ring on the panel button shows checklist progress at a glance.
export function renderProgressRing() {
    const run = lastTurn(currentThread())?.run
    const fraction = progressFraction(run)
    const toggle = element('task-toggle')
    toggle.classList.toggle('has-progress', fraction !== null)
    toggle.dataset.status = run?.status || ''
    element('progress-ring-value').style.setProperty('--progress', String(fraction ?? 0))
}
