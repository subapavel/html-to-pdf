# ADR-0012: A fork commit whose PR is not merged is reverted, not rebased, when another PR needs the branch without it

Date: 2026-11-04 · Status: accepted · First use: F183c (`docs/plans/2026-11-04-f183c-inline-block-baseline.md`), which carries
reverts of F217's two Blitz commits

## Context

A fork branch (`subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`, and the same for Parley, Taffy and fulgur) is shared by every
open PR. Each PR appends its commits and moves the workspace `Cargo.lock` to its head SHA. The standing rules are: history is
append-only (no force push, no rebase of pushed commits), and `Cargo.lock` can only name a commit that is reachable from the
branch head.

Two PRs are often open at once. When PR A has pushed its fork commits and PR B (a different finding) then needs the branch
without A's commits, three things are impossible:

- B cannot simply append its own commits: they would sit on top of A's, so B's lock (its head) would carry A's engine change
  into main without A's fixture, plan, gates and review. Locking the older commit does not help either, because B's own
  commits cannot be placed on it without a second line of history on the one branch.
- B cannot rebase or force-push the branch to drop A's commits: A's PR is open and its lock names them.
- B cannot take A's engine change into its own PR: merge-policy item 4 (one finding per PR, the diff matches the plan).

## Decision

1. **B appends reverts.** B's first fork commits are `git revert` of A's commits, newest first, so that the tree of B's first
   head equals the tree of main's lock. `git diff <main's lock SHA> <B's head>` must touch only B's files. Each revert commit
   says which commit it reverts, that it is ADR-0012, and that A re-applies.
2. **A's SHAs stay valid.** The reverted commits are ancestors of the new head, so A's `Cargo.lock` still resolves until A
   moves it.
3. **A re-applies by reverting the reverts**, on top of B's head after B has merged, then moves its lock to the new head and
   runs its gates again. The re-applied commits are new commits with new SHAs; A's PR records them in `docs/upstream.md`.
4. **If B is merged first** the branch head carries B's commits after the reverts, and A's re-apply is mechanical. **If A is
   merged first**, B does not need the reverts: B drops them from its plan, appends its own commits on A's head and rebases
   nothing.
5. **The reverts are owned by B's PR.** They are listed in B's `docs/upstream.md` row as "revert of <sha> (A), ADR-0012"
   so that the next agent reading the row knows why a tree on the branch went back and forth.
6. **No fork commit is dropped by force.** The only exception is a branch with no open PR and no lock naming the commit,
   and even then the owner's rule applies: append, do not rewrite.

## Consequences

- The fork history shows both changes, reverts included. It is longer, and `git log` on the branch is not a list of features;
  `docs/upstream.md` is the index.
- Each PR's diff in `Cargo.lock` is a single SHA per fork, and each PR's fork diff against main's lock is its own.
- A's gates run twice (once on its own branch, once after the re-apply). That is the price of keeping merge-policy item 4.
- The same rule covers Parley, Taffy and fulgur forks.
