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

## Where this is referenced

- **Cursor**: `.cursor/rules/yinyuan-conventions.mdc` points here so the same rules load in assisted sessions.
- Update **this file** when conventions change; avoid duplicating long text in multiple places.
