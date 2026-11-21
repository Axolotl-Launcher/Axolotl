/**
 * 选择器诊断的文案。
 *
 * 单独成文件的原因与 `nbt/group-messages.ts` 相同：`formatjs extract` 只认
 * `defineMessages` 里的静态 ID。`parse.ts` 是纯函数，只产出稳定的 issue ID 与
 * 参数，这里负责把 ID 映射成可翻译的描述符，组件再用 `formatMessage` 渲染。
 */

import { defineMessages } from '@modrinth/ui'

import type { SelectorIssueId } from './parse.ts'

export const selectorIssueMessages = defineMessages({
    notASelector: {
        id: 'app.lab.command-generator.selector.issue.not-a-selector',
        defaultMessage: 'A target selector starts with @, for example @e[type=zombie]',
    },
    unknownVariable: {
        id: 'app.lab.command-generator.selector.issue.unknown-variable',
        defaultMessage: '@{variable} is not a Java Edition target variable',
    },
    unclosedBrackets: {
        id: 'app.lab.command-generator.selector.issue.unclosed-brackets',
        defaultMessage: 'The square brackets are not closed',
    },
    bedrockArgument: {
        id: 'app.lab.command-generator.selector.issue.bedrock-argument',
        defaultMessage: 'Java Edition has no {key}=, use {equivalent}= instead',
    },
    unknownArgument: {
        id: 'app.lab.command-generator.selector.issue.unknown-argument',
        defaultMessage: 'Unknown argument {key}=; it is kept as written',
    },
    forbiddenOnVariable: {
        id: 'app.lab.command-generator.selector.issue.forbidden-on-variable',
        defaultMessage: '@{variable} cannot use {key}=, use @e or @n instead',
    },
    notANumber: {
        id: 'app.lab.command-generator.selector.issue.not-a-number',
        defaultMessage: '{key}= is not a number or a range',
    },
    notASingleNumber: {
        id: 'app.lab.command-generator.selector.issue.not-a-single-number',
        defaultMessage: '{key}= takes a single number, not a range',
    },
    notNegatable: {
        id: 'app.lab.command-generator.selector.issue.not-negatable',
        defaultMessage: '{key}= cannot be negated with !',
    },
    belowMinimum: {
        id: 'app.lab.command-generator.selector.issue.below-minimum',
        defaultMessage: '{key}= cannot be less than {min}',
    },
    aboveMaximum: {
        id: 'app.lab.command-generator.selector.issue.above-maximum',
        defaultMessage: '{key}= cannot be greater than {max}',
    },
    invalidEnum: {
        id: 'app.lab.command-generator.selector.issue.invalid-enum',
        defaultMessage: '{key}= must be one of {values}',
    },
    duplicateType: {
        id: 'app.lab.command-generator.selector.issue.duplicate-type',
        defaultMessage: 'type= can only appear once',
    },
    typeConflict: {
        id: 'app.lab.command-generator.selector.issue.type-conflict',
        defaultMessage: 'A positive type= and a negated type= cannot be used together',
    },
    duplicateArgument: {
        id: 'app.lab.command-generator.selector.issue.duplicate-argument',
        defaultMessage: '{key}= cannot be repeated',
    },
    sortWithoutLimit: {
        id: 'app.lab.command-generator.selector.issue.sort-without-limit',
        defaultMessage: 'Without limit=, sort= does not change how many targets are selected',
    },
    limitWithoutSort: {
        id: 'app.lab.command-generator.selector.issue.limit-without-sort',
        defaultMessage: 'Without sort=, the first targets are taken in arbitrary order',
    },
})

/** issue ID → `selectorIssueMessages` 里的键。漏一条编译期就会报错。 */
export const SELECTOR_ISSUE_MESSAGE: Record<SelectorIssueId, keyof typeof selectorIssueMessages> = {
    'not-a-selector': 'notASelector',
    'unknown-variable': 'unknownVariable',
    'unclosed-brackets': 'unclosedBrackets',
    'bedrock-argument': 'bedrockArgument',
    'unknown-argument': 'unknownArgument',
    'forbidden-on-variable': 'forbiddenOnVariable',
    'not-a-number': 'notANumber',
    'not-a-single-number': 'notASingleNumber',
    'not-negatable': 'notNegatable',
    'below-minimum': 'belowMinimum',
    'above-maximum': 'aboveMaximum',
    'invalid-enum': 'invalidEnum',
    'duplicate-type': 'duplicateType',
    'type-conflict': 'typeConflict',
    'duplicate-argument': 'duplicateArgument',
    'sort-without-limit': 'sortWithoutLimit',
    'limit-without-sort': 'limitWithoutSort',
}
