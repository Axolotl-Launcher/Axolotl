/**
 * SNBT 编辑器语言能力的公共契约。
 *
 * 这一层刻意不认识 Minecraft 的标签目录：扫描器只把「光标在哪一层、现在该写
 * 键还是值」算出来，候选由调用方提供（见 `SnbtCompletionSource`）。实验室用它
 * 接 NBT 目录，Studio 将来接自己的数据源即可。
 */

/** 光标处接下来要写什么。 */
export type SnbtExpect = 'key' | 'component' | 'value' | 'element' | 'after'

/** 扫描时维护的一层容器。列表元素不贡献键名，所以路径与目录的取法一致。 */
export interface SnbtFrame {
    kind: 'compound' | 'list'
    /** 进入这一层时的键名路径。 */
    keyPath: string[]
    /** 根层的 `[id=…]` 组件列表。 */
    components: boolean
    /** 这一层已经写过的键名，用来把「已添加」的候选排在后面。 */
    keys: string[]
}

/** 光标位置的语法上下文。 */
export interface SnbtScan {
    /** 由外到内的容器栈，最后一个是光标所在的层。 */
    frames: SnbtFrame[]
    /** 光标所在的容器；文档根为空。 */
    container?: SnbtFrame
    /** 光标这一层的键名路径。 */
    keyPath: string[]
    expect: SnbtExpect
    /**
     * `expect === 'value'` 时是所属的键名；`expect === 'key' | 'component'`
     * 时是光标处已输入的片段。
     */
    key: string
    /** 光标处已输入的词，用于打分。 */
    word: string
    /** 光标在未闭合的字符串里，插入时不该再补引号。 */
    quoted: boolean
    /** 光标之后第一个非空白字符；文档末尾为空串。 */
    next: string
}

/** 候选的类别，决定列表里的图标。 */
export type SnbtCandidateKind =
    | 'field'
    | 'value'
    | 'snippet'
    | 'enum'
    | 'item'
    | 'entity'
    | 'block'
    | 'potion'
    | 'enchantment'
    | 'effect'

/** 贴图集里的一块：物品图标与效果图标都从这里取。 */
export interface SnbtSprite {
    url: string
    x: number
    y: number
    width: number
    height: number
    /** 整张贴图集的尺寸，用来把这一块缩放到行高。 */
    sheetWidth: number
    sheetHeight: number
}

export interface SnbtCandidate {
    /** 列表里显示的文字，也是贴图注入的匹配键。 */
    label: string
    /** 实际插入的文本，默认与 `label` 相同。 */
    insertText?: string
    /** 跟在名字后面的灰字：类型、中文名。 */
    detail?: string
    /** 右侧详情面板的 markdown。 */
    documentation?: string
    kind: SnbtCandidateKind
    /** 0–100，越大越靠前。 */
    score?: number
    icon?: SnbtSprite
    /** 按代码片段插入（`$0` 定位光标）。 */
    snippet?: boolean
}

/** 候选来源：由使用方按当前版本与目标实现。 */
export interface SnbtCompletionSource {
    candidates(scan: SnbtScan): SnbtCandidate[] | Promise<SnbtCandidate[]>
}

export interface SnbtCompletionHandle {
    dispose(): void
}
