import type { ContentInstallCallback } from './content-install-types'

/**
 * Tracks the active install modal callback so a newer session can settle or
 * drop the previous one.
 */
export function createInstallSession() {
    let currentCallback: ContentInstallCallback = () => {}
    let currentSessionId = 0
    let currentCallbackSettled = true

    function beginInstallSession(callback: ContentInstallCallback) {
        if (!currentCallbackSettled) currentCallback()
        currentSessionId += 1
        currentCallback = callback
        currentCallbackSettled = false
        return currentSessionId
    }

    function settleCurrentCallback(...args: Parameters<ContentInstallCallback>) {
        if (currentCallbackSettled) return
        currentCallbackSettled = true
        currentCallback(...args)
    }

    function settleInstallSession(sessionId: number, ...args: Parameters<ContentInstallCallback>) {
        if (sessionId !== currentSessionId) return
        settleCurrentCallback(...args)
    }

    function cancelCurrentSession() {
        settleCurrentCallback()
        currentSessionId += 1
    }

    /** Current session counter (used to detect whether a request opened a modal). */
    function currentId() {
        return currentSessionId
    }

    function setCallback(callback: ContentInstallCallback) {
        currentCallback = callback
    }

    function getCallback() {
        return currentCallback
    }

    return {
        beginInstallSession,
        settleCurrentCallback,
        settleInstallSession,
        cancelCurrentSession,
        currentId,
        setCallback,
        getCallback,
    }
}
