;(() => {
    // This isolated entry has no bundler or global Tauri API. Keep the same
    // transport used by @tauri-apps/api/core.invoke in one replaceable bridge.
    const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args)
    const agentInvoke = (command, args) => invoke(`plugin:agent-window|${command}`, args)
    const minimize = document.getElementById('minimize')
    const maximize = document.getElementById('maximize')
    const close = document.getElementById('close')
    const errorBox = document.getElementById('window-error')
    let appearance
    let maximized = false
    let actionPending = false
    let stateRequest = 0

    function showError(error) {
        const prefix = appearance?.locale?.startsWith('zh')
            ? '窗口操作失败'
            : 'Window action failed'
        errorBox.textContent = `${prefix}: ${String(error)}`
        errorBox.hidden = false
        console.error(error)
    }

    function updateMaximizeLabel() {
        const labels = appearance?.labels
        const label = maximized ? labels?.restore || 'Restore' : labels?.maximize || 'Maximize'
        maximize.setAttribute('aria-label', label)
        maximize.title = label
        document.getElementById('maximize-icon').hidden = maximized
        document.getElementById('restore-icon').hidden = !maximized
    }

    async function refreshState() {
        const request = ++stateRequest
        try {
            const value = await agentInvoke('agent_window_is_maximized')
            if (request !== stateRequest) return
            maximized = value
            updateMaximizeLabel()
        } catch (error) {
            showError(error)
        }
    }

    async function refreshAppearance() {
        try {
            appearance = await agentInvoke('agent_window_get_appearance')
            document.documentElement.lang = appearance.locale
            for (const key of ['background', 'surface', 'text', 'accent']) {
                if (CSS.supports('color', appearance[key])) {
                    document.documentElement.style.setProperty(`--agent-${key}`, appearance[key])
                }
            }
            document.title = appearance.labels.title
            document.getElementById('window-title').textContent = appearance.labels.title
            for (const [button, name] of [
                [minimize, 'minimize'],
                [close, 'close'],
            ]) {
                button.setAttribute('aria-label', appearance.labels[name])
                button.title = appearance.labels[name]
            }
            updateMaximizeLabel()
        } catch (error) {
            showError(error)
        }
    }

    async function action(name) {
        if (actionPending) return
        actionPending = true
        errorBox.hidden = true
        for (const button of [minimize, maximize, close]) button.disabled = true
        try {
            await agentInvoke('agent_window_action', { action: name })
            if (name === 'toggle_maximize') await refreshState()
        } catch (error) {
            showError(error)
        } finally {
            actionPending = false
            for (const button of [minimize, maximize, close]) button.disabled = false
        }
    }

    minimize.addEventListener('click', () => action('minimize'))
    maximize.addEventListener('click', () => action('toggle_maximize'))
    close.addEventListener('click', () => action('close'))
    document.getElementById('drag-region').addEventListener('mousedown', (event) => {
        if (event.button === 0 && event.detail === 1) action('start_dragging')
    })
    document
        .getElementById('drag-region')
        .addEventListener('dblclick', () => action('toggle_maximize'))
    window.addEventListener('resize', refreshState)
    window.addEventListener('focus', refreshState)
    window.addEventListener('agent-appearance-changed', refreshAppearance)
    refreshAppearance()
    refreshState()
})()
