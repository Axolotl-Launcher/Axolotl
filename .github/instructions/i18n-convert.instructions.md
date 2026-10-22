---
applyTo: '**/*.{vue,ts,tsx,js,jsx,mts,cts,mjs,cjs}'
---

For tasks changing user-visible UI copy, read the [internationalization standard](../../standards/frontend/INTERNATIONALIZATION.md) and the target package's `AGENTS.md`. Use the [i18n-pass workflow](../../.agents/skills/i18n-pass/SKILL.md) for a requested localization audit or conversion.

Follow copy into its owning message modules, including TypeScript. Diagnostic logs, internal identifiers and unrelated code do not require localization. Preserve the website's existing locale API and the announcement catalog's separate bilingual and authorization rules. Review requests stay read-only; conversion requests preserve behavior, layout and bindings.
