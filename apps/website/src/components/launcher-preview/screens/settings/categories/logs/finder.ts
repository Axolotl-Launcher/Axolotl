import { type AppContext, getCurrentInstance, h, reactive, type Ref, render } from 'vue'

import { usePreviewDomain } from '../../../../lib/state/index'
import { injectPreviewWindow } from '../../../../lib/window'
import { useSettingsPage } from '../../state'
import { type FinderItem, parentOf } from './finder-model'
import FinderWindow from './FinderWindow.vue'
import { LAUNCHER_LOGS_DIR, launcherLogFiles } from './launcher-logs'

/**
 * The Finder window the launcher's "open folder" commands bring up on a Mac:
 * `open_path` shows a folder, `reveal_item_in_dir` shows its parent with the
 * item selected. It is another application's window, so it lives as long as
 * the launcher window rather than the settings page: an "Open in folder"
 * notification button still works after the visitor has moved on. It is drawn
 * inside the launcher window's overlay layer.
 */
export interface FinderState {
    visible: boolean
    /** Bumped on every open, so the window comes forward again. */
    opened: number
    path: string
    selected: string | null
    back: string[]
    forward: string[]
    /** Files written this session outside the launcher's own folders, by folder. */
    created: Record<string, FinderItem[]>
}

export interface FinderApi {
    state: FinderState
    items: (path: string) => FinderItem[]
    navigate: (path: string) => void
    goBack: () => void
    goForward: () => void
    select: (name: string | null) => void
    close: () => void
}

interface FinderDomain extends FinderApi {
    /** `plugin:utils|open_path` on a folder. */
    openFolder: (path: string) => void
    /** `plugin:utils|highlight_in_folder`. */
    revealItem: (path: string) => void
    /** Records a file the launcher wrote (an export), replacing one of the same name. */
    addFile: (path: string, file: Omit<FinderItem, 'name'>) => void
    attach: (context: MountContext) => void
}

interface MountContext {
    appContext: AppContext
    provides: Record<string | symbol, unknown>
    layer: Ref<HTMLElement | null>
}

export function useFinder(): FinderDomain {
    const { settings } = useSettingsPage()
    const windowContext = injectPreviewWindow()
    const instance = getCurrentInstance() as
        | (ReturnType<typeof getCurrentInstance> & { provides: Record<string | symbol, unknown> })
        | null

    const finder = usePreviewDomain<FinderDomain>('settings:finder', () => {
        const state = reactive<FinderState>({
            visible: false,
            opened: 0,
            path: LAUNCHER_LOGS_DIR,
            selected: null,
            back: [],
            forward: [],
            created: {},
        })
        let context: MountContext | null = null
        let host: HTMLElement | null = null

        function mount() {
            const layer = context?.layer.value
            if (!context || !layer) return
            if (host) {
                if (!host.isConnected) layer.appendChild(host)
                return
            }
            host = document.createElement('div')
            layer.appendChild(host)
            const vnode = h(FinderWindow, { finder: api })
            // Rendered outside the settings tree, but with its injections (the
            // window, the preview's state and i18n), like a programmatic modal.
            vnode.appContext = { ...context.appContext, provides: context.provides }
            render(vnode, host)
        }

        function show(path: string, selected: string | null) {
            if (state.visible && state.path !== path) {
                state.back.push(state.path)
                state.forward = []
            } else if (!state.visible) {
                state.back = []
                state.forward = []
            }
            state.path = path
            state.selected = selected
            state.visible = true
            state.opened++
            mount()
        }

        function items(path: string): FinderItem[] {
            if (path === LAUNCHER_LOGS_DIR) {
                return launcherLogFiles(Date.now(), settings.log_level).map((file) => ({
                    name: file.name,
                    kind: 'log' as const,
                    bytes: file.bytes,
                    modified: file.modified,
                }))
            }
            return [...(state.created[path] ?? [])]
        }

        const api: FinderDomain = {
            state,
            items,
            navigate(path) {
                if (path === state.path) return
                state.back.push(state.path)
                state.forward = []
                state.path = path
                state.selected = null
            },
            goBack() {
                const previous = state.back.pop()
                if (!previous) return
                state.forward.push(state.path)
                state.path = previous
                state.selected = null
            },
            goForward() {
                const next = state.forward.pop()
                if (!next) return
                state.back.push(state.path)
                state.path = next
                state.selected = null
            },
            select(name) {
                state.selected = name
            },
            close() {
                state.visible = false
            },
            openFolder(path) {
                show(path, null)
            },
            revealItem(path) {
                show(parentOf(path), path.slice(path.lastIndexOf('/') + 1))
            },
            addFile(path, file) {
                const folder = parentOf(path)
                const name = path.slice(path.lastIndexOf('/') + 1)
                const existing = (state.created[folder] ?? []).filter((item) => item.name !== name)
                state.created[folder] = [...existing, { name, ...file }]
            },
            attach(next) {
                context ??= next
            },
        }

        return {
            value: api,
            dispose: () => {
                if (host) render(null, host)
                host?.remove()
                host = null
                context = null
            },
        }
    })

    if (instance) {
        finder.attach({
            appContext: instance.appContext,
            provides: instance.provides,
            layer: windowContext.layer,
        })
    }
    return finder
}
