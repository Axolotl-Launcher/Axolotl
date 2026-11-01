// Live state shared by the controller and the views. Only `store` is persisted.
import { activeThread } from './state.js'

export const session = {
    store: null,
    loadError: false,
    storageError: false,
    context: { ai_enabled: false, providers: [], instances: [], execution_available: false },
    contextLoaded: false,
    contextError: false,
    runtime: null,
    permissions: { full_access: false, computer_use: false },
    permissionsLoading: false,
    permissionsSaving: false,
    permissionReadError: false,
    search: '',
    handles: new Map(),
}

export function currentThread() {
    return activeThread(session.store)
}

export function findThread(id) {
    return session.store.threads.find((thread) => thread.id === id) || null
}

export function findTurn(threadId, turnId) {
    const thread = findThread(threadId)
    const turn = thread?.turns.find((entry) => entry.id === turnId)
    return turn ? { thread, turn } : null
}

export function modelChoice(selection) {
    if (!selection) return null
    const provider = session.context.providers.find(
        (entry) => entry.provider_id === selection.provider_id,
    )
    const model = provider?.models.find((entry) => entry.id === selection.model_id)
    return model ? { provider, model } : null
}

export function instanceChoice(id) {
    return id ? session.context.instances.find((entry) => entry.id === id) || null : null
}

export function firstModel() {
    for (const provider of session.context.providers)
        if (provider.models.length)
            return { provider_id: provider.provider_id, model_id: provider.models[0].id }
    return null
}

export function modelsAvailable() {
    return session.context.ai_enabled && session.context.providers.some((p) => p.models.length)
}

export function isPreview() {
    return session.runtime?.preview === true
}
