# Roadmap for slussa

The product idea, what is planned in priority order, and how the code should get
better. What exists today is described in the [README](../README.md) and [KEYS.md](KEYS.md); how it is
built is in [ARCHITECTURE.md](ARCHITECTURE.md). History is in git.

## Where it stands today

A read-write PR client for GitHub (through `gh`) and Bitbucket Data Center (REST
and a personal access token). The list opens sorted by what needs you, and the PR
has its description, conversation, diff, commits and builds, with comment, review,
merge, decline and reopen. The AI-specific parts below are not built yet; 0.1.0 is
the base they will sit on.

## Positioning: where it is going

slussa is meant to be the **human approval surface for AI-generated pull
requests**: the place where a reviewer decides, not the place where the code gets
read line by line.

The assumption behind the backlog: agents write more of the code and open more of
the PRs, and AI reviewers do the line-level reading. What stays human is triage
("what needs me?"), intent-checking ("did the agent do what was asked, and what did
the AI review flag?"), and the sign-off itself (approve, merge, decline). Those are
overview-and-decision tasks, and a keyboard-driven terminal UI is better at them
than a web page. slussa should be the missing piece in a terminal workflow next to
an editor, a git client and a coding agent — never a competitor to the provider's
web UI.

**Simplicity is the constraint everything else bends to.** slussa has two views —
the list and the PR — and stays that way. New capability shows up as a better
default, a column, a marker or a single key inside those two views, not as a new
screen, a dashboard or a sidebar of widgets. If a feature cannot be explained in
one sentence and reached in one keypress, it is not ready. When in doubt, leave
it out.

What this rules in (each item says how far it is):

- **The list is the inbox** *(first version built)*. The PR list opens sorted by what needs the user —
  review requests, failed CI, new activity since last look — with a short reason
  on the row. No separate inbox screen; the plain list is the same view with the
  attention sort turned off.
- **AI review is a first-class thread** *(not built)*. Comments from an AI reviewer (Copilot,
  Claude, a team bot) are shown inline with their own marker and summarized in the
  header. Running a review from slussa and reading the result in place is a core
  action, not a plugin.
- **Fast act-on-suggestion** *(not built)*. Suggestions get applied, not just displayed; the
  approve → merge path is as short as the provider allows.
- **Handoff to the coding agent** *(not built)*. A thread, a file or a whole PR can be sent to
  Claude Code (or a configured command) with context, and the outcome shows up
  back in slussa on refresh.
- **Bitbucket Data Center is in maintenance.** Atlassian ends Data Center
  licence sales and expansions on 2028-03-30 and end of life is 2029-03-28, so
  the provider stays as it is: bugs that are found get fixed, nothing new is
  built for it and no further verification is planned. New capability is
  designed for GitHub first and added to Bitbucket only when it is cheap.

What this rules out:

- **Feature parity with the web UI.** Authoring, administration and metadata
  editing are not the job (see *Scope decision* below).
- **Being a chat.** slussa is deterministic and immediate. It shows state and takes
  actions; it does not host a conversation with a model. The agent does the
  analysis, slussa is where the decision gets made.
- **Being a dashboard.** No third view, no configurable panes, no widget grid.
  gh-dash already exists; slussa wins by being the one you do not have to
  configure or learn.

Priority order for anything new: attention signals in the list → AI-review
integration → act-on-suggestion / merge path → agent handoff → diff ergonomics →
everything else.

All upcoming features follow the [product and interaction principles](ARCHITECTURE.md#product-and-interaction-principles):
a calm default view, discoverable contextual actions, focused dialogs, consistent
keyboard behavior and optional features based on provider capabilities. Feature
scope includes how users find and leave the interaction, not only the API action.

---

## Features to build

Grouped by the priority order in *Positioning*. Within a group, items marked
*refined* have a settled design; the rest still need one.

### 1. Attention signals in the list

Still one list, still one PR view. The list just knows what needs the user and
says so on the row.

- [ ] **Attention sort with a reason column** — *first version built.* The
  list opens sorted by what needs you, with a "Needs you" column (90 columns
  or wider, only when a row has a reason). Reasons, most urgent first:
  `changes requested` and `CI failed` on your own PRs, `review requested` on
  someone else's, and `approved` on yours when every reviewer approved. Only
  open PRs count. `s` toggles plain newest-first order for the session;
  `sort = "recent"` in `config.toml` sets the default. GitHub gets review
  requests from `reviewRequests` (people only, not teams); Bitbucket from
  reviewer status. **Still open:** a reason for new comments and mentions (it
  needs the local *Unread* state below), team review requests, opening the PR
  on the tab its reason points at, and remembering the `s` choice between runs.
- [ ] **Unread / updated** *(refined)* — remember per PR when it was last opened
  and flag rows with activity since then. Local state, scoped like drafts.
- [ ] **Structured filters** — `author:`, `label:`, `review:approved`, `is:draft`,
  `status:`, plus `is:agent` (see *AI authorship* below).
- [ ] **Sorting** — recently updated, created, comment count, CI status.
- [ ] **Mergeability in the PR list** — conflict / behind-base indicators
  (the detail header badge is done; this extends it to rows).
- [ ] **Labels in the list** — colored and filterable (shown in Overview today).
- [ ] **Compact diff stats** (files / +/−) on list rows.
- [ ] **Search older PRs and show the total.** Merged and declined PRs are read a
  batch at a time and `L` reads older ones. Missing: searching older PRs at the
  provider, and showing how many exist in total.
- [ ] **Jump to PR by number** (`#123`).
- [ ] **Status bar** — provider, repo, match count, loading spinner. Only if it
  fits in the existing footer line; a second persistent bar is not wanted.

### 2. AI review integration

The AI reviewer's output has to be as easy to read and act on as a human's, and
easier to tell apart.

The model below follows how real autonomous reviewers already behave (see
*Engineering*, *From OpenShell's AI reviewer*): one batched review per head SHA,
a first-line marker on every comment, stable finding IDs carried across
rounds, and a severity split where only evidenced findings block.

- [ ] **AI-authored comments marked** *(refined)* — detect AI authorship by
  *either* account (GitHub `isBot` / app login suffix; Bitbucket DC a
  configurable account list) *or* a configurable first-line marker
  (`> **gator-agent**`, `> **🏗️ build-from-issue-agent**`, …). Render with a
  distinct marker and a per-file badge count separate from human threads.
  Filter in the Overview: humans / AI / all.
- [ ] **Review summary in the header** *(refined)* — one line:
  `AI: 1 blocker · 3 suggestions · reviewed a1b2c3d (2 behind)`. Derived from
  the latest AI review: severity counts where the review exposes them,
  open/resolved counts otherwise, and the reviewed head SHA against the
  current head so a stale review reads as stale rather than wrong. Collapses
  to nothing when no AI review exists.
- [ ] **Finding state per thread** — an AI thread is *open*, *fixed* (resolved
  after a later commit, or resolved by a maintainer) or *waived* (an explicit
  "won't fix" / "intentional" reply from a maintainer). Show the state on the
  thread, keep the finding ID when the review provides one, and never
  re-surface a waived thread as attention. Derivable from thread resolution +
  resolver identity + the marker; no reviewer-specific API.
- [ ] **Run a review from slussa** *(refined)* — a key on the PR that runs a
  configured command (default `claude -p` with a review prompt and the PR
  context: title, body, diff) and posts the result either as one batched
  review with line comments or as a local-only overlay the user can promote
  to comments. The default prompt asks for each concern as "Before this PR,
  `<who>` experienced `<old>`. With this PR, `<new>`, so `<impact>`." with a
  `Critical | Warning | Suggestion` severity, and the posted review names the
  head SHA it reviewed. Command and prompt in `config.toml`; the command runs
  off the UI thread with the same deadline rules as `gh`.
- [ ] **AI authorship of the PR** — flag PRs opened by an agent account or with
  an agent trailer / label, so the reviewer knows to read for intent. `is:agent`
  filter and an inbox reason.
- [ ] **Review as a group (display)** — render a review's comments + summary +
  state as one grouped timeline entry. Matters more once AI reviews arrive as one
  batch with many comments.
- [ ] **Jump to next / prev unresolved thread** (`]c` / `[c`) — the core loop for
  walking through flags.
- [ ] **Resolved / unresolved filter** in the Overview.
- [ ] **Outdated comments** — hide threads whose anchored line is gone from the
  diff; keep them in the Overview timeline.

### 3. Act on suggestions, then merge

- [ ] **Apply suggestions** *(refined)* — apply a suggested change, and batch
  several into one commit. No provider exposes a clean "apply" API: fetch the
  file → replace the anchored line(s) → commit on the head branch. GitHub-only and
  same-repo to start; multi-line needs the range anchor below. Needs a free key
  (`b` is unused).
- [ ] **Multi-line (range) comments** — the anchor model carries one line today;
  needed for both range comments and multi-line suggestions.
- [ ] **Mergeability detail** — *partly done.* A PR the provider will not merge
  yet, for a reason other than a conflict, shows `blocked` in the header, and
  the merge dialog lists why: GitHub from `mergeStateStatus` and
  `reviewDecision` (draft, behind base, review required, changes requested,
  required checks or rules), Bitbucket from the merge checks the server
  reports. It informs and does not forbid, since an administrator may still be
  allowed to merge. A merge the server refuses now shows Bitbucket's check
  names too. Checked against a real GitHub ruleset on 2026-09-30: a missing
  approval and a branch behind its base get specific reasons, a failing required
  check only the general one. **Open:** *N commits behind base*, naming which
  required check failed on GitHub (it reports `BLOCKED` without saying), and
  tasks on Bitbucket.
- [ ] **Update / sync branch** — merge or rebase base into the PR when behind.
- [ ] **Repo-allowed merge strategies** — pre-filter the merge picker from repo
  settings instead of letting the server reject.
- [ ] **Re-run CI checks** — re-trigger a failed (or all) check from Builds.
- [ ] **Request / re-request reviewers** — including re-request after a push.
- [ ] **Delete the source branch after merge** — moved here from *Scope
  decision*: it is part of the merge path, not administration.
- [ ] **Enable auto-merge** (GitHub) — same reasoning: "merge when green" is a
  decision, not admin.
- [ ] **React to a comment** — add / remove your own emoji reaction.

### 4. Handoff to the coding agent

slussa never hosts the conversation; it hands context over and reads the result
back on refresh.

- [ ] **Send to agent** *(refined)* — from a thread, a file or the PR: run a
  configured command with a context payload (PR ref, thread body, file path and
  line, or the whole diff). Default target `claude` in the repo directory.
  Three modes: *suspend* (slussa leaves the alternate screen, the agent takes
  the terminal, slussa resumes and refreshes when it exits — the pattern in
  *Engineering*, *Suspend / resume*), *detach* (spawn in a new tmux window /
  terminal pane and return immediately), or *headless* (`-p`, output in a
  dialog). Command per mode in `config.toml`; suspend is the default because
  it needs no multiplexer.
- [ ] **Check out PR locally** — precondition for most handoffs; also useful alone.
- [ ] **Open focused file / line in editor** — `$EDITOR` at the anchored line
  (PR-level browser opening is implemented).
- [ ] **Copy additional references** — SHA / branch / permalink to a line.
- [ ] **Linked issues / cross-references** — "closes #123", shown and openable.

### 5. Diff ergonomics

Still valuable, but the reviewer reads less of the diff than before, so these
rank below the decision path.

- [ ] **Word-level (intra-line) diff.**
- [ ] **Expand context** — unfold above/below a hunk (needs a full-file fetch).
- [ ] **Side-by-side (split) diff** as an alternative to unified.
- [ ] **Rename / move display** — "renamed from X" instead of delete + add.
- [ ] **Whitespace toggle.**
- [ ] **Binary / image files** — a clear "(binary file)" instead of a broken diff.
- [ ] **Viewed-files tracking** — local "mark file reviewed", saved per PR.
- [ ] **Branch ahead / behind base** info.
- [ ] **Assignees, milestones, projects** (reviewers + labels already shown).

### 6. Providers and platform

- [ ] **GitLab MR support** via `glab` (mirrors `gh` well).
- [ ] **Windows support** — the browser and clipboard code has Windows paths,
  but CI builds only macOS and Linux and nothing has run them. Either add a
  Windows CI job and release target, or keep it stated as unsupported.
- [ ] **Bitbucket Cloud client.** (Not a successor to Data Center in this tool
  unless someone asks for it.)
- [ ] **Unified cross-provider list** with a provider icon per row.
- [ ] **Normalized "requirements to merge"** — GitLab approvals, Bitbucket
  default reviewers / merge checks, GitHub branch protection → one shared model.
- [ ] **Config** — repos / providers, default filters, keybindings, agent
  commands. *(theme is done: `~/.config/slussa/config.toml` `theme = "…"`,
  overridden by `SLUSSA_THEME`)*
- [ ] **No-panic audit** — audit fetch/parse paths so a bad response never
  panics (use `LoadState::Failed` / the popup everywhere instead of
  `unwrap`/`unreachable!`).
- [ ] **Empty / loading / error states** per view (use `LoadState` everywhere).
- [ ] **Release** — *0.1.0 is out* (2026-10-01): the repository is public, the
  tag-driven workflow published four archives with `SHA256SUMS`, and a download
  was checked. Still open: a Homebrew tap and `cargo install slussa` from
  crates.io. The macOS binary stays unsigned and unnotarized; the README gives
  the `xattr` command. "Review requested" has only been seen against scripted
  `gh` output and needs a second account to check.

### Scope decision: authoring / management

slussa is review-and-act focused. Authoring and PR *administration* belong in the
editor, the coding agent or the web UI, and are explicitly out of scope unless
a decision-path feature needs them:

- [ ] **Edit PR title / description** (the description is shown, not editable).
- [ ] **Edit labels** — add / remove (display + filter is planned).
- [ ] **Create a PR** — the agent or `gh pr create` does this.
- [ ] **Draft ↔ Ready** — borderline; revisit if agent-opened drafts become the
  norm and flipping them is part of triage.

---

## Engineering

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
  than `github/pagination.rs` and the right shape for *Search older PRs and show the total*
  under *Features to build*.
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
  *Trigger:* the repository becomes public (see *Release* under *Features to build*).
  Third-party pages say it is free for public repositories and about 24 USD per
  user and month for private ones; the vendor's own pricing page was not
  checked, so check it first.
  1. **Install the app yourself.** It is a GitHub App: install it from GitHub
     Marketplace and give it `albinlju/slussa` only. Granting access is done in
     the browser and is not something an agent should do.
  2. **`.coderabbit.yaml` is in the root** (keys checked against its
     configuration reference on 2026-10-01). Keep it in step with AGENTS.md.
  3. It reviews pull requests, not direct pushes to `main`. `/code-review` is
     run by hand; the two cover different moments.

### From OpenShell's AI reviewer ("gator"), for the AI features

OpenShell runs an autonomous PR reviewer in a sandbox. None of its code is
reusable here, but the *contract* it enforces is what slussa will read and
display once AI reviews are first-class threads (*AI review integration* above). Design the
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

### Done (engineering)

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
  effects*, AGENTS.md) so agent handoff follows the same pattern.
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
- **Process docs:** AGENTS.md as the agent instruction surface (CLAUDE.md only imports it), ARCHITECTURE.md
  lifecycles and checklists instead of a separate skill, `//!` contract docs on
  the core modules, the file-size rule in AGENTS.md, CONTRIBUTING.md,
  SECURITY.md and the PR and issue templates.
- **Dropped:** automatic light/dark theme. All five themes are dark and
  `terminal` already follows a light terminal; revisit only if a light palette
  is added.

### Not borrowed (and why)

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
