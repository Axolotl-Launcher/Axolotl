import { createContext } from '@modrinth/ui/src/providers/create-context.ts'
import type { Ref } from 'vue'

/** Logical size of the launcher window; matches the desktop app's default macOS window. */
export const WINDOW_WIDTH = 1280
export const WINDOW_HEIGHT = 800

export interface PreviewWindowContext {
    /** The window element. It is CSS-scaled, so fixed descendants resolve against it. */
    root: Ref<HTMLElement | null>
    /** In-window host for modals, menus and floating bars. */
    layer: Ref<HTMLElement | null>
    scale: Ref<number>
    /** Whether the window fills the screen (the green titlebar control). */
    fullScreen: Ref<boolean>
    /** Hold the startup screen while panels are being mounted in separate tasks. */
    deferReady: () => () => void
    /** Shared readiness gate for first startup and a restart from settings. */
    waitUntilReady: (onProgress?: (value: number) => void) => Promise<void>
    /** Converts viewport coordinates to the window's unscaled coordinate space. */
    toLocal: (clientX: number, clientY: number) => { x: number; y: number }
}

export const [injectPreviewWindow, providePreviewWindow] = createContext<PreviewWindowContext>(
    'LauncherPreview',
    'launcher-preview-window',
)
