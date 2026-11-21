/** Monaco 的命名空间类型。纯类型导入，不把编辑器拉进打包结果。 */
export type Monaco = typeof import('monaco-editor/editor/editor.api.js')
