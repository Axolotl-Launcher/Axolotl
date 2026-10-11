import { useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import { ALMANAC_TERM_FALLBACK, ALMANAC_TERM_KEYS } from '@/components/home/almanac-terms-zh'
import {
    HOME_ALMANAC_DEFAULT_TERM_STYLE,
    type HomeAlmanacTermStyle,
} from '@/components/home/home-dashboard'

/**
 * ICU template for the `both` style, so the parentheses come from the locale
 * instead of a literal in this file.
 */
const TERM_BOTH = {
    id: 'app.home.almanac.term-both',
    defaultMessage: '{plain} ({term})',
}

export type AlmanacTermRenderer = {
    /** Renders one almanac term in the widget's style. */
    termLabel: (term: string) => string
    /** Renders a list of terms, keeping the input order. */
    termList: (terms: readonly string[]) => string[]
}

/**
 * Almanac activities use traditional vocabulary ("嫁娶", "移徙") that many
 * readers only know by its vernacular wording. Every known term has a message
 * at `app.home.almanac.term.<key>`; terms without one -- the deity names under
 * auspicious or inauspicious influences -- are shown exactly as the calendar
 * library returns them.
 */
export function useAlmanacTerms(
    style: () => HomeAlmanacTermStyle | null | undefined,
): AlmanacTermRenderer {
    const { formatMessage, locale } = useVIntl()
    const readsChinese = computed(() => locale.value.toLowerCase().startsWith('zh'))

    function termLabel(term: string): string {
        const suffix = ALMANAC_TERM_KEYS[term]
        if (!suffix) return term

        const mode = style() ?? HOME_ALMANAC_DEFAULT_TERM_STYLE
        if (mode === 'traditional') return term

        const plain = formatMessage({
            id: `app.home.almanac.term.${suffix}`,
            defaultMessage: ALMANAC_TERM_FALLBACK[term] ?? term,
        })
        // A label equal to the term reads better on its own than "X (X)".
        if (mode === 'plain' || !readsChinese.value || plain === term) return plain

        return formatMessage(TERM_BOTH, { plain, term })
    }

    return {
        termLabel,
        termList: (terms) => terms.map((term) => termLabel(term)),
    }
}
