import { expect, it } from 'vitest'

import { resolveAutoGcStrategy } from './auto-selector.ts'
import type { GcContext, GcReason } from './types.ts'

function createContext(overrides: Partial<GcContext> = {}): GcContext {
    return {
        javaMajorVersion: 21,
        allocatedMemoryMb: 8192,
        systemCpuCores: 8,
        systemLogicalProcessors: 8,
        modCount: 50,
        loader: 'forge',
        ...overrides,
    }
}

function reasonIds(result: { reasonChain: (string | GcReason)[] }): string[] {
    return result.reasonChain.flatMap((entry) => (typeof entry === 'string' ? [] : [entry.id]))
}

it('hard fallback: unknown Java version falls back to G1GC', () => {
    const context = createContext({ javaMajorVersion: null })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('g1gc-mojang')
    expect(
        reasonIds(result).includes('app.java-arguments.gc.reason.java-version-unknown'),
    ).toBeTruthy()
})

it('hard fallback: Java < 15 falls back to G1GC', () => {
    const context = createContext({ javaMajorVersion: 11 })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('g1gc-mojang')
    expect(reasonIds(result).includes('app.java-arguments.gc.reason.java-too-old')).toBeTruthy()
})

it('hard fallback: memory < 4GB falls back to G1GC', () => {
    const context = createContext({ allocatedMemoryMb: 2048 })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('g1gc-mojang')
    expect(
        reasonIds(result).includes('app.java-arguments.gc.reason.insufficient-memory'),
    ).toBeTruthy()
})

it('hard fallback: insufficient CPU resources falls back to G1GC', () => {
    const context = createContext({
        systemCpuCores: 4,
        systemLogicalProcessors: 4,
    })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('g1gc-mojang')
    expect(reasonIds(result).includes('app.java-arguments.gc.reason.insufficient-cpu')).toBeTruthy()
})

it('hard fallback: large modpack with insufficient resources falls back to G1GC', () => {
    const context = createContext({
        modCount: 200,
        allocatedMemoryMb: 6144,
    })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('g1gc-mojang')
    expect(
        reasonIds(result).includes('app.java-arguments.gc.reason.large-modpack-low-memory'),
    ).toBeTruthy()
})

it('lightweight vanilla instance selects G1GC', () => {
    const context = createContext({
        loader: 'vanilla',
        modCount: 5,
    })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('g1gc-mojang')
    expect(
        reasonIds(result).includes('app.java-arguments.gc.reason.lightweight-instance'),
    ).toBeTruthy()
})

it('lightweight fabric instance selects G1GC', () => {
    const context = createContext({
        loader: 'fabric',
        modCount: 20,
    })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('g1gc-mojang')
    expect(
        reasonIds(result).includes('app.java-arguments.gc.reason.lightweight-instance'),
    ).toBeTruthy()
})

it('low resources selects G1GC', () => {
    const context = createContext({
        allocatedMemoryMb: 6143,
        systemCpuCores: 6,
    })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('g1gc-mojang')
    expect(reasonIds(result).includes('app.java-arguments.gc.reason.low-resources')).toBeTruthy()
})

it('medium resources selects Shenandoah', () => {
    const context = createContext({
        allocatedMemoryMb: 8192,
        systemCpuCores: 8,
    })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('shenandoah')
    expect(reasonIds(result).includes('app.java-arguments.gc.reason.medium-resources')).toBeTruthy()
})

it('high resources with Java < 21 selects Shenandoah', () => {
    const context = createContext({
        javaMajorVersion: 17,
        allocatedMemoryMb: 16384,
        systemCpuCores: 16,
    })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('shenandoah')
    expect(
        reasonIds(result).includes('app.java-arguments.gc.reason.zgc-non-generational'),
    ).toBeTruthy()
})

it('high resources with Java >= 21 selects ZGC', () => {
    const context = createContext({
        javaMajorVersion: 21,
        allocatedMemoryMb: 16384,
        systemCpuCores: 16,
    })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('zgc')
    expect(
        reasonIds(result).includes('app.java-arguments.gc.reason.ample-memory-high-cores'),
    ).toBeTruthy()
})

it('Java 21 with insufficient resources for ZGC selects Shenandoah', () => {
    const context = createContext({
        javaMajorVersion: 21,
        allocatedMemoryMb: 10240,
        systemCpuCores: 12,
    })
    const result = resolveAutoGcStrategy(context)
    expect(result.resolvedStrategy).toBe('shenandoah')
    expect(
        reasonIds(result).includes('app.java-arguments.gc.reason.below-zgc-recommendation'),
    ).toBeTruthy()
})

it('reason chain contains all decision nodes', () => {
    const context = createContext({
        javaMajorVersion: 21,
        allocatedMemoryMb: 16384,
        systemCpuCores: 16,
        modCount: 100,
        loader: 'forge',
    })
    const result = resolveAutoGcStrategy(context)
    expect(result.reasonChain.length > 0).toBeTruthy()
    expect(result.reasonChain[0]).toBe('Java 21')
})
