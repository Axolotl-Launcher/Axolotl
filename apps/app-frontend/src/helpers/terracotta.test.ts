import { expect, it } from 'vitest'

import { parseTerracottaPublicNodes } from './terracotta.ts'

it('parses Terracotta public nodes from lines and commas', () => {
    expect(
        parseTerracottaPublicNodes(
            'wss://center.node.1tmc.top\ntcp://example.com:11010, udp://example.net:11010',
        ),
    ).toEqual({
        nodes: ['wss://center.node.1tmc.top', 'tcp://example.com:11010', 'udp://example.net:11010'],
        invalidNode: null,
    })
})

it('allows an empty Terracotta public node list', () => {
    expect(parseTerracottaPublicNodes('  \n')).toEqual({ nodes: [], invalidNode: null })
})

it('rejects unsupported or incomplete Terracotta public nodes', () => {
    expect(parseTerracottaPublicNodes('ftp://example.com').invalidNode).toBe('ftp://example.com')
    expect(parseTerracottaPublicNodes('wss://').invalidNode).toBe('wss://')
})
