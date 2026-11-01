// The window talks to an agent runtime through a small event contract, so the
// interface is the same for the scripted preview and the future execution service.
//
// runtime.preview                 true only for the scripted preview
// runtime.start(request, emit)   -> { stop() }   works on a new request
// runtime.approve(request, emit) -> { stop() }   writes the approved plan
// runtime.check(request, emit)   -> { stop() }   re-reads the outcome of interrupted writes
//
// Every request carries:
//   thread        { id, title }
//   turn          { id, text, attachments: [{ id, name, size, kind?, text?, lines? }] }
//   instance      the instance the task works in, or null for the whole launcher
//   instances     every instance the launcher offers
//   model         { provider, model } for the turn, or null
//   history       the thread's earlier turns, as stored
//   resume        the turn's last `resume` data, or null
//   instructions  standing preferences from Settings
// approve adds planId and excluded, the names of the changes the user took out.
//
// emit() receives these events, in any order a run needs:
//   { type: 'status', label }                 what the agent is doing right now
//   { type: 'title', title }                  a better task title (kept unless renamed)
//   { type: 'step', step, status }            one tool call { id, title, detail, tool }:
//                                             running | done | failed
//   { type: 'text', delta }                   streamed answer text
//   { type: 'progress', steps }               the task checklist: [{ title, detail, status }]
//   { type: 'plan', plan }                    changes that need the user's approval:
//                                             { title, summary, approval, notes, scope,
//                                             changes: [{ name, note, from, to, kind, glyph,
//                                             icon, meta, manual }] }
//   { type: 'applied', done, complete }       how many plan changes are written; complete
//                                             marks the plan written
//   { type: 'result', title, text, next }     the outcome shown at the end of the run; next
//                                             offers up to three follow-up requests
//   { type: 'error', title, text, retryable } the run failed
//   { type: 'resume', data }                  opaque data the runtime needs after a restart
//   { type: 'done' }                          the runtime has nothing more to do
import { ACTIVE_STATUSES } from './state.js'

const id = () => crypto.randomUUID()

function currentActivity(run) {
    const last = run.items.at(-1)
    return last?.type === 'activity' && !last.endedAt ? last : null
}

function closeActivity(run, now) {
    const activity = currentActivity(run)
    if (activity) activity.endedAt = now
}

export function applyEvent(run, event, now = Date.now()) {
    switch (event.type) {
        case 'status':
            run.label = String(event.label || '')
            if (run.status === 'thinking') run.status = 'running'
            break
        case 'step': {
            if (run.status === 'thinking') run.status = 'running'
            let activity = run.items.findLast(
                (item) =>
                    item.type === 'activity' &&
                    item.steps.some((step) => step.id === event.step.id),
            )
            if (!activity) {
                activity = currentActivity(run)
                if (!activity) {
                    activity = {
                        id: id(),
                        type: 'activity',
                        steps: [],
                        startedAt: now,
                        endedAt: null,
                    }
                    run.items.push(activity)
                }
            }
            let step = activity.steps.find((entry) => entry.id === event.step.id)
            if (!step) {
                step = {
                    id: event.step.id,
                    title: '',
                    detail: '',
                    tool: null,
                    status: 'running',
                    startedAt: now,
                    endedAt: null,
                }
                activity.steps.push(step)
            }
            Object.assign(step, {
                title: event.step.title ?? step.title,
                detail: event.step.detail ?? step.detail,
                tool: event.step.tool ?? step.tool,
                status: event.status || 'done',
            })
            if (step.status !== 'running') step.endedAt = now
            break
        }
        case 'text': {
            if (run.status === 'thinking') run.status = 'running'
            closeActivity(run, now)
            const last = run.items.at(-1)
            if (last?.type === 'text') last.text += event.delta
            else run.items.push({ id: id(), type: 'text', text: String(event.delta) })
            break
        }
        case 'progress':
            run.progress = event.steps.map((step) => ({
                title: step.title,
                detail: step.detail || '',
                status: step.status || 'pending',
            }))
            break
        case 'plan':
            closeActivity(run, now)
            run.items.push({
                id: id(),
                type: 'plan',
                plan: { id: id(), notes: [], scope: [], ...event.plan },
                state: 'pending',
                done: 0,
                excluded: [],
            })
            run.status = 'awaiting'
            run.label = ''
            break
        case 'applied': {
            const plan = run.items.findLast((item) => item.type === 'plan')
            if (plan) {
                plan.done = event.done
                if (event.complete) plan.state = 'applied'
            }
            break
        }
        case 'result':
            closeActivity(run, now)
            run.items.push({
                id: id(),
                type: 'result',
                title: event.title || '',
                text: event.text || '',
                retryable: false,
                next: Array.isArray(event.next)
                    ? event.next.filter((entry) => typeof entry === 'string' && entry).slice(0, 3)
                    : [],
            })
            break
        case 'error':
            closeActivity(run, now)
            for (const activity of run.items)
                if (activity.type === 'activity')
                    for (const step of activity.steps)
                        if (step.status === 'running') {
                            step.status = 'failed'
                            step.endedAt = now
                        }
            run.items.push({
                id: id(),
                type: 'error',
                title: event.title || '',
                text: event.text || '',
                retryable: event.retryable !== false,
            })
            run.status = 'failed'
            run.endedAt = now
            run.label = ''
            break
        case 'resume':
            run.resume = event.data ?? null
            break
        case 'done': {
            closeActivity(run, now)
            const plan = run.items.findLast((item) => item.type === 'plan')
            if (run.status === 'applying') {
                if (plan) plan.state = 'applied'
                run.status = 'done'
            } else if (run.status !== 'awaiting' && run.status !== 'failed') run.status = 'done'
            if (run.status !== 'awaiting') run.endedAt = now
            run.label = ''
            break
        }
    }
    return run
}

// Marks the plan as being written; the runtime then reports steps and results.
export function beginApply(run) {
    const plan = run.items.findLast((item) => item.type === 'plan')
    if (!plan || plan.state !== 'pending' || !includedChanges(plan).length) return false
    plan.state = 'applying'
    run.status = 'applying'
    run.endedAt = null
    run.phaseAt = Date.now()
    return true
}

// A stopped run that was only reading changed nothing. Stopping while writing
// leaves the outcome unknown until it is checked again.
export function stopRun(run, now = Date.now()) {
    if (!ACTIVE_STATUSES.includes(run.status)) return false
    const writing = run.status === 'applying'
    for (const item of run.items) {
        if (item.type === 'activity') {
            for (const step of item.steps)
                if (step.status === 'running') {
                    step.status = 'stopped'
                    step.endedAt = now
                }
            item.endedAt ||= now
        }
        if (item.type === 'plan' && item.state === 'applying') item.state = 'unknown'
    }
    for (const step of run.progress) if (step.status === 'current') step.status = 'pending'
    run.status = writing ? 'detached' : 'stopped'
    run.endedAt = now
    run.label = ''
    return true
}

export function declinePlan(run, now = Date.now()) {
    const plan = run.items.findLast((item) => item.type === 'plan')
    if (run.status !== 'awaiting' || !plan || plan.state !== 'pending') return false
    plan.state = 'declined'
    run.status = 'declined'
    run.endedAt = now
    return true
}

export function supersedePlan(run, now = Date.now()) {
    const plan = run.items.findLast((item) => item.type === 'plan')
    if (run.status !== 'awaiting' || !plan || plan.state !== 'pending') return false
    plan.state = 'superseded'
    run.status = 'superseded'
    run.endedAt = now
    return true
}

export function pendingPlan(run) {
    if (run?.status !== 'awaiting') return null
    const plan = run.items.findLast((item) => item.type === 'plan')
    return plan?.state === 'pending' ? plan : null
}

// Takes one change out of a waiting plan, or puts it back. Manual changes are
// advice the agent cannot carry out, so they are never part of the choice.
export function toggleChange(run, index) {
    const plan = pendingPlan(run)
    const change = plan?.plan.changes[index]
    if (!change || change.manual) return false
    plan.excluded = plan.excluded.includes(index)
        ? plan.excluded.filter((entry) => entry !== index)
        : [...plan.excluded, index].sort((a, b) => a - b)
    return true
}

// The changes approving would write: not manual and not excluded.
export function includedChanges(plan) {
    return plan.plan.changes.filter(
        (change, index) => !change.manual && !(plan.excluded || []).includes(index),
    )
}

export function activitySteps(run) {
    return run.items.flatMap((item) => (item.type === 'activity' ? item.steps : []))
}

// Uses the launcher's execution service when it is connected; otherwise the
// scripted preview runs, and the window labels every result as simulated.
export async function createRuntime(host, context) {
    if (host?.runtime && context.execution_available === true) return host.runtime
    const { DemoRuntime } = await import('./demo-runtime.js')
    return new DemoRuntime(host?.demo)
}
