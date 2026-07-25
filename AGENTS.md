# Amari — Agent Operating Rules

This file is the repository-level authority for branch and release operations.
Use `/skill:ia-gitflow` for the general IA doctrine, but apply the Amari-specific
rules below whenever the generic skill says `main`: Amari's production branch is
**`master`**.

## Gitflow — hard rules

```text
feature/*, fix/*, chore/* ──PR──▶ develop ──release/v* PR──▶ master
                                      ▲                         │
                                      └── backmerge PR ─────────┘
                                          (merge commit)

hotfix/* ──PR──▶ master ──backmerge PR──▶ develop
```

1. **Never push directly to `develop` or `master`.** Both branches receive
   changes only through reviewed PRs with green required checks.
2. **Branch normal work from `origin/develop`.** Use `feature/*`, `fix/*`, or
   `chore/*`, and target `develop`.
3. **Branch release work from `origin/develop`.** Use `release/vX.Y.Z`, keep
   release-only changes there, and target `master`.
4. **Branch urgent production fixes from `origin/master`.** Use `hotfix/*`,
   target `master`, and immediately backmerge after the fix lands.
5. **Every merge to `master` requires a `master → develop` backmerge.** Create a
   backmerge branch from `origin/develop`, merge `origin/master`, open a PR to
   `develop`, and merge that PR with a **merge commit, never a squash**.
6. **Release PRs use merge commits.** Feature/fix/chore PRs into `develop` may
   use squash or merge according to the PR's needs; release and backmerge PRs
   must preserve graph ancestry.
7. **Use an isolated worktree for every branch.** Never perform release work in
   the primary checkout.

If a release PR conflicts with `master`, stop and diagnose the missing prior
backmerge. Do not resolve by overwriting one branch wholesale. Compare each
file, merge `origin/master` into the release branch, preserve the release
branch's intended superset, and rerun the approved matrix.

## Human release gates — mandatory

Agents may prepare branches, run verification, request independent review, push
non-protected branches, and open PRs. They may not infer permission for the
following operations from a broad request such as "continue", "ship it", or
"finish the release".

### Gate A — production merge approval

Before merging any release or publication-recovery PR into `master`, present:

- the PR URL and exact head/base branches;
- the diff scope and release commit intended for `master`;
- required-check status and fresh local verification evidence;
- independent-review verdict and unresolved findings;
- every explicit test exclusion, workflow exception, and known risk;
- the exact merge method.

Then use `ask_user` to request explicit approval to merge that specific PR.
Without an affirmative answer, leave the PR open. Never enable auto-merge on a
release PR.

### Gate B — tag and publication approval

After the production merge, separately present:

- the exact `master` commit to be tagged or used for workflow dispatch;
- whether the tag already exists and where it points;
- the workflow trigger and exact crates.io/npm publication actions;
- credential/preflight status without exposing secrets;
- current registry state, dependency order, rollback limits, and known risks.

Then use `ask_user` to request explicit approval for the stated tag/publication
actions. Gate A approval does not imply Gate B approval. Do not create, push,
move, delete, or recreate a release tag; dispatch a publish workflow; create a
GitHub Release; or publish manually without Gate B approval.

If validation or publication fails, stop and report evidence. Do not bypass the
failed gate, mutate the tag, rerun with broader permissions, or switch to manual
publication without another explicit approval.

## Release lifecycle

1. Version bump on a branch from `develop` → reviewed PR to `develop`.
2. Cut `release/vX.Y.Z` from updated `origin/develop`; date the changelog and
   complete release-only verification.
3. Open `release/vX.Y.Z → master`; obtain **Gate A** approval; merge with a merge
   commit.
4. Reconfirm tag target, workflow behavior, registry state, credentials, and
   package order; obtain **Gate B** approval.
5. Push `vX.Y.Z` on the approved `master` commit or dispatch the explicitly
   approved recovery workflow. In Amari, a `v*` tag push triggers
   `.github/workflows/publish.yml`.
6. Verify every expected Rust and npm artifact from the public registries and
   perform clean install/smoke tests.
7. Backmerge `master → develop` through a PR using a merge commit.
8. Record release evidence and only then announce the release.

A version bump, merged release PR, tag, or green workflow alone is not
"shipped". For Amari, shipped means: the approved tag is verified; every
required crates.io and npm artifact is published and install-tested; and the
mandatory `master → develop` merge-commit backmerge is complete.

## Verification and exceptions

Use the release-specific gate document and CI workflows as the executable test
matrix. Before a release decision, run formatting, warning-denied Clippy,
scoped tests, rustdoc, version/catalog checks, package dry-runs, publish-order
checks, and registry smoke tests applicable to that release.

Never silently weaken a matrix. Hardware-dependent or legacy feature exclusions
must be named in the release PR and Gate A evidence, documented with a reason
and owner milestone, and implemented as explicit package/test exclusions—not as
unbounded retries or global serialization.

## Related doctrine

- `/skill:ia-gitflow` — branch graph and backmerge discipline
- `/skill:ia-version-bump` — synchronized workspace release versions
- `/skill:ia-release-polish` — package/publication evidence and shipped criteria
- `/skill:verification-before-completion` — evidence before completion claims
- `/skill:ask-user` — required decision handshake for Gates A and B
