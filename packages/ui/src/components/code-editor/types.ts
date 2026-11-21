/** Monaco 的命名空间类型。纯类型导入，不把编辑器拉进打包结果。 */
export type Monaco = typeof import('monaco-editor/editor/editor.api.js')

/** 常用类型别名，省得使用方去索引命名空间。 */
export type MonacoEditor = import('monaco-editor/editor/editor.api.js').editor.IStandaloneCodeEditor
export type MonacoModel = import('monaco-editor/editor/editor.api.js').editor.ITextModel
export type MonacoEditorOptions =
    import('monaco-editor/editor/editor.api.js').editor.IStandaloneEditorConstructionOptions
