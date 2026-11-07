// Monaco's language workers also load editor services; register them before creating an editor.
import 'monaco-editor/internal/common/workers.js'
import 'monaco-editor/languages/definitions/css/register.js'
import 'monaco-editor/languages/definitions/html/register.js'
import 'monaco-editor/languages/definitions/typescript/register.js'
import 'monaco-editor/languages/definitions/javascript/register.js'
import 'monaco-editor/language/json/monaco.contribution.js'
import 'monaco-editor/language/css/monaco.contribution.js'
import 'monaco-editor/language/html/monaco.contribution.js'
import 'monaco-editor/language/typescript/monaco.contribution.js'
import 'monaco-editor/languages/definitions/java/register.js'
import 'monaco-editor/languages/definitions/kotlin/register.js'
import 'monaco-editor/languages/definitions/shell/register.js'
import 'monaco-editor/languages/definitions/bat/register.js'
import 'monaco-editor/languages/definitions/powershell/register.js'
import 'monaco-editor/languages/definitions/yaml/register.js'
import 'monaco-editor/languages/definitions/python/register.js'
import 'monaco-editor/languages/definitions/ruby/register.js'
import 'monaco-editor/languages/definitions/php/register.js'
import 'monaco-editor/languages/definitions/cpp/register.js'
import 'monaco-editor/languages/definitions/rust/register.js'
import 'monaco-editor/languages/definitions/go/register.js'
import 'monaco-editor/languages/definitions/markdown/register.js'
import 'monaco-editor/languages/definitions/ini/register.js'
import 'monaco-editor/languages/definitions/xml/register.js'

import * as monaco from 'monaco-editor/editor/editor.api.js'
import EditorWorker from 'monaco-editor/editor/editor.worker.js?worker'
import CssWorker from 'monaco-editor/language/css/css.worker.js?worker'
import HtmlWorker from 'monaco-editor/language/html/html.worker.js?worker'
import JsonWorker from 'monaco-editor/language/json/json.worker.js?worker'
import TypeScriptWorker from 'monaco-editor/language/typescript/ts.worker.js?worker'

self.MonacoEnvironment = {
    getWorker(_moduleId: string, label: string) {
        switch (label) {
            case 'json':
                return new JsonWorker()
            case 'css':
            case 'scss':
            case 'less':
                return new CssWorker()
            case 'html':
            case 'handlebars':
            case 'razor':
                return new HtmlWorker()
            case 'typescript':
            case 'javascript':
                return new TypeScriptWorker()
            default:
                return new EditorWorker()
        }
    },
}

function registerCustomLanguages() {
    if (!monaco.languages.getLanguages().some(({ id }) => id === 'toml')) {
        monaco.languages.register({ id: 'toml', extensions: ['.toml'] })
        monaco.languages.setMonarchTokensProvider('toml', {
            tokenizer: {
                root: [
                    [/#.*/, 'comment'],
                    [/\[\[?.*?\]\]?/, 'type.identifier'],
                    [/^[\w.-]+(?=\s*=)/, 'key'],
                    [/"([^"\\]|\\.)*"/, 'string'],
                    [/'[^']*'/, 'string'],
                    [/\b(true|false)\b/, 'keyword'],
                    [/[-+]?\b\d+(\.\d+)?\b/, 'number'],
                ],
            },
        })
    }

    if (!monaco.languages.getLanguages().some(({ id }) => id === 'properties')) {
        monaco.languages.register({ id: 'properties', extensions: ['.properties'] })
        monaco.languages.setMonarchTokensProvider('properties', {
            tokenizer: {
                root: [
                    [/^[#!].*$/, 'comment'],
                    [/^[^\s:=]+(?=\s*[:=])/, 'key'],
                    [/[:=]/, 'delimiter'],
                    [/\\./, 'string.escape'],
                ],
            },
        })
    }

    if (!monaco.languages.getLanguages().some(({ id }) => id === 'snbt')) {
        monaco.languages.register({ id: 'snbt' })
        monaco.languages.setMonarchTokensProvider('snbt', {
            tokenizer: {
                root: [
                    [/\/\/.*$/, 'comment'],
                    [/[{}[\],:]/, 'delimiter'],
                    [/(?:true|false)\b/, 'keyword'],
                    [/-?(?:\d+\.?\d*|\.\d+)(?:[bBsSlLfFdD])?\b/, 'number'],
                    [/'(?:[^'\\]|\\.)*'/, 'string'],
                    [/"(?:[^"\\]|\\.)*"/, 'string'],
                    [/[A-Za-z0-9_.+-]+(?=\s*:)/, 'key'],
                ],
            },
        })
    }
}

registerCustomLanguages()

monaco.languages.register({ id: 'mclog', extensions: ['.log'] })
monaco.languages.setMonarchTokensProvider('mclog', {
    tokenizer: {
        root: [
            [/^\[\d\d:\d\d:\d\d\]/, 'comment'],
            [/\[.+?\/ERROR\]:?/, 'invalid'],
            [/\[.+?\/WARN\]:?/, 'keyword'],
            [/\[.+?\/INFO\]:?/, 'string'],
            [/: \/.+/, 'keyword'],
            [/\tat\s.+/, 'comment'],
            [/\w+?\[\/\d+?\.\d+?\.\d+?\.\d+?:\d+?\]/, 'type.identifier'],
            [/\[CHAT\]/, 'type.identifier'],
        ],
    },
})

// The mode module can finish loading before its onLanguage callback registers
// providers. Formatting must wait for the registry, or Monaco's action is a no-op.
export async function prepareFormatting(model: monaco.editor.ITextModel) {
    const language = model.getLanguageId()
    if (
        ![
            'json',
            'css',
            'less',
            'scss',
            'html',
            'handlebars',
            'razor',
            'javascript',
            'typescript',
        ].includes(language)
    )
        return

    const [{ StandaloneServices }, { ILanguageFeaturesService }, { ILanguageService }] =
        await Promise.all([
            import('monaco-editor/editor/standalone/browser/standaloneServices.js'),
            import('monaco-editor/editor/common/services/languageFeatures.js'),
            import('monaco-editor/editor/common/languages/language.js'),
        ])
    const features = StandaloneServices.get(ILanguageFeaturesService)
    const ready = () =>
        features.documentFormattingEditProvider.has(model) ||
        features.documentRangeFormattingEditProvider.has(model)
    if (model.isDisposed() || model.getLanguageId() !== language || ready()) return

    await new Promise<void>((resolve) => {
        const listeners: monaco.IDisposable[] = []
        const finish = () => {
            listeners.forEach((listener) => listener.dispose())
            resolve()
        }
        const check = () => {
            if (model.isDisposed() || model.getLanguageId() !== language || ready()) finish()
        }
        listeners.push(features.documentFormattingEditProvider.onDidChange(check))
        listeners.push(features.documentRangeFormattingEditProvider.onDidChange(check))
        listeners.push(model.onWillDispose(finish))
        listeners.push(model.onDidChangeLanguage(check))
        StandaloneServices.get(ILanguageService).requestRichLanguageFeatures(language)
        check()
    })
}

export { monaco }
