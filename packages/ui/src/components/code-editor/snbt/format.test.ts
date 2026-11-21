import assert from 'node:assert/strict'
import { test } from 'node:test'

import { formatSnbt } from './format.ts'

test('复合标签一行一个键', () => {
    assert.equal(formatSnbt('{a:1,b:2}'), '{\n    a:1,\n    b:2\n}')
})

test('嵌套按层缩进', () => {
    assert.equal(formatSnbt('{a:{b:1}}'), '{\n    a:{\n        b:1\n    }\n}')
})

test('列表的元素各占一行', () => {
    assert.equal(formatSnbt('[1,2]'), '[\n    1,\n    2\n]')
})

test('空容器保持一行', () => {
    assert.equal(formatSnbt('{}'), '{}')
    assert.equal(formatSnbt('{a:[]}'), '{\n    a:[]\n}')
})

test('数组不拆行', () => {
    assert.equal(formatSnbt('[B;1b,2b]'), '[B;1b,2b]')
    assert.equal(formatSnbt('{Pos:[L;1L,2L]}'), '{\n    Pos:[L;1L,2L]\n}')
})

test('字符串里的逗号和括号不算结构', () => {
    assert.equal(formatSnbt('{a:"x,y",b:"}"}'), '{\n    a:"x,y",\n    b:"}"\n}')
})

test('保留原来的写法', () => {
    assert.equal(
        formatSnbt('{id:"minecraft:stone",Health:20.0f,on:1b}'),
        '{\n    id:"minecraft:stone",\n    Health:20.0f,\n    on:1b\n}',
    )
})

test('组件列表按根层缩进', () => {
    assert.equal(
        formatSnbt('[custom_name="a",lore=["b"]]'),
        '[\n    custom_name="a",\n    lore=[\n        "b"\n    ]\n]',
    )
})

test('已经排好版的文本再排一次不变', () => {
    const formatted = formatSnbt('{a:1,b:[2,3]}')
    assert.equal(formatSnbt(formatted), formatted)
})

test('多余空白会被收掉', () => {
    assert.equal(formatSnbt('{\n  a : 1 ,\n  b : 2\n}'), '{\n    a:1,\n    b:2\n}')
})

test('残缺的文本也能排版', () => {
    // 还没闭合：光标该待的地方（缩进后的新行）留着。
    assert.equal(formatSnbt('{a:1,'), '{\n    a:1,\n    ')
    assert.equal(formatSnbt(''), '')
})

test('尾逗号不会空出一行', () => {
    assert.equal(formatSnbt('{a:1,}'), '{\n    a:1,\n}')
})
