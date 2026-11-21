import assert from 'node:assert/strict'
import { test } from 'node:test'

import { scanSnbt } from './scan.ts'

/** 用 `|` 标出光标，省得手数偏移量。 */
function scan(marked: string) {
    const offset = marked.indexOf('|')
    assert.notEqual(offset, -1, 'marked text needs a caret')
    return scanSnbt(marked.slice(0, offset) + marked.slice(offset + 1), offset)
}

test('空文档可以写根容器', () => {
    const context = scan('|')
    assert.equal(context.expect, 'value')
    assert.deepEqual(context.keyPath, [])
    assert.equal(context.container, undefined)
})

test('刚打开花括号时期望键名', () => {
    const context = scan('{|}')
    assert.equal(context.expect, 'key')
    assert.deepEqual(context.keyPath, [])
})

test('冒号之后期望值，并记住所属的键', () => {
    const context = scan('{display:|}')
    assert.equal(context.expect, 'value')
    assert.equal(context.key, 'display')
})

test('逗号之后回到键名', () => {
    const context = scan('{a:1,|}')
    assert.equal(context.expect, 'key')
})

test('字符串键名同样能定位到值', () => {
    const context = scan('{"custom name":|}')
    assert.equal(context.expect, 'value')
    assert.equal(context.key, 'custom name')
})

test('值写完之后既不是键也不是值', () => {
    assert.equal(scan('{a:1|}').expect, 'after')
    assert.equal(scan('{a:"x"|}').expect, 'after')
})

test('列表里期望元素，路径停在列表本身', () => {
    const context = scan('{Items:[|]}')
    assert.equal(context.expect, 'element')
    assert.deepEqual(context.keyPath, ['Items'])
})

test('列表元素是复合时，元素的键继续挂在列表路径下', () => {
    const context = scan('{Items:[{id:|}]}')
    assert.equal(context.expect, 'value')
    assert.equal(context.key, 'id')
    assert.deepEqual(context.keyPath, ['Items'])
})

test('嵌套复合会往下走一层路径', () => {
    const context = scan('{display:{Name:|}}')
    assert.deepEqual(context.keyPath, ['display'])
    assert.equal(context.key, 'Name')
})

test('顶到光标的词就是正在输入的片段', () => {
    const context = scan('{dis|}')
    assert.equal(context.expect, 'key')
    assert.equal(context.word, 'dis')
})

test('未闭合的字符串标记为 quoted', () => {
    const context = scan('{a:"minecraft:sto|')
    assert.equal(context.quoted, true)
    assert.equal(context.word, 'minecraft:sto')
})

test('未闭合的字符串只取到光标，不吞掉后面的内容', () => {
    const context = scan('{a:"dia|",b:1}')
    assert.equal(context.quoted, true)
    assert.equal(context.word, 'dia')
})

test('字符串不跨行：上一行落单的引号不会把这一行也算进词里', () => {
    // SNBT 的字符串不能换行，光标前有换行时词只能从行首算起，
    // 否则替换范围会跨行，候选又会被过滤光。
    const context = scan('{a:"x\n{id:"dia|}')
    assert.equal(context.quoted, true)
    assert.equal(context.word, 'dia')
})

test('光标之后的字符用于判断该不该补分隔符', () => {
    assert.equal(scan('{dis|:1}').next, ':')
    assert.equal(scan('{a:1|').next, '')
})

test('根层的 name= 是数据组件列表', () => {
    const context = scan('[minecraft:custom_name=|]')
    assert.equal(context.container?.components, true)
    assert.equal(context.expect, 'value')
    assert.equal(context.key, 'minecraft:custom_name')
})

test('组件列表逗号之后期望下一个组件名', () => {
    const context = scan('[minecraft:custom_name="x",|]')
    assert.equal(context.expect, 'component')
})

test('根层的普通列表不是组件列表', () => {
    assert.equal(scan('[1,|]').container?.components, false)
    assert.equal(scan('[1,|]').expect, 'element')
})

test('记下这一层已经写过的键名', () => {
    assert.deepEqual(scan('{a:1,b:2,|}').container?.keys, ['a', 'b'])
    assert.deepEqual(scan('[minecraft:custom_name="x",|]').container?.keys, [
        'minecraft:custom_name',
    ])
})

test('数组前缀之后是数字', () => {
    const context = scan('{Pos:[B;|]}')
    assert.equal(context.expect, 'value')
    assert.deepEqual(context.keyPath, ['Pos'])
})

test('组件值里的复合继续挂在组件名下', () => {
    const context = scan('[potion_contents={|}]')
    assert.equal(context.expect, 'key')
    assert.deepEqual(context.keyPath, ['potion_contents'])
})

test('组件值里的列表元素沿用组件的路径', () => {
    const context = scan('[banner_patterns=[{|}]')
    assert.equal(context.expect, 'key')
    assert.deepEqual(context.keyPath, ['banner_patterns'])
})
