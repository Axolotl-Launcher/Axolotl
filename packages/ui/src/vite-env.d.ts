/// <reference types="vite/client" />
/// <reference types="vite-svg-loader" />

// Monaco ships these internal ESM entry points without declaration files. The
// runtime modules are stable, but their types are only available through the
// public `monaco-editor` entry point.
declare module 'monaco-editor/editor/standalone/browser/standaloneServices.js' {
    import type * as monaco from 'monaco-editor'

    export const StandaloneServices: {
        get<T>(service: new (...args: never[]) => T): T
    }
    export type { monaco }
}

declare module 'monaco-editor/editor/common/services/languageFeatures.js' {
    import type * as monaco from 'monaco-editor'

    export const ILanguageFeaturesService: new (...args: never[]) => {
        documentFormattingEditProvider: {
            has(model: monaco.editor.ITextModel): boolean
            onDidChange(listener: () => void): monaco.IDisposable
        }
        documentRangeFormattingEditProvider: {
            has(model: monaco.editor.ITextModel): boolean
            onDidChange(listener: () => void): monaco.IDisposable
        }
    }
}

declare module 'monaco-editor/editor/common/languages/language.js' {
    export const ILanguageService: new (...args: never[]) => unknown
}

declare module 'fuse.js/dist/fuse.basic' {
    export { default } from 'fuse.js'
    export * from 'fuse.js'
}
