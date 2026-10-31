import {
    activeThread,
    commitStore,
    initialStore,
    loadStore,
    newThread,
    saveStore,
} from './state.js'
import { dateLabel, setLocale, text } from './messages.js'

const element = (id) => document.getElementById(id)
const icon = (name) => `<svg aria-hidden="true"><use href="#i-${name}"/></svg>`
const escape = (value) =>
    String(value ?? '').replace(
        /[&<>"']/g,
        (character) =>
            ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character],
    )
let store
let loadError = false
try {
    store = loadStore()
} catch {
    store = initialStore()
    loadError = true
}
if (window.innerWidth < 760) store.sidebarOpen = false
const displayedMessages = new Set(
    store.threads.flatMap((thread) => thread.messages.map((message) => message.id)),
)
let firstRender = true
let context = { ai_enabled: false, providers: [], instances: [], execution_available: false }
let loading = true
let contextError = false
let contextRequest = 0
let contextFlight = null
let refreshTimer
let disposed = false
let permissionPolicy = { full_access: false, computer_use: false }
let permissionsLoading = false
let permissionsSaving = false
let permissionReadError = false
let permissionRequest = 0
let search = ''
let taskOpen = false
let pickerKind = null
let editKind = null
let noticeTimer

const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)')
const panelAnimations = new Map()
const contentAnimations = new Map()
const motionCurve = () =>
    getComputedStyle(document.documentElement).getPropertyValue('--ease-out').trim()
function cancelContentMotion() {
    for (const animation of contentAnimations.values()) animation.cancel()
    contentAnimations.clear()
}
function animateContent(node, frames, duration = 200) {
    contentAnimations.get(node)?.cancel()
    if (reducedMotion.matches || disposed || !node.getClientRects().length) return
    const animation = node.animate(frames, { duration, easing: motionCurve() })
    contentAnimations.set(node, animation)
    animation.finished
        .then(() => {
            if (contentAnimations.get(node) === animation) contentAnimations.delete(node)
        })
        .catch(() => {})
}
function composerPosition() {
    return document.querySelector('.composer-wrap').getBoundingClientRect()
}
function moveComposer(previous) {
    const node = document.querySelector('.composer-wrap')
    const next = node.getBoundingClientRect()
    const x = previous.left - next.left
    const y = previous.top - next.top
    if (Math.abs(x) + Math.abs(y) > 1)
        animateContent(
            node,
            [{ transform: `translate(${x}px, ${y}px)` }, { transform: 'translate(0, 0)' }],
            240,
        )
}
function finishPanel(node, animation) {
    if (panelAnimations.get(node)?.animation !== animation) return
    panelAnimations.delete(node)
    for (const property of ['position', 'left', 'top', 'width', 'height', 'display', 'z-index'])
        node.style.removeProperty(property)
    node.removeAttribute('data-motion')
    if (node.id === 'task-panel') node.hidden = !taskOpen
    node.inert = node.id === 'task-panel' ? !taskOpen : !store.sidebarOpen
}
function settlePanels() {
    for (const [node, state] of panelAnimations) {
        state.animation.cancel()
        finishPanel(node, state.animation)
    }
}
function transitionWorkspace(change, animate = true) {
    const panels = [element('sidebar'), element('task-panel')]
    const previous = panels.map((node) => ({
        node,
        visible: !!node.getClientRects().length,
        rect: node.getBoundingClientRect(),
        opacity: getComputedStyle(node).opacity,
    }))
    const previousComposer = composerPosition()
    const scroll = element('message-scroll')
    const previousScroll = scroll.getBoundingClientRect()
    settlePanels()
    cancelContentMotion()
    change()
    if (!animate || reducedMotion.matches || disposed) return
    for (const before of previous) {
        const { node, rect } = before
        const visible = !!node.getClientRects().length
        if (!visible && !before.visible) continue
        const closing = !visible
        const nextRect = visible ? node.getBoundingClientRect() : rect
        const direction = node.id === 'sidebar' ? -1 : 1
        let from = before.visible
            ? rect.left - nextRect.left
            : direction * (window.innerWidth < 760 ? nextRect.width : 28)
        let to = 0
        if (!closing && before.visible && Math.abs(from) < 1 && Number(before.opacity) > 0.99)
            continue
        if (closing) {
            node.hidden = false
            Object.assign(node.style, {
                position: 'fixed',
                left: `${rect.left}px`,
                top: `${rect.top}px`,
                width: `${rect.width}px`,
                height: `${rect.height}px`,
                display: 'flex',
                zIndex: '15',
            })
            from = 0
            to = direction * (window.innerWidth < 760 ? rect.width : 36)
        }
        node.inert = closing
        node.dataset.motion = closing ? 'closing' : 'opening'
        const animation = node.animate(
            [
                {
                    transform: `translateX(${from}px)`,
                    opacity: before.visible ? before.opacity : 0,
                },
                { transform: `translateX(${to}px)`, opacity: closing ? 0 : 1 },
            ],
            { duration: closing ? 160 : 240, easing: motionCurve(), fill: 'both' },
        )
        panelAnimations.set(node, { animation, closing })
        animation.finished
            .then(() => {
                animation.cancel()
                finishPanel(node, animation)
            })
            .catch(() => {})
    }
    const nextScroll = scroll.getBoundingClientRect()
    const shift =
        previousScroll.left + previousScroll.width / 2 - nextScroll.left - nextScroll.width / 2
    if (Math.abs(shift) > 1)
        animateContent(
            scroll,
            [{ transform: `translateX(${shift}px)` }, { transform: 'translateX(0)' }],
            240,
        )
    if (
        document.querySelector('.composer-dock').parentElement ===
        document.querySelector('.conversation')
    )
        moveComposer(previousComposer)
}
reducedMotion.addEventListener('change', () => {
    if (reducedMotion.matches) {
        settlePanels()
        cancelContentMotion()
    }
})

function notify(message) {
    clearTimeout(noticeTimer)
    element('notification').textContent = message
    element('notification').hidden = false
    noticeTimer = setTimeout(() => {
        element('notification').hidden = true
    }, 5000)
}
function update(change, { renderView = true, motion = true } = {}) {
    const previousId = store.activeId
    const wasEmpty = !activeThread(store).messages.length
    const previousComposer = composerPosition()
    const next = structuredClone(store)
    change(next, activeThread(next))
    try {
        store = commitStore(next)
    } catch {
        notify(text('storageError'))
        return false
    }
    if (renderView) {
        render()
        if (
            motion &&
            (previousId !== store.activeId || wasEmpty !== !activeThread(store).messages.length)
        ) {
            moveComposer(previousComposer)
            animateContent(element('message-scroll'), [{ opacity: 0.55 }, { opacity: 1 }], 160)
        }
    }
    return true
}
function selectedModel(thread = activeThread(store)) {
    const provider = context.providers.find(
        (item) => item.provider_id === thread.selection?.provider_id,
    )
    const model = provider?.models.find((item) => item.id === thread.selection?.model_id)
    return model ? { provider, model } : null
}
function selectedInstance(thread = activeThread(store)) {
    return context.instances.find((item) => item.id === thread.instanceId)
}
function waitingMessages(thread = activeThread(store)) {
    return thread.messages.filter((message) => message.status === 'waiting')
}
function localize() {
    setLocale(window.AgentHost?.appearance?.locale || document.documentElement.lang)
    document.querySelectorAll('[data-text]').forEach((node) => {
        node.textContent = text(node.dataset.text)
    })
    document.querySelectorAll('[data-placeholder]').forEach((node) => {
        node.placeholder = text(node.dataset.placeholder)
    })
    document.querySelectorAll('[data-label]').forEach((node) => {
        node.setAttribute('aria-label', text(node.dataset.label))
        node.title = text(node.dataset.label)
    })
}
function renderList() {
    const visible = store.threads
        .filter((thread) => thread.messages.length || thread.draft || thread.title)
        .filter((thread) =>
            `${thread.title} ${thread.messages.map((message) => message.text).join(' ')}`
                .toLocaleLowerCase()
                .includes(search.toLocaleLowerCase()),
        )
        .sort((a, b) => b.updatedAt - a.updatedAt)
    const today = new Date().toDateString()
    let previousGroup
    element('conversation-list').innerHTML = visible.length
        ? visible
              .map((thread) => {
                  const group =
                      new Date(thread.updatedAt).toDateString() === today ? 'today' : 'earlier'
                  const heading =
                      group !== previousGroup ? `<div class="list-group">${text(group)}</div>` : ''
                  previousGroup = group
                  return `${heading}<button class="conversation-item${thread.id === store.activeId ? ' selected' : ''}" type="button" data-thread="${escape(thread.id)}" aria-current="${thread.id === store.activeId ? 'page' : 'false'}"><span class="thread-name">${escape(thread.title || text('untitled'))}</span>${waitingMessages(thread).length ? '<span class="pending-dot"></span>' : ''}</button>`
              })
              .join('')
        : `<p class="list-empty">${text(search ? 'noResults' : 'noChats')}</p>`
}
function renderMessages() {
    const thread = activeThread(store)
    const empty = !thread.messages.length
    const dock = document.querySelector('.composer-dock')
    const emptyState = element('empty-state')
    const conversation = document.querySelector('.conversation')
    conversation.classList.toggle('is-empty', empty)
    if (empty && dock.parentElement !== emptyState)
        emptyState.insertBefore(dock, emptyState.querySelector('.suggestions'))
    else if (!empty && dock.parentElement !== conversation) conversation.append(dock)
    element('conversation-title').hidden = empty
    element('empty-state').hidden = !empty
    element('messages').hidden = !thread.messages.length
    element('conversation-title').textContent = thread.title || text('untitled')
    element('conversation-more').hidden = !thread.messages.length && !thread.draft
    element('messages').innerHTML = thread.messages
        .map(
            (message) =>
                `<article class="message${displayedMessages.has(message.id) ? '' : ' message-arriving'}" data-message="${escape(message.id)}"><div class="user-message">${escape(message.text)}</div><div class="message-meta"><time>${dateLabel(message.createdAt)}</time><button class="icon-button copy-message" type="button" data-copy="${escape(message.id)}" title="${text('copy')}" aria-label="${text('copy')}">${icon('copy')}</button></div><div class="message-state${message.status === 'waiting' ? ' is-waiting' : ''}">${icon(message.status === 'waiting' ? 'clock' : 'stop')}<span>${text(message.status === 'waiting' ? 'waiting' : message.status === 'interrupted' ? 'interrupted' : 'stopped')}</span>${message.status === 'waiting' ? `<button type="button" class="text-button" data-action="toggle-tasks">${text('details')}</button><button type="button" class="text-button" data-cancel-request="${escape(message.id)}">${text('stop')}</button>` : ''}</div></article>`,
        )
        .join('')
}
function renderComposer() {
    const thread = activeThread(store)
    const model = selectedModel()
    const instance = selectedInstance()
    element('instance-label').textContent =
        instance?.name || (thread.instanceId ? text('contextRemoved') : text('allLauncher'))
    element('model-label').textContent =
        model?.model.name || (thread.selection ? text('modelRemoved') : text('noModel'))
    element('model-button').title = model
        ? `${model.provider.name} · ${model.model.name}`
        : text('model')
    element('selection-caption').textContent = model?.provider.name || ''
    const input = element('message-input')
    if (input.value !== thread.draft) input.value = thread.draft
    resizeInput()
    const button = element('send-button')
    button.innerHTML = icon('up')
    button.setAttribute('aria-label', text('send'))
    button.title = text('send')
    button.disabled = !thread.draft.trim()
}
function renderTasks() {
    const thread = activeThread(store)
    const pending = waitingMessages()
    const taskContext = pending.at(-1) || thread
    const model = selectedModel(taskContext)
    const instance = selectedInstance(taskContext)
    element('task-content').innerHTML =
        `<section class="task-section"><h3>${text('queued')}${pending.length ? `<span class="count">${pending.length}</span>` : ''}</h3>${pending.length ? pending.map((message) => `<div class="pending-request">${icon('clock')}<div><p>${escape(message.text)}</p><span>${text('executionDisconnected')}</span></div></div>`).join('') : `<p class="panel-empty">${text('queuedEmpty')}</p>`}</section><section class="task-section context-section"><h3>${text('context')}</h3><div class="instance-summary">${icon('cube')}<div><strong>${escape(instance?.name || (taskContext.instanceId ? text('contextRemoved') : text('allLauncher')))}</strong>${instance ? `<span>${escape([instance.loader, instance.game_version].filter(Boolean).join(' · '))}</span>` : ''}</div></div><dl><dt>${text('provider')}</dt><dd>${escape(model?.provider.name || text('unselected'))}</dd><dt>${text('modelLabel')}</dt><dd>${escape(model?.model.name || text('unselected'))}</dd></dl></section>`
}
function permissionsAvailable() {
    return (
        typeof window.AgentHost?.getPermissions === 'function' &&
        typeof window.AgentHost?.setPermissions === 'function'
    )
}
function renderSettings() {
    const available = permissionsAvailable()
    const busy = permissionsLoading || permissionsSaving
    element('permission-full-access').checked = permissionPolicy.full_access
    element('permission-full-access').disabled = !available || busy || permissionReadError
    element('permission-computer-use').checked = permissionPolicy.computer_use
    element('permission-computer-use').disabled =
        !available || busy || permissionReadError || !permissionPolicy.full_access
    element('permission-status').textContent = text(
        !available
            ? 'permissionUnavailable'
            : permissionReadError
              ? 'permissionError'
              : permissionsLoading
                ? 'permissionPending'
                : 'permissionFuture',
    )
    element('permissions-confirm').disabled = busy
}
async function refreshPermissions() {
    if (!permissionsAvailable() || permissionsSaving) {
        renderSettings()
        return
    }
    const request = ++permissionRequest
    permissionsLoading = true
    renderSettings()
    try {
        const policy = await window.AgentHost.getPermissions()
        if (request !== permissionRequest || disposed) return
        permissionPolicy = {
            full_access: policy.full_access === true,
            computer_use: policy.full_access === true && policy.computer_use === true,
        }
        permissionReadError = false
    } catch {
        if (request === permissionRequest) permissionReadError = true
    } finally {
        if (request === permissionRequest) {
            permissionsLoading = false
            renderSettings()
        }
    }
}
async function savePermissions(next) {
    if (!permissionsAvailable() || permissionsSaving || permissionsLoading) return
    ++permissionRequest
    permissionsSaving = true
    renderSettings()
    try {
        const policy = await window.AgentHost.setPermissions(next)
        if (disposed) return
        permissionPolicy = {
            full_access: policy.full_access === true,
            computer_use: policy.full_access === true && policy.computer_use === true,
        }
        permissionReadError = false
        element('permissions-risk-dialog').close()
    } catch {
        notify(text('permissionSaveError'))
    } finally {
        permissionsSaving = false
        renderSettings()
    }
}
function renderBanner() {
    const banner = element('context-banner')
    banner.hidden = !contextError && (context.ai_enabled || loading)
    banner.innerHTML = contextError
        ? `<span>${text('contextError')}</span><button class="text-button" type="button" data-action="refresh-context">${text('retry')}</button>`
        : `<span>${text('aiDisabled')}</span>`
}
function render() {
    localize()
    element('workspace').classList.toggle('sidebar-collapsed', !store.sidebarOpen)
    element('workspace').classList.toggle('tasks-open', taskOpen)
    element('sidebar-toggle').setAttribute('aria-expanded', String(store.sidebarOpen))
    element('task-toggle').setAttribute('aria-expanded', String(taskOpen))
    element('task-toggle').classList.toggle('is-active', taskOpen)
    element('task-panel').hidden = !taskOpen && !panelAnimations.has(element('task-panel'))
    if (!panelAnimations.has(element('task-panel'))) element('task-panel').inert = !taskOpen
    if (!panelAnimations.has(element('sidebar'))) element('sidebar').inert = !store.sidebarOpen
    if (firstRender) {
        const empty = element('empty-state')
        empty.classList.add('first-arrival')
        empty.addEventListener('animationend', () => empty.classList.remove('first-arrival'), {
            once: true,
        })
    }
    renderList()
    renderMessages()
    renderComposer()
    renderTasks()
    renderSettings()
    renderBanner()
    for (const message of activeThread(store).messages) displayedMessages.add(message.id)
    firstRender = false
}
function resizeInput() {
    const input = element('message-input')
    input.style.height = 'auto'
    input.style.height = `${Math.min(180, Math.max(76, input.scrollHeight))}px`
}
function refreshContext() {
    if (disposed || !window.AgentHost?.getContext) return Promise.resolve()
    if (contextFlight) return contextFlight
    const request = ++contextRequest
    loading = true
    renderBanner()
    contextFlight = (async () => {
        try {
            const next = structuredClone(await window.AgentHost.getContext())
            if (request !== contextRequest || disposed) return
            const normalized = {
                ai_enabled: next.ai_enabled === true,
                providers: Array.isArray(next.providers)
                    ? next.providers.map((provider) => ({
                          ...provider,
                          models: Array.isArray(provider.models) ? provider.models : [],
                      }))
                    : [],
                instances: Array.isArray(next.instances) ? next.instances : [],
                execution_available: false,
            }
            const changed = JSON.stringify(context) !== JSON.stringify(normalized)
            context = normalized
            contextError = false
            if (changed) {
                renderComposer()
                renderTasks()
                if (element('picker').matches(':popover-open')) {
                    const list = element('picker-options')
                    const focused = document.activeElement
                    const selected = focused?.matches('[data-model]')
                        ? { provider: focused.dataset.provider, model: focused.dataset.model }
                        : focused?.matches('[data-instance]')
                          ? { instance: focused.dataset.instance }
                          : null
                    const scrollTop = list.scrollTop
                    renderPicker()
                    list.scrollTop = scrollTop
                    if (selected) {
                        const options = [...list.querySelectorAll('button')]
                        const restored = options.find((option) =>
                            selected.model !== undefined
                                ? option.dataset.provider === selected.provider &&
                                  option.dataset.model === selected.model
                                : option.dataset.instance === selected.instance,
                        )
                        ;(restored || element('picker-search')).focus({ preventScroll: true })
                    }
                }
            }
        } catch {
            if (request === contextRequest && !disposed) contextError = true
        } finally {
            if (request === contextRequest && !disposed) {
                loading = false
                renderBanner()
            }
        }
    })().finally(() => {
        contextFlight = null
    })
    return contextFlight
}
function scheduleRefresh() {
    clearTimeout(refreshTimer)
    if (document.hidden || disposed) return
    refreshTimer = setTimeout(async () => {
        await refreshContext()
        scheduleRefresh()
    }, 5000)
}
function openPicker(kind) {
    refreshContext()
    const picker = element('picker')
    if (picker.matches(':popover-open')) picker.hidePopover()
    pickerKind = kind
    const searchInput = element('picker-search')
    searchInput.value = ''
    searchInput.placeholder = text(kind === 'model' ? 'modelSearch' : 'instanceSearch')
    searchInput.setAttribute('aria-label', searchInput.placeholder)
    renderPicker()
    const trigger = element(kind === 'model' ? 'model-button' : 'instance-button')
    const rect = trigger.getBoundingClientRect()
    const width = Math.min(kind === 'model' ? 340 : 320, window.innerWidth - 24)
    picker.style.transformOrigin = kind === 'model' ? 'bottom right' : 'bottom left'
    picker.style.width = `${width}px`
    const targetLeft = kind === 'model' ? rect.right - width : rect.left
    picker.style.left = `${Math.max(12, Math.min(targetLeft, window.innerWidth - width - 12))}px`
    picker.style.top = 'auto'
    picker.style.bottom = `${window.innerHeight - rect.top + 8}px`
    picker.style.maxHeight = `${Math.max(150, rect.top - 24)}px`
    picker.showPopover()
    searchInput.focus()
}
function renderPicker() {
    const query = element('picker-search').value.toLocaleLowerCase()
    const thread = activeThread(store)
    let html = ''
    if (pickerKind === 'model') {
        if (!context.ai_enabled)
            html = `<div class="picker-empty"><strong>${text('aiDisabled')}</strong><p>${text('aiDisabledHint')}</p></div>`
        else
            for (const provider of context.providers) {
                const models = provider.models.filter((model) =>
                    `${provider.name} ${model.name} ${model.id}`
                        .toLocaleLowerCase()
                        .includes(query),
                )
                if (!models.length) continue
                html += `<div class="picker-group">${escape(provider.name)}</div>${models.map((model) => `<button class="picker-option" type="button" data-provider="${escape(provider.provider_id)}" data-model="${escape(model.id)}"><span><strong>${escape(model.name)}</strong><small>${escape(model.id)}</small></span>${thread.selection?.provider_id === provider.provider_id && thread.selection?.model_id === model.id ? icon('check') : ''}</button>`).join('')}`
            }
        if (!html)
            html = `<div class="picker-empty"><strong>${text(query ? 'noModelMatches' : 'noModels')}</strong>${query ? '' : `<p>${text('configureModel')}</p>`}</div>`
    } else {
        if (!query || text('allLauncher').toLocaleLowerCase().includes(query))
            html = `<button class="picker-option" type="button" data-instance="">${icon('cube')}<span><strong>${text('allLauncher')}</strong><small>${text('allLauncherHint')}</small></span>${!thread.instanceId ? icon('check') : ''}</button>`
        const instances = context.instances.filter((instance) =>
            `${instance.name} ${instance.game_version} ${instance.loader}`
                .toLocaleLowerCase()
                .includes(query),
        )
        html += instances
            .map(
                (instance) =>
                    `<button class="picker-option" type="button" data-instance="${escape(instance.id)}">${icon('cube')}<span><strong>${escape(instance.name)}</strong><small>${escape([instance.loader, instance.game_version].filter(Boolean).join(' · '))}</small></span>${thread.instanceId === instance.id ? icon('check') : ''}</button>`,
            )
            .join('')
        if (!instances.length)
            html += `<div class="picker-empty">${text(query ? 'noInstanceMatches' : 'noInstances')}</div>`
    }
    element('picker-options').innerHTML = html
}
function stopWaiting(messageId) {
    update((next, thread) => {
        for (const message of thread.messages)
            if (message.id === messageId && message.status === 'waiting') message.status = 'stopped'
    })
    const message = [...element('messages').querySelectorAll('[data-message]')].find(
        (node) => node.dataset.message === messageId,
    )
    if (message)
        animateContent(
            message.querySelector('.message-state'),
            [
                { opacity: 0.35, transform: 'translateY(2px)' },
                { opacity: 1, transform: 'translateY(0)' },
            ],
            160,
        )
}
function submit() {
    const input = element('message-input')
    const value = input.value.trim()
    if (!value) return
    if (!context.ai_enabled || !selectedModel()) {
        openPicker('model')
        return
    }
    const succeeded = update((next, thread) => {
        thread.messages.push({
            id: crypto.randomUUID(),
            role: 'user',
            text: value,
            createdAt: Date.now(),
            status: 'waiting',
            instanceId: thread.instanceId,
            selection: structuredClone(thread.selection),
        })
        if (!thread.title) thread.title = value.slice(0, 48)
        thread.updatedAt = Date.now()
        thread.draft = ''
    })
    if (succeeded) element('message-scroll').scrollTop = element('message-scroll').scrollHeight
}
function createThread(motion = true) {
    if (
        !activeThread(store).messages.length &&
        !activeThread(store).draft &&
        !activeThread(store).title
    ) {
        element('message-input').focus()
        return
    }
    if (
        update(
            (next) => {
                const thread = newThread()
                thread.selection = structuredClone(activeThread(next).selection)
                next.threads.push(thread)
                next.activeId = thread.id
            },
            { motion },
        )
    )
        element('message-input').focus()
}
function openEdit(kind) {
    element('conversation-menu').hidePopover()
    editKind = kind
    element('edit-title').textContent = text(kind === 'rename' ? 'rename' : 'deleteTitle')
    element('rename-field').hidden = kind !== 'rename'
    element('rename-input').required = kind === 'rename'
    element('delete-description').hidden = kind !== 'delete'
    element('edit-confirm').textContent = text(kind === 'rename' ? 'save' : 'delete')
    element('edit-confirm').classList.toggle('destructive', kind === 'delete')
    element('rename-input').value = activeThread(store).title || text('untitled')
    element('edit-dialog').showModal()
    if (kind === 'rename') {
        element('rename-input').focus()
        element('rename-input').select()
    }
}
const actions = {
    'toggle-sidebar': (event) =>
        transitionWorkspace(
            () =>
                update((next) => {
                    next.sidebarOpen = !next.sidebarOpen
                }),
            event?.detail !== 0,
        ),
    'new-chat': (event) => createThread(event?.detail !== 0),
    settings: () => {
        renderSettings()
        element('settings-dialog').showModal()
        refreshPermissions()
    },
    'toggle-tasks': (event) =>
        transitionWorkspace(() => {
            taskOpen = !taskOpen
            render()
        }, event?.detail !== 0),
    'model-picker': () => openPicker('model'),
    'instance-picker': () => openPicker('instance'),
    'close-dialog': () =>
        document.querySelectorAll('dialog[open]').forEach((dialog) => dialog.close()),
    'refresh-context': refreshContext,
    'cancel-full-access': () => element('permissions-risk-dialog').close(),
    'confirm-full-access': () => savePermissions({ full_access: true, computer_use: false }),
    rename: () => openEdit('rename'),
    delete: () => openEdit('delete'),
    'conversation-menu': () => {
        const rect = element('conversation-more').getBoundingClientRect()
        const menu = element('conversation-menu')
        menu.style.transformOrigin = 'top right'
        menu.style.left = `${Math.max(12, rect.right - 210)}px`
        menu.style.top = `${rect.bottom + 6}px`
        menu.togglePopover()
    },
}
document.addEventListener('click', async (event) => {
    const cancel = event.target.closest('[data-cancel-request]')
    if (cancel) {
        stopWaiting(cancel.dataset.cancelRequest)
        return
    }
    const action = event.target.closest('[data-action]')
    if (action) {
        actions[action.dataset.action]?.(event)
        return
    }
    const suggestion = event.target.closest('[data-prompt]')
    if (suggestion) {
        const value = text(suggestion.dataset.prompt)
        if (
            update((next, thread) => {
                thread.draft = value
            })
        )
            element('message-input').focus()
        return
    }
    const item = event.target.closest('[data-thread]')
    if (item) {
        update((next) => {
            next.activeId = item.dataset.thread
            if (window.innerWidth < 760) next.sidebarOpen = false
        })
        return
    }
    const model = event.target.closest('[data-model]')
    if (model) {
        if (
            update((next, thread) => {
                thread.selection = {
                    provider_id: model.dataset.provider,
                    model_id: model.dataset.model,
                }
            })
        )
            element('picker').hidePopover()
        return
    }
    const instance = event.target.closest('[data-instance]')
    if (instance) {
        if (
            update((next, thread) => {
                thread.instanceId = instance.dataset.instance || null
            })
        )
            element('picker').hidePopover()
        return
    }
    const copy = event.target.closest('[data-copy]')
    if (copy) {
        try {
            await navigator.clipboard.writeText(
                activeThread(store).messages.find((message) => message.id === copy.dataset.copy)
                    .text,
            )
            notify(text('copied'))
        } catch {
            notify(text('copyError'))
        }
    }
})
element('permission-full-access').addEventListener('change', (event) => {
    const enabling = event.target.checked
    renderSettings()
    if (enabling) element('permissions-risk-dialog').showModal()
    else savePermissions({ full_access: false, computer_use: false })
})
element('permission-computer-use').addEventListener('change', (event) => {
    const enabling = event.target.checked
    renderSettings()
    if (permissionPolicy.full_access) savePermissions({ full_access: true, computer_use: enabling })
})
element('composer').addEventListener('submit', (event) => {
    event.preventDefault()
    submit()
})
element('message-input').addEventListener('input', (event) => {
    const value = event.target.value
    // Keep unsaved text in memory when storage is unavailable; sending is transactional.
    activeThread(store).draft = value
    try {
        saveStore(store)
    } catch {
        notify(text('storageError'))
    }
    renderComposer()
    renderList()
})
element('message-input').addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
        event.preventDefault()
        submit()
    }
})
element('conversation-search').addEventListener('input', (event) => {
    search = event.target.value
    renderList()
})
element('picker-search').addEventListener('input', renderPicker)
element('edit-form').addEventListener('submit', (event) => {
    event.preventDefault()
    if (editKind === 'rename') {
        const value = element('rename-input').value.trim()
        if (
            value &&
            update((next, thread) => {
                thread.title = value
            })
        )
            element('edit-dialog').close()
    } else if (
        update((next) => {
            next.threads = next.threads.filter((thread) => thread.id !== next.activeId)
            if (!next.threads.length) next.threads.push(newThread())
            next.activeId = next.threads[0].id
        })
    )
        element('edit-dialog').close()
})
document.querySelectorAll('dialog').forEach((dialog) =>
    dialog.addEventListener('click', (event) => {
        if (event.target === dialog) {
            const rect = dialog.getBoundingClientRect()
            if (
                event.clientX < rect.left ||
                event.clientX > rect.right ||
                event.clientY < rect.top ||
                event.clientY > rect.bottom
            )
                dialog.close()
        }
    }),
)
document.addEventListener('keydown', (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'n') {
        event.preventDefault()
        createThread(false)
    }
    if (
        event.key === 'Escape' &&
        !document.querySelector('dialog[open]') &&
        !element('picker').matches(':popover-open') &&
        !element('conversation-menu').matches(':popover-open')
    ) {
        if (taskOpen)
            transitionWorkspace(() => {
                taskOpen = false
                render()
            }, false)
        else if (window.innerWidth < 760 && store.sidebarOpen)
            transitionWorkspace(
                () =>
                    update(
                        (next) => {
                            next.sidebarOpen = false
                        },
                        { motion: false },
                    ),
                false,
            )
    }
})
let booted = false
async function boot() {
    if (booted) return
    booted = true
    await window.AgentHost?.ready
    render()
    document.documentElement.dataset.ready = 'true'
    await window.AgentHost?.reveal?.()
    refreshContext()
    scheduleRefresh()
}
window.addEventListener('agent-host-ready', boot)
window.addEventListener('agent-appearance-updated', () => {
    if (booted) render()
})
window.addEventListener('focus', () => {
    refreshContext()
    scheduleRefresh()
})
document.addEventListener('visibilitychange', () => {
    if (!document.hidden) refreshContext()
    scheduleRefresh()
})
window.addEventListener('pagehide', () => {
    settlePanels()
    cancelContentMotion()
    disposed = true
    ++contextRequest
    ++permissionRequest
    clearTimeout(refreshTimer)
    clearTimeout(noticeTimer)
})
window.addEventListener('pageshow', (event) => {
    if (event.persisted) {
        disposed = false
        refreshContext()
        scheduleRefresh()
    }
})
window.addEventListener('resize', () => {
    settlePanels()
    cancelContentMotion()
    render()
    if (element('picker').matches(':popover-open')) element('picker').hidePopover()
})
await boot()
if (loadError) notify(text('storageLoadError'))
