# Repository Agent Configuration

## Ownership

- `AGENTS.md` files are ordinary UTF-8 files containing instructions for their subtree. Keep repository-wide safeguards and routing at the root; put specialized rules beside their owning code.
- `.agents/skills/<name>/SKILL.md` defines a reusable task workflow. Keep `name` and `description` focused on the actual trigger. Express inputs in normal language so implicit and explicit invocation both work.
- `standards/` owns shared technical facts. Link to the relevant standard instead of copying its tables or implementation recipes into multiple skills and instruction files.
- Paths in Markdown links are relative to their containing file. When naming code paths in prose, state that they are repository-relative. Use real files for examples; clearly label hypothetical paths in code examples.
- Read only the skill references relevant to the current task. Do not load all frontend or design references by default.
- A review request stays read-only. A skill does not authorize implementation, dependencies, commits, messages or remote actions beyond the user's request.
- The delivery workflow — commits, pushes, pull requests, reviews and archiving — is owned by the root [REVIEW.md](../REVIEW.md). Read it before any of those actions; this file owns the agent resources themselves.

## Maintenance

- Keep skills and references tracked. Runtime scratch files belong outside `.agents/skills/`, in ignored `.agents/tmp/` or `.agents/cache/`; never ignore all of `.agents/`.
- Agent intermediate and archived artifacts — plans, specs, worklogs and captured evidence — belong in the ignored `.agents/plans/`. Keep their filenames descriptive, because a later session reads the matching plan to restore the context of that work. Never force-add them past `.gitignore`.
- Before removing a reference, inspect its callers. Preserve domain constraints when moving rules and update the root routing table when adding a new scoped entry point.
- Run `pnpm axolotl:check-agents` after changing agent configuration. The check validates Skill frontmatter and evaluation fixtures and catches leftover legacy files and stale paths. Review Markdown links in changed instruction files; this check does not prove model behavior.
- For workflow changes, exercise representative task scenarios and report the checks actually performed. Keep behavioral evals self-contained and do not claim they ran merely because their JSON is valid.
