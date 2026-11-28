# Agent notes

## Git workflow

- After every round of changes: `git add -A` and create a **local** commit.
- Never push to a remote. Commits are local checkpoints, recoverable with
  `git restore` / `git reset`.
- Use the milestone-style subject (`M3: ...`, `M2.5: ...`) and list the areas
  touched in the body.
- Update `docs/HANDOFF.md` and `TODO.md` in the same commit; they are the
  resume point for the next session.
- Before committing, verify: `cargo build` (0 warnings), `cargo test`,
  `showcase --self-test` and `showcase --a11y-dump` (commands in
  `docs/HANDOFF.md`).
