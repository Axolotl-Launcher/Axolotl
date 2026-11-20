import { describe, expect, it } from 'vitest'

import {
    argumentsForVariable,
    SELECTOR_ARGUMENTS,
    selectorArgumentSpec,
    suggestSelectorArguments,
} from './arguments.ts'
import {
    completionContext,
    formatNumericRange,
    parseAdvancements,
    parseNumericRange,
    parseScores,
    parseSelector,
    validateSelector,
} from './parse.ts'
import {
    draftFromSelector,
    nbtTargetFromSelector,
    removeArgument,
    serializeSelector,
    setArgument,
    toggleNegated,
} from './serialize.ts'

describe('选择器参数表', () => {
    it('只有 Java 版参数', () => {
        const keys = SELECTOR_ARGUMENTS.map((spec) => spec.key)
        for (const bedrockOnly of ['r', 'rm', 'rx', 'm', 'c', 'family', 'hasitem']) {
            expect(keys).not.toContain(bedrockOnly)
        }
        expect(keys).toContain('distance')
        expect(keys).toContain('advancements')
    })

    it('@a / @p / @r 上没有 type', () => {
        for (const variable of ['a', 'p', 'r'] as const) {
            expect(argumentsForVariable(variable).map((spec) => spec.key)).not.toContain('type')
        }
        expect(argumentsForVariable('e').map((spec) => spec.key)).toContain('type')
    })

    it('补全时按前缀过滤并跳过已用的不可重复参数', () => {
        const prefix = suggestSelectorArguments('e', 't').map((spec) => spec.key)
        expect(prefix).toContain('type')
        expect(prefix).toContain('tag')
        expect(prefix).not.toContain('distance')

        const used = suggestSelectorArguments('e', '', ['type'])
        expect(used.map((spec) => spec.key)).not.toContain('type')
        expect(used.map((spec) => spec.key)).toContain('tag')

        expect(suggestSelectorArguments('e', '', ['tag']).map((spec) => spec.key)).toContain('tag')
    })
})

describe('选择器解析', () => {
    it('拆出变量与参数', () => {
        const parsed = parseSelector('@e[type=zombie,limit=3]')!
        expect(parsed.variable).toBe('e')
        expect(parsed.arguments.map((argument) => `${argument.key}=${argument.value}`)).toEqual([
            'type=zombie',
            'limit=3',
        ])
        expect(parsed.error).toBeUndefined()
    })

    it('NBT 里的逗号不当作参数分隔符', () => {
        const parsed = parseSelector('@e[nbt={Tags:["a","b"],Health:10f},limit=1]')!
        expect(parsed.arguments).toHaveLength(2)
        expect(parsed.arguments[0].key).toBe('nbt')
        expect(parsed.arguments[0].value).toBe('{Tags:["a","b"],Health:10f}')
        expect(parsed.arguments[1].value).toBe('1')
    })

    it('引号里的逗号也不切分', () => {
        const parsed = parseSelector('@e[name="a,b"]')!
        expect(parsed.arguments[0].value).toBe('"a,b"')
    })

    it('取反标记单独记下来', () => {
        const parsed = parseSelector('@e[type=!zombie,tag=!hero]')!
        expect(parsed.arguments.map((argument) => argument.negated)).toEqual([true, true])
        expect(parsed.arguments[0].value).toBe('!zombie')
    })

    it('没闭合的方括号不抛异常', () => {
        const parsed = parseSelector('@e[type=zombie')!
        expect(parsed.error).toBe('unclosed')
        expect(parsed.arguments[0].value).toBe('zombie')
    })

    it('没有方括号时只返回变量', () => {
        expect(parseSelector('@s')!.arguments).toEqual([])
        expect(parseSelector('stone')).toBeUndefined()
    })

    it('前导空格不破坏参数位置', () => {
        const text = '  @e[type=zombie]'
        const parsed = parseSelector(text)!
        const argument = parsed.arguments[0]
        expect(text.slice(argument.start, argument.end)).toBe('zombie')
    })

    it('基岩版变量会被标出来', () => {
        expect(parseSelector('@c')!.unknownVariable).toBe('c')
    })
})

describe('参数值解析', () => {
    it('数字区间四种写法', () => {
        expect(parseNumericRange('10')).toEqual({ min: 10, max: 10 })
        expect(parseNumericRange('8..16')).toEqual({ min: 8, max: 16 })
        expect(parseNumericRange('..10')).toEqual({ min: undefined, max: 10 })
        expect(parseNumericRange('10..')).toEqual({ min: 10, max: undefined })
        expect(parseNumericRange('abc')).toBeUndefined()
    })

    it('区间写回去保持原样', () => {
        for (const text of ['10', '8..16', '..10', '10..']) {
            expect(formatNumericRange(parseNumericRange(text)!)).toBe(text)
        }
    })

    it('scores 与 advancements', () => {
        expect(parseScores('{foo=10,bar=1..5}')).toEqual({ foo: '10', bar: '1..5' })
        expect(parseAdvancements('{story/smelt_iron=true}')).toEqual({
            'story/smelt_iron': true,
        })
        expect(parseAdvancements('{adventure/kill_all_mobs={witch=true}}')).toEqual({
            'adventure/kill_all_mobs': { witch: true },
        })
    })
})

describe('逐字符补全', () => {
    it('@e[ 列出全部参数', () => {
        const context = completionContext('@e[', 3)!
        expect(context.insideBrackets).toBe(true)
        expect(context.prefix).toBe('')
        expect(context.usedKeys).toEqual([])
    })

    it('@e[t 只剩 type 与 tag', () => {
        const context = completionContext('@e[t', 4)!
        expect(context.prefix).toBe('t')
        const keys = suggestSelectorArguments(context.variable, context.prefix).map(
            (spec) => spec.key,
        )
        expect(keys).toContain('type')
        expect(keys).not.toContain('limit')
    })

    it('逗号之后跳过已用的参数', () => {
        const context = completionContext('@e[type=zombie,', 16)!
        expect(context.usedKeys).toEqual(['type'])
        expect(context.prefix).toBe('')
    })

    it('取反过的参数不算已用', () => {
        const context = completionContext('@e[name=!a,', 11)!
        expect(context.usedKeys).toEqual([])
    })

    it('@a[ 不出现 type', () => {
        const context = completionContext('@a[', 3)!
        expect(
            suggestSelectorArguments(context.variable, context.prefix).map((s) => s.key),
        ).not.toContain('type')
    })

    it('等号之后知道在填哪个参数的值', () => {
        const context = completionContext('@e[type=', 8)!
        expect(context.expectsValueFor).toBe('type')
    })
})

describe('选择器校验', () => {
    const issuesOf = (text: string) => validateSelector(parseSelector(text)!)

    it('@a 不能带 type', () => {
        const issues = issuesOf('@a[type=zombie]')
        expect(issues.some((issue) => issue.level === 'error' && issue.argument === 'type')).toBe(
            true,
        )
    })

    it('不可重复的参数不能写两次', () => {
        expect(issuesOf('@e[distance=1,distance=2]').some((i) => i.level === 'error')).toBe(true)
        expect(issuesOf('@e[tag=a,tag=b]').some((i) => i.level === 'error')).toBe(false)
        expect(issuesOf('@e[nbt={a:1b},nbt={b:2b}]').some((i) => i.level === 'error')).toBe(false)
    })

    it('取反的出现不算重复', () => {
        // wiki：测试相等的不能重复，测试不等的可以重复
        expect(issuesOf('@e[name=!a,name=!b]').some((i) => i.level === 'error')).toBe(false)
        expect(issuesOf('@e[name=a,name=!b]').some((i) => i.level === 'error')).toBe(false)
        expect(issuesOf('@e[name=a,name=b]').some((i) => i.level === 'error')).toBe(true)
        expect(
            issuesOf('@e[gamemode=!creative,gamemode=!survival]').some((i) => i.level === 'error'),
        ).toBe(false)
    })

    it('type 只能有一个正向值', () => {
        expect(issuesOf('@e[type=creeper,type=pig]').some((i) => i.level === 'error')).toBe(true)
        expect(issuesOf('@e[type=creeper,type=!pig]').some((i) => i.level === 'error')).toBe(true)
        expect(issuesOf('@e[type=!creeper,type=!pig]').some((i) => i.level === 'error')).toBe(false)
    })

    it('基岩版参数给出 Java 版写法', () => {
        const issues = issuesOf('@e[r=10]')
        const issue = issues.find((entry) => entry.argument === 'r')!
        expect(issue.level).toBe('error')
        expect(issue.id).toBe('bedrock-argument')
        expect(issue.values?.equivalent).toBe('distance')
    })

    it('数值范围与枚举', () => {
        expect(issuesOf('@e[distance=abc]').some((i) => i.level === 'error')).toBe(true)
        expect(issuesOf('@e[x_rotation=120]').some((i) => i.level === 'error')).toBe(true)
        expect(issuesOf('@e[sort=wrong]').some((i) => i.level === 'error')).toBe(true)
        expect(issuesOf('@e[sort=random,limit=1]').some((i) => i.level === 'error')).toBe(false)
    })

    it('只写了一边的区间也要检查越界', () => {
        const issue = issuesOf('@e[distance=..-5]').find((i) => i.id === 'below-minimum')
        expect(issue).toBeDefined()
        expect(issuesOf('@e[distance=..10]').some((i) => i.level === 'error')).toBe(false)
        expect(issuesOf('@e[y_rotation=..200]').some((i) => i.id === 'above-maximum')).toBe(true)
        expect(issuesOf('@e[x_rotation=-120..0]').some((i) => i.id === 'below-minimum')).toBe(true)
    })

    it('sort 与 limit 互相提示默认行为', () => {
        expect(issuesOf('@e[sort=nearest]').some((i) => i.level === 'warning')).toBe(true)
        expect(issuesOf('@e[limit=3]').some((i) => i.level === 'warning')).toBe(true)
        expect(issuesOf('@p[limit=3]').some((i) => i.level === 'warning')).toBe(false)
    })
})

describe('选择器拼装', () => {
    it('按 wiki 顺序重排参数', () => {
        const text = serializeSelector({
            variable: 'e',
            arguments: [
                { key: 'limit', value: '3' },
                { key: 'type', value: 'zombie' },
                { key: 'distance', value: '..10' },
            ],
        })
        expect(text).toBe('@e[distance=..10,type=zombie,limit=3]')
    })

    it('未知参数保留在原位置之后', () => {
        const text = serializeSelector({
            variable: 'e',
            arguments: [
                { key: 'custom_mod_arg', value: 'x' },
                { key: 'type', value: 'zombie' },
            ],
        })
        expect(text).toBe('@e[type=zombie,custom_mod_arg=x]')
    })

    it('没有参数时只有变量', () => {
        expect(serializeSelector({ variable: 's', arguments: [] })).toBe('@s')
    })

    it('草稿往返不丢参数，且再拼一次结果不变', () => {
        const text = '@e[type=zombie,nbt={Tags:["a"]},tag=!hero]'
        const once = serializeSelector(draftFromSelector(text)!)
        expect(once).toBe('@e[tag=!hero,type=zombie,nbt={Tags:["a"]}]')
        expect(serializeSelector(draftFromSelector(once)!)).toBe(once)
    })

    it('改参数与删参数', () => {
        let draft = draftFromSelector('@e[type=zombie]')!
        draft = setArgument(draft, 'limit', '1')
        expect(serializeSelector(draft)).toBe('@e[type=zombie,limit=1]')
        draft = setArgument(draft, 'limit', '5')
        expect(serializeSelector(draft)).toBe('@e[type=zombie,limit=5]')
        draft = removeArgument(draft, 'limit')
        expect(serializeSelector(draft)).toBe('@e[type=zombie]')
    })

    it('取反是纯文本翻转', () => {
        expect(toggleNegated('zombie')).toBe('!zombie')
        expect(toggleNegated('!zombie')).toBe('zombie')
    })
})

describe('NBT 上下文联动', () => {
    it('type=<实体> 决定字段集合', () => {
        expect(nbtTargetFromSelector('@e[type=zombie]')).toBe('zombie')
        expect(nbtTargetFromSelector('@e[type=minecraft:zombie]')).toBe('zombie')
    })

    it('取反、标签、多个 type 都退回通用字段', () => {
        expect(nbtTargetFromSelector('@e[type=!zombie]')).toBeUndefined()
        expect(nbtTargetFromSelector('@e[type=#minecraft:undead]')).toBeUndefined()
        expect(nbtTargetFromSelector('@e[type=zombie,type=!pig]')).toBeUndefined()
        expect(nbtTargetFromSelector('@e')).toBeUndefined()
    })

    it('只看玩家的变量按玩家处理', () => {
        expect(nbtTargetFromSelector('@a')).toBe('player')
        expect(nbtTargetFromSelector('@p[distance=..5]')).toBe('player')
    })
})

describe('参数元数据自洽', () => {
    it('键名唯一且都有中文标签', () => {
        const keys = SELECTOR_ARGUMENTS.map((spec) => spec.key)
        expect(new Set(keys).size).toBe(keys.length)
        for (const spec of SELECTOR_ARGUMENTS) {
            expect(spec.zh.length).toBeGreaterThan(0)
            expect(spec.hint.length).toBeGreaterThan(0)
        }
    })

    it('implies 指向真实参数', () => {
        for (const spec of SELECTOR_ARGUMENTS) {
            if (spec.implies) expect(selectorArgumentSpec(spec.implies)).toBeDefined()
        }
    })

    it('predicate 可重复且可取反', () => {
        const spec = selectorArgumentSpec('predicate')!
        expect(spec.repeatable).toBe(true)
        expect(spec.negatable).toBe(true)
    })
})
