export type Handler = (args: Record<string, unknown>) => unknown

/** Tauri command name → what the demo answers in place of the Rust backend. */
export type Handlers = Record<string, Handler>
