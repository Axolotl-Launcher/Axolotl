let language = 'en-US'
let catalog = {}

export function setLocale(locale, messages = {}) {
    try {
        language = Intl.getCanonicalLocales(locale || 'en-US')[0] || 'en-US'
    } catch {
        language = 'en-US'
    }
    catalog = messages
}

export function locale() {
    return language
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
    return new Date(timestamp).toLocaleTimeString(language, {
        hour: '2-digit',
        minute: '2-digit',
    })
}

export function dayLabel(timestamp) {
    return new Date(timestamp).toLocaleDateString(language, { month: 'short', day: 'numeric' })
}

// The time alone today, the day and time before that.
export function whenLabel(timestamp) {
    const time = dateLabel(timestamp)
    return new Date(timestamp).toDateString() === new Date().toDateString()
        ? time
        : `${dayLabel(timestamp)} ${time}`
}

export function durationLabel(milliseconds) {
    const seconds = Math.max(0, Math.round(milliseconds / 1000))
    const unit = (value, name) =>
        new Intl.NumberFormat(language, {
            style: 'unit',
            unit: name,
            unitDisplay: 'narrow',
        }).format(value)
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
    return new Intl.NumberFormat(language, {
        style: 'unit',
        unit: units[index],
        unitDisplay: 'short',
        maximumFractionDigits: index ? 1 : 0,
    }).format(value)
}
