---
name: i18n-pass
description: Audit and localize user-visible copy in Vue components and their TypeScript message modules using the project's i18n system. Use when localizing code changes or reviewing translatable UI copy.
---

Read the [i18n standard](../../../standards/frontend/INTERNATIONALIZATION.md) and the target package instructions. Review requests produce findings only; edit strings only when implementation is requested. Announcement catalogs retain their own authorization and bilingual rules.

## Steps

1. **Identify the scope from the request.** For an explicit file or PR, include its changed files and copy sources. Otherwise inspect unstaged, staged and untracked files under the requested frontend scope (`git status --short` plus the relevant diffs). Do not scan every locale or unrelated product area by default.
2. **Read the standard above** for the message definition pattern, ICU format rules, and `IntlFormatted` usage.
3. **Follow visible copy to its source.** Inspect changed `.vue`, `.ts` and `.js` modules that define labels, messages, notifications or status text. Do not skip a message module because it is not a component.
4. **Find user-visible copy and its existing localization mechanism:** inspect Vue templates/scripts and TypeScript modules used by the requested UI. Skip identifiers, CSS classes, diagnostic logs and dynamic values. Preserve ICU meaning and translator context.
5. **Define messages** with `defineMessages` or the repository's existing catalog API and stable, domain-based IDs. Read [the i18n standard](../../../standards/frontend/INTERNATIONALIZATION.md) for ICU and rich-text rules.
6. **Replace strings in templates** with `formatMessage()` calls, or `<IntlFormatted>` for strings containing links or markup.
7. **Handle ICU edge cases** — add a space before `}}` if an ICU placeholder ends at a Vue template delimiter boundary.
8. **Verify** changed UI copy has a localization source and run `pnpm axolotl:i18n-check` when edited files are covered by the check. Run affected package tests. Preserve behavior and avoid unrelated locale changes.
