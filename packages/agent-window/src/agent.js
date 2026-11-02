// The window talks to an agent runtime through a small event contract, so the
// interface is the same for the scripted preview and the future execution service.
//
// runtime.preview                 true only for the scripted preview; anything else
//                                 (false or left out) counts as native
// runtime.start(request, emit)   -> { stop() }   works on a new request
// runtime.approve(request, emit) -> { stop() }   writes the approved plan
// runtime.check(request, emit)   -> { stop() }   re-reads the outcome of interrupted writes
//
// Every request carries:
//   thread        { id, title }
//   turn          { id, text, attachments: [{ id, name, size, kind?, text?, lines? }] }
//   scope         'launcher' (the whole launcher), 'instance' (instance is set) or
//                 'missing' (the chosen instance is not in the context)
//   instance      the instance the task works in, or null unless scope is 'instance'
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
import { text } from './messages.js'
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

function applyReceipts(run, summary, terminal) {
    for (const item of run.items) {
        if (item.type !== 'plan') continue
        const actions = (summary.actions || []).filter((action) =>
            item.plan.changes.some((change) => change.name === action.call_id),
        )
        const selected = actions.filter((action) => !['pending', 'rejected'].includes(action.state))
        const applied = selected.filter((action) => action.state === 'applied').length
        const unknown = selected.filter((action) => action.state === 'unknown').length
        const failed = selected.filter((action) => action.state === 'failed').length
        item.receipts = {
            ...summary,
            actions,
            selected: selected.length,
            applied,
            unknown,
            failed,
            complete: selected.length > 0 && applied === selected.length,
        }
        item.done = applied
        if (selected.length && applied === selected.length) item.state = 'applied'
        else if (unknown) item.state = 'unknown'
        else if (actions.some((action) => action.state === 'pending')) continue
        else if (!selected.length && actions.some((action) => action.state === 'rejected'))
            item.state = 'declined'
        else if (terminal && item.state !== 'declined') item.state = 'stopped'
    }
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
        case 'error': {
            const writing = run.status === 'applying'
            closeActivity(run, now)
            for (const item of run.items) {
                if (item.type === 'activity')
                    for (const step of item.steps)
                        if (step.status === 'running') {
                            step.status = 'failed'
                            step.endedAt = now
                        }
                // A plan the failed run left waiting can no longer be approved.
                if (item.type === 'plan' && item.state === 'pending') item.state = 'stopped'
                if (item.type === 'plan' && item.state === 'applying') item.state = 'unknown'
            }
            run.items.push({
                id: id(),
                type: 'error',
                title: event.title || '',
                text: event.text || '',
                retryable: event.retryable !== false,
            })
            run.status = writing ? 'detached' : 'failed'
            run.endedAt = now
            run.label = ''
            break
        }
        case 'resume':
            run.resume = event.data ?? null
            break
        case 'done': {
            if (event.applied) applyReceipts(run, event.applied, true)
            closeActivity(run, now)
            const plan = run.items.findLast((item) => item.type === 'plan')
            if (run.status === 'applying') {
                if (plan && plan.state === 'applying') plan.state = 'unknown'
                run.status = 'done'
            } else if (run.status !== 'awaiting' && run.status !== 'failed') run.status = 'done'
            if (!run.preview && (plan?.state === 'unknown' || event.applied?.unknown > 0))
                run.status = 'detached'
            else if (event.applied?.failed > 0) run.status = 'failed'
            if (run.status !== 'awaiting') run.endedAt = now
            run.label = ''
            break
        }
    }
    return run
}

// Marks the plan as being written; the runtime then reports steps and results.
export function beginApply(run) {
    const plan = pendingPlan(run)
    if (!plan || !includedChanges(plan).length) return false
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

// Preview is an explicit development host capability, never a native fallback.
export async function createRuntime(host, context) {
    if (host?.runtimeInvoke) {
        if (!(host.runtime instanceof NativeRuntime))
            host.runtime = new NativeRuntime(host.runtimeInvoke)
        return host.runtime
    }
    if (host?.runtime && context.execution_available === true) return host.runtime
    if (host?.preview === true && !host?.native) {
        const { DemoRuntime } = await import('./demo-runtime.js')
        return new DemoRuntime(host.demo)
    }
    return {
        preview: false,
        start() {
            throw new Error(text('nativeUnavailable'))
        },
        approve() {
            throw new Error(text('nativeUnavailable'))
        },
        check() {
            throw new Error(text('nativeUnavailable'))
        },
    }
}

// Journal data supplies content; server snapshots supply the current lifecycle.
// This also covers interrupted workers that cannot publish a final journal event.
export function applyNativeSnapshot(run, state, receipts, now = Date.now()) {
    const plan = run.items.findLast((item) => item.type === 'plan')
    const terminal = ['completed', 'failed', 'cancelled', 'interrupted'].includes(state)
    const uncertain = receipts
        ? receipts.unknown > 0
        : !!plan && ['applying', 'unknown'].includes(plan.state)
    if (receipts) applyReceipts(run, receipts, terminal)
    if (state === 'awaiting') {
        if (plan) plan.state = 'pending'
        run.status = 'awaiting'
    } else if (state === 'queued' || state === 'running') {
        run.status =
            plan?.state === 'applying' || plan?.state === 'unknown' ? 'applying' : 'running'
        run.label = state === 'queued' ? text('queuedTag') : ''
    } else if (terminal) {
        closeActivity(run, now)
        for (const item of run.items) {
            if (item.type === 'activity')
                for (const step of item.steps) {
                    if (step.status === 'running') {
                        step.status = 'stopped'
                        step.endedAt = now
                    }
                }
        }
        if (plan && ['pending', 'applying'].includes(plan.state))
            plan.state = uncertain ? 'unknown' : 'stopped'
        run.status = uncertain
            ? 'detached'
            : state === 'completed' && receipts?.failed > 0
              ? 'failed'
              : {
                    completed: 'done',
                    failed: 'failed',
                    cancelled: 'stopped',
                    interrupted: 'interrupted',
                }[state]
        run.endedAt = now
        run.label = ''
    }
    if (!terminal) run.endedAt = null
    return terminal
}

export class NativeRuntime {
    preview = false
    constructor(invoke) {
        this.invoke = invoke
    }

    start(request, emit) {
        return this.submit(request, emit, false)
    }
    check(request, emit) {
        return this.submit(request, emit, true)
    }

    submit(request, emit, checkOnly) {
        if (request.scope === 'missing') throw new Error(text('contextRemoved'))
        const previous = request.resume?.verify_attempt || request.resume?.attempt_id
        const attemptId = request.attemptId || request.resume?.attempt_id || crypto.randomUUID()
        const resume = { attempt_id: attemptId, cursor: 0 }
        emit({ type: 'resume', data: resume })
        emit({ type: 'status', label: text('nativeConnecting') })
        const handle = this.follow(request.thread.id, attemptId, resume.cursor, emit, false)
        const selection = request.model
        Promise.resolve()
            .then(() =>
                this.invoke('submit', {
                    request: {
                        attempt_id: attemptId,
                        thread_id: request.thread.id,
                        turn_id: request.turn.id,
                        provider_id: selection?.provider?.provider_id,
                        model_id: selection?.model?.id,
                        text: request.turn.text,
                        attachments: request.turn.attachments,
                        instance_id: request.instance?.id || null,
                        instructions: request.instructions || '',
                        locale: document.documentElement.lang || 'en-US',
                        check_only: checkOnly,
                        computer_use: request.computerUse === true,
                        verify_attempt: checkOnly ? previous : null,
                    },
                }),
            )
            .then(() => handle.attach())
            .catch((error) => {
                handle.detach()
                emit({ type: 'error', text: String(error), retryable: true })
            })
        return handle
    }

    approve(request, emit) {
        const attemptId = request.resume?.attempt_id
        const handle = this.follow(
            request.thread.id,
            attemptId,
            request.resume?.cursor || 0,
            emit,
            false,
        )
        this.invoke('decide', {
            batchId: request.planId,
            revision: request.revision,
            selected: request.selected || [],
        })
            .then(() => handle.attach())
            .catch((error) => {
                emit({ type: 'decision-error', error: String(error) })
                handle.attach()
            })
        return handle
    }

    follow(threadId, attemptId, cursor, emit, attach = true) {
        let detached = !attach
        let timer
        let running = false
        const poll = async () => {
            if (detached || running) return
            running = true
            try {
                let snapshot = await this.invoke('snapshot', { threadId })
                if (detached) return
                let attempt = snapshot.attempts.find((entry) => entry.id === attemptId)
                let terminalJournal = false
                let events
                do {
                    events = await this.invoke('events', { threadId, after: cursor })
                    if (detached) return
                    for (const event of events) {
                        if (event.sequence <= cursor) continue
                        if (event.attempt_id === attemptId) {
                            if (event.payload.type === 'done' || event.payload.type === 'error')
                                terminalJournal = true
                            emit({
                                ...event.payload,
                                transient: true,
                                nativeResume: { attempt_id: attemptId, cursor: event.sequence },
                            })
                        }
                        cursor = event.sequence
                    }
                } while (!detached && events.length === 500)
                if (!detached && terminalJournal) {
                    snapshot = await this.invoke('snapshot', { threadId })
                    if (detached) return
                    attempt = snapshot.attempts.find((entry) => entry.id === attemptId)
                }
                if (!detached && attempt) {
                    const terminal = ['completed', 'failed', 'cancelled', 'interrupted'].includes(
                        attempt.state,
                    )
                    emit({
                        type: 'snapshot',
                        state: attempt.state,
                        computerUse: attempt.computer_use === true,
                        receipts: snapshot.results?.[attemptId],
                        terminal,
                        nativeResume: { attempt_id: attemptId, cursor },
                    })
                    if (terminal) detached = true
                }
            } catch (error) {
                if (!detached) emit({ type: 'connection', error: String(error) })
            } finally {
                running = false
                if (!detached) timer = setTimeout(poll, 700)
            }
        }
        const handle = {
            detach() {
                detached = true
                clearTimeout(timer)
            },
            attach() {
                detached = false
                void poll()
            },
            stop: async () => {
                await this.invoke('cancel', { attemptId })
                handle.detach()
                try {
                    const snapshot = await this.invoke('snapshot', { threadId })
                    const attempt = snapshot.attempts.find((entry) => entry.id === attemptId)
                    if (attempt)
                        emit({
                            type: 'snapshot',
                            state: attempt.state,
                            receipts: snapshot.results?.[attemptId],
                            terminal: ['completed', 'failed', 'cancelled', 'interrupted'].includes(
                                attempt.state,
                            ),
                            nativeResume: { attempt_id: attemptId, cursor },
                        })
                } catch (error) {
                    emit({ type: 'connection', error: String(error) })
                }
            },
        }
        if (attach) void poll()
        return handle
    }
}
