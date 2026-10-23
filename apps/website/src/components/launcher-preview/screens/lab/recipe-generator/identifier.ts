export type ItemRef = {
    namespace: string
    id: string
    data?: number
}

export function parseIdentifier(raw: string, data?: number): ItemRef {
    const firstColon = raw.indexOf(':')
    if (firstColon < 0) {
        return { namespace: 'minecraft', id: raw, ...(data !== undefined ? { data } : {}) }
    }

    const namespace = raw.slice(0, firstColon)
    const rest = raw.slice(firstColon + 1)
    const lastColon = rest.lastIndexOf(':')
    const maybeData = Number(rest.slice(lastColon + 1))
    const hasLegacyData = lastColon >= 0 && Number.isInteger(maybeData)

    if (hasLegacyData) {
        return {
            namespace,
            id: rest.slice(0, lastColon),
            data: data ?? maybeData,
        }
    }

    return { namespace, id: rest, ...(data !== undefined ? { data } : {}) }
}

export function rawId(ref: ItemRef): string {
    return `${ref.namespace}:${ref.id}`
}

export function fullId(ref: ItemRef): string {
    return `${rawId(ref)}${ref.data === undefined ? '' : `:${ref.data}`}`
}
