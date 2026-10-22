export type PackFormatRange = {
    min: string
    max?: string
}

/**
 * Data pack `pack_format` → Minecraft version range (1.13+). Data pack formats
 * are independent of resource pack formats; keep this table in sync with the
 * game's data pack format history.
 */
const PACK_FORMAT_TO_VERSION: Record<number, PackFormatRange> = {
    4: { min: '1.13', max: '1.14.4' },
    5: { min: '1.15', max: '1.15.2' },
    6: { min: '1.16', max: '1.16.1' },
    7: { min: '1.16.2', max: '1.16.5' },
    8: { min: '1.17', max: '1.17.1' },
    9: { min: '1.18', max: '1.18.2' },
    10: { min: '1.19', max: '1.19.2' },
    12: { min: '1.19.3', max: '1.19.3' },
    13: { min: '1.19.4', max: '1.19.4' },
    15: { min: '1.20', max: '1.20.1' },
    18: { min: '1.20.2', max: '1.20.2' },
    26: { min: '1.20.3', max: '1.20.4' },
    41: { min: '1.20.5', max: '1.20.6' },
    48: { min: '1.21', max: '1.21.1' },
    57: { min: '1.21.2', max: '1.21.3' },
    61: { min: '1.21.4', max: '1.21.4' },
    71: { min: '1.21.5', max: '1.21.5' },
    80: { min: '1.21.6', max: '1.21.6' },
    81: { min: '1.21.7', max: '1.21.7' },
    88: { min: '1.21.9', max: '1.21.9' },
    94: { min: '1.21.11', max: '1.21.11' },
    101: { min: '26.1', max: '26.1' },
    107: { min: '26.2', max: '26.2' },
    121: { min: '26.3', max: '26.3' },
}

export function getPackFormatRange(packFormat?: number): PackFormatRange | undefined {
    if (packFormat == null) return undefined
    return PACK_FORMAT_TO_VERSION[packFormat]
}

/** Formats normalized major/minor bounds without discarding unknown formats. */
export function formatDatapackVersion(pack: {
    pack_format?: number
    min_format?: [number, number]
    max_format?: [number, number]
}): string {
    const min = pack.min_format ?? (pack.pack_format == null ? undefined : [pack.pack_format, 0])
    if (!min) return ''
    const max = pack.max_format ?? min
    const label = (format: number[], bound: 'min' | 'max') => {
        if (format[1] === 2_147_483_647) return `${format[0]}.*`
        const minor = ({ 88: 0, 94: 1, 101: 1, 107: 1, 121: 0 } as Record<number, number>)[
            format[0]
        ]
        const versions = (minor ?? 0) === format[1] ? getPackFormatRange(format[0]) : undefined
        return versions?.[bound] ?? versions?.min ?? `${format[0]}.${format[1]}`
    }
    const first = label(min, 'min')
    const last = label(max, 'max')
    return first === last ? first : `${first}–${last}`
}
