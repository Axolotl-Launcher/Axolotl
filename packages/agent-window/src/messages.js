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

export function text(key) {
    return catalog[key] || key
}

export function dateLabel(timestamp) {
    return new Date(timestamp).toLocaleTimeString(language, {
        hour: '2-digit',
        minute: '2-digit',
    })
}
