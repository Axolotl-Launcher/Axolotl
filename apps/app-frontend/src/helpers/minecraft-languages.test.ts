import { expect, it } from 'vitest'

import { minecraftLanguageOptions } from './minecraft-languages'

it('provides unique, searchable Minecraft language options sorted by label', () => {
    const values = minecraftLanguageOptions.map((option) => option.value)

    expect(new Set(values).size).toBe(values.length)
    expect(values).toContain('en_us')
    expect(values).toContain('zh_cn')

    for (const option of minecraftLanguageOptions) {
        expect(option.searchTerms).toContain(option.value)
    }

    for (let index = 1; index < minecraftLanguageOptions.length; index += 1) {
        expect(
            minecraftLanguageOptions[index - 1].label.localeCompare(
                minecraftLanguageOptions[index].label,
            ),
        ).toBeLessThanOrEqual(0)
    }
})
