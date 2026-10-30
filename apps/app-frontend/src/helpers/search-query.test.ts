import { expect, it } from 'vitest'

import {
    compactSearchText,
    curseForgeQueryVariants,
    expandSearchQuery,
    modrinthQueryVariants,
    normalizeSearchText,
    slugifySearchText,
    splitCamelCaseSearchText,
} from './search-query.ts'

it('normalizes whitespace, diacritics and case', () => {
    expect(normalizeSearchText('  Example   Mod  ')).toBe('example mod')
    expect(normalizeSearchText('Bélicraft')).toBe('belicraft')
    expect(normalizeSearchText('Sodium Extra')).toBe('sodium extra')
})

it('compacts queries to alphanumerics only', () => {
    expect(compactSearchText('Example Mod!')).toBe('examplemod')
    expect(compactSearchText('sodium_extra')).toBe('sodiumextra')
    expect(compactSearchText('铁 锭')).toBe('铁锭')
})

it('slugifies queries for CurseForge', () => {
    expect(slugifySearchText('Example Mod!')).toBe('example-mod')
    expect(slugifySearchText('Sodium Extra')).toBe('sodium-extra')
    expect(slugifySearchText('--sodium--')).toBe('sodium')
})

it('splits camelCase and PascalCase words', () => {
    expect(splitCamelCaseSearchText('SodiumExtra')).toBe('Sodium Extra')
    expect(splitCamelCaseSearchText('AE2Stuff')).toBe('AE2 Stuff')
    expect(splitCamelCaseSearchText('sodiumextra')).toBe('sodiumextra')
})

it('modrinth variants preserve the typed form first', () => {
    expect(modrinthQueryVariants('example mod')).toEqual(['example mod', 'examplemod'])
    expect(modrinthQueryVariants('sodium-extra')).toEqual([
        'sodium-extra',
        'sodium extra',
        'sodiumextra',
    ])
    expect(modrinthQueryVariants('SodiumExtra')).toEqual(['sodiumextra', 'sodium extra'])
    expect(modrinthQueryVariants('')).toEqual([])
})

it('curseforge variants put the slug form first', () => {
    expect(curseForgeQueryVariants('example mod')).toEqual([
        'example-mod',
        'example mod',
        'examplemod',
    ])
    expect(curseForgeQueryVariants('sodium-extra')).toEqual(['sodium-extra', 'sodiumextra'])
    expect(curseForgeQueryVariants('SodiumExtra')).toEqual(['sodiumextra', 'sodium-extra'])
    expect(curseForgeQueryVariants('sodium extra')).toEqual([
        'sodium-extra',
        'sodium extra',
        'sodiumextra',
    ])
    expect(curseForgeQueryVariants('')).toEqual([])
})

it('expandSearchQuery returns null for empty and whitespace-only queries', () => {
    expect(expandSearchQuery('')).toBe(null)
    expect(expandSearchQuery('   ')).toBe(null)
})

it('expandSearchQuery flags compact queries', () => {
    const expansion = expandSearchQuery('sodiumextra')
    expect(expansion).toBeTruthy()
    expect(expansion.compact).toBe(true)
    expect(expansion.normalized).toBe('sodiumextra')
    expect(expansion.modrinthVariants).toEqual(['sodiumextra'])
    expect(expansion.curseforgeVariants).toEqual(['sodiumextra'])
})

it('expandSearchQuery never treats spaced queries as compact', () => {
    const expansion = expandSearchQuery('sodium extra')
    expect(expansion).toBeTruthy()
    expect(expansion.compact).toBe(false)
})

it('expansion of hyphenated queries covers both providers', () => {
    const expansion = expandSearchQuery('sodium-extra')
    expect(expansion).toBeTruthy()
    expect(expansion.modrinthVariants).toEqual(['sodium-extra', 'sodium extra', 'sodiumextra'])
    expect(expansion.curseforgeVariants).toEqual(['sodium-extra', 'sodiumextra'])
})

it('expansion dedupes and caps variants', () => {
    const expansion = expandSearchQuery('a b c')
    expect(expansion).toBeTruthy()
    expect(expansion.modrinthVariants.length <= 3).toBeTruthy()
    expect(expansion.curseforgeVariants.length <= 3).toBeTruthy()
    expect(new Set(expansion.modrinthVariants).size).toBe(expansion.modrinthVariants.length)
})
