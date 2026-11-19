// @vitest-environment happy-dom
import { beforeEach, expect, it } from 'vitest'

import { applyCustomThemeColors } from './theme.ts'

const dark = { background: '#101215', surface: '#262b34', text: '#ffffff' }
const light = { background: '#ebebeb', surface: '#c9c9c9', text: '#111111' }
const CUSTOM_VARIABLES = [
    '--surface-1',
    '--surface-1-5',
    '--surface-2',
    '--surface-2-5',
    '--surface-3',
    '--surface-4',
    '--surface-5',
    '--color-text-primary',
    '--color-text-default',
    '--color-text-tertiary',
]

beforeEach(() => {
    document.documentElement.removeAttribute('style')
    document.documentElement.className = ''
})

function styleValue(variable: string) {
    return document.documentElement.style.getPropertyValue(variable)
}

function writtenVariables() {
    return document.documentElement.style.cssText
        .split(';')
        .map((declaration) => declaration.split(':')[0]?.trim() ?? '')
        .filter((name) => name.startsWith('--'))
        .sort()
}

it('writes the derived palette to the document and removes it again', () => {
    applyCustomThemeColors('dark', { dark })

    expect(writtenVariables()).toEqual([...CUSTOM_VARIABLES].sort())
    expect(styleValue('--surface-1')).toBe('#101215')
    expect(styleValue('--surface-3')).toBe('#262b34')
    expect(styleValue('--color-text-primary')).toBe('#ffffff')
    expect(styleValue('--surface-5')).not.toBe('')

    applyCustomThemeColors('dark', null)

    for (const variable of CUSTOM_VARIABLES) {
        expect(styleValue(variable), variable).toBe('')
    }
})

it('chooses the bucket from the rendered system theme', () => {
    applyCustomThemeColors('system', { dark, light })
    expect(styleValue('--surface-1')).toBe('#101215')

    document.documentElement.className = 'light-mode'
    applyCustomThemeColors('system', { dark, light })
    expect(styleValue('--surface-1')).toBe('#ebebeb')
})

it('clears overrides when the active bucket has no palette', () => {
    applyCustomThemeColors('dark', { dark })
    applyCustomThemeColors('dark', { light })

    for (const variable of CUSTOM_VARIABLES) {
        expect(styleValue(variable), variable).toBe('')
    }
})
