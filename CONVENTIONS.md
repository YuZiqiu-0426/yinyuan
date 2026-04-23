# Engineering conventions

This document applies to all contributors and automation (IDE agents, CI notes, etc.). It is plain Markdown at the repository root so any toolchain can read it without vendor-specific formats.

---

## Language

- Human-facing prose in this repo may be **Chinese (Simplified)** where existing docs already use it.
- **Code identifiers, commit messages, and public API names** remain in English unless an established exception exists.

---

## Code size

1. **Functions**: keep each function **at most 50 lines** (including blank lines and comments). If logic grows beyond that, split into smaller private helpers or modules and preserve single responsibility.
2. **Files**: keep each source file **at most 500 lines**. If a file approaches or exceeds this limit, split by type or responsibility (e.g. extra `mod` files or sibling modules).

---

## Git workflow

3. **After completing a task**: land the outcome in the codebase, then **commit in Git** (`git add` the relevant paths and `git commit`). Use a clear, full-sentence message describing *what* changed and *why*.

Automation (agents) should perform the commit when finishing a coherent task, not only print commands for the user to run manually.

---

## Runtime and tooling

4. **tmux**: Start and manage local dev processes in **tmux** (sessions / windows / panes) so services stay in one place—avoid scattering long-lived servers across unrelated terminal tabs without a session layout. Use a predictable session name per repo or per feature when helpful.
5. **Frontend package manager**: For **frontend** projects in this repository (e.g. under `frontend-monorepo/`), use **pnpm** only—`pnpm install`, `pnpm run <script>`, etc. Do not introduce npm or Yarn for day-to-day installs or script runs unless an exception is documented in this file.

---

## Project conventions

6. **Repository layout**: Treat the repository root as the documentation and coordination layer. The current Rust workspace lives under **`y2m-rs/`**. Frontend applications live under **`frontend-monorepo/`**.
7. **Rust command location**: Run all `cargo` commands from **`y2m-rs/`** (or use `--manifest-path y2m-rs/Cargo.toml`). Do not assume the repository root is itself a Rust workspace.
8. **Current implementation baseline**: When describing what the system currently does, use **`docs/当前实现说明.md`** as the authoritative source of truth.
9. **Status and priority baseline**: When deciding what to build next, check **`docs/工作进度.md`** first.
10. **Navigation guidance**: Use **`CLAUDE.md`** for repository navigation, build/test entry points, and codebase orientation, but not as the final authority on current feature behavior when it conflicts with `docs/当前实现说明.md`.
11. **Document conflict rule**: If documents conflict, use this order:
    1. `docs/当前实现说明.md`
    2. `docs/工作进度.md`
    3. `CONVENTIONS.md`
    4. `CLAUDE.md`
    5. other requirements / design / API documents
12. **Current messaging baseline**: Preserve the behavior documented in `docs/当前实现说明.md` for the existing `y2m-rs` implementation, including current routing, reconnect behavior, file transfer behavior, and test coverage expectations.
13. **Frontend project locations**: The user-facing web app is **`frontend-monorepo/apps/y2-chat`** and the admin app is **`frontend-monorepo/apps/y2-manage`**.

---

## Process conventions

14. **Read-before-change**: Before starting non-trivial work, read the relevant sections of:
    - `docs/当前实现说明.md`
    - `docs/工作进度.md`
    - `CLAUDE.md` when repository layout, commands, or test entry points matter
15. **Change against reality**: Do not implement against stale assumptions. If a navigation or design document disagrees with the current implementation baseline, align the work to `docs/当前实现说明.md` first.
16. **Document update order**: When behavior changes, update documents in this order:
    1. `docs/当前实现说明.md`
    2. `docs/工作进度.md`
    3. `CLAUDE.md` if navigation/build/test guidance changed
    4. `CONVENTIONS.md` if engineering rules or workflow changed
    5. related requirement / design / API / runbook documents as needed
17. **Keep guidance layered**: Put engineering rules and workflow in `CONVENTIONS.md`, repository navigation in `CLAUDE.md`, and implementation facts in `docs/当前实现说明.md`. Do not duplicate long sections across all three files.
18. **Testing expectation**: After a coherent code change, run the narrowest meaningful validation first, then broader workspace checks when risk or scope requires it.
19. **Commit expectation**: After completing a coherent task, stage only the relevant files and create a Git commit with a full-sentence message that states what changed and why.
20. **Long-lived process management**: When local development requires servers, watchers, or other long-running processes, keep them in `tmux` rather than ad hoc terminal tabs.

---

## Where this is referenced

- **Cursor**: `.cursor/rules/yinyuan-conventions.mdc` points here so the same rules load in assisted sessions.
- Update **this file** when conventions change; avoid duplicating long text in multiple places.
