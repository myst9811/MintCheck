# MintCheck — Automated Development Workflow

This document defines the mandatory development loop for all agent-driven work in
this repository. It is not advisory. Every change, regardless of size, follows
this protocol. Quality gates (clippy, fmt, test) are defined in
`.claude/CLAUDE.md` and apply at every commit point below.

---

## 1. Milestone Discipline — Atomic Commits

- **Commit at the atomic level.** One commit = one coherent, self-contained unit
  of work: a single detector, a single IR node kind, a single bug fix with its
  test. A commit must never mix unrelated concerns (e.g., a new detector plus a
  reporter refactor).
- **Hard cap: 300 lines of code (LOC) per commit**, measured as total lines
  added + removed across source files (`git diff --stat`). Generated lockfiles
  (`Cargo.lock`) and test fixtures are exempt from the count but should still
  accompany the change that requires them.
- If a task cannot fit in 300 LOC, **decompose it before writing code**: split
  along crate boundaries first (parser change → detector change → reporter
  change as separate commits), then along trait/struct boundaries. Each
  intermediate commit must still build, lint clean, and pass the full test
  suite — no "part 1 of 3 (broken)" commits.
- Commit messages follow Conventional Commits (`feat:`, `fix:`, `test:`,
  `refactor:`, `chore:`, `docs:`) with an imperative subject line describing
  the single change.

## 2. Test-Driven Execution

- **Tests are written alongside or prior to implementation — never after the
  fact, never "in a follow-up".** For a new detector: write the positive fixture
  (vulnerable code that MUST flag) and negative fixture (safe code that MUST NOT
  flag) under `fixtures/`, plus the failing test, before or together with the
  detector logic.
- The required ordering per unit of work:
  1. Write or update the test expressing the desired behavior.
  2. Run `cargo test` — confirm the new test fails for the expected reason.
  3. Implement the minimal code to make it pass.
  4. Run the full gate (see §5).
- **`cargo test` (full workspace) must pass locally before any commit is
  generated.** A green run on only the changed crate is insufficient — the full
  suite catches cross-crate contract breaks.
- Every public API change in `auditor-parser`'s IR or `auditor-reporter`'s
  report model requires a corresponding snapshot/serialization test update in
  the same commit.

## 3. Git Protocol — Push Every Commit

- **Every single commit is automatically pushed immediately after creation**
  (`git push`, or `git push -u origin <branch>` on first push) to guarantee
  remote state synchronization. There is no local-only commit state: if it is
  committed, it is pushed.
- Work happens on the designated feature branch for the session. Never push to
  a different branch, and never force-push, without explicit user permission.
- If a push fails due to network errors, retry up to 4 times with exponential
  backoff (2s, 4s, 8s, 16s). If it still fails, report the failure rather than
  continuing to stack unpushed commits.
- Pull requests are created only when explicitly requested by the user.

## 4. Architecture Verification

- **Before coding any new detector or core feature, read `PROJECT_SPEC.md`**
  (at minimum: §2 Workspace Architecture, §4 Core Data Structures, §5 Analysis
  Pipeline, and any section governing the area being changed).
- After reading, **print an "Architecture Compliance Verification" summary to
  the terminal** before writing the first line of code. Required format:

  ```text
  === Architecture Compliance Verification ===
  Feature:        <what is being built, one line>
  Target crate:   <crate> (spec §<n>)
  Touches IR:     <yes/no — which NodeKind/struct, or "none">
  Dependency check: <confirm no new internal crate edges, or justify>
  Pipeline stage: <which of Stages 1–5 / 4b this affects>
  Spec deviations: <"none" or an explicit list — each requires user sign-off>
  ============================================
  ```

- If the planned work would deviate from `PROJECT_SPEC.md` (new crate edge, IR
  schema change, new pipeline stage), **stop and surface the deviation to the
  user before implementing**. The spec is updated first or the design changes —
  silent drift is prohibited.

## 5. The Loop (every unit of work)

```text
┌─▶ 1. Read PROJECT_SPEC.md → print Architecture Compliance Verification
│   2. Write/extend tests (fixtures + assertions) — confirm they fail
│   3. Implement (≤ 300 LOC total for the commit)
│   4. Gate: cargo fmt --check
│            cargo clippy --all-targets -- -D warnings
│            cargo test                      (full workspace, must be green)
│   5. git add → git commit (conventional message)
│   6. git push (immediately; retry w/ backoff on network failure)
└── 7. Next atomic unit
```

Any gate failure at step 4 returns to step 3 (or step 2 if the test itself was
wrong). Steps 5–6 are unreachable while any gate is red.
