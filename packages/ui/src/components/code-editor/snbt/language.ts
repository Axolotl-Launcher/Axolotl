/**
 * SNBT 在 Monaco 里的语言定义：词法、括号配对、缩进规则。
 *
 * 这里只做**纯词法**的事情，不认识任何 Minecraft 数据。补全由
 * `completions.ts` 通过 `SnbtCompletionSource` 注入，所以同一套语言既能给
 * 命令生成器用，也能给 Studio 的 `.snbt` 文件编辑器用。
 *
 * 注册是幂等的：`monaco-runtime` 与编辑器挂载时都会调用一次。
 */

import type { Monaco } from '../types.ts'
import { scanSnbt } from './scan.ts'

export const SNBT_LANGUAGE_ID = 'snbt'

/** 缩进宽度；与编辑器默认的 `tabSize` 保持一致。 */
const INDENT_WIDTH = 4

let registered = false

function monarchTokens(): Parameters<Monaco['languages']['setMonarchTokensProvider']>[1] {
    return {
        brackets: [
            { open: '{', close: '}', token: 'delimiter.bracket' },
            { open: '[', close: ']', token: 'delimiter.bracket' },
        ],
        tokenizer: {
            root: [
                [/\s+/, 'white'],
                // `[B;` / `[I;` / `[L;` 的数组前缀；`[` 仍然按括号算，配对才不会断。
                [/[BIL];/, 'keyword'],
                [/[{}[\]]/, '@brackets'],
                [/[,:]/, 'delimiter'],
                [/=/, 'operator'],
                [/"([^"\\]|\\.)*"/, 'string'],
                [/'([^'\\]|\\.)*'/, 'string'],
                [/-?\d+\.\d+[fFdD]?/, 'number.float'],
                [/-?\d+[bBsSlLfFdD]/, 'number'],
                [/-?\d+/, 'number'],
                [/\b(true|false)\b/, 'keyword'],
                // 键名与组件名靠后面的分隔符区分，先于普通标识符匹配。
                [/[-\w.+]+(?=\s*:)/, 'type'],
                [/[a-z0-9_.-]+(?=\s*=)/, 'attribute.name'],
                [/[-\w.+:]+/, 'identifier'],
            ],
        },
    }
}

export function registerSnbtLanguage(monaco: Monaco): void {
    if (registered) return
    registered = true

    if (!monaco.languages.getLanguages().some(({ id }) => id === SNBT_LANGUAGE_ID)) {
        monaco.languages.register({ id: SNBT_LANGUAGE_ID, extensions: ['.snbt'] })
    }

    monaco.languages.setMonarchTokensProvider(SNBT_LANGUAGE_ID, monarchTokens())

    monaco.languages.setLanguageConfiguration(SNBT_LANGUAGE_ID, {
        brackets: [
            ['{', '}'],
            ['[', ']'],
        ],
        autoClosingPairs: [
            { open: '{', close: '}' },
            { open: '[', close: ']' },
            { open: '"', close: '"', notIn: ['string'] },
            { open: "'", close: "'", notIn: ['string'] },
        ],
        surroundingPairs: [
            { open: '{', close: '}' },
            { open: '[', close: ']' },
            { open: '"', close: '"' },
            { open: "'", close: "'" },
        ],
        colorizedBracketPairs: [
            ['{', '}'],
            ['[', ']'],
        ],
        // 不能把 `:` 算进单词：Monaco 用 `getWordUntilPosition` 过滤候选，
        // `display:` 会变成过滤词，冒号后面的值候选就全被筛掉了。
        // id 的替换范围由补全自己按词法算，见 `completions.ts`。
        wordPattern: /[-\w.+]+/,
        indentationRules: {
            increaseIndentPattern: /[[{]\s*$/,
            decreaseIndentPattern: /^\s*[}\]]/,
        },
        onEnterRules: [
            {
                beforeText: /[[{]\s*$/,
                action: { indentAction: monaco.languages.IndentAction.Indent },
            },
            {
                beforeText: /^\s*[}\]]/,
                action: { indentAction: monaco.languages.IndentAction.Outdent },
            },
        ],
    })

    // Monaco 只在回车时缩进；敲 `}` / `]` 不会自己回退，这里补上。
    monaco.languages.registerOnTypeFormattingEditProvider(SNBT_LANGUAGE_ID, {
        autoFormatTriggerCharacters: ['}', ']'],
        provideOnTypeFormattingEdits(model, position) {
            const line = model.getLineContent(position.lineNumber)
            if (!/^\s*[}\]]/.test(line)) return []
            const depth = Math.max(
                0,
                scanSnbt(model.getValue(), model.getOffsetAt(position) - 1).frames.length - 1,
            )
            const indent = ' '.repeat(depth * INDENT_WIDTH)
            const text = indent + line.trimStart()
            if (text === line) return []
            return [
                {
                    range: {
                        startLineNumber: position.lineNumber,
                        startColumn: 1,
                        endLineNumber: position.lineNumber,
                        endColumn: line.length + 1,
                    },
                    text,
                },
            ]
        },
    })
}
