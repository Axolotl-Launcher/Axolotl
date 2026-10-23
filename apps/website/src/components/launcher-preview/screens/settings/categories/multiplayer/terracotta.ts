/**
 * Terracotta public-node parsing (apps/app-frontend/src/helpers/terracotta.ts)
 * and the settings patch's own check (`validate_public_nodes`,
 * packages/app-lib/src/api/terracotta.rs).
 */
const TERRACOTTA_PUBLIC_NODE_SCHEMES = new Set([
    'http:',
    'https:',
    'tcp:',
    'tls:',
    'udp:',
    'ws:',
    'wss:',
])

export function parseTerracottaPublicNodes(value: string): {
    nodes: string[]
    invalidNode: string | null
} {
    const nodes = value
        .split(/[\n,]+/)
        .map((node) => node.trim())
        .filter(Boolean)

    for (const node of nodes) {
        try {
            const url = new URL(node)
            if (!TERRACOTTA_PUBLIC_NODE_SCHEMES.has(url.protocol) || !url.hostname) {
                return { nodes, invalidNode: node }
            }
        } catch {
            return { nodes, invalidNode: node }
        }
    }

    return { nodes, invalidNode: null }
}

/** `validate_public_nodes`: the error the settings patch fails with, or null. */
export function validatePublicNodes(nodes: string[]): string | null {
    for (const node of nodes) {
        let url: URL
        try {
            url = new URL(node)
        } catch {
            return `invalid terracotta public node: ${node}`
        }
        if (!TERRACOTTA_PUBLIC_NODE_SCHEMES.has(url.protocol) || !url.hostname) {
            return `unsupported terracotta public node: ${node}`
        }
    }
    return null
}
