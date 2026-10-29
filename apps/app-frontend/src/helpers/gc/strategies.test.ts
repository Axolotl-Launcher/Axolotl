import { expect, it } from 'vitest'

import { buildGcCandidateChain, detectGcStrategy, GC_STRATEGY_DEFINITIONS } from './strategies.ts'

function createContext(overrides: Partial<Parameters<typeof buildGcCandidateChain>[0]> = {}) {
    return {
        javaMajorVersion: 21,
        allocatedMemoryMb: 16384,
        systemCpuCores: 16,
        systemLogicalProcessors: 16,
        modCount: 100,
        loader: 'forge',
        ...overrides,
    }
}

it('Mojang G1GC args include official launcher parameters', () => {
    const args = GC_STRATEGY_DEFINITIONS['g1gc-mojang'].buildArgs()
    expect(args.includes('-XX:+UseG1GC')).toBeTruthy()
    expect(args.includes('-XX:MaxGCPauseMillis=200')).toBeTruthy()
    expect(args.includes('-XX:G1MixedGCCountTarget=4')).toBeTruthy()
    expect(args.includes('-XX:SurvivorRatio=8')).toBeTruthy()
    expect(!args.includes('-XX:G1UncommitBias=1')).toBeTruthy()
})

it('PCL args are Shenandoah-adaptive without large pages', () => {
    const args = GC_STRATEGY_DEFINITIONS.pcl.buildArgs()
    expect(args.includes('-XX:+UseShenandoahGC')).toBeTruthy()
    expect(args.includes('-XX:ShenandoahGCHeuristics=adaptive')).toBeTruthy()
    expect(!args.includes('-XX:+UseLargePages')).toBeTruthy()
})

it('Shenandoah args include large pages', () => {
    const args = GC_STRATEGY_DEFINITIONS.shenandoah.buildArgs()
    expect(args.includes('-XX:+UseShenandoahGC')).toBeTruthy()
    expect(args.includes('-XX:ShenandoahGCHeuristics=adaptive')).toBeTruthy()
    expect(args.includes('-XX:+UseLargePages')).toBeTruthy()
})

it('ZGC args include -XX:+ZGenerational for Java 21+', () => {
    const args = GC_STRATEGY_DEFINITIONS.zgc.buildArgs(createContext({ javaMajorVersion: 21 }))
    expect(args.includes('-XX:+UseZGC')).toBeTruthy()
    expect(args.includes('-XX:+ZGenerational')).toBeTruthy()
    expect(args.includes('-XX:+AlwaysPreTouch')).toBeTruthy()
    expect(args.includes('-XX:-ZUncommit')).toBeTruthy()
})

it('ZGC args do not include -XX:+ZGenerational for Java < 21', () => {
    const args = GC_STRATEGY_DEFINITIONS.zgc.buildArgs(createContext({ javaMajorVersion: 17 }))
    expect(args.includes('-XX:+UseZGC')).toBeTruthy()
    expect(!args.includes('-XX:+ZGenerational')).toBeTruthy()
})

it('detectGcStrategy correctly identifies Mojang G1GC', () => {
    const args = GC_STRATEGY_DEFINITIONS['g1gc-mojang'].buildArgs()
    expect(detectGcStrategy(args)).toBe('g1gc-mojang')
})

it('detectGcStrategy correctly identifies PCL', () => {
    const args = GC_STRATEGY_DEFINITIONS.pcl.buildArgs()
    expect(detectGcStrategy(args)).toBe('pcl')
})

it('detectGcStrategy correctly identifies Shenandoah', () => {
    const args = GC_STRATEGY_DEFINITIONS.shenandoah.buildArgs()
    expect(detectGcStrategy(args)).toBe('shenandoah')
})

it('detectGcStrategy correctly identifies ZGC', () => {
    const args = GC_STRATEGY_DEFINITIONS.zgc.buildArgs()
    expect(detectGcStrategy(args)).toBe('zgc')
})

it('detectGcStrategy returns null for unknown strategy', () => {
    expect(detectGcStrategy('-Xmx4G')).toBe(null)
})

it('Mojang G1GC is not misidentified as PCL or Shenandoah', () => {
    const args = GC_STRATEGY_DEFINITIONS['g1gc-mojang'].buildArgs()
    const detected = detectGcStrategy(args)
    expect(detected).toBe('g1gc-mojang')
    expect(detected).not.toBe('pcl')
})

it('PCL is not misidentified as Mojang or Shenandoah', () => {
    const args = GC_STRATEGY_DEFINITIONS.pcl.buildArgs()
    const detected = detectGcStrategy(args)
    expect(detected).toBe('pcl')
    expect(detected).not.toBe('shenandoah')
    expect(detected).not.toBe('g1gc-mojang')
})

it('Shenandoah is not misidentified as PCL', () => {
    const args = GC_STRATEGY_DEFINITIONS.shenandoah.buildArgs()
    expect(detectGcStrategy(args)).toBe('shenandoah')
})

it('a bare -XX:+UseG1GC is not treated as the full official preset', () => {
    expect(detectGcStrategy('-XX:+UseG1GC')).toBe(null)
})

it('a partial ZGC arg list is not auto-tagged', () => {
    expect(detectGcStrategy('-XX:+UseZGC')).toBe(null)
})

it('a partial Shenandoah arg list is not auto-tagged', () => {
    expect(detectGcStrategy('-XX:+UseShenandoahGC -XX:ShenandoahGCHeuristics=adaptive')).toBe(null)
})

it('ZGC is recognized if the complete base set is present regardless of order', () => {
    const args = '-XX:-ZUncommit -XX:+AlwaysPreTouch -XX:+UseZGC'
    expect(detectGcStrategy(args)).toBe('zgc')
})

it('ZGC with -XX:+ZGenerational on top of the base set is still ZGC', () => {
    const args = GC_STRATEGY_DEFINITIONS.zgc.buildArgs(createContext({ javaMajorVersion: 21 }))
    expect(detectGcStrategy(args)).toBe('zgc')
})

it('PCL complete set with large pages added is Shenandoah, not PCL', () => {
    const args = GC_STRATEGY_DEFINITIONS.pcl.buildArgs() + ' -XX:+UseLargePages'
    expect(detectGcStrategy(args)).toBe('shenandoah')
})

it('buildGcCandidateChain puts preferred first, dedupes, ends at minimal G1', () => {
    const { ids, args } = buildGcCandidateChain(createContext(), 'zgc')
    expect(ids).toEqual(['zgc', 'shenandoah', 'pcl', 'g1gc-mojang', 'minimal-g1'])
    expect(args.length).toBe(ids.length)
    expect(args[args.length - 1]).toEqual(['-XX:+UseG1GC'])
})

it('buildGcCandidateChain starts at a non-ZGC preferred strategy', () => {
    const { ids } = buildGcCandidateChain(createContext(), 'shenandoah')
    expect(ids).toEqual(['shenandoah', 'pcl', 'g1gc-mojang', 'minimal-g1'])
})

it('buildGcCandidateChain for PCL only falls back to G1', () => {
    const { ids } = buildGcCandidateChain(createContext(), 'pcl')
    expect(ids).toEqual(['pcl', 'g1gc-mojang', 'minimal-g1'])
})

it('buildGcCandidateChain never repeats the preferred strategy', () => {
    for (const preferred of ['zgc', 'shenandoah', 'pcl', 'g1gc-mojang']) {
        const { ids } = buildGcCandidateChain(createContext(), preferred)
        expect(ids[0]).toBe(preferred)
        expect(new Set(ids).size).toBe(ids.length)
    }
})
