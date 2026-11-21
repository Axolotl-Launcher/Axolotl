/**
 * SNBT 的补全接线：把「光标上下文」交给调用方的 `SnbtCompletionSource`，
 * 再把候选翻译成 Monaco 的补全项。
 *
 * 补全 provider 是**语言级**的，一个语言只能注册一次；每个编辑器实例的上下文
 * 通过模型 URI 挂在 `attachments` 上。没挂上下文的模型（例如 Studio 打开的
 * `.snbt` 文件）不会得到任何候选，行为与注册前一致。
 */

import type * as MonacoTypes from 'monaco-editor/editor/editor.api.js'

import type { Monaco } from '../types.ts'
import { SNBT_LANGUAGE_ID } from './language.ts'
import { scanSnbt } from './scan.ts'
import type {
    SnbtCandidateKind,
    SnbtCompletionHandle,
    SnbtCompletionSource,
    SnbtSprite,
} from './types.ts'

interface Attachment {
    source: SnbtCompletionSource
    /** 当前这批候选的贴图，供列表行注入用。 */
    icons: Map<string, SnbtSprite>
}

const attachments = new Map<string, Attachment>()

let registered = false
let kinds: Record<SnbtCandidateKind, number> | null = null

function buildKindMap(monaco: Monaco): Record<SnbtCandidateKind, number> {
    const kind = monaco.languages.CompletionItemKind
    return {
        field: kind.Field,
        value: kind.Value,
        snippet: kind.Snippet,
        enum: kind.EnumMember,
        item: kind.Constant,
        entity: kind.Class,
        block: kind.Module,
        potion: kind.Color,
        enchantment: kind.Keyword,
        effect: kind.Event,
    }
}

function stripQuotes(text: string): string {
    const trimmed = text.trim()
    const quote = trimmed[0]
    if (trimmed.length >= 2 && (quote === '"' || quote === "'") && trimmed.endsWith(quote)) {
        return trimmed.slice(1, -1)
    }
    return text
}

export function registerSnbtCompletions(monaco: Monaco): void {
    if (registered) return
    registered = true
    kinds = buildKindMap(monaco)

    monaco.languages.registerCompletionItemProvider(SNBT_LANGUAGE_ID, {
        triggerCharacters: ['{', '[', ',', ':', '=', '"', "'"],
        async provideCompletionItems(model, position) {
            const attachment = attachments.get(model.uri.toString())
            const kindMap = kinds
            if (!attachment || !kindMap) return { suggestions: [] }

            const scan = scanSnbt(model.getValue(), model.getOffsetAt(position))
            const candidates = await attachment.source.candidates(scan)
            // 替换范围按扫描器认出来的词算，而不是 Monaco 的单词定义：
            // `minecraft:sto` 整个要换掉，`display:` 后面的冒号不能算进去。
            const range = {
                startLineNumber: position.lineNumber,
                startColumn: Math.max(1, position.column - scan.word.length),
                endLineNumber: position.lineNumber,
                endColumn: position.column,
            }

            attachment.icons.clear()
            const suggestions = candidates.map((candidate, index) => {
                if (candidate.icon) attachment.icons.set(candidate.label, candidate.icon)
                const insertText = scan.quoted
                    ? stripQuotes(candidate.insertText ?? candidate.label)
                    : (candidate.insertText ?? candidate.label)
                return {
                    label: candidate.detail
                        ? { label: candidate.label, detail: ` ${candidate.detail}` }
                        : candidate.label,
                    kind: kindMap[candidate.kind],
                    insertText,
                    insertTextRules: candidate.snippet
                        ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
                        : undefined,
                    documentation: candidate.documentation
                        ? { value: candidate.documentation }
                        : undefined,
                    filterText: candidate.filterText ?? candidate.label,
                    // Monaco 按 sortText 升序排；分数越高越靠前。
                    sortText: `${String(999 - Math.round((candidate.score ?? 50) * 10)).padStart(4, '0')}-${String(index).padStart(4, '0')}`,
                    range,
                }
            })

            return { suggestions }
        },
    })
}

/**
 * 把模型和候选来源绑起来，并在补全列表里注入物品贴图。
 *
 * Monaco 的补全行只认 codicon（`CompletionItemKinds.toIcon`），没有图片通道，
 * 所以这里在列表渲染后用观察器补一层：按行里的标签名找到贴图，画到行首的
 * `.icon` 槽位上。找不到就保持原样，不会破坏列表。
 */
export function attachSnbtCompletion(
    editor: MonacoTypes.editor.IStandaloneCodeEditor,
    model: MonacoTypes.editor.ITextModel,
    source: SnbtCompletionSource,
): SnbtCompletionHandle {
    const key = model.uri.toString()
    const attachment: Attachment = { source, icons: new Map() }
    attachments.set(key, attachment)

    const dom = editor.getDomNode()
    if (!dom) {
        return {
            dispose() {
                attachments.delete(key)
            },
        }
    }

    let frame = 0
    let disposed = false

    const paint = () => {
        const current = attachments.get(key)
        if (!current) return
        dom.querySelectorAll('.suggest-widget .monaco-list-row').forEach((row) => {
            const slot = row.querySelector('.icon')
            const glyph = row.querySelector('.icon-label')
            if (!(slot instanceof HTMLElement)) return
            const label = row.querySelector('.label-name')?.textContent?.trim()
            const icon = label ? current.icons.get(label) : undefined

            if (!icon) {
                if (slot.style.backgroundImage) {
                    slot.style.backgroundImage = ''
                    slot.classList.add('hide')
                    if (glyph instanceof HTMLElement) glyph.classList.remove('hide')
                }
                return
            }

            slot.classList.remove('hide')
            const size = slot.clientHeight || 16
            const scale = size / Math.max(1, icon.width, icon.height)
            slot.style.backgroundImage = `url("${icon.url}")`
            slot.style.backgroundRepeat = 'no-repeat'
            slot.style.backgroundSize = `${icon.sheetWidth * scale}px ${icon.sheetHeight * scale}px`
            slot.style.backgroundPosition = `${-icon.x * scale}px ${-icon.y * scale}px`
            slot.style.imageRendering = 'pixelated'
            if (glyph instanceof HTMLElement) glyph.classList.add('hide')
        })
    }

    const schedule = () => {
        if (frame || disposed) return
        frame = requestAnimationFrame(() => {
            frame = 0
            paint()
        })
    }

    const observer = new MutationObserver(schedule)
    observer.observe(dom, { childList: true, subtree: true, characterData: true })

    return {
        dispose() {
            disposed = true
            observer.disconnect()
            if (frame) cancelAnimationFrame(frame)
            attachments.delete(key)
        },
    }
}
