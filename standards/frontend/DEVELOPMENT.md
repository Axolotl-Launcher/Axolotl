# Frontend Development

Applies to frontend work in `apps/` and `packages/`. Package scripts are the executable source of truth; run commands from the repository root.

## Validation

- During development, use the affected package's existing read-only `lint`, `test`, `test:visual`, or build script as appropriate. Builds may generate output. Do not invoke an undeclared `check` script or invent ad hoc compiler commands.
- App frontend: `pnpm --filter @modrinth/app-frontend lint`, `pnpm --filter @modrinth/app-frontend test`, `pnpm --filter @modrinth/app-frontend test:visual`, and `pnpm --filter @modrinth/app-frontend build`.
- Shared UI: `pnpm --filter @modrinth/ui lint` and `pnpm --filter @modrinth/ui test`. API client: `pnpm --filter @modrinth/api-client lint` and `pnpm --filter @modrinth/api-client build`.
- Website: `pnpm --filter @axolotl/website lint` and `pnpm website:build`.
- Choose checks proportional to the changed behavior. Do not run full repository workflows after every prompt. Report what was actually verified and any remaining limitations.
- Automated app visual and interaction tests used in CI may be run locally. The user still performs acceptance verification.
- Do not perform excessive website browser automation (interaction flows, theme toggling, responsive checks, screenshots) unless explicitly requested. The user verifies visual and interaction changes.

## Formatting Before Commits

- Before committing frontend changes, run the affected package's existing `fix` script from the repository root. It runs ESLint with `--fix` followed by Prettier with `--write`, fixing import sorting, Vue attribute order, and formatting together.
- For app frontend changes, run `pnpm --filter @modrinth/app-frontend fix`. For shared UI changes, run `pnpm --filter @modrinth/ui fix`. Run both when both packages are affected; use the corresponding package's `fix` script for other frontend packages.
- Running Prettier alone is insufficient: import sorting and Vue attribute order are ESLint rules.
- Inspect `git diff` after automatic fixes. Include relevant fixes in the commit, preserve existing user changes, and avoid staging unrelated formatting changes.
- Resolve any remaining errors before committing. If files were already staged, stage the corrected files again so the commit includes the fixes.
- This commit-time formatting step is authorized whenever a frontend commit is requested or already authorized. It does not require running the full `prepr` workflow after every prompt.
- Before a pull request, follow the root `AGENTS.md` pre-PR rules. The `prepr` tasks write files; inspect their diff and run the affected package lint, tests, and build separately as appropriate.

## Styling

- Express UI styling with Tailwind utility classes directly on elements. Never add a custom CSS class that only aliases one or a few declarations a utility can express verbatim. BAD: `.foo { margin-top: 0.5rem }` plus `class="foo"`. GOOD: `class="mt-2"`.
- Use the shared tokens and nothing else: the Tailwind preset lives at `packages/tooling-config/tailwind/tailwind-preset.ts`, theme variables in `packages/assets/styles/variables.scss`. Standard mappings: `--gap-xs/sm/md/lg/xl` → `gap-1/2/3/4/6`, `var(--color-text-primary)` → `text-[var(--color-text-primary)]`, `var(--color-text-tertiary)` → `text-[var(--color-text-tertiary)]`, `var(--surface-N)` → `bg-surface-N` / `border-surface-N`, `var(--color-divider)` → `border-divider`, `var(--radius-*)` → `rounded-[var(--radius-*)]`. Never reference CSS variables that are not defined in the shared theme (e.g. `--color-text-secondary` is undefined in the desktop app) — an undefined variable silently disables the declaration.
- Custom classes are acceptable only where utilities cannot express the styling:
  - Anchors for `:deep`/child-element/pseudo-element selectors and `[data-size]`-style compound state rules that must keep working;
  - Animated states (`@keyframes`, transitions), `::before`/`::after` with `content`, scrollbar pseudo-elements;
  - Platform-conditional rules (`.mac`/`.windows`), non-standard-breakpoint media queries, and third-party/rendered content (markdown, terminals, canvas, editors);
  - Genuinely reusable visual variants composed with `@apply` (prefer `@apply` over hand-written declarations when composing utilities into a component class).
- In scoped `<style>` blocks, a rule whose declarations each map 1:1 to utilities is a violation: put the utilities on the element and delete the rule. Do not define utility-alias classes even when scoped — they get duplicated across components and drift.
- Do not copy a style block from one component into another; extract a shared component, an `@apply`-based class, or use utilities inline.

## Types

Types in `@modrinth/utils` may be outdated. Prefer current API response types from `@modrinth/api-client` when they describe the same data.

## Product Copy

- Write for the person completing the task. Labels, instructions and feedback should help them choose an input, take an action, understand a result or recover from a problem. Do not use the product interface to narrate implementation work, explain the algorithm or defend incomplete coverage.
- Before adding helper text, identify the concrete decision or mistake it changes. If deleting it leaves the user's input, next action and interpretation of the result unchanged, omit it. A technically true limitation is not sufficient reason to display it.
- Do not add implementation details, internal thresholds, traversal rules, unsupported-case inventories or development caveats by default. Keep those in code documentation or the implementation report when relevant. Do not move unnecessary prose into a tooltip merely to retain it.
- When a limitation materially affects the user's decision or could make the result misleading, state its practical consequence briefly in the relevant state or an on-demand help surface. Use the user's terms and explain an available action when there is one. Do not hide uncertainty or claim complete results when coverage is partial.
- Prefer an accurate label, sensible default or direct result state over a paragraph describing how the control works. Avoid restating the title, button text, obvious interaction or current status.
- Example: for a chest-search action, use a label such as “Search chests in this area”. Do not append structure-start offsets, a 512-block search margin or loot-table implementation details unless they change what the user can meaningfully select or conclude. If coverage must be disclosed, use a short consequence such as “Some chests may be missing” in the relevant result context instead of an algorithm explanation.
- Review newly added visible copy before completing a UI change. Remove sentences whose only audience is the developer or reviewer; summarize implementation details in the work report instead.
