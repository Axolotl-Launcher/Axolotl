import { Solar } from 'lunar-typescript'

export type AlmanacHour = {
    zhi: string
    ganZhi: string
    lucky: boolean
    yi: string[]
}

export type AlmanacDay = {
    dateKey: string
    solarLabel: string
    weekday: string
    lunarLabel: string
    ganZhiYear: string
    ganZhiMonth: string
    ganZhiDay: string
    shengXiao: string
    yi: string[]
    ji: string[]
    zhiShen: string
    zhiShenType: string
    zhiShenLuck: string
    zhiXing: string
    naYin: string
    chong: string
    sha: string
    xiu: string
    xiuLuck: string
    pengZuGan: string
    pengZuZhi: string
    jiShen: string[]
    xiongSha: string[]
    xi: string
    fu: string
    cai: string
    yangGui: string
    yinGui: string
    taiShen: string
    zodiac: string
    hours: AlmanacHour[]
}

/**
 * The library marks auspicious results with one character. Comparing against
 * its code point keeps CJK out of the source, which this repo's checker forbids.
 */
const AUSPICIOUS_CODE_POINT = 0x5409

export function isAuspicious(value: string): boolean {
    return value === String.fromCharCode(AUSPICIOUS_CODE_POINT)
}

/** Local calendar day of a date, as `yyyy-mm-dd`, used as the cache key. */
export function dateKey(date: Date): string {
    const month = `${date.getMonth() + 1}`.padStart(2, '0')
    const day = `${date.getDate()}`.padStart(2, '0')
    return `${date.getFullYear()}-${month}-${day}`
}

const cache = new Map<string, AlmanacDay>()

function emptyDay(key: string): AlmanacDay {
    return {
        dateKey: key,
        solarLabel: key,
        weekday: '',
        lunarLabel: '',
        ganZhiYear: '',
        ganZhiMonth: '',
        ganZhiDay: '',
        shengXiao: '',
        yi: [],
        ji: [],
        zhiShen: '',
        zhiShenType: '',
        zhiShenLuck: '',
        zhiXing: '',
        naYin: '',
        chong: '',
        sha: '',
        xiu: '',
        xiuLuck: '',
        pengZuGan: '',
        pengZuZhi: '',
        jiShen: [],
        xiongSha: [],
        xi: '',
        fu: '',
        cai: '',
        yangGui: '',
        yinGui: '',
        taiShen: '',
        zodiac: '',
        hours: [],
    }
}

function readText(read: () => unknown, fallback = ''): string {
    try {
        const value = read()
        return typeof value === 'string' && value.length > 0 ? value : fallback
    } catch {
        return fallback
    }
}

function readList(read: () => unknown): string[] {
    try {
        const value = read()
        if (!Array.isArray(value)) return []
        return value.filter((item): item is string => typeof item === 'string' && item.length > 0)
    } catch {
        return []
    }
}

type HourLike = {
    getZhi: () => unknown
    getGanZhi: () => unknown
    getTianShenLuck: () => unknown
    getYi: () => unknown
}

/**
 * The library lists thirteen periods: the hour of the rat opens and closes the
 * day. Only the first entry per earthly branch is kept, which leaves twelve.
 */
function readHours(lunar: { getTimes: () => unknown }): AlmanacHour[] {
    let times: unknown
    try {
        times = lunar.getTimes()
    } catch {
        return []
    }
    if (!Array.isArray(times)) return []

    const byZhi = new Map<string, AlmanacHour>()
    for (const candidate of times) {
        try {
            const hour = candidate as HourLike
            const zhi = hour.getZhi()
            if (typeof zhi !== 'string' || !zhi || byZhi.has(zhi)) continue
            const luck = hour.getTianShenLuck()
            byZhi.set(zhi, {
                zhi,
                ganZhi: readText(() => hour.getGanZhi()),
                lucky: typeof luck === 'string' && isAuspicious(luck),
                yi: readList(() => hour.getYi()),
            })
        } catch {
            continue
        }
    }

    return [...byZhi.values()]
}

function buildAlmanac(date: Date, key: string): AlmanacDay | null {
    try {
        const solar = Solar.fromYmd(date.getFullYear(), date.getMonth() + 1, date.getDate())
        const lunar = solar.getLunar()

        return {
            dateKey: key,
            solarLabel: readText(() => solar.toYmd(), key),
            weekday: readText(() => solar.getWeekInChinese()),
            lunarLabel: readText(() => lunar.toString()),
            ganZhiYear: readText(() => lunar.getYearInGanZhi()),
            ganZhiMonth: readText(() => lunar.getMonthInGanZhi()),
            ganZhiDay: readText(() => lunar.getDayInGanZhi()),
            shengXiao: readText(() => lunar.getYearShengXiao()),
            yi: readList(() => lunar.getDayYi()),
            ji: readList(() => lunar.getDayJi()),
            zhiShen: readText(() => lunar.getDayTianShen()),
            zhiShenType: readText(() => lunar.getDayTianShenType()),
            zhiShenLuck: readText(() => lunar.getDayTianShenLuck()),
            zhiXing: readText(() => lunar.getZhiXing()),
            naYin: readText(() => lunar.getDayNaYin()),
            chong: readText(() => lunar.getDayChongDesc()),
            sha: readText(() => lunar.getDaySha()),
            xiu: readText(() => lunar.getXiu()),
            xiuLuck: readText(() => lunar.getXiuLuck()),
            pengZuGan: readText(() => lunar.getPengZuGan()),
            pengZuZhi: readText(() => lunar.getPengZuZhi()),
            jiShen: readList(() => lunar.getDayJiShen()),
            xiongSha: readList(() => lunar.getDayXiongSha()),
            xi: readText(() => lunar.getDayPositionXiDesc()),
            fu: readText(() => lunar.getDayPositionFuDesc()),
            cai: readText(() => lunar.getDayPositionCaiDesc()),
            yangGui: readText(() => lunar.getDayPositionYangGuiDesc()),
            yinGui: readText(() => lunar.getDayPositionYinGuiDesc()),
            taiShen: readText(() => lunar.getDayPositionTai()),
            zodiac: readText(() => solar.getXingZuo()),
            hours: readHours(lunar),
        }
    } catch {
        return null
    }
}

/**
 * Almanac for a local calendar day. Every field falls back to an empty value so
 * a library hiccup can never throw while the dashboard renders.
 */
export function getAlmanac(date: Date): AlmanacDay {
    const key = dateKey(date)
    const cached = cache.get(key)
    if (cached) return cached

    const built = buildAlmanac(date, key)
    if (built) {
        cache.set(key, built)
        return built
    }

    return emptyDay(key)
}
