export const STORAGE_KEY = 'axolotl.agent-window.conversations.v1'
const MAX_TEXT = 50000
const MAX_STORE_LENGTH = 1_000_000
const MAX_THREADS = 100
const MAX_MESSAGES = 1000

export function newThread() {
    return {
        id: crypto.randomUUID(),
        title: '',
        createdAt: Date.now(),
        updatedAt: Date.now(),
        draft: '',
        instanceId: null,
        selection: null,
        messages: [],
    }
}
export function initialStore() {
    const thread = newThread()
    return { version: 1, activeId: thread.id, sidebarOpen: true, threads: [thread] }
}
export function loadStore(storage = localStorage) {
    const raw = storage.getItem(STORAGE_KEY)
    if (!raw) return initialStore()
    const value = JSON.parse(raw)
    if (value.version !== 1 || !Array.isArray(value.threads))
        throw new Error('Invalid conversation store')
    const ids = new Set()
    const messageIds = new Set()
    const threads = value.threads
        .filter(
            (thread) =>
                thread &&
                typeof thread.id === 'string' &&
                !ids.has(thread.id) &&
                ids.add(thread.id),
        )
        .map((thread) => ({
            id: thread.id,
            title: String(thread.title || '').slice(0, 120),
            createdAt: Number(thread.createdAt) || Date.now(),
            updatedAt: Number(thread.updatedAt) || Date.now(),
            draft: String(thread.draft || '').slice(0, MAX_TEXT),
            instanceId: typeof thread.instanceId === 'string' ? thread.instanceId : null,
            selection:
                thread.selection &&
                typeof thread.selection.provider_id === 'string' &&
                typeof thread.selection.model_id === 'string'
                    ? {
                          provider_id: thread.selection.provider_id,
                          model_id: thread.selection.model_id,
                      }
                    : null,
            messages: Array.isArray(thread.messages)
                ? thread.messages
                      .filter(
                          (message) =>
                              message &&
                              typeof message.id === 'string' &&
                              typeof message.text === 'string' &&
                              !messageIds.has(message.id) &&
                              messageIds.add(message.id),
                      )
                      .map((message) => ({
                          id: message.id,
                          role: 'user',
                          text: message.text.slice(0, MAX_TEXT),
                          createdAt: Number(message.createdAt) || Date.now(),
                          status:
                              message.status === 'waiting'
                                  ? 'interrupted'
                                  : ['stopped', 'interrupted'].includes(message.status)
                                    ? message.status
                                    : 'stopped',
                          instanceId:
                              typeof message.instanceId === 'string' ? message.instanceId : null,
                          selection:
                              message.selection &&
                              typeof message.selection.provider_id === 'string' &&
                              typeof message.selection.model_id === 'string'
                                  ? {
                                        provider_id: message.selection.provider_id,
                                        model_id: message.selection.model_id,
                                    }
                                  : null,
                      }))
                : [],
        }))
    if (!threads.length) return initialStore()
    return {
        version: 1,
        activeId: threads.some((thread) => thread.id === value.activeId)
            ? value.activeId
            : threads[0].id,
        sidebarOpen: value.sidebarOpen !== false,
        threads,
    }
}
export function saveStore(store, storage = localStorage) {
    const next = structuredClone(store)
    const removableThreads = next.threads
        .filter(
            (thread) =>
                thread.id !== next.activeId &&
                !thread.draft &&
                !thread.messages.some((message) => message.status === 'waiting'),
        )
        .sort((a, b) => a.updatedAt - b.updatedAt)
    while (next.threads.length > MAX_THREADS && removableThreads.length) {
        const oldest = removableThreads.shift()
        next.threads = next.threads.filter((thread) => thread.id !== oldest.id)
    }
    const history = next.threads
        .flatMap((thread) =>
            thread.messages
                .filter((message) => message.status !== 'waiting')
                .map((message) => ({ thread, message })),
        )
        .sort((a, b) => a.message.createdAt - b.message.createdAt)
    let count = next.threads.reduce((total, thread) => total + thread.messages.length, 0)
    let serialized = JSON.stringify(next)
    while ((serialized.length > MAX_STORE_LENGTH || count > MAX_MESSAGES) && history.length) {
        const { thread, message } = history.shift()
        thread.messages = thread.messages.filter((item) => item.id !== message.id)
        count--
        serialized = JSON.stringify(next)
    }
    if (
        serialized.length > MAX_STORE_LENGTH ||
        count > MAX_MESSAGES ||
        next.threads.length > MAX_THREADS
    )
        throw new Error('Conversation storage limit reached')
    storage.setItem(STORAGE_KEY, serialized)
    return next
}
export function activeThread(store) {
    return store.threads.find((thread) => thread.id === store.activeId)
}
export function commitStore(next, storage = localStorage) {
    return saveStore(next, storage)
}
