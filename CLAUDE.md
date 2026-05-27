# CLAUDE.md

Read `INSTRUCTION.md` for full project context.

## ABSOLUTE RULE — NO EXCEPTIONS

**NEVER add AI (Claude, Copilot, or any AI) as co-author, committer, or contributor in git commits.**
Only the user's registered email may appear in commits. This is company policy — commits with AI
authorship WILL BE REJECTED. Do not use `--author`, `Co-authored-by`, or any other mechanism to
attribute commits to AI. This applies to ALL commits, including those made by tools and subagents.

## Critical Rules (never forget)

- Always use `task <name>` to run commands once the Taskfile exists — never run raw commands directly. Run `task --list` to discover tasks.
- Node.js: always `bun`/`bunx` (never node, npm, npx).
- Use brainstorming skill when user starts a new topic or plans something.
- Check and update `INSTRUCTION.md`, `README.md`, and `docs/porting-tracker.md` when making significant changes.
- Conventional Commits: `<type>(<scope>): <description>`. Scope = crate name without `altair-` prefix.
- Branch per change, squash merge. Use `gh` for PR and CI checks.
- `thiserror` for library errors; `anyhow` allowed in binaries/examples only.
- Public APIs documented (`#![deny(missing_docs)]`); every public function has at least one doc-test.
- Re-export the underlying library types users need — consumers should depend only on `altair-*` crates.
