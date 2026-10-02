# Roadmap for slussa

The product idea, what is planned in priority order, and how the code should get
better. What exists today is described in the [README](../README.md) and [KEYS.md](KEYS.md); how it is
built is in [ARCHITECTURE.md](ARCHITECTURE.md). History is in git.

## Where it stands today

A read-write PR client for GitHub (through `gh`) and Bitbucket Data Center (REST
and a personal access token). The list opens sorted by what needs you, and the PR
has its description, conversation, diff, commits and builds, with comment, review,
merge, decline and reopen. What is left of the AI-specific parts is below.

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
- **Merge risk beside the reason** *(not built)*. Whether a merge can be taken
  back, and how far a mistake reaches, decides how closely a PR is read. slussa
  shows what the PR declares next to what the changed paths say, so a
  contradiction is visible.
- **AI review is first-class** *(not built)*. What an AI reviewer (Copilot,
  Claude, a team bot) did is marked as AI and summarized in the header, whether
  it commented or committed fixes: that a review happened, and against which
  commit. Running a review from slussa comes after handoff, since an automated
  review belongs before the human opens the PR.
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
- **Agents deciding.** An agent may read and propose through the CLI; approving
  and merging stay human.

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

- [ ] **Attention reasons, the rest.** The list is sorted by what needs you,
  with a reason column (the README says how). Still open: a reason for new
  comments and mentions (it needs the local *Unread* state below), team review
  requests (GitHub counts people only), opening the PR on the tab its reason
  points at, and remembering the `s` choice between runs.
- [ ] **Unread / updated** *(refined)* — remember per PR when it was last opened
  and flag rows with activity since then. Local state, scoped like drafts.
- [ ] **Merge risk** — how dangerous the merge is, beside why the PR needs
  you: can it be reverted (a two-way door) or not (a one-way door: a
  migration, data loss, something sent to users), and how far a mistake
  reaches. Two sources, shown side by side and never folded into one. The
  *declared* risk is read from the PR description by a configurable convention
  (a heading or a first-line marker, as for AI comments). The *floor* comes
  from path rules in `config.toml` (`one_way = ["migrations/**"]`) matched
  against the changed files. The declared risk is a claim by the agent that
  wrote the PR, so a floor that contradicts it is the signal: `declared
  two-way · touches migrations/`. A marker in the PR header first; nothing is
  shown when neither source says anything. Path rules cannot see a one-line
  change that sends an email to 60 000 people, so the floor adds to reading
  the PR and does not replace it. **Open:** which convention to read (none has
  settled: the PR skill the idea comes from was unreleased on 2026-10-01, so
  nothing in a description is parsed until there is a convention to read); whether
  a one-way door is an attention reason of its own or a marker beside the
  existing one; and the row, which needs the description and the changed
  paths, neither of which the list query reads today.
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

An AI reviewer shows up in one of two ways, and slussa will meet both. One
*comments*, as the "gator" reviewer in
[NVIDIA/OpenShell](https://github.com/NVIDIA/openshell) does: one batched
review per head SHA, a first-line marker on every comment, stable finding IDs
carried across rounds, and a severity split where only evidenced findings
block. The other *commits*, the model in the talk
[Fixing the PR Bottleneck](https://www.youtube.com/watch?v=LlgiOCmFG_w): it
fixes what it finds on the branch and comments only when it is unsure, so its
review is a set of commits and may leave no thread at all. That reviewers will
commonly work the second way is an assumption, which is why both are kept. "AI
actor" is therefore one notion, applied to the PR's author, to comments and to
commits.

- [ ] **AI-authored commits marked** *(refined)* — mark the commits an agent
  made in the Commits tab, by author account or a configurable trailer, so the
  commits a reviewing agent added can be told from the implementer's and read
  as one diff. It is the same notion of an AI actor as for comments (a bot
  account, or a configured marker). **Open:** `Commit` carries only an author
  name today, and an implementer and a reviewer that commit under the same
  account can be told apart only by a trailer or a marker in the message.
- [ ] **Review summary in the header** *(refined for comments)* — one line:
  `AI: 1 blocker · 3 suggestions · reviewed a1b2c3d (2 behind)`. Derived from
  the latest AI review: severity counts where the review exposes them,
  open/resolved counts otherwise, and the reviewed head SHA against the
  current head so a stale review reads as stale rather than wrong. A review
  that committed reads `AI review: 3 commits · a1b2c3d`. Collapses to nothing
  when no AI review exists, which is itself what the reviewer needs to know:
  no AI review has checked this PR. Where the review exposes them, also the
  round and "unchanged since last review": a reviewer that has gone to
  critical findings only after three rounds, or that skipped a rebase with the
  same patch, has stopped adding to what the human knows. Only a finding with
  its evidence (behaviour on the base, on the head, the impact and a
  reproducer) counts as a blocker in the summary; one without is a hypothesis.
- [ ] **Linked issues / cross-references** — "closes #123", shown in the
  header and openable. Moved here from *Handoff*: the linked issue is what was
  asked for, and checking the PR against it is the intent check the
  positioning promises.
- [ ] **Finding state per thread** — an AI thread is *open*, *fixed* (resolved
  after a later commit, or resolved by a maintainer) or *waived* (an explicit
  "won't fix" / "intentional" reply from a maintainer). Show the state on the
  thread, keep the finding ID when the review provides one, and never
  re-surface a waived thread as attention. Derivable from thread resolution +
  resolver identity + the marker; no reviewer-specific API.
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
  (`b` is unused). Worth less where the AI reviewer commits its fixes, since
  there is then no suggestion left to apply; check which kind of reviewer is
  in use before building it.
- [ ] **Multi-line (range) comments** — the anchor model carries one line today;
  needed for both range comments and multi-line suggestions.
- [ ] **Mergeability detail.** The header shows `blocked` and the merge dialog
  lists why; it informs and does not forbid, since an administrator may still
  be allowed to merge. **Open:** *N commits behind base*, naming which required
  check failed on GitHub (it reports `BLOCKED` without saying), and tasks on
  Bitbucket.
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
- [ ] **Run a review from slussa** *(refined)* — a key on the PR that runs a
  configured command (default `claude -p` with a review prompt and the PR
  context: title, body, diff) and posts the result either as one batched
  review with line comments or as a local-only overlay the user can promote
  to comments. The default prompt asks for each concern as "Before this PR,
  `<who>` experienced `<old>`. With this PR, `<new>`, so `<impact>`." with a
  `Critical | Warning | Suggestion` severity, and the posted review names the
  head SHA it reviewed. Command and prompt in `config.toml`; the command runs
  off the UI thread with the same deadline rules as `gh`. Moved here from *AI
  review integration* and placed after *Send to agent*: an automated review
  belongs before the human opens the PR, and one that posts comments gives the
  human more to read. When the header says no review has happened, sending
  the PR to an agent that fixes is the shorter path; this stays for the
  repository that has no reviewer in its pipeline.
- [ ] **Check out PR locally** — precondition for most handoffs; also useful alone.
- [ ] **Open focused file / line in editor** — `$EDITOR` at the anchored line
  (PR-level browser opening is implemented).
- [ ] **Copy additional references** — SHA / branch / permalink to a line.

#### The other direction: an agent calling slussa

Handoff above sends context *to* an agent. The reverse is an agent that triages
or checks a PR by calling slussa itself. `gh` already gives an agent raw data;
slussa adds what it computes: why a PR needs the human (the attention reason) and
why a merge is blocked, the same on both providers. The rule that keeps this in
step with the positioning: **agents may read and propose; only the human
decides.** No new view: these are non-interactive subcommands that print and exit.

- [ ] **`slussa <number>` (or a PR URL)** starts the TUI on that PR. The natural
  landing point when an agent says "PR 123 is ready for you"; it complements
  *Jump to PR by number* inside the TUI.
- [ ] **`slussa list --json`** — open PRs in the TUI's order with the "Needs you"
  reason (reuses `domain::attention`). Output has `"schema": 1`, snake_case
  identifiers (`ci_failed`, not "CI failed") and flat usernames; errors go to
  stderr as JSON, never mixed into stdout. The JSON types are separate from the
  domain types, so internal changes do not change the output. A headless command
  never asks for input: it connects with `preflight::connect` and fails when the
  account is not logged in, instead of going through `cli::connect`, which starts
  the interactive `gh auth login`.
- [ ] **`slussa blocked <number>`** — why the PR cannot be merged: the provider's
  reasons from `Mergeability` (`Conflicts` or `Blocked`), plus CI and review
  state, which are separate from it. Exit 0 mergeable, 3 not mergeable, 1 error,
  2 usage; what `Unknown` should give is still to decide. One function defines
  "not mergeable" once.
- [ ] **Exit codes that mean something** — `kind()` on `FetchError` and
  `PreflightError` (see *Error classification for the caller* in Engineering):
  retryable, needs auth, not found, invalid.
- [ ] **`slussa threads <number> --unresolved`** — unresolved threads with file and
  line, and finding state once *Finding state per thread* exists.
- [ ] **`slussa threads --since <duration>`** (retro export) — what humans
  wrote in review, and which AI findings a maintainer waived, across the PRs
  of a period instead of one PR. It is the input to a retrospective that turns
  a repeated comment into an automated check or a coding standard, so that the
  same comment is not written twice. `gh` gives the raw comments; slussa adds
  who is human, who is AI and what was waived. Depends on *AI-authored
  comments and commits marked* and *Finding state per thread*. **Open:** the
  name and flags, and how many PRs one run may read.
- [ ] **`slussa context <number>`** — one compact text package for an LLM (title,
  description, unresolved threads, CI, blockers). It is the package *Send to
  agent* needs too, so build it once; it needs a size rule for long threads and
  diffs.
- [ ] **Agents propose, the human decides** — `slussa draft comment …` and
  `slussa review import …` put proposals in a local inbox that the TUI shows
  marked as AI, and you send, edit or discard each. Depends on *AI-authored
  comments and commits marked* and on a store separate from the draft file, which the TUI
  holds locked for as long as it runs (`DraftStorage`) and rewrites whole when its
  content changes, so a second process cannot write into it. Needs de-duplication (a content hash or finding ID) and
  anchors that carry their `DiffRevision`. This is the part nobody else has;
  build it after the marker.
- [ ] **`slussa agent-instructions`** — prints how an agent should use slussa, as a
  short snippet for AGENTS.md or a skill. A CLI plus this is simpler than an MCP
  server for a local tool built on `gh` and `git`; revisit MCP later (it is also
  the trigger for a `slussa-core`).

Left out on purpose: `wait --ci` (`gh pr checks --watch` does it) and approve or
merge for agents. The JSON is a public contract, so keep it marked experimental
(`"schema": 1`) until it is used. GitHub first: Bitbucket is in maintenance, and
these commands go through the same provider code but are not planned to be tested
against it.

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
  commands.
- [ ] **Empty / loading / error states** per view (use `LoadState` everywhere).
- [ ] **Release gaps.** What is not there yet: a Homebrew tap; publishing to
  crates.io from the release workflow (today `cargo publish` is run by hand
  from the tag, which needs a token); signing and notarizing the macOS binary
  (the README gives the `xattr` command); and a check of "Review requested",
  which has only been seen against scripted `gh` output and needs a second
  account.

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

Rule of thumb for everything below: **do the refactors when a feature touches
the code anyway, do the tooling now.** Every open item has a trigger; do not do
it ahead of the feature that needs it.

### Code, with the next feature that touches the area

- [ ] **Keybinding table for the PR view.** `pr_detail/keys.rs` is a long
  function of conditions, each mixing key, tab, modifiers and PR state, and
  the help is a second table written apart from it (`dialogs/help.rs`, where
  each entry names what it needs, held to `docs/KEYS.md` by a test). Replace
  both with a slice of `Binding { key, mods, tabs, gate: fn(&DetailView,
  pr_id) -> bool, action, help }`; routing and help are then derived from one
  source, and `supports_action` in `view.rs` becomes the gate. With the keys
  in a table, `clippy::wildcard_enum_match_arm` can cover `tui` as well:
  today 27 matches on crossterm's `KeyCode` end in a catch-all, so the lint
  is on only in `app`, `domain` and `providers`. *Trigger:* the next feature
  that adds a key to the PR view; the AI-comment work added two (`f` and
  `space`) without it.
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
  classification instead of from prose in the message. `FetchError` already
  keeps its cause as a value and `may_have_reached_server()` decides whether a
  failed write is marked for checking; the split between log message and user
  message is right and should be kept. *Trigger:* next change to the error
  dialog or a new provider.
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
- [ ] **Keep the files under the size rule** (modules stay under about 500
  lines, `mod.rs` composes; `tests/repo_rules.rs` fails a file over 600). Five
  `mod.rs` files still implement instead of composing: `tui/`, `tui/widgets/`,
  `providers/`, `providers/github/` and `providers/bitbucket_dc/`; the two
  provider ones go with *Provider trait*. Not a refactor-only change.
- [ ] **A workspace, when something else needs the core.** slussa is one crate
  with clear module boundaries; a workspace adds compile-unit overhead and
  manifest churn without a consumer for the split crates, and an attempt on a
  branch (`domain` and `providers` as crates) was dropped for that reason. The
  pieces would be `slussa-core`, `slussa-provider-github`,
  `slussa-provider-bitbucket-dc` and `slussa-tui`. *Trigger:* a `slussa-core`
  becomes a dependency of something else (a Neovim plugin, an MCP server).
- [ ] **Cache rendered Markdown.** `markdown::render` runs for the
  description, and `render_no_margin` for every comment in the Overview and in
  the diff, on every frame. Keep the lines per comment and width, and drop
  them when the activity is re-read. *Trigger:* a PR whose conversation
  scrolls slowly, or the AI-review threads, which make conversations longer.
- [ ] **One place for text width.** Width is measured by `chars().count()` in
  three places and by ratatui's `width()` in the rest; they disagree on wide
  and combining characters. Move truncation, padding and wrapping into one
  module. *Trigger:* the next layout bug with CJK text or emoji.
- [ ] **Ratatui's `Scrollbar` and `Tabs`** instead of the hand-drawn ones in
  `widgets/mod.rs` and the PR screen's tab row. *Trigger:* the next change to
  either.
- [ ] **Select by id, not by index.** The list and the commit list keep a
  position and reconcile it by id when the data changes. Holding the id makes
  the reconciling unnecessary. *Trigger:* *Unread / updated*, or any feature
  that reorders the list while it is open.
- [ ] **The open tab is stored twice**, in `Screen::Detail { tab }` and in
  `PrDetailScreen::active_tab`, and kept in step by `navigate`. One should be
  derived from the other. *Trigger:* the next change to tab navigation.
- [ ] **Functions over 100 lines.** `too_many_lines` is allowed crate-wide;
  the long ones are renderers (`pane.rs`, `comment.rs`, `timeline.rs`) and
  `pr_detail/keys.rs`. *Trigger:* *Render-context structs* and *Keybinding
  table*, which split them anyway.
- [ ] **Small type changes left out of the 2026-10 type work**, because none
  removes a check today: a `PrInfoSource` in `Capabilities` instead of
  `Feature::PrInfo`, and `sort` and `theme` parsed by serde instead of by
  `Sort::from_config` and `theme::init` (each is already parsed once, at
  startup). *Trigger:* *Config*, for the second.
- [ ] **The first load is slow on a large repository.** On `cli/cli` (63 open
  PRs) the first frame took about 0.75 s for `git`, `gh --version`, `gh auth
  status` and `gh api user`, and the first page of 30 open PRs 2.0 to 2.7 s.
  The open pages are a cursor chain and cannot run in parallel; what a page
  costs is the selection. **Left, if it is still not enough:** two phases (core
  fields first, reviews, CI and requests after; about 1.1 to 1.4 s a page) and
  re-reading only what changed instead of the whole list every minute.
  Searching for the PRs that need the viewer was considered and declined,
  because the search index lags, and so was a persisted last list. Timings are
  noisy: one network, one repository.

### Tests and tooling

- [ ] **Snapshots with realistic content.** `tui/testdata/screens.txt` covers
  100x30 and 40x12 for the list and all five detail tabs, but with placeholder
  data: one PR, no diff, no threads. Add a snapshot with a diff with an inline
  thread, several commits and a failing build at both sizes, and one at 80x24.
  Consider `insta` then: one file per screen and a review step for changes,
  instead of one text file compared whole.
- [ ] **`cargo nextest`** in CI (parallel, per-test timeouts, clearer failure
  output). Local `cargo test` stays fine.
- [ ] **reqwest 0.13.** The crate is on 0.12; 0.13 is out. Not looked into.
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

### Done (engineering)

Kept as one line each; the detail is in git history.

- **Panic audit:** no `expect`, `unreachable!` or `#[allow]` is left in
  non-test code. A GraphQL or REST answer of the wrong shape is a load error
  (it indexed into the value and could panic), and scroll positions past
  65 535 rows are capped instead of wrapping.
- **Types before runtime checks (2026-10):** the rule in AGENTS.md and what it
  produced, listed in ARCHITECTURE.md (*Types that carry the rules*).
  `Action` is split into `Action` (input), `Effect` (work for the app) and
  `TaskResult` (what came back), and `DetailAction` into a sub-enum per
  handler, which removed the last four `unreachable!`. Components have
  separate `Input` and `View` contexts. A read needs a `FetchTicket` and a
  write a `WriteTicket`. `FetchError` is a typed enum carried as a value. The
  PR screen's dialogs, the list's overlay, the editor and the diff pane each
  hold one state instead of parallel flags. `PrId`, `CommentId` and
  `CommitOid` replace bare numbers and strings. `wildcard_enum_match_arm` is
  on in `app`, `domain` and `providers`. Bugs it found: `r` on the commit list
  replied to the thread the Diff tab had left focused; the footer there offered
  two keys that did nothing; every failed write was marked "may have reached
  the server", also one refused before it was sent; keys moved the diff cursor
  over rows no longer drawn; a Bitbucket verdict that failed with nothing
  else sent was reported as a partly sent review; an HTTP 408 on a write was
  read as a refusal. A printed Bitbucket provider would also have included
  its token.
- **Fixes from the 2026-10 quality review:** drafts are no longer synced to
  disk on every key typed in the editor; a Bitbucket 403 is shown as the
  server's refusal and not as a missing login; a failed account lookup at
  startup says why; diff paths with spaces, quotes or non-ASCII letters are
  read whole (they were cut at the first space, which also misplaced a comment
  on such a file); a test build has no `gh` unless a fake is installed; a
  thread taller than the diff pane is shown from its first row, as in the
  Overview (it was scrolled to its last row); a terminal that fails to start
  is handed back out of raw mode.
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
  (`app/flow_tests/`), the transport doubles `FakeGh` and `MockHttp` and their
  18 transport tests, shared fixtures in `src/test_support.rs`, and CLI
  integration tests (`tests/cli_integration.rs`, isolated HOME, no network).
- **Lints:** `clippy::{all, pedantic, nursery}` and a `[lints.rust]` block, two
  nursery lints allowed by name. The no-panic rule is a lint (`unwrap_used`,
  `expect_used`, `panic`, `unreachable`, `unimplemented`, `indexing_slicing`),
  `unsafe_code` is forbidden, truncating casts are refused again, and an
  exception is an `#[expect]` with a reason: the switch from `#[allow]` found
  two that were no longer needed. `rust-toolchain.toml` pins 1.98.1 and
  `rust-version` stays 1.95, which CI builds with as well.
- **CI** on Ubuntu and macOS (fmt, clippy, test, cargo-deny, a build with the
  minimum Rust version, and cargo-deny again every week), actions pinned by
  SHA; **cargo-deny** found RUSTSEC-2026-0285 in `rustls` 0.23.40 (fixed by
  updating to 0.23.45); `colored` and `option-ext` are named MPL-2.0 exceptions.
- **Dependabot** (weekly for Actions, monthly grouped for Cargo) and release
  profile `strip = true`, `lto = "thin"`, `codegen-units = 1` (the macOS arm64
  binary went from 7.0 to 6.1 MB), dev `debug = 1`. Not `panic = "abort"`:
  Markdown rendering relies on catching a panic in the renderer.
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
