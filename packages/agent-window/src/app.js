import {
    activitySteps,
    applyEvent,
    beginApply,
    createRuntime,
    declinePlan,
    pendingPlan,
    stopRun,
    supersedePlan,
    toggleChange,
} from './agent.js'
import {
    button,
    element,
    focusable,
    focusPlace,
    guardFocus,
    h,
    icon,
    plainText,
    restoreFocus,
} from './dom.js'
import { setLocale, text } from './messages.js'
import { animate, morph, reducedMotion, ScrollFollower } from './motion.js'
import {
    currentThread,
    findThread,
    findTurn,
    firstModel,
    instanceChoice,
    isPreview,
    modelChoice,
    modelsAvailable,
    permissionsAvailable,
    session,
} from './session.js'
import {
    ACTIVE_STATUSES,
    activeThread,
    initialStore,
    lastTurn,
    LEGACY_STORAGE_KEY,
    loadStore,
    MAX_ATTACHMENTS,
    MAX_INSTRUCTIONS,
    MAX_PASTE,
    newRun,
    newThread,
    newTurn,
    saveStore,
    STORAGE_KEY,
    threadStatus,
} from './state.js'
import {
    appliedChanges,
    placeComposer,
    renderApprovalBar,
    renderComposer,
    resizeInput,
} from './view-composer.js'
import {
    closeTour,
    heroListens,
    homeSettled,
    openTour,
    refreshTour,
    renderHelp,
    renderHome,
    tourOpen,
} from './view-home.js'
import {
    acceptConfirm,
    announce,
    bindOverlays,
    bindTips,
    closeMenu,
    closePicker,
    closePopover,
    confirm,
    confirmBusy,
    dialogError,
    focusPickerOption,
    menuItem,
    openChanges,
    openDialog,
    openFilePeek,
    openMenu,
    openPicker,
    pickerKeydown,
    pickerOpen,
    renderPicker,
    renderSettings,
    toast,
} from './view-overlays.js'
import { bindPalette, openPalette } from './view-palette.js'
import { renderPanel, renderProgressRing, stateLabel } from './view-panel.js'
import { bindRail, renderRail } from './view-rail.js'
import { progressOf, renderSidebar } from './view-sidebar.js'
import { bindSlash, closeSlash, slashKeydown, syncSlash } from './view-slash.js'
import {
    bindTranscript,
    patchTranscript,
    renderTranscript,
    showAllSteps,
    syncTurn,
    toggleActivity,
    turnNode,
    updateElapsed,
} from './view-transcript.js'

const NARROW = 760
// Below this width the progress panel floats over the conversation.
const WIDE = 1100
// Pastes this long are attached instead of typed into the request.
const PASTE_LINES = 16
const PASTE_CHARS = 2500

try {
    session.store = loadStore()
} catch {
    session.store = initialStore()
    session.loadError = true
}

let renderedThread = null
let disposed = false
let booted = false
let persistTimer = 0
let contextRequest = 0
let contextFlight = null
let permissionRequest = 0
let ticker = 0
const follower = new ScrollFollower(element('message-scroll'), () => renderJump())

// The thread or turn with this id, from a store or the copy a commit changes.
const byId = (list, id) => list.find((entry) => entry.id === id)

// ---------- persistence ----------

// User actions are all-or-nothing: if history cannot be saved, nothing changes
// and the draft stays in the composer.
function commit(change) {
    const next = structuredClone(session.store)
    change(next, activeThread(next))
    try {
        if (session.loadError) throw new Error('Saved history requires recovery')
        session.store = saveStore(next)
        clearTimeout(persistTimer)
        persistTimer = 0
        if (session.storageError) {
            session.storageError = false
            renderBanners()
        }
        return true
    } catch {
        session.storageError = true
        renderBanners()
        notify(text('storageError'), 'error')
        return false
    }
}

// Agent events must never be lost because storage is full, so they change the
// live store and are saved shortly after.
function persistSoon(delay = 400) {
    if (!persistTimer) persistTimer = setTimeout(persistNow, delay)
}

function persistNow() {
    clearTimeout(persistTimer)
    persistTimer = 0
    if (session.loadError) return
    try {
        session.store = saveStore(session.store)
        if (session.storageError) {
            session.storageError = false
            renderBanners()
        }
    } catch {
        if (!session.storageError) {
            session.storageError = true
            renderBanners()
            notify(text('storageError'), 'error')
        }
    }
}

// ---------- rendering ----------

function catalogReady() {
    const messages = window.AgentHost?.appearance?.messages
    return (
        messages &&
        typeof messages === 'object' &&
        !Array.isArray(messages) &&
        Object.keys(messages).length > 0
    )
}

function showCatalogError() {
    const error = element('window-error')
    error.dataset.catalogError = 'true'
    error.textContent =
        'Agent window message catalog is unavailable. Reopen the window from the launcher.'
    error.hidden = false
}

function localize() {
    setLocale(
        window.AgentHost?.appearance?.locale || document.documentElement.lang,
        window.AgentHost?.appearance?.messages,
    )
    document.querySelectorAll('[data-text]').forEach((node) => {
        node.textContent = text(node.dataset.text)
    })
    document.querySelectorAll('[data-placeholder]').forEach((node) => {
        node.placeholder = text(node.dataset.placeholder)
    })
    document.querySelectorAll('[data-label]').forEach((node) => {
        node.setAttribute('aria-label', text(node.dataset.label))
        if (node.matches('button')) node.dataset.tip = text(node.dataset.label)
    })
}

function narrow() {
    return window.innerWidth < NARROW
}

// The task list and the progress panel keep their saved open state on wide
// windows. On small windows they become drawers over the conversation, open one
// at a time and are never saved.
let drawer = null

function sidebarShown() {
    return narrow() ? drawer === 'sidebar' : session.store.sidebarOpen
}

function panelShown() {
    return window.innerWidth < WIDE ? drawer === 'panel' : session.store.panelOpen
}

function layout() {
    const workspace = element('workspace')
    const sidebar = sidebarShown()
    const panel = panelShown()
    workspace.classList.toggle('sidebar-collapsed', !sidebar)
    workspace.classList.toggle('tasks-open', panel)
    workspace.classList.toggle('drawer-open', drawer !== null)
    element('sidebar-toggle').setAttribute('aria-expanded', String(sidebar))
    element('task-toggle').setAttribute('aria-expanded', String(panel))
    element('task-toggle').classList.toggle('is-active', panel)
    // A pane that closes around the focus hands it to its toggle first, so
    // the focus never falls behind an inert pane.
    for (const [pane, shown, toggle] of [
        ['sidebar', sidebar, 'sidebar-toggle'],
        ['task-panel', panel, 'task-toggle'],
    ])
        if (!shown && element(pane).contains(document.activeElement)) element(toggle).focus()
    element('sidebar').inert = !sidebar
    element('task-panel').inert = !panel
}

function renderHeader() {
    const thread = currentThread()
    const last = lastTurn(thread)
    const title = element('conversation-title')
    title.textContent = thread.title || text('untitled')
    element('conversation-more').hidden = !thread.turns.length && !thread.draft
    const state = element('thread-state')
    const status = last?.run.status
    const show =
        !!status &&
        (ACTIVE_STATUSES.includes(status) || ['awaiting', 'failed', 'detached'].includes(status))
    state.hidden = !show
    if (show) {
        state.dataset.status = status
        state.textContent = stateLabel(status)
    }
}

function renderBanners() {
    const banners = []
    const banner = (tone, message, buttons) =>
        h('div', { class: 'banner', data: { tone } }, [
            icon(tone === 'error' ? 'alert' : 'warning'),
            h('span', { text: message }),
            ...buttons.map(([label, action]) =>
                button({ class: 'text-button', text: label, data: { action } }),
            ),
        ])
    if (session.contextError)
        banners.push(banner('warning', text('contextError'), [[text('retry'), 'refresh-context']]))
    if (session.loadError)
        banners.push(
            banner('error', text('storageRecovery'), [
                [text('exportHistory'), 'export-history'],
                [text('resetHistory'), 'reset-history'],
                [text('exportDraft'), 'export-draft'],
            ]),
        )
    else if (session.storageError)
        banners.push(
            banner('error', text('storageError'), [
                [text('retry'), 'retry-storage'],
                [text('exportDraft'), 'export-draft'],
            ]),
        )
    element('banners').replaceChildren(...banners)
}

function renderJump() {
    const thread = currentThread()
    element('jump-latest').hidden = follower.pinned || !thread.turns.length
}

// Reconciles the transcript with the active thread: a different thread is
// rebuilt, the same thread is patched so new turns can animate in.
function syncTranscript(fresh = true) {
    const thread = currentThread()
    if (renderedThread !== thread.id) {
        renderedThread = thread.id
        renderTranscript(thread)
        follower.pinned = true
        // A conversation opens at its latest turn; the new task page at its top,
        // so its mascot and heading are never scrolled away.
        requestAnimationFrame(() => {
            if (currentThread().turns.length) follower.toBottom(false)
            else element('message-scroll').scrollTop = 0
        })
    } else patchTranscript(thread, fresh)
    element('empty-state').hidden = thread.turns.length > 0
    element('messages').hidden = !thread.turns.length
    observePlan()
    renderJump()
    renderRail(thread)
}

function render() {
    document.documentElement.dataset.steps = session.store.detail
    const ready = catalogReady()
    for (const id of ['workspace', 'help-button', 'settings-button']) element(id).hidden = !ready
    if (!ready) {
        showCatalogError()
        return false
    }
    const error = element('window-error')
    if (error.dataset.catalogError) {
        delete error.dataset.catalogError
        error.textContent = ''
        error.hidden = true
    }
    localize()
    layout()
    renderSidebar()
    renderHeader()
    placeComposer()
    renderHome()
    syncTranscript()
    renderComposer()
    renderPanel()
    renderProgressRing()
    renderBanners()
    renderSettings()
    syncTour()
    element('preview-badge').hidden = !isPreview()
    updateTicker()
    return true
}

// ---------- first-run tour ----------

// The tour is offered once per window on a first run that opens on the new
// task page. It waits for a quiet moment, and anything the user does first (a
// click, a key, a focus, typing) lets the offer go until the window opens
// again: someone already finding their way is not interrupted.
const TOUR_DELAY = 1200

function offerTour() {
    if (session.store.tourDone || session.store.introDismissed) return
    const quiet = new AbortController()
    const timer = setTimeout(() => {
        quiet.abort()
        if (tourAllowed()) startTour()
    }, TOUR_DELAY)
    const busy = () => {
        clearTimeout(timer)
        quiet.abort()
    }
    for (const name of ['pointerdown', 'keydown', 'focusin', 'input'])
        document.addEventListener(name, busy, { capture: true, signal: quiet.signal })
}

function tourAllowed() {
    const thread = currentThread()
    return (
        catalogReady() &&
        !session.loadError &&
        !document.hidden &&
        !session.store.tourDone &&
        !session.store.introDismissed &&
        !thread.turns.length &&
        !thread.draft.trim() &&
        !thread.attachments.length &&
        !drawer &&
        !document.querySelector('dialog[open], .popover:popover-open')
    )
}

// However it ends, the tour is not offered again; settings can replay it.
function startTour() {
    openTour(() => {
        session.store.tourDone = true
        persistSoon()
    })
}

// The tour belongs to the new task page: a task in the open thread ends it.
function syncTour() {
    if (!tourOpen()) return
    if (currentThread().turns.length) closeTour()
    else refreshTour()
}

// Agent events arrive quickly; patch the affected turn once per frame.
const dirtyTurns = new Map()
let frame = 0
// What the task list and the parts around the open turn last showed.
let listKey = ''
let shapeKey = ''
// User actions pass `now` so the button they pressed reacts in the same frame.
function refreshTurn(threadId, turnId, now = false) {
    dirtyTurns.set(turnId, threadId)
    if (now) flushTurns()
    else if (!frame) frame = requestAnimationFrame(flushTurns)
}

function flushTurns() {
    cancelAnimationFrame(frame)
    frame = 0
    const active = currentThread()
    let touchedActive = false
    for (const [turnId, threadId] of dirtyTurns) {
        if (threadId !== active.id) continue
        const turn = byId(active.turns, turnId)
        if (turn) {
            syncTurn(active, turn)
            touchedActive = true
        }
    }
    dirtyTurns.clear()
    // Streaming text arrives every few dozen milliseconds. Only the turn itself
    // changes then, so everything around it re-renders only when what it shows
    // (a status, a step, a plan state, a title) actually changed.
    const list = [...session.store.threads]
        .sort((a, b) => b.updatedAt - a.updatedAt)
        .map(
            (thread) =>
                `${thread.id}:${thread.title}:${threadStatus(thread)}:${!!thread.unread}:${progressOf(thread)}`,
        )
        .join('|')
    if (list !== listKey) {
        listKey = list
        renderSidebar()
    }
    if (touchedActive) {
        follower.follow()
        const last = lastTurn(active)
        const shape = [
            active.title,
            active.turns.length,
            active.queue.length,
            last?.run.status,
            last?.run.items.length,
            last &&
                activitySteps(last.run)
                    .map((step) => step.status)
                    .join(),
            last?.run.progress.map((step) => step.status).join(),
            last?.run.items
                .filter((item) => item.type === 'plan')
                .map((item) => `${item.state}${item.done}`)
                .join(),
        ].join('|')
        if (shape !== shapeKey) {
            shapeKey = shape
            renderHeader()
            renderComposer()
            if (panelShown()) renderPanel()
            renderProgressRing()
            observePlan()
            renderRail(active)
        }
    }
    updateTicker()
}

function updateTicker() {
    const running = ACTIVE_STATUSES.includes(lastTurn(currentThread())?.run.status)
    if (running && !ticker)
        ticker = setInterval(() => {
            const thread = currentThread()
            const turn = lastTurn(thread)
            const node = turn && turnNode(turn.id)?.querySelector('.run-status')
            if (node && !node.hidden) updateElapsed(node, turn)
        }, 1000)
    else if (!running && ticker) {
        clearInterval(ticker)
        ticker = 0
    }
}

// The approval bar above the composer appears while the plan's own buttons are
// out of view, so the decision is always one click away.
let planObserver = null
let planVisible = true
let observed = null
function observePlan() {
    const last = lastTurn(currentThread())
    const actions =
        (pendingPlan(last?.run) && turnNode(last.id)?.querySelector('.plan-card .plan-actions')) ||
        null
    if (actions && actions === observed) return
    planObserver?.disconnect()
    observed = actions
    if (!actions) {
        planVisible = true
        renderApprovalBar(false)
        return
    }
    planObserver = new IntersectionObserver(
        ([entry]) => {
            planVisible = entry.isIntersecting
            renderApprovalBar(!planVisible)
        },
        { root: element('message-scroll'), threshold: 0.6 },
    )
    planObserver.observe(actions)
    renderApprovalBar(!planVisible)
}

function notify(message, tone = 'info', action = null) {
    if (tone === 'error' && dialogError(message)) return
    toast(message, action, tone)
}

// Screen readers hear the open task's latest run once per change of phase:
// that it started, that a plan waits, that changes are being written, and how
// it ended with the result, error or answer. Labels and steps that change
// within a phase are not read. Other tasks report through toasts.
const PHASES = { thinking: 'working', running: 'working' }
const ENDINGS = {
    stopped: 'noticeStopped',
    detached: 'noticeDetached',
    interrupted: 'noticeInterrupted',
    declined: 'planDeclined',
}

function announceRun(threadId, turnId, before) {
    if (threadId !== session.store.activeId || lastTurn(currentThread())?.id !== turnId) return
    const run = lastTurn(currentThread()).run
    const phase = (status) => PHASES[status] || status
    if (before && phase(before) === phase(run.status)) return
    const last = (type) => run.items.findLast((item) => item.type === type)
    if (phase(run.status) === 'working') announce(run.label || text('thinking'))
    else if (run.status === 'applying') announce(text('applying'))
    else if (run.status === 'awaiting')
        announce(stateLabel('awaiting'), pendingPlan(run)?.plan.title)
    else if (run.status === 'failed')
        announce(stateLabel('failed'), last('error')?.title, last('error')?.text)
    else if (run.status === 'done' && last('result'))
        announce(last('result').title, last('result').text)
    else if (run.status === 'done')
        announce(stateLabel('done'), last('text') && plainText(last('text').text))
    else if (ENDINGS[run.status]) announce(text(ENDINGS[run.status]))
}

// ---------- context, runtime and permissions ----------

function normalizeContext(value) {
    return {
        ai_enabled: value.ai_enabled === true,
        providers: Array.isArray(value.providers)
            ? value.providers.map((provider) => ({
                  ...provider,
                  models: Array.isArray(provider.models) ? provider.models : [],
              }))
            : [],
        instances: Array.isArray(value.instances) ? value.instances : [],
        execution_available: value.execution_available === true,
    }
}

// New tasks start with the last model used, or the first one available, so
// people who never opened the picker can still send.
function fillSelection() {
    if (!modelsAvailable()) return
    const thread = currentThread()
    if (thread.selection) return
    const last = session.store.lastSelection
    thread.selection = (modelChoice(last) && last) || firstModel()
    persistSoon()
}

function refreshContext() {
    if (disposed || !catalogReady() || !window.AgentHost?.getContext) return Promise.resolve()
    if (contextFlight) return contextFlight
    const request = ++contextRequest
    let timeout
    const flight = (async () => {
        try {
            const result = await Promise.race([
                Promise.resolve().then(() => window.AgentHost.getContext()),
                new Promise((_, reject) => {
                    timeout = setTimeout(
                        () => reject(new Error('Context request timed out')),
                        15_000,
                    )
                }),
            ])
            if (request !== contextRequest || disposed) return
            const next = normalizeContext(structuredClone(result))
            const changed = JSON.stringify(next) !== JSON.stringify(session.context)
            session.context = next
            session.contextError = false
            session.contextLoaded = true
            fillSelection()
            await ensureRuntime()
            if (changed) {
                renderComposer()
                if (panelShown()) renderPanel()
                if (pickerOpen()) {
                    const list = element('picker-options')
                    const scroll = list.scrollTop
                    renderPicker()
                    list.scrollTop = scroll
                }
            }
        } catch {
            if (request === contextRequest && !disposed) session.contextError = true
        } finally {
            clearTimeout(timeout)
            if (request === contextRequest && !disposed) {
                renderBanners()
                renderComposer()
            }
        }
    })().finally(() => {
        if (contextFlight === flight) contextFlight = null
    })
    contextFlight = flight
    return flight
}

let runtimeFlight = null
function ensureRuntime() {
    const wantsNative = !!window.AgentHost?.runtime && session.context.execution_available
    if (session.runtime && session.runtime.preview !== wantsNative) return Promise.resolve()
    if (session.runtime && session.handles.size) return Promise.resolve()
    runtimeFlight ||= createRuntime(window.AgentHost, session.context)
        .then((runtime) => {
            session.runtime = runtime
            element('preview-badge').hidden = !isPreview()
            renderComposer()
        })
        .finally(() => {
            runtimeFlight = null
        })
    return runtimeFlight
}

function readPolicy(policy) {
    return {
        full_access: policy.full_access === true,
        computer_use: policy.full_access === true && policy.computer_use === true,
    }
}

async function refreshPermissions() {
    if (!permissionsAvailable() || session.permissionsSaving) {
        renderSettings()
        return
    }
    const request = ++permissionRequest
    session.permissionsLoading = true
    renderSettings()
    try {
        const policy = await window.AgentHost.getPermissions()
        if (request !== permissionRequest || disposed) return
        session.permissions = readPolicy(policy)
        session.permissionReadError = false
    } catch {
        if (request === permissionRequest) session.permissionReadError = true
    } finally {
        if (request === permissionRequest) {
            session.permissionsLoading = false
            renderSettings()
        }
    }
}

async function savePermissions(next) {
    if (!permissionsAvailable() || session.permissionsSaving || session.permissionsLoading)
        return false
    ++permissionRequest
    session.permissionsSaving = true
    document.querySelectorAll('[data-dialog-error]').forEach((error) => {
        error.hidden = true
    })
    renderSettings()
    try {
        const policy = await window.AgentHost.setPermissions(next)
        if (disposed) return false
        session.permissions = readPolicy(policy)
        session.permissionReadError = false
        return true
    } catch {
        notify(text('permissionSaveError'), 'error')
        return false
    } finally {
        session.permissionsSaving = false
        renderSettings()
        if (panelShown()) renderPanel()
    }
}

// ---------- runs ----------

function request(thread, turn, extra = {}) {
    const index = thread.turns.findIndex((entry) => entry.id === turn.id)
    return {
        thread: { id: thread.id, title: thread.title },
        turn: { id: turn.id, text: turn.text, attachments: structuredClone(turn.attachments) },
        instance: instanceChoice(turn.instanceId),
        instances: session.context.instances,
        model: modelChoice(turn.selection),
        history: structuredClone(thread.turns.slice(0, index)),
        resume: structuredClone(turn.run.resume),
        // Standing preferences from Settings, sent with every request.
        instructions: session.store.instructions,
        ...extra,
    }
}

async function launch(threadId, turnId, mode) {
    await ensureRuntime()
    const found = findTurn(threadId, turnId)
    if (!found || !session.runtime?.[mode]) return
    const emit = (event) => onRuntimeEvent(threadId, turnId, event)
    const extra =
        mode === 'approve'
            ? (() => {
                  const plan = found.turn.run.items.findLast((item) => item.type === 'plan')
                  return {
                      planId: plan?.plan.id,
                      excluded: (plan?.excluded || []).map(
                          (index) => plan.plan.changes[index].name,
                      ),
                  }
              })()
            : {}
    try {
        session.handles.set(
            turnId,
            session.runtime[mode](request(found.thread, found.turn, extra), emit),
        )
    } catch (error) {
        emit({ type: 'error', title: text('runtimeError'), text: String(error), retryable: true })
    }
}

function onRuntimeEvent(threadId, turnId, event) {
    const found = findTurn(threadId, turnId)
    if (!found) {
        session.handles.get(turnId)?.stop()
        session.handles.delete(turnId)
        return
    }
    const { thread, turn } = found
    const before = turn.run.status
    if (event.type === 'title') {
        if (!thread.titleEdited && event.title) thread.title = String(event.title).slice(0, 120)
        renderHeader()
    } else applyEvent(turn.run, event)
    announceRun(threadId, turnId, before)
    thread.updatedAt = Date.now()
    persistSoon()
    refreshTurn(threadId, turnId)
    if (event.type === 'done' || event.type === 'error') {
        session.handles.delete(turnId)
        persistNow()
        settle(thread.id, before)
    }
}

// After a run settles, tell the user if they are looking elsewhere, and send
// the next queued follow-up.
function settle(threadId, before) {
    const thread = findThread(threadId)
    const turn = lastTurn(thread)
    if (!thread || !turn) return
    const status = turn.run.status
    if (
        status === 'done' &&
        turn.run.items.some((item) => item.type === 'plan' && item.state === 'applied')
    ) {
        if (!session.store.introDismissed) {
            session.store.introDismissed = true
            persistSoon()
        }
    }
    if (
        status !== before &&
        thread.id !== session.store.activeId &&
        ['awaiting', 'done', 'failed'].includes(status)
    ) {
        thread.unread = true
        persistSoon()
        renderSidebar()
        const key =
            status === 'awaiting'
                ? 'toastAwaiting'
                : status === 'failed'
                  ? 'toastFailed'
                  : 'toastDone'
        notify(
            text(key, { title: thread.title || text('untitled') }),
            status === 'failed' ? 'error' : 'info',
            {
                label: text('view'),
                run: () => selectThread(thread.id),
            },
        )
    }
    if (thread.queue.length && !ACTIVE_STATUSES.includes(status))
        sendQueued(thread.id, thread.queue[0].id)
}

function startTurn(threadId, message) {
    let created = null
    const first = !findThread(threadId)?.turns.length
    const change = () =>
        commit((next) => {
            const thread = byId(next.threads, threadId)
            const last = lastTurn(thread)
            if (last) supersedePlan(last.run)
            const turn = newTurn(thread, message)
            turn.run.preview = isPreview()
            thread.turns.push(turn)
            if (!thread.title)
                thread.title = (
                    message.text.replace(/\s+/g, ' ') ||
                    message.attachments?.map((file) => file.name).join(' · ') ||
                    ''
                ).slice(0, 48)
            thread.updatedAt = Date.now()
            if (thread.selection) next.lastSelection = structuredClone(thread.selection)
            created = turn.id
            message.apply?.(thread)
        })
    // Runs the new turn once it is on screen.
    const begin = () => {
        if (!created) return false
        follower.toBottom(!first)
        launch(threadId, created, 'start')
        announceRun(threadId, created)
        return true
    }
    if (first && threadId === session.store.activeId)
        return morph(() => {
            if (change()) render()
        }, 'send').then(begin)
    change()
    if (created) render()
    return Promise.resolve(begin())
}

async function submit() {
    const thread = currentThread()
    const value = element('message-input').value.trim()
    // A pasted crash report or a file can be the whole request.
    if (!value && !thread.attachments.length) return
    await ensureRuntime()
    if (!isPreview() && !modelChoice(thread.selection)) {
        openPicker('model', element('model-button'))
        return
    }
    const attachments = structuredClone(thread.attachments)
    const last = lastTurn(thread)
    if (last && ACTIVE_STATUSES.includes(last.run.status)) {
        if (
            commit((next, active) => {
                active.queue.push({
                    id: crypto.randomUUID(),
                    text: value,
                    attachments,
                    createdAt: Date.now(),
                })
                active.draft = ''
                active.attachments = []
            })
        )
            render()
        return
    }
    startTurn(thread.id, {
        text: value,
        attachments,
        apply: (active) => {
            active.draft = ''
            active.attachments = []
        },
    })
}

function sendQueued(threadId, queuedId) {
    const thread = findThread(threadId)
    const entry = thread?.queue.find((item) => item.id === queuedId)
    if (!entry) return
    startTurn(threadId, {
        text: entry.text,
        attachments: entry.attachments,
        apply: (active) => {
            active.queue = active.queue.filter((item) => item.id !== queuedId)
        },
    })
}

function stop() {
    const thread = currentThread()
    const turn = lastTurn(thread)
    if (!turn || !ACTIVE_STATUSES.includes(turn.run.status)) return
    const before = turn.run.status
    session.handles.get(turn.id)?.stop()
    session.handles.delete(turn.id)
    stopRun(turn.run)
    persistNow()
    refreshTurn(thread.id, turn.id, true)
    announceRun(thread.id, turn.id, before)
}

function approve(turnId) {
    const thread = currentThread()
    let ok = false
    if (
        commit((next, active) => {
            const turn = byId(active.turns, turnId)
            ok = !!turn && beginApply(turn.run)
            if (ok) active.updatedAt = Date.now()
        }) &&
        ok
    ) {
        refreshTurn(thread.id, turnId, true)
        launch(thread.id, turnId, 'approve')
        announceRun(thread.id, turnId, 'awaiting')
    }
}

function decline(turnId) {
    const thread = currentThread()
    let ok = false
    commit((next, active) => {
        const turn = byId(active.turns, turnId)
        ok = !!turn && declinePlan(turn.run)
    })
    if (!ok) return
    refreshTurn(thread.id, turnId, true)
    announceRun(thread.id, turnId, 'awaiting')
    if (thread.queue.length) sendQueued(thread.id, currentThread().queue[0].id)
}

function retry(turnId) {
    const thread = currentThread()
    if (
        commit((next, active) => {
            const turn = byId(active.turns, turnId)
            if (!turn) return
            turn.run = newRun()
            turn.run.preview = isPreview()
            active.updatedAt = Date.now()
        })
    ) {
        const node = turnNode(turnId)
        node?.querySelector('.agent-items')?.replaceChildren()
        refreshTurn(thread.id, turnId, true)
        launch(thread.id, turnId, 'start')
        announceRun(thread.id, turnId)
    }
}

function checkStatus(turnId) {
    const thread = currentThread()
    if (
        commit((next, active) => {
            const turn = byId(active.turns, turnId)
            if (!turn || turn.run.status !== 'detached') return
            turn.run.status = 'running'
            turn.run.endedAt = null
            turn.run.phaseAt = Date.now()
        })
    ) {
        refreshTurn(thread.id, turnId, true)
        launch(thread.id, turnId, 'check')
        announceRun(thread.id, turnId)
    }
}

// ---------- threads ----------

function selectThread(id) {
    // Picked from the drawer, the task is worked on next: the focus goes to
    // its composer rather than to the drawer's toggle.
    const fromDrawer = drawer === 'sidebar' && element('sidebar').contains(document.activeElement)
    if (id === session.store.activeId) {
        if (narrow()) setSidebar(false)
        if (fromDrawer) element('message-input').focus()
        return
    }
    morph(() => {
        if (
            commit((next) => {
                next.activeId = id
                delete byId(next.threads, id)?.unread
            })
        ) {
            drawer = null
            fillSelection()
            render()
            if (fromDrawer) element('message-input').focus()
        }
    }, 'thread')
}

// Settles once the new task page is showing, with the focus in its composer.
function createThread() {
    const thread = currentThread()
    if (!thread.turns.length && !thread.draft && !thread.title) {
        element('message-input').focus()
        if (narrow()) setSidebar(false)
        return Promise.resolve()
    }
    return morph(() => {
        if (
            commit((next) => {
                const fresh = newThread(structuredClone(next.lastSelection))
                fresh.instanceId = activeThread(next).instanceId
                next.threads.push(fresh)
                next.activeId = fresh.id
            })
        ) {
            drawer = null
            fillSelection()
            render()
        }
    }, 'thread').then(() => element('message-input').focus())
}

function deleteThread(id) {
    session.handles.forEach((handle, turnId) => {
        if (findThread(id)?.turns.some((turn) => turn.id === turnId)) {
            handle.stop()
            session.handles.delete(turnId)
        }
    })
    const ok = commit((next) => {
        next.threads = next.threads.filter((thread) => thread.id !== id)
        if (!next.threads.length) next.threads.push(newThread(structuredClone(next.lastSelection)))
        if (!next.threads.some((thread) => thread.id === next.activeId))
            next.activeId = [...next.threads].sort((a, b) => b.updatedAt - a.updatedAt)[0].id
    })
    if (ok) {
        fillSelection()
        render()
    }
    return ok
}

function renameThread(id, value) {
    if (!value) return false
    const ok = commit((next) => {
        const thread = byId(next.threads, id)
        if (!thread) return
        thread.title = value.slice(0, 120)
        thread.titleEdited = true
    })
    if (ok) {
        renderSidebar()
        renderHeader()
    }
    return ok
}

function threadMenu(id, anchor) {
    const thread = findThread(id)
    const rename = menuItem('edit', text('rename'), { menuRename: id })
    const remove = menuItem('trash', text('deleteChat'), { menuDelete: id }, true)
    rename.setAttribute('aria-keyshortcuts', 'F2')
    remove.setAttribute('aria-keyshortcuts', 'Delete')
    openMenu(anchor, [
        menuItem('pin', text(thread?.pinned ? 'unpinChat' : 'pinChat'), { menuPin: id }),
        rename,
        menuItem('copy', text('copyChat'), { menuCopy: id }),
        h('hr', { class: 'menu-divider' }),
        remove,
    ])
}

// A pasted attachment, in the draft or in any request of the open task.
function findAttachment(id) {
    const thread = currentThread()
    return [...thread.attachments, ...thread.turns.flatMap((turn) => turn.attachments)].find(
        (file) => file.id === id && file.kind,
    )
}

function togglePin(id) {
    if (
        commit((next) => {
            const thread = byId(next.threads, id)
            if (thread?.pinned) delete thread.pinned
            else if (thread) thread.pinned = true
        })
    )
        renderSidebar()
}

function askRename(id) {
    confirm({
        title: text('rename'),
        accept: text('save'),
        rename: findThread(id)?.title || text('untitled'),
        onAccept: (value) => renameThread(id, value),
    })
}

function askDelete(id) {
    confirm({
        title: text('deleteTitle'),
        body: text('deleteBody'),
        accept: text('delete'),
        icon: 'trash',
        danger: true,
        onAccept: () => deleteThread(id),
    })
}

function copyThread(id) {
    const thread = findThread(id)
    if (thread) copyText(threadText(thread))
}

// The whole task as Markdown: each request quoted, each answer as written.
function threadText(thread) {
    const parts = [`# ${thread.title || text('untitled')}`]
    for (const turn of thread.turns) {
        const files = turn.attachments.map((file) => file.name).join(', ')
        const request = [turn.text, files && `(${files})`].filter(Boolean).join(' ')
        parts.push(request.replace(/^/gm, '> '))
        const answer = answerText(turn)
        if (answer) parts.push(answer)
    }
    return parts.join('\n\n')
}

// What the agent wrote in a turn, without its steps, plan or result.
function answerText(turn) {
    return turn.run.items
        .filter((item) => item.type === 'text')
        .map((item) => item.text)
        .join('\n\n')
}

// ---------- layout actions ----------

// A drawer covers the conversation, so opening one moves the focus into it.
// A docked pane leaves the focus on its toggle.
function setSidebar(open) {
    if (sidebarShown() === open) return
    if (narrow()) drawer = open ? 'sidebar' : null
    else {
        session.store.sidebarOpen = open
        persistSoon()
    }
    layout()
    if (open && drawer === 'sidebar') element('sidebar').querySelector('.new-task').focus()
}

function setPanel(open) {
    if (panelShown() === open) return
    if (window.innerWidth < WIDE) drawer = open ? 'panel' : null
    else {
        session.store.panelOpen = open
        persistSoon()
    }
    if (open) renderPanel()
    layout()
    if (open && drawer === 'panel')
        element('task-panel').querySelector('[data-action="toggle-tasks"]').focus()
}

// ---------- files ----------

function exportFile(name, content) {
    const url = URL.createObjectURL(new Blob([content], { type: 'text/plain;charset=utf-8' }))
    const link = document.createElement('a')
    link.href = url
    link.download = name
    link.click()
    setTimeout(() => URL.revokeObjectURL(url), 1000)
}

function exportHistory() {
    try {
        const raw = localStorage.getItem(STORAGE_KEY) ?? localStorage.getItem(LEGACY_STORAGE_KEY)
        if (raw === null) throw new Error('History unavailable')
        exportFile('axolotl-agent-history.json', raw)
    } catch {
        notify(text('storageExportError'), 'error')
    }
}

function resetHistory() {
    const draft = currentThread()
    try {
        const fresh = initialStore()
        Object.assign(activeThread(fresh), {
            draft: draft.draft,
            attachments: draft.attachments,
            instanceId: draft.instanceId,
            selection: draft.selection,
        })
        fresh.introDismissed = session.store.introDismissed
        fresh.tourDone = session.store.tourDone
        fresh.sidebarOpen = session.store.sidebarOpen
        for (const handle of session.handles.values()) handle.stop()
        session.handles.clear()
        session.store = saveStore(fresh)
        session.loadError = false
        session.storageError = false
        renderedThread = null
        render()
        return true
    } catch {
        dialogError(text('storageError'))
        return false
    }
}

// Text files a player is likely to drop (latest.log, a crash report, a
// config) are read, so they preview and are recognised like a paste.
const TEXT_FILE = /\.(log|txt|json|toml|cfg|properties)$/i
const MAX_READ = 2 * 1024 * 1024

function addFiles(files) {
    const list = [...files]
    if (!list.length) return
    const thread = currentThread()
    const room = MAX_ATTACHMENTS - thread.attachments.length
    if (list.length > room) notify(text('attachmentLimit', { count: MAX_ATTACHMENTS }), 'error')
    const picked = list.slice(0, Math.max(0, room)).map((file) => ({
        file,
        entry: { id: crypto.randomUUID(), name: file.name, size: file.size },
    }))
    thread.attachments.push(...picked.map(({ entry }) => entry))
    persistSoon()
    renderComposer()
    element('message-input').focus()
    for (const { file, entry } of picked)
        if (TEXT_FILE.test(file.name) && file.size <= MAX_READ && typeof file.text === 'function')
            file.text().then(
                (value) => readInto(entry.id, value),
                () => {},
            )
}

// Gives an attached text file its contents once read, wherever its draft is now.
function readInto(id, value) {
    const attachment = session.store.threads
        .flatMap((thread) => thread.attachments)
        .find((file) => file.id === id)
    if (!attachment) return
    Object.assign(attachment, {
        kind: pasteKind(value),
        text: value.slice(0, MAX_PASTE),
        lines: value.split(/\r\n|\r|\n/).length,
    })
    persistSoon()
    renderComposer()
}

// Names a paste by what it looks like, so a crash report reads as one.
function pasteKind(value) {
    if (
        /---- Minecraft Crash Report ----|#@!@# Game crashed!|A fatal error has been detected by the Java Runtime/.test(
            value,
        )
    )
        return 'crash'
    if (/^\[[\d:.]+\] \[[^\]\n]+\/(INFO|WARN|ERROR|DEBUG|FATAL)\]/m.test(value)) return 'log'
    return 'paste'
}

function addPaste(value, lines) {
    const thread = currentThread()
    if (thread.attachments.length >= MAX_ATTACHMENTS) {
        notify(text('attachmentLimit', { count: MAX_ATTACHMENTS }), 'error')
        return null
    }
    const kind = pasteKind(value)
    const base = text({ crash: 'pasteCrashName', log: 'pasteLogName' }[kind] || 'pasteName')
    const taken = thread.attachments.filter((file) => file.name.startsWith(base)).length
    const id = crypto.randomUUID()
    thread.attachments.push({
        id,
        name: taken ? `${base} ${taken + 1}` : base,
        size: new Blob([value]).size,
        kind,
        text: value.slice(0, MAX_PASTE),
        lines,
    })
    persistSoon()
    renderComposer()
    return id
}

async function copyText(value, done = text('copied')) {
    try {
        await navigator.clipboard.writeText(value)
        notify(done, 'success')
    } catch {
        notify(text('copyError'), 'error')
    }
}

function copy(turnId, kind) {
    const turn = byId(currentThread().turns, turnId)
    if (turn) copyText(kind === 'user' ? turn.text : answerText(turn))
}

// ---------- events ----------

const actions = {
    'toggle-sidebar': () => setSidebar(!sidebarShown()),
    'toggle-tasks': () => setPanel(!panelShown()),
    'close-drawers': () => {
        drawer = null
        layout()
    },
    'new-chat': () => createThread(),
    help: () => {
        renderHelp()
        openDialog('help-dialog')
    },
    settings: () => {
        renderSettings()
        openDialog('settings-dialog')
        refreshPermissions()
    },
    'model-picker': (event, target) => {
        refreshContext()
        openPicker('model', target)
    },
    'instance-picker': (event, target) => {
        refreshContext()
        openPicker('instance', target)
    },
    'close-dialog': () => {
        for (const dialog of document.querySelectorAll('dialog[open]')) {
            if (dialog.id === 'confirm-dialog' && confirmBusy()) continue
            if (dialog.id === 'settings-dialog' && session.permissionsSaving) continue
            dialog.close()
        }
    },
    'refresh-context': () => refreshContext(),
    'thread-menu': (event, target) => threadMenu(session.store.activeId, target),
    'preview-info': (event, target) =>
        openMenu(
            target,
            [
                h('div', { class: 'menu-note' }, [
                    h('strong', { text: text('previewTitle') }),
                    h('p', { text: text('previewBody') }),
                ]),
            ],
            'start',
            text('previewTitle'),
        ),
    'export-history': exportHistory,
    'export-draft': () => exportFile('axolotl-agent-draft.txt', currentThread().draft),
    'reset-history': () =>
        confirm({
            title: text('resetHistory'),
            body: text('resetHistoryWarning'),
            accept: text('resetHistory'),
            danger: true,
            onAccept: resetHistory,
        }),
    'clear-history': () =>
        confirm({
            title: text('clearHistoryTitle'),
            body: text('clearHistoryBody'),
            accept: text('clearHistory'),
            icon: 'trash',
            danger: true,
            onAccept: () => {
                const ok = resetHistory()
                if (ok) element('settings-dialog').close()
                return ok
            },
        }),
    'retry-storage': () => {
        persistNow()
        if (!session.storageError) notify(text('storageSaved'), 'success')
    },
    'show-intro': () => {
        session.store.introDismissed = false
        persistSoon()
        element('settings-dialog').close()
        createThread()
        renderHome()
    },
    // Replays on the new task page, which the tour is about, once that page has
    // settled in, and only if nothing was started there in the meantime.
    'replay-tour': () => {
        element('settings-dialog').close()
        createThread()
            .then(homeSettled)
            .then(() => {
                const thread = currentThread()
                if (
                    !tourOpen() &&
                    !thread.turns.length &&
                    !thread.draft.trim() &&
                    !document.querySelector('dialog[open], .popover:popover-open')
                )
                    startTour()
            })
    },
    'dismiss-intro': () => {
        const intro = element('intro')
        session.store.introDismissed = true
        persistSoon()
        const leave = animate(
            intro,
            [{ opacity: 1 }, { opacity: 0, transform: 'translateY(6px)' }],
            { duration: 180 },
        )
        if (leave) leave.finished.then(renderHome, renderHome)
        else renderHome()
    },
    attach: () => element('file-input').click(),
    stop,
    approve: (event, target) => approve(target.dataset.turn),
    decline: (event, target) => decline(target.dataset.turn),
    retry: (event, target) => retry(target.dataset.turn),
    'check-status': (event, target) => checkStatus(target.dataset.turn),
    'review-plan': (event, target) => {
        const card = turnNode(target.dataset.turn)?.querySelector('.plan-card')
        card?.scrollIntoView({
            behavior: reducedMotion.matches ? 'instant' : 'smooth',
            block: 'center',
        })
        // The bar hides once the card is in view, so the focus goes with the
        // reader to the plan, named by its title.
        card?.focus({ preventScroll: true })
        if (card)
            animate(
                card,
                [
                    { boxShadow: '0 0 0 0 var(--agent-accent-soft)' },
                    { boxShadow: '0 0 0 6px var(--agent-accent-soft)' },
                    { boxShadow: '0 0 0 0 transparent' },
                ],
                { duration: 700 },
            )
    },
    'toggle-activity': (event, target) => toggleActivity(target),
    'show-changes': (event, target) => openChanges(appliedChanges(currentThread()), target),
    'show-steps': (event, target) => showAllSteps(target),
    'jump-latest': () => follower.toBottom(),
}

const RUN_ACTIONS = new Set(['approve', 'decline', 'retry', 'check-status'])

document.addEventListener('click', (event) => {
    const target = event.target.closest(
        '[data-action], [data-thread], [data-thread-menu], [data-prompt], [data-model], [data-instance], [data-copy-turn], [data-remove-attachment], [data-remove-queued], [data-send-queued], [data-menu-rename], [data-menu-delete], [data-toggle-change], [data-copy-code], [data-copy-link], [data-edit-turn], [data-mark-turn], [data-menu-pin], [data-menu-copy], [data-followup], [data-file-preview], [data-copy-paste]',
    )
    if (!target || target.disabled || target.getAttribute('aria-disabled') === 'true') return
    const data = target.dataset
    if (data.action) {
        if (target.type === 'submit') return
        actions[data.action]?.(event, target)
        // These start or settle a run and redraw the button that was pressed;
        // the focus waits in the composer, where Escape stops the run.
        if (RUN_ACTIONS.has(data.action) && !focusable(target)) element('message-input').focus()
    } else if (data.threadMenu) threadMenu(data.threadMenu, target)
    else if (data.thread) selectThread(data.thread)
    else if (data.menuPin) {
        closeMenu()
        togglePin(data.menuPin)
    } else if (data.menuCopy) {
        closeMenu()
        copyThread(data.menuCopy)
    } else if (data.menuRename) {
        closeMenu()
        askRename(data.menuRename)
    } else if (data.menuDelete) {
        closeMenu()
        askDelete(data.menuDelete)
    } else if (data.prompt) {
        const thread = currentThread()
        thread.draft = text(data.prompt)
        persistSoon()
        renderComposer()
        renderHeader()
        renderSidebar()
        const input = element('message-input')
        input.focus()
        input.setSelectionRange(input.value.length, input.value.length)
        animate(element('composer'), [{ transform: 'scale(0.99)' }, { transform: 'scale(1)' }], {
            duration: 260,
        })
    } else if (data.model !== undefined) {
        if (
            commit((next, thread) => {
                thread.selection = { provider_id: data.provider, model_id: data.model }
                next.lastSelection = structuredClone(thread.selection)
            })
        ) {
            closePicker()
            renderComposer()
            element('message-input').focus()
        }
    } else if (data.instance !== undefined) {
        if (
            commit((next, thread) => {
                thread.instanceId = data.instance || null
            })
        ) {
            closePicker()
            renderComposer()
            element('message-input').focus()
        }
    } else if (data.copyTurn) copy(data.copyTurn, data.copyKind)
    else if (data.filePreview) {
        const file = findAttachment(data.filePreview)
        if (file) openFilePeek(file, target)
    } else if (data.copyPaste) {
        const file = findAttachment(data.copyPaste)
        if (file) copyText(file.text)
    } else if (data.followup) {
        closePopover(element('changes-peek'))
        const input = element('message-input')
        // The chip goes once the request is sent; the focus waits in the composer.
        input.focus()
        input.value = data.followup
        input.dispatchEvent(new Event('input', { bubbles: true }))
        submit()
    } else if (data.markTurn) {
        // Marks an answer so the navigator flags it for coming back to.
        const turnId = data.markTurn
        if (
            commit((next, thread) => {
                const turn = byId(thread.turns, turnId)
                if (turn?.marked) delete turn.marked
                else if (turn) turn.marked = true
            })
        ) {
            refreshTurn(session.store.activeId, turnId, true)
            renderRail(currentThread())
        }
    } else if (data.copyCode !== undefined)
        copyText(target.closest('.code-block').querySelector('code').textContent)
    else if (data.copyLink) copyText(data.copyLink, text('linkCopied'))
    else if (data.editTurn) {
        // Puts the request back in the composer to change and send again.
        const turn = byId(currentThread().turns, data.editTurn)
        const input = element('message-input')
        if (!turn) return
        input.value = turn.text
        input.dispatchEvent(new Event('input', { bubbles: true }))
        input.focus()
        input.setSelectionRange(input.value.length, input.value.length)
    } else if (data.removeAttachment) {
        const thread = currentThread()
        thread.attachments = thread.attachments.filter((file) => file.id !== data.removeAttachment)
        persistSoon()
        renderComposer()
    } else if (data.removeQueued) {
        if (
            commit((next, thread) => {
                thread.queue = thread.queue.filter((entry) => entry.id !== data.removeQueued)
            })
        )
            renderComposer()
    } else if (data.sendQueued) sendQueued(session.store.activeId, data.sendQueued)
    else if (data.toggleChange) {
        const [turnId, index] = data.toggleChange.split(':')
        let ok = false
        if (
            commit((next, thread) => {
                const turn = byId(thread.turns, turnId)
                ok = !!turn && toggleChange(turn.run, Number(index))
            }) &&
            ok
        )
            refreshTurn(session.store.activeId, turnId, true)
    }
})

element('composer').addEventListener('submit', (event) => {
    event.preventDefault()
    submit()
})

element('message-input').addEventListener('input', (event) => {
    // Unsaved text stays in memory when storage fails; sending is transactional.
    const thread = currentThread()
    thread.draft = event.target.value
    persistSoon(250)
    heroListens(!!event.target.value.trim())
    renderComposer()
    syncSlash(event.target.value)
    element('conversation-more').hidden = !thread.turns.length && !thread.draft
    if (!thread.turns.length && thread.title === '' && thread.draft) renderSidebar()
})

element('message-input').addEventListener('keydown', (event) => {
    if (slashKeydown(event)) return
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
        event.preventDefault()
        submit()
    } else if (event.key === 'ArrowUp' && !event.target.value && !event.isComposing) {
        // As in a terminal: an empty box recalls the last request to edit and resend.
        const last = lastTurn(currentThread())
        if (!last) return
        event.preventDefault()
        event.target.value = last.text
        event.target.dispatchEvent(new Event('input', { bubbles: true }))
        event.target.setSelectionRange(last.text.length, last.text.length)
    }
})

element('file-input').addEventListener('change', (event) => {
    addFiles(event.target.files)
    event.target.value = ''
})

// A long paste, such as a crash report or a log, becomes an attachment so the
// request stays readable; the toast puts it back as text if that was wanted.
// Pasted files are attached like picked ones.
element('message-input').addEventListener('paste', (event) => {
    const files = event.clipboardData?.files
    if (files?.length) {
        event.preventDefault()
        addFiles(files)
        return
    }
    const value = event.clipboardData?.getData('text/plain') || ''
    const lines = value.split(/\r\n|\r|\n/).length
    if (lines < PASTE_LINES && value.length < PASTE_CHARS) return
    event.preventDefault()
    const input = event.target
    const [start, end] = [input.selectionStart, input.selectionEnd]
    const added = addPaste(value, lines)
    if (!added) return
    notify(text('pastedAsFile'), 'info', {
        label: text('pasteAsText'),
        run: () => {
            const thread = currentThread()
            thread.attachments = thread.attachments.filter((file) => file.id !== added)
            input.setRangeText(value, start, end, 'end')
            input.dispatchEvent(new Event('input', { bubbles: true }))
            renderComposer()
            input.focus()
        },
    })
})

// Files can be dropped anywhere on the conversation, which shows where they go.
const conversationNode = element('conversation')
const dragging = (event) => [...(event.dataTransfer?.types || [])].includes('Files')
let dragDepth = 0
conversationNode.addEventListener('dragenter', (event) => {
    if (!dragging(event)) return
    dragDepth++
    element('drop-overlay').hidden = false
})
conversationNode.addEventListener('dragover', (event) => {
    if (dragging(event)) event.preventDefault()
})
conversationNode.addEventListener('dragleave', (event) => {
    if (!dragging(event)) return
    dragDepth = Math.max(0, dragDepth - 1)
    if (!dragDepth) element('drop-overlay').hidden = true
})
conversationNode.addEventListener('drop', (event) => {
    if (!dragging(event)) return
    event.preventDefault()
    dragDepth = 0
    element('drop-overlay').hidden = true
    addFiles(event.dataTransfer?.files || [])
})

// The detail choice is a radio group: arrow keys move both the focus and the
// choice, and Home and End jump to the ends.
element('detail-choice').addEventListener('keydown', (event) => {
    const choices = [...element('detail-choice').querySelectorAll('[data-detail]')]
    const index = choices.indexOf(document.activeElement)
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key]
    const next =
        step === undefined
            ? { Home: 0, End: choices.length - 1 }[event.key]
            : (index + step + choices.length) % choices.length
    if (index < 0 || next === undefined) return
    event.preventDefault()
    choices[next].focus()
    choices[next].click()
})

// Settings that only change this window apply as soon as they are picked.
element('detail-choice').addEventListener('click', (event) => {
    const choice = event.target.closest('[data-detail]')
    if (!choice || choice.dataset.detail === session.store.detail) return
    session.store.detail = choice.dataset.detail
    persistSoon(0)
    document.documentElement.dataset.steps = session.store.detail
    renderSettings()
    // Patching every turn refolds the step groups without losing the scroll.
    syncTranscript(false)
})
element('instructions-input').addEventListener('input', (event) => {
    session.store.instructions = event.target.value.slice(0, MAX_INSTRUCTIONS)
    persistSoon()
})

// The task title renames in place, by double-click or F2: Enter or leaving
// the field saves, Escape keeps the old name. Ending it by keyboard puts the
// focus back where it was.
function renameInPlace() {
    const thread = currentThread()
    const title = element('conversation-title')
    if (!thread.turns.length || title.hidden) return false
    const back = focusPlace()
    const input = h('input', {
        class: 'title-input',
        value: thread.title || text('untitled'),
        maxlength: '120',
        'aria-label': text('rename'),
    })
    title.hidden = true
    title.after(input)
    input.focus()
    input.select()
    // Removing the focused field fires blur on the way out, so the first
    // ending wins.
    let done = false
    const finish = (save, keyboard) => {
        if (done) return
        done = true
        const value = input.value.trim()
        input.remove()
        title.hidden = false
        if (save && value && value !== thread.title) renameThread(thread.id, value)
        if (keyboard) restoreFocus(back, element('message-input'))
    }
    input.addEventListener('keydown', (event) => {
        if (event.key === 'Enter' && !event.isComposing) {
            event.preventDefault()
            finish(true, true)
        } else if (event.key === 'Escape') {
            event.preventDefault()
            event.stopPropagation()
            finish(false, true)
        }
    })
    input.addEventListener('blur', () => finish(true, false))
    return true
}
element('conversation-title').addEventListener('dblclick', renameInPlace)

// A right click on a task, or the menu key on it, opens the same menu as its
// ⋯ button.
element('conversation-list').addEventListener('contextmenu', (event) => {
    const row = event.target.closest('.thread-row')
    if (!row) return
    event.preventDefault()
    threadMenu(row.dataset.row, row.querySelector('.thread-more'))
})

// Up and down walk the task list, Home and End jump to its ends, and Delete
// asks to delete the focused task.
element('conversation-list').addEventListener('keydown', (event) => {
    const row = event.target.closest('.thread-row')
    if (!row || event.ctrlKey || event.metaKey || event.altKey) return
    if (event.key === 'Delete') {
        event.preventDefault()
        askDelete(row.dataset.row)
        return
    }
    const links = [...element('conversation-list').querySelectorAll('.thread-link')]
    const index = links.indexOf(row.querySelector('.thread-link'))
    const next = { ArrowDown: index + 1, ArrowUp: index - 1, Home: 0, End: links.length - 1 }[
        event.key
    ]
    if (next === undefined || !links[next]) return
    event.preventDefault()
    links[next].focus()
})

element('conversation-search').addEventListener('input', (event) => {
    session.search = event.target.value
    renderSidebar()
})

element('picker-search').addEventListener('input', renderPicker)
element('picker-search').addEventListener('keydown', (event) => {
    if (event.key === 'ArrowDown') {
        event.preventDefault()
        focusPickerOption(0)
    } else if (event.key === 'Enter') {
        event.preventDefault()
        element('picker-options').querySelector('.picker-option')?.click()
    }
})
element('picker-options').addEventListener('keydown', pickerKeydown)
bindPalette()
bindRail()
bindTranscript()
bindTips()
// Focus that would drop to the page lands in the composer at the latest.
guardFocus(() => element('message-input'))
bindOverlays(() => element('message-input'))
// A picked shortcut either fills in a starter request or runs a window action.
bindSlash((entry) => {
    const input = element('message-input')
    if (entry.mention) {
        // The "@name" goes; the instance becomes the task's context, and its
        // button in the toolbar nods to show where the choice went.
        const end = input.selectionEnd ?? input.value.length
        input.setRangeText('', entry.mention.start, end, 'end')
        input.dispatchEvent(new Event('input', { bubbles: true }))
        if (
            commit((next, thread) => {
                thread.instanceId = entry.instance?.id || null
            })
        ) {
            renderComposer()
            animate(
                element('instance-button'),
                [
                    { transform: 'scale(1)' },
                    { transform: 'scale(1.08)' },
                    { transform: 'scale(1)' },
                ],
                { duration: 360 },
            )
        }
        input.focus()
        return
    }
    if (entry.prompt) {
        input.value = text(entry.prompt)
        input.dispatchEvent(new Event('input', { bubbles: true }))
        input.focus()
        input.setSelectionRange(input.value.length, input.value.length)
        return
    }
    input.value = ''
    input.dispatchEvent(new Event('input', { bubbles: true }))
    const anchors = { 'model-picker': 'model-button', 'instance-picker': 'instance-button' }
    actions[entry.action]?.(null, element(anchors[entry.action] || 'message-input'))
})
element('message-input').addEventListener('blur', closeSlash)

element('permission-full-access').addEventListener('change', (event) => {
    const enabling = event.target.checked
    renderSettings()
    if (enabling)
        confirm({
            title: text('fullAccessWarningTitle'),
            body: text('fullAccessWarning'),
            accept: text('enableFullAccess'),
            icon: 'shield',
            danger: true,
            onAccept: () => savePermissions({ full_access: true, computer_use: false }),
        })
    else savePermissions({ full_access: false, computer_use: false })
})

element('permission-computer-use').addEventListener('change', (event) => {
    const enabling = event.target.checked
    renderSettings()
    if (session.permissions.full_access)
        savePermissions({ full_access: true, computer_use: enabling })
})

element('confirm-form').addEventListener('submit', (event) => {
    event.preventDefault()
    acceptConfirm()
})

for (const dialog of document.querySelectorAll('dialog')) {
    dialog.addEventListener('cancel', (event) => {
        if (confirmBusy() || session.permissionsSaving) event.preventDefault()
    })
    dialog.addEventListener('click', (event) => {
        if (event.target !== dialog) return
        const rect = dialog.getBoundingClientRect()
        const outside =
            event.clientX < rect.left ||
            event.clientX > rect.right ||
            event.clientY < rect.top ||
            event.clientY > rect.bottom
        if (outside && !confirmBusy() && !session.permissionsSaving) dialog.close()
    })
}

// The command menu lists what the window can do, then recent tasks.
function showPalette() {
    const action = (icon, label, run, keys) => ({ icon, label: text(label), run, keys })
    const thread = currentThread()
    const current = thread.turns.length
        ? [
              action('pin', thread.pinned ? 'unpinChat' : 'pinChat', () => togglePin(thread.id)),
              action('panel', 'togglePanel', () => setPanel(!panelShown())),
              action('copy', 'copyChat', () => copyThread(thread.id)),
              action('edit', 'rename', () => askRename(thread.id)),
              action('trash', 'deleteChat', () => askDelete(thread.id)),
          ]
        : []
    openPalette({
        current: { title: thread.title || text('untitled'), entries: current },
        actions: [
            action('plus', 'newChat', () => createThread(), 'Ctrl N'),
            action('sidebar', 'toggleSidebar', () => setSidebar(!sidebarShown()), 'Ctrl B'),
            ...(current.length
                ? []
                : [action('panel', 'togglePanel', () => setPanel(!panelShown()))]),
            action('sparkle', 'model', () =>
                actions['model-picker'](null, element('model-button')),
            ),
            action('cube', 'instance', () =>
                actions['instance-picker'](null, element('instance-button')),
            ),
            action('attach', 'attach', () => actions.attach()),
            action('settings', 'settings', () => actions.settings(), 'Ctrl ,'),
            action('help', 'help', () => actions.help()),
        ],
        threads: [...session.store.threads]
            .filter((entry) => entry !== thread || !current.length)
            .filter((entry) => entry.turns.length || entry.draft)
            .sort((a, b) => b.updatedAt - a.updatedAt)
            .map((entry) => ({
                icon: 'message',
                label: entry.title || text('untitled'),
                detail: lastTurn(entry) ? stateLabel(lastTurn(entry).run.status) : '',
                status: threadStatus(entry),
                run: () => selectThread(entry.id),
            })),
    })
}

document.addEventListener('keydown', (event) => {
    const modifier = event.ctrlKey || event.metaKey
    // Escape belongs to an open dialog, picker, menu or preview: it closes
    // that, and never also stops the run behind it. Looked up only for the
    // keys handled here, as the query walks the whole page.
    const overlay = () => document.querySelector('dialog[open], .popover:popover-open')
    if (event.key === 'F2' && !modifier && !event.shiftKey && !event.altKey) {
        // F2 renames: the focused task in the list, otherwise the open one in place.
        if (overlay()) return
        event.preventDefault()
        const row = event.target.closest?.('.thread-row')?.dataset.row
        if (row && row !== session.store.activeId) askRename(row)
        else if (!renameInPlace() && row) askRename(row)
        return
    }
    if (modifier && !event.shiftKey && !event.altKey) {
        const key = event.key.toLowerCase()
        if (key === 'n') {
            event.preventDefault()
            if (!document.querySelector('dialog[open]')) createThread()
        } else if (key === 'k') {
            event.preventDefault()
            if (element('palette').open) element('palette').close()
            else if (!overlay()) showPalette()
        } else if (key === ',') {
            event.preventDefault()
            if (!overlay()) actions.settings()
        } else if (key === 'b') {
            event.preventDefault()
            if (!overlay()) setSidebar(!sidebarShown())
        } else if (key === '[' || key === ']') {
            // Steps through the tasks in the order the list shows them.
            event.preventDefault()
            const rows = [...element('conversation-list').querySelectorAll('.thread-row')]
            const at = rows.findIndex((row) => row.dataset.row === session.store.activeId)
            const next = rows[at + (key === ']' ? 1 : -1)]
            if (!overlay() && next) selectThread(next.dataset.row)
        }
        return
    }
    if (event.key !== 'Escape' || overlay()) return
    const turn = lastTurn(currentThread())
    if (turn && ACTIVE_STATUSES.includes(turn.run.status)) {
        event.preventDefault()
        stop()
    } else if (drawer) {
        drawer = null
        layout()
    } else if (document.activeElement === element('conversation-search') && session.search) {
        session.search = ''
        element('conversation-search').value = ''
        renderSidebar()
    }
})

// ---------- lifecycle ----------

async function revealWindow() {
    for (let attempt = 0; attempt < 3 && !disposed; attempt++) {
        try {
            await window.AgentHost?.reveal?.()
            return
        } catch {
            if (attempt === 2) {
                if (catalogReady()) notify(text('windowRevealError'), 'error')
                else showCatalogError()
            } else await new Promise((resolve) => setTimeout(resolve, 500))
        }
    }
}

async function boot() {
    if (booted) return
    booted = true
    await window.AgentHost?.ready
    render()
    document.documentElement.dataset.ready = 'true'
    refreshContext()
    ensureRuntime()
    void revealWindow()
    if (session.loadError && catalogReady()) notify(text('storageLoadError'), 'error')
    offerTour()
}

window.addEventListener('agent-host-ready', boot)
window.addEventListener('agent-appearance-updated', () => {
    if (booted && render()) refreshContext()
})
window.addEventListener('focus', () => refreshContext())
document.addEventListener('visibilitychange', () => {
    if (!document.hidden) refreshContext()
    else if (persistTimer) persistNow()
})
window.addEventListener('pagehide', () => {
    if (persistTimer) persistNow()
    disposed = true
    ++contextRequest
    contextFlight = null
    ++permissionRequest
    clearInterval(ticker)
    ticker = 0
})
window.addEventListener('pageshow', (event) => {
    if (event.persisted) {
        disposed = false
        refreshContext()
    }
})
window.addEventListener('resize', () => {
    closePicker()
    if ((drawer === 'sidebar' && !narrow()) || (drawer === 'panel' && window.innerWidth >= WIDE))
        drawer = null
    layout()
    resizeInput()
})

await boot()
