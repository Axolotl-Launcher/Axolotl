// Runs before module loading, including startup failures and subsequent reloads.
;(() => {
    const key = 'axolotl-e2e-errors'
    const errors = JSON.parse(sessionStorage.getItem(key) ?? '[]')
    window.__e2eErrors = errors
    const warningKey = 'axolotl-e2e-browser-warnings'
    const warnings = JSON.parse(sessionStorage.getItem(warningKey) ?? '[]')
    window.__e2eBrowserWarnings = warnings
    const describe = (value) => {
        if (value instanceof Error) return value.stack ?? value.message
        if (typeof value === 'string') return value
        try {
            return JSON.stringify(value)
        } catch {
            return String(value)
        }
    }
    const record = (kind, values) => {
        errors.push(`${kind}: ${values.map(describe).join(' ')}`)
        sessionStorage.setItem(key, JSON.stringify(errors))
    }
    const originalError = console.error.bind(console)
    console.error = (...values) => {
        record('console.error', values)
        originalError(...values)
    }
    window.addEventListener('error', (event) => {
        if (!(event instanceof ErrorEvent)) return
        // Blink reports deferred observer deliveries as an ErrorEvent without an
        // exception. It is not a failed application action; real exceptions,
        // including ones with this same message, still fail the suite.
        if (
            event.error == null &&
            event.message === 'ResizeObserver loop completed with undelivered notifications.'
        ) {
            warnings.push(event.message)
            sessionStorage.setItem(warningKey, JSON.stringify(warnings))
            return
        }
        record('error', [event.error ?? event.message])
    })
    window.addEventListener('unhandledrejection', (event) => {
        record('unhandledrejection', [event.reason])
    })
})()
