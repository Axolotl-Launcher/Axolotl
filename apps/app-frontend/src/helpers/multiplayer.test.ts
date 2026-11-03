import { expect, it } from 'vitest'

import {
    type DetectedLanPort,
    type HongshiNode,
    multiplayerErrorText,
    selectedDetectedInstance,
    selectedNodePreference,
    storedMultiplayerProvider,
    validLocalPort,
} from './multiplayer.ts'

const detectedPort = (instanceId: string): DetectedLanPort => ({
    instance_id: instanceId,
    instance_name: `Instance ${instanceId}`,
    process_id: `process-${instanceId}`,
    port: 25565,
    detected_at: '2026-08-12 12:00:00',
})

const node = (name: string): HongshiNode => ({
    name,
    address: '203.0.113.1',
    latency_ms: 20,
    reachable: true,
    cached: false,
})

it('defaults the provider preference to Terracotta', () => {
    expect(storedMultiplayerProvider(null)).toBe('terracotta')
    expect(storedMultiplayerProvider('invalid')).toBe('terracotta')
    expect(storedMultiplayerProvider('hongshi')).toBe('hongshi')
})

it('accepts only complete ports in the Minecraft port range', () => {
    expect(validLocalPort('1')).toBe(1)
    expect(validLocalPort('65535')).toBe(65535)
    for (const value of ['', '0', '65536', '25565abc', ' 25565']) {
        expect(validLocalPort(value)).toBe(null)
    }
})

it('auto-selects exactly one detected instance and preserves valid choices', () => {
    expect(selectedDetectedInstance('manual', [detectedPort('a')])).toBe('a')
    expect(selectedDetectedInstance('a', [detectedPort('a'), detectedPort('b')])).toBe('a')
    expect(selectedDetectedInstance('missing', [detectedPort('a'), detectedPort('b')])).toBe(
        'manual',
    )
})

it('falls back to automatic node selection when a cached preference disappears', () => {
    expect(selectedNodePreference('Nanjing', [node('Nanjing')])).toBe('Nanjing')
    expect(selectedNodePreference('Missing', [node('Nanjing')])).toBe('auto')
})

it('reads cancellation messages from serialized Tauri errors', () => {
    expect(
        multiplayerErrorText({ field_name: 'Theseus', message: 'RedStone operation cancelled' }),
    ).toBe('RedStone operation cancelled')
    expect(multiplayerErrorText('RedStone operation cancelled')).toBe(
        'RedStone operation cancelled',
    )
})
