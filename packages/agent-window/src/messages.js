let language = 'en-US'
let catalog = {}
// Building an Intl formatter costs far more than using one, and a long history
// formats a time for every turn, so each one is made once per locale.
let formatters = new Map()

export function setLocale(locale, messages = {}) {
    let next
    try {
        next = Intl.getCanonicalLocales(locale || 'en-US')[0] || 'en-US'
    } catch {
        next = 'en-US'
    }
    if (next !== language) formatters = new Map()
    language = next
    catalog = messages
}

export function locale() {
    return language
}

function formatter(key, make) {
    let value = formatters.get(key)
    if (!value) {
        value = make()
        formatters.set(key, value)
    }
    return value
}

// The launcher formats every catalog entry before sending it here and keeps
// simple `{name}` arguments as literal placeholders, which this fills in.
export function text(key, values) {
    const message = catalog[key] || key
    if (!values) return message
    return message.replace(/\{(\w+)\}/g, (match, name) =>
        Object.hasOwn(values, name) ? String(values[name]) : match,
    )
}

export function dateLabel(timestamp) {
    return formatter(
        'time',
        () => new Intl.DateTimeFormat(language, { hour: '2-digit', minute: '2-digit' }),
    ).format(timestamp)
}

function dayLabel(timestamp) {
    return formatter(
        'day',
        () => new Intl.DateTimeFormat(language, { month: 'short', day: 'numeric' }),
    ).format(timestamp)
}

// The time alone today, the day and time before that.
export function whenLabel(timestamp) {
    const time = dateLabel(timestamp)
    return new Date(timestamp).toDateString() === new Date().toDateString()
        ? time
        : `${dayLabel(timestamp)} ${time}`
}

const unitFormat = (unit, display, digits = 0) =>
    formatter(
        `${unit}:${display}:${digits}`,
        () =>
            new Intl.NumberFormat(language, {
                style: 'unit',
                unit,
                unitDisplay: display,
                maximumFractionDigits: digits,
            }),
    )

export function durationLabel(milliseconds) {
    const seconds = Math.max(0, Math.round(milliseconds / 1000))
    const unit = (value, name) => unitFormat(name, 'narrow').format(value)
    if (seconds < 60) return unit(seconds, 'second')
    const minutes = Math.floor(seconds / 60)
    const rest = seconds % 60
    return rest ? `${unit(minutes, 'minute')} ${unit(rest, 'second')}` : unit(minutes, 'minute')
}

export function sizeLabel(bytes) {
    const units = ['byte', 'kilobyte', 'megabyte', 'gigabyte']
    let value = Math.max(0, Number(bytes) || 0)
    let index = 0
    while (value >= 1024 && index < units.length - 1) {
        value /= 1024
        index++
    }
    return unitFormat(units[index], 'short', index ? 1 : 0).format(value)
}
