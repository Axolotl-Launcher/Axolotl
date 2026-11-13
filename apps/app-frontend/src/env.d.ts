/// <reference types="vite/client" />

declare module '*.svg?component' {
    import type { FunctionalComponent, SVGAttributes } from 'vue'

    const component: FunctionalComponent<SVGAttributes>
    export default component
}

// Monaco exposes these internal ESM entry points without declaration files.
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
    export const ILanguageService: new (...args: never[]) => {
        requestRichLanguageFeatures(languageId: string): void
    }
}

declare module 'fuse.js/dist/fuse.basic' {
    export { default } from 'fuse.js'
    export * from 'fuse.js'
}
