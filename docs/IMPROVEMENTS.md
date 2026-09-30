# Engineering improvements for slussa

Companion to [FEATURES.md](FEATURES.md) (what to build) and
[ARCHITECTURE.md](ARCHITECTURE.md) (how it is built). This file tracks how the
codebase, tooling and delivery should get better. It is not a bug list.

**State on 2026-09-30:** single crate, about 23 600 lines of Rust, 254 tests,
`clippy::pedantic` clean, CI on Linux and macOS, a release workflow that has
been rehearsed, a README.

Rule of thumb for everything below: **do the refactors when a feature touches
the code anyway, do the tooling now.** Every open item has a trigger; do not do
it ahead of the feature that needs it. Part of this was assembled by reading
[NVIDIA/OpenShell](https://github.com/NVIDIA/openshell) (about 40 crates, an
agent-first Rust project with a ratatui TUI). What is borrowed is their
engineering hygiene, not their TUI: slussa's component/store design is already
tighter than their `App` struct with 20 `pending_*` flags, and it stays.

## Open

### Code, with the next feature that touches the area

- [ ] **Split `Action` into local and app-level.** Today one enum carries both
  component-local actions (`Detail`, `Diff`, `List`, `Search`, `Commits`,
  `Paste`, `HelpScroll`) and application effects (`Navigate`, `Refresh`,
  `Command`, `Loaded`, `PrLink`). `App::apply_inner` and `Ui::update` assert
  with `unreachable!` that the local ones were consumed. Make components return
  `Option<Effect>` where `Effect` holds only the app-level variants; local
  messages never reach the app. Removes the last `unreachable!`s (four remain
  after the panic audit: `app/mod.rs` twice, `pr_detail/interactions.rs`,
  `pr_detail/mod.rs`; do not patch them separately) and makes the contract
  visible in the signature. *Trigger:* first feature that adds an app-level
  action (agent handoff, run-a-review).
- [ ] **Keybinding table for the PR view.** `pr_detail/keys.rs` is a long
  function of conditions, each mixing key, tab, modifiers and PR state, with
  the help overlay maintained separately by hand. Replace with a slice of
  `Binding { key, mods, tabs, gate: fn(&DetailView, pr_id) -> bool, action,
  help }`; routing and help are then derived from one source, and
  `supports_action` in `view.rs` becomes the gate. *Trigger:* first feature
  that adds two or more keys to the PR view.
- [ ] **Provider trait instead of enum dispatch.** `providers/mod.rs` matches
  on `Provider` in every method; fine for two providers. Move to a
  `trait ProviderApi` (or keep the enum and implement it via the trait).
  Capabilities stay data. *Trigger:* a third provider.
- [ ] **Render-context structs for diff/thread rendering.** Four
  `too_many_arguments` allowances in `diff_viewer/pane.rs` and
  `widgets/comment.rs`. Bundle the per-render inputs (theme, focus, width,
  anchors, queued comments, current user) into one borrowed struct. *Trigger:*
  the AI-authored comment marker (adds one more argument otherwise).
- [ ] **Share thread assembly between providers.** `github/activities.rs` and
  `bitbucket_dc/activities.rs` both turn a flat event list into threads,
  replies and reactions. Move the assembly into `domain/` and have each
  provider produce flat `domain` events. *Trigger:* outdated-comment detection
  or review grouping, which need the same logic.
- [ ] **Suspend / resume for child processes.** Agent handoff and *open in
  `$EDITOR`* both need to hand the terminal to a child and take it back. The
  reference sequence (from OpenShell's `handle_shell_connect`): cancel
  background refreshes, pause the input reader, leave the alternate screen and
  raw mode, run the child with inherited stdio on `spawn_blocking`, restore raw
  mode and the alternate screen, clear and redraw, drain stale events, resume
  the reader and restart refreshes. slussa uses crossterm's `EventStream`,
  which has no pause: drop and recreate it around the child, or gate it with an
  `AtomicBool`. Put it in `app/desktop.rs` as `run_in_terminal(cmd)`.
  *Trigger:* send-to-agent or open-in-editor.
- [ ] **Error classification for the caller.** Add `FetchError::kind()`
  (`Retryable | NeedsAuth | Gone | Invalid | Unknown`) so the error dialog
  decides whether to offer *retry*, *re-login* or just *dismiss* from the
  classification instead of from prose in the message. `user_message()`
  already covers the human text, and the split between log message and user
  message is right and should be kept. *Trigger:* next change to the error
  dialog or a new provider.
- [ ] **Types that carry invariants.** slussa already does this for
  `CommentAnchor` + `DiffRevision`. Extend it to the screen/app boundary: a
  `ResolvedCommand` that only `pr_detail/interactions.rs` can construct, so
  `app/commands.rs` never re-checks dialog state. Pairs with *Split `Action`*.
- [ ] **Conformance test for the provider boundary.** One
  `tests/provider_conformance.rs` over a fake GitHub and a fake Bitbucket,
  covering list, open, comment, review, merge and the error paths. *Trigger:*
  the provider trait.
- [ ] **Shape `providers/` as domain + trait + leaves.** `domain/` and the
  trait in the middle, `github/` and `bitbucket_dc/` as leaves that depend on
  it, never on each other, with the conformance test next to the trait.
  *Trigger:* the provider trait.
- [ ] **`Pager<T>`.** A lazy pager built from a fetch closure and a token, with
  `next_page()` and `collect_all()`, tested with a counting closure. Cleaner
  than `github/pagination.rs` and the right shape for *Pagination / load more*
  in FEATURES.md.
- [ ] **Split the files that exceed the size rule when next touched
  substantially** (modules stay under about 500 lines, `mod.rs` composes): the
  non-test files `widgets/comment.rs`, `diff_viewer/pane.rs`, `pr_list/mod.rs`
  and `timeline.rs`, and the test files `tui/regression_tests.rs` and
  `app/tests.rs`. Split tests by concern with `use super::*`. Not a
  refactor-only change.
- [ ] **The first load is slow on a large repository.** On `cli/cli` (63 open
  PRs) the first frame took about 0.75 s for `git`, `gh --version`,
  `gh auth status` and `gh api user`, and the first page of 30 open PRs 2.0 to
  2.7 s. The open pages are a cursor chain and cannot run in parallel; what a
  page costs is the selection. Done: closed PRs are read per view; the list
  query no longer reads the body and labels (read per PR on open); the open
  group appears with its first page and the rest is appended in arrival order,
  with the attention order applied once at the end; reading stops at 90 open
  PRs and `L` reads 90 more. **Left, if it is still not enough:** two phases
  (core fields first, reviews, CI and requests after; about 1.1 to 1.4 s a
  page) and re-reading only what changed instead of the whole list every
  minute. Searching for the PRs that need the viewer was considered and
  declined, because the search index lags, and so was a persisted last list.
  Timings are noisy: one network, one repository.

### Tests and tooling

- [ ] **Snapshots with realistic content.** `tui/testdata/screens.txt` covers
  100x30 and 40x12 for the list and all five detail tabs, but with placeholder
  data: one PR, no diff, no threads. Add a snapshot with a diff with an inline
  thread, several commits and a failing build at both sizes, and one at 80x24.
- [ ] **`cargo nextest`** in CI (parallel, per-test timeouts, clearer failure
  output). Local `cargo test` stays fine.
- [ ] **CodeRabbit on the repository.** An automatic AI reviewer on every PR.
  *Trigger:* the repository becomes public (see *Release* in FEATURES.md).
  Third-party pages say it is free for public repositories and about 24 USD per
  user and month for private ones; the vendor's own pricing page was not
  checked, so check it first.
  1. **Install the app yourself.** It is a GitHub App: install it from GitHub
     Marketplace and give it `albinlju/slussa` only. Granting access is done in
     the browser and is not something an agent should do.
  2. **Add `.coderabbit.yaml` at the root.** Per its documentation the file sets
     `language`, `reviews.profile` (for example `chill`), `reviews.auto_review`
     (`enabled`, `drafts`), `request_changes_workflow` and `path_instructions`.
     Only a short excerpt of the schema was read, so verify the keys against its
     configuration reference before writing it.
  3. **Put the project rules in `path_instructions`**, taken from `CLAUDE.md`:
     two views only; provider and process calls block and run off the UI thread
     through `App::spawn_fetch`, with no async HTTP and no ad hoc threads; no new
     `unwrap`, `expect` or `unreachable!` in non-test code; modules stay under
     about 500 lines and tests live inline or in one sibling `tests.rs`; show
     only what the provider supports. Keep drafts out of automatic review.
  4. Decide how it sits beside `/code-review` and CONTRIBUTING.md: it reviews
     every PR, `/code-review` is run by hand.

### From OpenShell's AI reviewer ("gator"), for the AI features

OpenShell runs an autonomous PR reviewer in a sandbox. None of its code is
reusable here, but the *contract* it enforces is what slussa will read and
display once AI reviews are first-class threads (FEATURES.md §2). Design the
domain model against it:

- [ ] **Marker-based AI detection, not just bot accounts.** Every gator comment
  starts with a first-line marker (`> **gator-agent**`); other skills use their
  own. Detecting AI authorship needs a configurable list of first-line markers
  *and* account names, not one or the other.
- [ ] **One disposition per head SHA.** A review is one batched GitHub review
  (summary + inline comments) that names the head SHA it reviewed. Show
  *reviewed SHA vs. current head* on the AI summary line; a review of an older
  SHA is stale, not wrong.
- [ ] **Stable finding IDs across rounds.** Findings carry `GATOR-<sha8>-<nn>`
  and are carried, resolved or waived across later commits; a maintainer's
  "won't fix" reply is a waiver, an author's "fixed" is a claim to verify. The
  open/fixed/waived state per finding is what a reviewer wants at a glance, and
  it is derivable from thread resolution + resolver identity + the marker.
- [ ] **Severity and evidence.** Findings are `Critical | Warning | Suggestion`,
  and only ones with a full evidence record (base behaviour, head behaviour,
  observable impact, reproducer, changed location) count as blockers; the rest
  are hypotheses. Suggestions never block. If slussa's own *run a review*
  command emits structured output, use this split: it gives the reviewer a
  defensible "N blockers, M suggestions" header instead of a wall of comments.
- [ ] **Concern format for the review prompt.** Their `review-github-pr` skill
  requires every concern as "Before this PR, `<persona>` experienced `<old>`.
  With this PR, `<new>`, so `<impact>`." with file:line only as evidence. A good
  default prompt for slussa's run-a-review.
- [ ] **Convergence rules worth copying into the display.** After three
  finding-bearing rounds the reviewer goes `critical_only`; rebase-equivalent
  patches (same patch-id) are not re-reviewed. Show the round count and
  "unchanged since last review" so a human knows when the AI has stopped adding
  value.

## Done

Kept as one line each; the detail is in git history.

- **Panic audit:** 12 of 16 `expect`/`unreachable!` in non-test code removed;
  four remain and go with *Split `Action`*.
- **README** (what it is, providers, install, usage, keys, config, develop) and
  a demo gif recorded against a real repository.
- **Readable GraphQL:** templates in `providers/github/graphql.rs`, compacted
  when sent; a test pins the wire format.
- **Bounded PR list:** open PRs in pages (limit 90, `L` for more), closed PRs
  per view in batches; GitHub cursor, Bitbucket `merged|declined` offsets.
- **Merge blocked is explained:** `Mergeability::Blocked` with reasons in the
  header and the merge dialog; checked against real GitHub rulesets (a failing
  required check only gives a general reason).
- **GitHub's time limit:** a page of 100 PRs took 7 to 11 s on `cli/cli` and
  once failed; the list reads 30 per request.
- **Blocking-I/O rule** written down (ARCHITECTURE.md *Rules for I/O and
  effects*, CLAUDE.md) so agent handoff follows the same pattern.
- **Tests:** domain thread logic (10 tests), the full app loop against `FakeGh`
  (`app/flow_tests.rs`), the transport doubles `FakeGh` and `MockHttp` and their
  18 transport tests, shared fixtures in `src/test_support.rs`, and CLI
  integration tests (`tests/cli_integration.rs`, isolated HOME, no network).
- **Lints:** `clippy::{all, pedantic, nursery}` and a `[lints.rust]` block, two
  nursery lints allowed by name; `rust-toolchain.toml` (1.95.0) and
  `rust-version`.
- **CI** on Ubuntu and macOS (fmt, clippy, test, cargo-deny), actions pinned by
  SHA; **cargo-deny** found RUSTSEC-2026-0285 in `rustls` 0.23.40 (fixed by
  updating to 0.23.45); `colored` and `option-ext` are named MPL-2.0 exceptions.
- **Dependabot** (weekly for Actions, monthly grouped for Cargo) and release
  profile `strip = true`, dev `debug = 1`.
- **Release workflow** (`release.yml` + `package.sh`), dry-run twice on GitHub;
  the publish step has still never run.
- **OSC 52 clipboard:** helper first, OSC 52 as fallback, and first over SSH.
- **Process docs:** CLAUDE.md as the agent instruction surface, ARCHITECTURE.md
  lifecycles and checklists instead of a separate skill, `//!` contract docs on
  the core modules, the file-size rule in CLAUDE.md, CONTRIBUTING.md,
  SECURITY.md and the PR and issue templates.
- **Dropped:** automatic light/dark theme. All five themes are dark and
  `terminal` already follows a light terminal; revisit only if a light palette
  is added.

## Not borrowed (and why)

- **A Cargo workspace with many crates.** slussa is about 23 000 lines with
  clear module boundaries; a workspace adds compile-unit overhead and manifest
  churn without a consumer for the split crates. An earlier attempt on a
  branch (a workspace with `domain` and `providers` crates) was dropped for the
  same reason. Revisit only if a
  `slussa-core` becomes a dependency for something else (a Neovim plugin, an
  MCP server); then name the pieces `slussa-core`, `slussa-provider-github`,
  `slussa-provider-bitbucket-dc`, `slussa-tui`.
- **mise / Nix toolchain management.** `rust-toolchain.toml` covers a
  one-language project.
- **SPDX headers, CODEOWNERS, DCO and a vouch system.** Corporate open-source
  process; nothing to gain with one maintainer.
- **Their TUI structure.** Mouse capture, a splash screen, 20 `pending_*`
  booleans polled after each key. slussa's typed `Action`/`Component` design is
  the better pattern.
- **OpenTelemetry / OCSF logging.** `tracing` with an env filter is enough for
  a local TUI.
- **Their file structure.** 39 files over 100 KB, `too_many_lines = "allow"`
  used in full, three sibling-test conventions. slussa keeps `mod.rs` for
  composition and one test convention, and writes the size rule down.
- **Facade crates bridging parallel type trees** and the twenty-field
  `Mutex<Option<...>>` mock-state bags.
- **The gator agent itself** (sandboxed reviewer, label state machine, ledger
  scripts). slussa *displays* reviews; it does not run an autonomous reviewer.
  The contract above is what to read, not what to build.
