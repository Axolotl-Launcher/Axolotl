// Conversation history kept on this device. Everything read back from storage is
// rebuilt field by field, so a damaged or hand-edited entry cannot inject markup,
// unexpected types or a run that silently resumes.
export const STORAGE_KEY = 'axolotl.agent-window.store.v2'
export const LEGACY_STORAGE_KEY = 'axolotl.agent-window.conversations.v1'
const MAX_TEXT = 50000
const MAX_STORE_LENGTH = 1_500_000
export const MAX_NATIVE_STORE_BYTES = 16 * 1024 * 1024
const MAX_THREADS = 100
const MAX_TURNS = 600
export const MAX_ATTACHMENTS = 10
// A long paste (a crash report, a log) is kept as an attachment with its text.
export const MAX_PASTE = 200_000
const PASTE_KINDS = ['paste', 'crash', 'log']

export const ACTIVE_STATUSES = ['thinking', 'running', 'applying']
const RUN_STATUSES = [
    'thinking',
    'running',
    'awaiting',
    'applying',
    'done',
    'failed',
    'stopped',
    'detached',
    'declined',
    'superseded',
    'interrupted',
]
const PLAN_STATES = [
    'pending',
    'applying',
    'applied',
    'declined',
    'superseded',
    'stopped',
    'unknown',
]
const STEP_STATES = ['running', 'done', 'failed', 'stopped']
// How much of the agent's work the transcript shows.
const DETAIL_LEVELS = ['concise', 'balanced', 'detailed']
export const MAX_INSTRUCTIONS = 2000
const CHANGE_KINDS = ['update', 'rollback', 'new', 'install', 'setting', 'backup', 'remove']

/**
 * @typedef {{ provider_id: string, model_id: string }} Selection
 * @typedef {{
 *   id: string, name: string, size: number, kind?: string, text?: string, lines?: number,
 * }} Attachment
 * @typedef {{ id: string, type: string, [field: string]: any }} Item
 * @typedef {{ title: string, detail: string, status: string }} ProgressStep
 * @typedef {{
 *   status: string, startedAt: number | null, endedAt: number | null, phaseAt?: number,
 *   label: string, items: Item[], progress: ProgressStep[], resume: any, preview: boolean,
 * }} Run
 * @typedef {{
 *   id: string, createdAt: number, text: string, attachments: Attachment[],
 *   instanceId: string | null, selection: Selection | null, run: Run, marked?: boolean,
 * }} Turn
 * @typedef {{ id: string, text: string, attachments: Attachment[], createdAt: number }} Queued
 * @typedef {{
 *   id: string, title: string, titleEdited: boolean, createdAt: number, updatedAt: number,
 *   pinned?: boolean, rank?: number, unread?: boolean, draft: string, attachments: Attachment[], instanceId: string | null,
 *   selection: Selection | null, turns: Turn[], queue: Queued[],
 * }} Thread
 * @typedef {{
 *   version: 2, activeId: string, sidebarOpen: boolean, panelOpen: boolean,
 *   introDismissed: boolean, tourDone: boolean, lastSelection: Selection | null,
 *   threads: Thread[], detail: string, instructions: string,
 * }} Store
 * @typedef {Pick<Storage, 'getItem' | 'setItem'>} StorageArea
 */

const id = () => crypto.randomUUID()
const time = (value) => (Number.isFinite(value) && value > 0 ? value : null)
const oneOf = (value, options, fallback) => (options.includes(value) ? value : fallback)

/** @param {Selection | null} [selection] @returns {Thread} */
export function newThread(selection = null) {
    const now = Date.now()
    return {
        id: id(),
        title: '',
        titleEdited: false,
        createdAt: now,
        updatedAt: now,
        draft: '',
        attachments: [],
        instanceId: null,
        selection,
        turns: [],
        queue: [],
    }
}

/** @param {Thread} thread @param {{ text: string, attachments?: Attachment[] }} message @returns {Turn} */
export function newTurn(thread, message) {
    return {
        id: id(),
        createdAt: Date.now(),
        text: message.text,
        attachments: structuredClone(message.attachments || []),
        instanceId: thread.instanceId,
        selection: thread.selection ? structuredClone(thread.selection) : null,
        run: newRun(),
    }
}

// A native follow-up is admitted to the backend as soon as its stable identity
// and captured task context have been acknowledged by workspace storage.
export function newNativeQueued(thread, message, instructions, computerUse) {
    return { ...newTurn(thread, message), native: true, instructions, computerUse }
}

export function adoptNativeQueued(thread, queuedId) {
    const entry = thread.queue.find((queued) => queued.id === queuedId)
    const existing = thread.turns.find((turn) => turn.id === queuedId)
    if (!entry) return existing || null
    const turn = existing || {
        id: entry.id,
        createdAt: entry.createdAt,
        text: entry.text,
        attachments: entry.attachments,
        instanceId: entry.instanceId,
        selection: entry.selection,
        run: entry.run,
    }
    if (!existing) {
        thread.turns.push(turn)
        thread.turns.sort((left, right) => left.createdAt - right.createdAt)
    }
    thread.queue = thread.queue.filter((queued) => queued.id !== queuedId)
    return turn
}

/** @returns {Run} */
export function newRun() {
    return {
        status: 'thinking',
        startedAt: Date.now(),
        endedAt: null,
        label: '',
        items: [],
        progress: [],
        resume: { attempt_id: id(), cursor: 0 },
        preview: false,
    }
}

/** @returns {Store} */
export function initialStore() {
    const thread = newThread()
    return {
        version: 2,
        activeId: thread.id,
        sidebarOpen: true,
        panelOpen: false,
        introDismissed: false,
        tourDone: false,
        lastSelection: null,
        threads: [thread],
        detail: 'balanced',
        instructions: '',
    }
}

function createStoreParser(native) {
    const string = (value, max = 500) =>
        typeof value === 'string' ? (native ? value : value.slice(0, max)) : ''
    const optionalString = (value, max = 500) =>
        typeof value === 'string' && value ? string(value, max) : null
    const list = (value, map, max = 200) => {
        if (!Array.isArray(value)) return []
        const mapped = (native ? value : value.slice(0, max)).map(map)
        if (native && mapped.some((entry) => entry === null || entry === undefined))
            throw new Error('Invalid native conversation record')
        return native ? mapped : mapped.filter(Boolean)
    }
    function selection(value) {
        return value && typeof value.provider_id === 'string' && typeof value.model_id === 'string'
            ? { provider_id: string(value.provider_id, 200), model_id: string(value.model_id, 200) }
            : null
    }

    function attachment(value) {
        if (!value || typeof value.name !== 'string') return null
        const pasted = PASTE_KINDS.includes(value.kind) && typeof value.text === 'string'
        return {
            id: typeof value.id === 'string' ? string(value.id, 100) : id(),
            name: string(value.name, 260),
            size: Number.isFinite(value.size) && value.size >= 0 ? value.size : 0,
            ...(pasted && {
                kind: value.kind,
                text: string(value.text, MAX_PASTE),
                lines: Number.isInteger(value.lines) && value.lines > 0 ? value.lines : 1,
            }),
        }
    }

    function step(value) {
        if (!value || typeof value.title !== 'string') return null
        return {
            id: string(value.id, 100) || id(),
            title: string(value.title),
            detail: string(value.detail, 2000),
            tool: optionalString(value.tool, 120),
            status: oneOf(value.status, STEP_STATES, 'done'),
            startedAt: time(value.startedAt),
            endedAt: time(value.endedAt),
        }
    }

    function change(value) {
        if (!value || typeof value.name !== 'string') return null
        return {
            name: string(value.name, 200),
            note: string(value.note, 8192),
            from: optionalString(value.from, 100),
            to: optionalString(value.to, 100),
            glyph: optionalString(value.glyph, 3),
            icon: optionalString(value.icon, 40),
            kind: oneOf(value.kind, CHANGE_KINDS, null),
            meta: string(value.meta, 200),
            tool: optionalString(value.tool, 200),
            arguments:
                value.arguments && typeof value.arguments === 'object'
                    ? structuredClone(value.arguments)
                    : null,
            review: installationReview(value.review),
            manual: value.manual === true,
        }
    }

    function installationReview(value) {
        if (value == null) return null
        const valid =
            value &&
            typeof value === 'object' &&
            !Array.isArray(value) &&
            ((value.mode === 'resources' &&
                Array.isArray(value.actions) &&
                value.actions.every(
                    (action) =>
                        action &&
                        typeof action.title === 'string' &&
                        typeof action.target_path === 'string',
                )) ||
                (value.mode === 'modpack' && value.pack && typeof value.pack === 'object'))
        if (!valid) {
            if (native) throw new Error('Invalid installation review')
            return null
        }
        return structuredClone(value)
    }

    function plan(value) {
        if (!value || typeof value !== 'object') return null
        return {
            id: string(value.id, 100) || id(),
            revision: Number.isInteger(value.revision) ? value.revision : 1,
            expires_at: value.expires_at,
            title: string(value.title, 200),
            title_key:
                value.title === 'Review launcher changes'
                    ? 'reviewLauncherChanges'
                    : optionalString(value.title_key, 100),
            summary: string(value.summary, 200),
            approval: string(value.approval, 400),
            changes: list(value.changes, change, 50),
            notes: list(
                value.notes,
                (note) => (typeof note === 'string' ? string(note, 200) : null),
                6,
            ),
            scope: list(
                value.scope,
                (entry) =>
                    entry && typeof entry.label === 'string'
                        ? {
                              label: string(entry.label, 100),
                              count: Number.isInteger(entry.count) ? Math.max(0, entry.count) : 0,
                          }
                        : null,
                8,
            ),
        }
    }

    function item(value) {
        if (!value || typeof value !== 'object') return null
        const base = { id: string(value.id, 100) || id() }
        switch (value.type) {
            case 'activity':
                return {
                    ...base,
                    type: 'activity',
                    steps: list(value.steps, step, 100),
                    startedAt: time(value.startedAt),
                    endedAt: time(value.endedAt),
                }
            case 'text':
                return { ...base, type: 'text', text: string(value.text, MAX_TEXT) }
            case 'plan': {
                const details = plan(value.plan)
                return details
                    ? {
                          ...base,
                          type: 'plan',
                          plan: details,
                          state: oneOf(value.state, PLAN_STATES, 'unknown'),
                          receipts:
                              value.receipts && typeof value.receipts === 'object'
                                  ? JSON.parse(JSON.stringify(value.receipts))
                                  : null,
                          done: Number.isInteger(value.done) ? Math.max(0, value.done) : 0,
                          // Changes the user took out of the plan before approving it.
                          excluded: list(value.excluded, (index) =>
                              Number.isInteger(index) &&
                              index >= 0 &&
                              index < details.changes.length
                                  ? index
                                  : null,
                          ),
                      }
                    : null
            }
            case 'result':
            case 'error':
                return {
                    ...base,
                    type: value.type,
                    title: string(value.title, 200),
                    text: string(value.text, 2000),
                    retryable: value.retryable === true,
                    ...(value.type === 'result' && {
                        next: list(value.next, (entry) => string(entry, 120) || null, 3),
                    }),
                }
            default:
                return null
        }
    }

    function progress(value) {
        if (!value || typeof value.title !== 'string') return null
        return {
            title: string(value.title, 200),
            detail: string(value.detail, 300),
            status: oneOf(value.status, ['pending', 'current', 'done', 'failed'], 'pending'),
        }
    }

    function run(value) {
        if (!value || typeof value !== 'object') {
            return { ...newRun(), status: 'interrupted', startedAt: null }
        }
        const result = {
            status: oneOf(value.status, RUN_STATUSES, 'interrupted'),
            startedAt: time(value.startedAt),
            endedAt: time(value.endedAt),
            label: '',
            items: list(value.items, item, 200),
            progress: list(value.progress, progress, 12),
            resume:
                value.resume && typeof value.resume === 'object' && !Array.isArray(value.resume)
                    ? JSON.parse(JSON.stringify(value.resume))
                    : null,
            preview: value.preview === true,
        }
        // Nothing runs again by itself after a restart. Work that was only being
        // prepared is interrupted; work that was writing has an unknown outcome.
        if (result.status === 'thinking' || result.status === 'running')
            result.status = 'interrupted'
        if (result.status === 'applying') result.status = 'detached'
        for (const entry of result.items) {
            if (entry.type === 'activity')
                for (const part of entry.steps)
                    if (part.status === 'running') part.status = 'stopped'
            if (entry.type === 'plan' && entry.state === 'applying') entry.state = 'unknown'
        }
        for (const entry of result.progress)
            if (entry.status === 'current') entry.status = 'pending'
        return result
    }

    function turn(value, seen) {
        if (!value || typeof value.id !== 'string' || typeof value.text !== 'string') return null
        if (seen.has(value.id)) return null
        seen.add(value.id)
        return {
            id: value.id,
            createdAt: time(value.createdAt) || Date.now(),
            text: string(value.text, MAX_TEXT),
            attachments: list(value.attachments, attachment, MAX_ATTACHMENTS),
            instanceId: optionalString(value.instanceId),
            selection: selection(value.selection),
            run: run(value.run),
            ...(value.marked === true && { marked: true }),
        }
    }

    function queued(value) {
        if (!value || typeof value.text !== 'string') return null
        return {
            id: typeof value.id === 'string' ? string(value.id, 100) : id(),
            text: string(value.text, MAX_TEXT),
            attachments: list(value.attachments, attachment, MAX_ATTACHMENTS),
            createdAt: time(value.createdAt) || Date.now(),
            ...(native &&
                value.native === true && {
                    native: true,
                    instanceId: optionalString(value.instanceId),
                    selection: selection(value.selection),
                    run: run(value.run),
                    instructions: string(value.instructions, MAX_INSTRUCTIONS),
                    computerUse: value.computerUse === true,
                }),
        }
    }

    function thread(value, threadIds, turnIds) {
        if (!value || typeof value.id !== 'string' || threadIds.has(value.id)) return null
        threadIds.add(value.id)
        return {
            id: value.id,
            title: string(value.title, 120),
            titleEdited: value.titleEdited === true,
            createdAt: time(value.createdAt) || Date.now(),
            updatedAt: time(value.updatedAt) || Date.now(),
            ...(value.pinned === true && { pinned: true }),
            ...(value.pinned === true && Number.isFinite(value.rank) && { rank: value.rank }),
            ...(value.unread === true && { unread: true }),
            draft: string(value.draft, MAX_TEXT),
            attachments: list(value.attachments, attachment, MAX_ATTACHMENTS),
            instanceId: optionalString(value.instanceId),
            selection: selection(value.selection),
            turns: list(value.turns, (entry) => turn(entry, turnIds), MAX_TURNS),
            queue: list(value.queue, queued, 20),
        }
    }

    // The first preview stored only user messages that were never executed.
    function migrateLegacy(value) {
        if (value.version !== 1 || !Array.isArray(value.threads)) return null
        return {
            version: 2,
            activeId: value.activeId,
            sidebarOpen: value.sidebarOpen,
            threads: value.threads.map((entry) => ({
                ...entry,
                turns: Array.isArray(entry?.messages)
                    ? entry.messages.map((message) => ({
                          ...message,
                          run: { status: 'interrupted', startedAt: null, items: [] },
                      }))
                    : [],
            })),
        }
    }

    /** @param {string} raw @returns {Store} */
    function parse(raw) {
        if (native && new TextEncoder().encode(raw).byteLength > MAX_NATIVE_STORE_BYTES)
            throw new Error('Native conversation store exceeds 16 MiB')
        let value = JSON.parse(raw)
        if (value?.version === 1) value = migrateLegacy(value)
        if (value?.version !== 2 || !Array.isArray(value.threads))
            throw new Error('Invalid conversation store')
        const threadIds = new Set()
        const turnIds = new Set()
        const parsedThreads = value.threads.map((entry) => thread(entry, threadIds, turnIds))
        if (native && parsedThreads.some((entry) => !entry))
            throw new Error('Invalid or duplicate native thread')
        const threads = parsedThreads.filter(Boolean)
        if (!threads.length) return initialStore()
        return {
            version: 2,
            activeId: threads.some((entry) => entry.id === value.activeId)
                ? value.activeId
                : threads[0].id,
            sidebarOpen: value.sidebarOpen !== false,
            panelOpen: value.panelOpen === true,
            introDismissed: value.introDismissed === true,
            tourDone: value.tourDone === true,
            lastSelection: selection(value.lastSelection),
            threads,
            detail: oneOf(value.detail, DETAIL_LEVELS, 'balanced'),
            instructions: string(value.instructions, MAX_INSTRUCTIONS),
        }
    }

    return parse
}

export const parseStore = createStoreParser(false)
export const parseNativeStore = createStoreParser(true)

/** @param {StorageArea} [storage] @returns {Store} */
export function loadStore(storage = localStorage) {
    const raw = storage.getItem(STORAGE_KEY) ?? storage.getItem(LEGACY_STORAGE_KEY)
    return raw ? parseStore(raw) : initialStore()
}

function isProtected(thread, activeId) {
    return (
        thread.id === activeId ||
        !!thread.draft ||
        thread.queue.length > 0 ||
        thread.turns.some(
            (entry) =>
                ACTIVE_STATUSES.includes(entry.run.status) || entry.run.status === 'awaiting',
        )
    )
}

// Older settled work makes room when the store grows past its budget. Drafts,
// queued follow-ups and runs that are active or waiting for approval are kept.
/** @param {Store} store @param {StorageArea} [storage] @returns {Store} */
export function saveStore(store, storage = localStorage) {
    // Usually everything fits: serialize once and keep the caller's object, so
    // saving during a run costs one stringify instead of a deep copy as well.
    const turns = store.threads.reduce((total, entry) => total + entry.turns.length, 0)
    if (store.threads.length <= MAX_THREADS && turns <= MAX_TURNS) {
        const serialized = JSON.stringify(store)
        if (serialized.length <= MAX_STORE_LENGTH) {
            storage.setItem(STORAGE_KEY, serialized)
            return store
        }
    }
    const next = structuredClone(store)
    const removable = next.threads
        .filter((entry) => !isProtected(entry, next.activeId))
        .sort((a, b) => a.updatedAt - b.updatedAt)
    while (next.threads.length > MAX_THREADS && removable.length) {
        const oldest = removable.shift()
        next.threads = next.threads.filter((entry) => entry.id !== oldest.id)
    }
    const history = next.threads
        .flatMap((entry) =>
            entry.turns
                .filter(
                    (part) =>
                        !ACTIVE_STATUSES.includes(part.run.status) &&
                        part.run.status !== 'awaiting',
                )
                .map((part) => ({ thread: entry, turn: part })),
        )
        .sort((a, b) => a.turn.createdAt - b.turn.createdAt)
    let count = next.threads.reduce((total, entry) => total + entry.turns.length, 0)
    let serialized = JSON.stringify(next)
    while ((serialized.length > MAX_STORE_LENGTH || count > MAX_TURNS) && history.length) {
        const { thread: owner, turn: oldest } = history.shift()
        owner.turns = owner.turns.filter((part) => part.id !== oldest.id)
        count--
        serialized = JSON.stringify(next)
    }
    if (
        serialized.length > MAX_STORE_LENGTH ||
        count > MAX_TURNS ||
        next.threads.length > MAX_THREADS
    )
        throw new Error('Conversation storage limit reached')
    storage.setItem(STORAGE_KEY, serialized)
    return next
}

/** @param {Store} store */
export function activeThread(store) {
    return store.threads.find((entry) => entry.id === store.activeId)
}

export function lastTurn(thread) {
    return thread.turns.at(-1) || null
}

// The state that a list row or header shows for a whole thread.
export function threadStatus(thread) {
    const status = lastTurn(thread)?.run.status
    if (ACTIVE_STATUSES.includes(status)) return 'active'
    if (status === 'awaiting') return 'awaiting'
    if (status === 'failed' || status === 'detached') return 'attention'
    return 'idle'
}

// Native storage is acknowledged by Rust. Browser storage is limited to dev preview.
export function createStorage(host, storage) {
    let revision = 0
    let acknowledgedThreads = []
    return {
        async load() {
            if (!host?.runtimeInvoke) {
                if (host?.preview === true && !host?.native) return loadStore(storage)
                throw new Error('Agent storage unavailable')
            }
            const result = await host.runtimeInvoke('load')
            revision = result.workspace_revision
            if (!Number.isInteger(revision)) throw new Error('Invalid workspace revision')
            if (result.workspace) {
                const loaded = parseNativeStore(JSON.stringify(result.workspace))
                acknowledgedThreads = loaded.threads.map((thread) => thread.id)
                return loaded
            }
            acknowledgedThreads = []
            // Import the old browser history once; the server owns idempotence.
            const raw = storage?.getItem(STORAGE_KEY) ?? storage?.getItem(LEGACY_STORAGE_KEY)
            if (raw) {
                const imported = parseNativeStore(raw)
                await host.runtimeInvoke('import', {
                    importId: STORAGE_KEY,
                    threads: imported.threads,
                })
                await this.save(imported)
                return imported
            }
            const fresh = initialStore()
            if (result.threads?.length) fresh.threads = result.threads
            return parseNativeStore(JSON.stringify(fresh))
        },
        async save(store) {
            if (!host?.runtimeInvoke) {
                if (host?.preview === true && !host?.native) return saveStore(store, storage)
                throw new Error('Agent storage unavailable')
            }
            if (new TextEncoder().encode(JSON.stringify(store)).byteLength > MAX_NATIVE_STORE_BYTES)
                throw new Error('Native conversation store exceeds 16 MiB')
            const nextRevision = await host.runtimeInvoke('save', {
                workspace: store,
                expectedRevision: revision,
                deletedThreadIds: acknowledgedThreads.filter(
                    (id) => !store.threads.some((thread) => thread.id === id),
                ),
            })
            if (!Number.isInteger(nextRevision)) throw new Error('Invalid workspace revision')
            revision = nextRevision
            acknowledgedThreads = store.threads.map((thread) => thread.id)
            return store
        },
    }
}

// Merge only acknowledged changes; streaming events and edits made during the
// write remain live. Arrays with identities preserve their existing row objects.
export function mergeAcknowledged(base, next, live) {
    if (JSON.stringify(base) === JSON.stringify(next)) return live
    if (JSON.stringify(base) === JSON.stringify(live)) return next
    if (
        Array.isArray(base) &&
        Array.isArray(next) &&
        Array.isArray(live) &&
        [...base, ...next, ...live].every((entry) => entry && typeof entry.id === 'string')
    ) {
        const result = live.filter(
            (entry) =>
                !base.some((old) => old.id === entry.id) ||
                next.some((entry2) => entry2.id === entry.id),
        )
        for (const entry of next) {
            const index = result.findIndex((value) => value.id === entry.id)
            const old = base.find((value) => value.id === entry.id)
            if (index < 0) result.push(entry)
            else result[index] = mergeAcknowledged(old, entry, result[index])
        }
        return result
    }
    if (base && next && live && !Array.isArray(base) && typeof base === 'object') {
        if (next.titleEdited === true && base.titleEdited !== true) live.title = next.title
        for (const key of new Set([...Object.keys(base), ...Object.keys(next)])) {
            if (
                ['title', 'titleEdited', 'pinned', 'marked', 'selection', 'instanceId'].includes(
                    key,
                ) &&
                JSON.stringify(base[key]) !== JSON.stringify(next[key])
            ) {
                live[key] = next[key]
                continue
            }
            if (!(key in next)) {
                if (JSON.stringify(live[key]) === JSON.stringify(base[key])) delete live[key]
            } else live[key] = mergeAcknowledged(base[key], next[key], live[key])
        }
        return live
    }
    return live
}
