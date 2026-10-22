import type { ResourceSearchExpression, SourceId } from './resources'

export async function loadResourceSearchCategories(
    source: SourceId | 'all',
    load: () => Promise<void>,
): Promise<string | null> {
    if (source !== 'all' && source !== 'curseforge') return null
    try {
        await load()
        return null
    } catch (error) {
        if (source === 'curseforge') throw error
        return `CurseForge: ${error instanceof Error ? error.message : String(error)}`
    }
}

const fieldNames: Record<string, string> = {
    project_types: 'type',
    all_project_types: 'allTypes',
    categories: 'category',
    game_versions: 'gameVersion',
    environment: 'environment',
    license: 'license',
    open_source: 'openSource',
    project_id: 'projectId',
    compatible_dependency_project_ids: 'dependencyProject',
    required_dependency_project_ids: 'requiredDependency',
    optional_dependency_project_ids: 'optionalDependency',
    incompatible_dependency_project_ids: 'incompatibleDependency',
    embedded_dependency_project_ids: 'embeddedDependency',
}

/** Converts the shared browse filter language into the neutral boolean contract. */
export function parseResourceSearchFilter(
    input: string,
    categoryValues: (value: string) => string[] = (value) => [value],
    isLoader: (value: string) => boolean = () => false,
): ResourceSearchExpression | null {
    if (!input.trim()) return null
    const pattern =
        /\s*(`(?:\\.|[^`])*`|"(?:\\.|[^"])*"|!=|=|\(|\)|\[|\]|,|[a-zA-Z_][a-zA-Z_0-9.]*)/gy
    const tokens: string[] = []
    let position = 0
    while (position < input.trimEnd().length) {
        pattern.lastIndex = position
        const token = pattern.exec(input)
        if (!token) throw new Error('Invalid browse search filter')
        tokens.push(token[1])
        position = pattern.lastIndex
    }
    let index = 0
    const take = () => tokens[index++]
    const consume = (expected: string) => {
        if (take() !== expected) throw new Error('Invalid browse search filter')
    }
    const value = () => {
        const token = take()
        if (token === 'true' || token === 'false') return token
        if (!token || !['`', '"'].includes(token[0])) throw new Error('Invalid search filter value')
        return token[0] === '"'
            ? (JSON.parse(token) as string)
            : token.slice(1, -1).replace(/\\(.)/g, '$1')
    }
    function predicate(depth: number): ResourceSearchExpression {
        if (depth > 16) throw new Error('Search filter nesting exceeded')
        if (tokens[index] === '(') {
            take()
            const group = any(depth + 1)
            consume(')')
            return group
        }
        const originalField = take()
        const field = fieldNames[originalField]
        if (!field) throw new Error('Unsupported search filter field')
        let operator = take()
        if (operator === 'NOT') {
            consume('IN')
            operator = 'NOT IN'
        }
        if (!['=', '!=', 'IN', 'NOT IN'].includes(operator))
            throw new Error('Invalid search filter operator')
        const values: string[] = []
        if (operator.endsWith('IN')) {
            consume('[')
            values.push(value())
            while (tokens[index] === ',') {
                take()
                values.push(value())
            }
            consume(']')
        } else {
            values.push(value())
        }
        const loader = field === 'category' && values.every(isLoader)
        return {
            kind: 'predicate',
            field: loader ? 'loader' : field,
            values: field === 'category' && !loader ? values.flatMap(categoryValues) : values,
            excluded: operator === '!=' || operator === 'NOT IN',
        }
    }
    function all(depth: number): ResourceSearchExpression {
        const terms = [predicate(depth)]
        while (tokens[index] === 'AND') {
            take()
            terms.push(predicate(depth))
        }
        return terms.length === 1 ? terms[0] : { kind: 'all', terms }
    }
    function any(depth: number): ResourceSearchExpression {
        const terms = [all(depth)]
        while (tokens[index] === 'OR') {
            take()
            terms.push(all(depth))
        }
        return terms.length === 1 ? terms[0] : { kind: 'any', terms }
    }
    const result = any(0)
    if (index !== tokens.length) throw new Error('Incomplete browse search filter')
    return result
}
