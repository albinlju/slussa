# Feature backlog for tuipr

**Where it stands today:** a read-write PR client over two providers — GitHub
(via the `gh` CLI) and Bitbucket Data Center (REST + PAT). You can read, comment,
review, merge, and decline.

## Positioning

tuipr is the **human approval surface for AI-generated pull requests**: the place
where a reviewer decides, not the place where the code gets read line by line.

The assumption behind the backlog: agents write more of the code and open more of
the PRs, and AI reviewers do the line-level reading. What stays human is triage
("what needs me?"), intent-checking ("did the agent do what was asked, and what did
the AI review flag?"), and the sign-off itself (approve, merge, decline). Those are
overview-and-decision tasks, and a keyboard-driven terminal UI is better at them
than a web page. tuipr should be the missing piece in a terminal workflow next to
an editor, a git client and a coding agent — never a competitor to the provider's
web UI.

**Simplicity is the constraint everything else bends to.** tuipr has two views —
the list and the PR — and stays that way. New capability shows up as a better
default, a column, a marker or a single key inside those two views, not as a new
screen, a dashboard or a sidebar of widgets. If a feature cannot be explained in
one sentence and reached in one keypress, it is not ready. When in doubt, leave
it out.

What this rules in:

- **The list is the inbox.** The PR list opens sorted by what needs the user —
  review requests, failed CI, new activity since last look — with a short reason
  on the row. No separate inbox screen; the plain list is the same view with the
  attention sort turned off.
- **AI review is a first-class thread.** Comments from an AI reviewer (Copilot,
  Claude, a team bot) are shown inline with their own marker and summarized in the
  header. Running a review from tuipr and reading the result in place is a core
  action, not a plugin.
- **Fast act-on-suggestion.** Suggestions get applied, not just displayed; the
  approve → merge path is as short as the provider allows.
- **Handoff to the coding agent.** A thread, a file or a whole PR can be sent to
  Claude Code (or a configured command) with context, and the outcome shows up
  back in tuipr on refresh.
- **Providers others neglect.** Bitbucket Data Center support is a real gap in
  the terminal-tooling space and stays a supported provider, not a port.

What this rules out:

- **Feature parity with the web UI.** Authoring, administration and metadata
  editing are not the job (see *Scope decision* below).
- **Being a chat.** tuipr is deterministic and immediate. It shows state and takes
  actions; it does not host a conversation with a model. The agent does the
  analysis, tuipr is where the decision gets made.
- **Being a dashboard.** No third view, no configurable panes, no widget grid.
  gh-dash already exists; tuipr wins by being the one you do not have to
  configure or learn.

Priority order for anything new: attention signals in the list → AI-review
integration → act-on-suggestion / merge path → agent handoff → diff ergonomics →
everything else.

All upcoming features follow the [product and interaction principles](ARCHITECTURE.md#product-and-interaction-principles):
a calm default view, discoverable contextual actions, focused dialogs, consistent
keyboard behavior and optional features based on provider capabilities. Feature
scope includes how users find and leave the interaction, not only the API action.

---

## Done

- [x] **Existing UI polish** — consistent dialog footers, compact PR headers,
  adaptive Files/Code panels, contextual hints, scrollable errors and CI lists,
  useful empty states and visible recovery from failed refreshes.

- [x] **Open/copy PR links** — `o` opens the selected PR in the default browser;
  `y` copies its link. Both work in list and detail views, appear in help only
  when a URL is available, and show brief non-modal feedback.

- [x] **Multiline comment editor** — focused dialog, Enter for newline, Ctrl+S to
  submit, cursor movement and bracketed paste. Esc keeps the draft; `c` resumes
  it. Ctrl+X explicitly confirms discarding it.
- [x] **Persistent local drafts** — editor drafts and review queues survive
  restart, scoped by repo/provider/account. Atomic saves, one writer per scope,
  partial-review receipts and interrupted-request notices protect recovery.

- [x] **Component architecture** — local UI state and behavior live with their
  components; a shared Store holds provider data and review drafts keyed by PR.
  Rendering snapshots and interaction regression tests protect existing flows.

- [x] **PR list** with status filter (open / draft / merged / declined / all).
- [x] **Reviewers / approvals in the list** — a state icon per reviewer.
- [x] **Description tab**, rendered above the tab row.
- [x] **Overview tab** — conversation timeline (comments + lifecycle events) with a sidebar (reviewers, builds summary, labels, details).
- [x] **Diff tab** — file tree on the left, diff pane on the right, +/− row coloring, per-file +A/−D stats.
- [x] **Inline review threads in the diff** — comments anchored to their line (new- and old-side), a comment-count badge per file in the tree, and a cursor that can focus a thread (accent border) as groundwork for replying.
- [x] **Commits tab** — commit list with a per-commit diff drill-in (`enter` opens, `[`/`]` prev/next, `esc` back); reuses the Diff widget.
- [x] **Builds / Checks tab** — CI build statuses for the source commit with a pass/total summary.
- [x] **Bitbucket Data Center provider** — PRs, diff, commits, comments/events/threads, builds — alongside GitHub.
- [x] **Markdown rendering** in descriptions and comments.
- [x] **File filter** (`/`) in the diff tree — case-insensitive substring match.
- [x] **PR list search** — free text on title / author.
- [x] **Reactions** — emoji reactions on comments (read-only), GitHub and
  Bitbucket DC. GitHub via `reactionGroups`; Bitbucket DC reads `properties.reactions`
  from the activities feed (emoji decoded from the twemoji URL codepoint).
- [x] **Suggestions (display)** — `suggestion` blocks are parsed out of the comment
  body (there's no dedicated suggestion API — they ride along in the comment markdown)
  and rendered as a "◆ Suggested change" box: the anchored line as `−`, the proposed
  lines as `+`, a `−1 +N` stat. Display only — there is no apply action.
- [x] **Comment actions** — reply (`r`), edit (`e`) / delete (`d`) your own comments
  (Ctrl-j/k sub-cursor to pick one in a thread), and resolve/unresolve (`R`). Resolved
  threads collapse to a one-line summary in the diff (`space` to expand); the Overview
  keeps the full thread.
- [x] **Auto / manual refresh** — the active view re-fetches in the background (Builds
  every 15s, everything else every 60s) and on a manual `F`; a `⟳ refreshing` indicator
  shows in the footer. A failed reload keeps the stale data instead of blanking the view.
- [x] **Review verdicts** (`a`) — Approve / Request changes (with a summary body) /
  Comment / Unapprove (where the provider allows withdrawing approval). On your own PR
  only Comment is offered; the others are dimmed with a reason in the picker.
- [x] **Batched review** (`v`) — start a review, queue line comments (shown as `pending`
  in the diff, `d` removes one), then finish with a verdict that flushes them in one
  submission. GitHub sends one atomic call; Bitbucket posts the comments then flips status.
- [x] **Merge** (`m`) — strategy picker (Merge / Squash / Rebase on GitHub, the repo
  default on Bitbucket); gated on the mergeability status.
- [x] **Decline / close** (`x`) — confirm, then decline (Bitbucket) / close (GitHub).
- [x] **Mergeability status** — a header badge (mergeable / conflicts / unknown), fetched
  lazily per PR; gates the merge action.
- [x] **Disabled-with-affordance** — lifecycle actions stay visible in the footer but
  dimmed with their reason (e.g. `m: merge (conflicts)`) rather than being hidden.
- [x] **Help overlay** (`?`) — lists the keybindings.

---

## Backlog

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
- [ ] **Pagination / load more** — the list is currently capped at 50.
- [ ] **Jump to PR by number** (`#123`).
- [ ] **Status bar** — provider, repo, match count, loading spinner. Only if it
  fits in the existing footer line; a second persistent bar is not wanted.

### 2. AI review integration

The AI reviewer's output has to be as easy to read and act on as a human's, and
easier to tell apart.

The model below follows how real autonomous reviewers already behave (see
IMPROVEMENTS.md, *From their AI reviewer*): one batched review per head SHA,
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
- [ ] **Run a review from tuipr** *(refined)* — a key on the PR that runs a
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
- [ ] **Mergeability detail** — *N commits behind base* and *required-checks
  gating* (both currently collapse to `Unknown`).
- [ ] **Update / sync branch** — merge or rebase base into the PR when behind.
- [ ] **Repo-allowed merge strategies** — pre-filter the merge picker from repo
  settings instead of letting the server reject.
- [ ] **Re-run CI checks** — re-trigger a failed (or all) check from Builds.
- [ ] **Request / re-request reviewers** — including re-request after a push.
- [ ] **Delete the source branch after merge** — moved here from *Scope
  decision*: it is part of the merge path, not administration.
- [ ] **Enable auto-merge** (GitHub) — same reasoning: "merge when green" is a
  decision, not admin.
- [ ] **Reopen a closed PR.**
- [ ] **React to a comment** — add / remove your own emoji reaction.

### 4. Handoff to the coding agent

tuipr never hosts the conversation; it hands context over and reads the result
back on refresh.

- [ ] **Send to agent** *(refined)* — from a thread, a file or the PR: run a
  configured command with a context payload (PR ref, thread body, file path and
  line, or the whole diff). Default target `claude` in the repo directory.
  Three modes: *suspend* (tuipr leaves the alternate screen, the agent takes
  the terminal, tuipr resumes and refreshes when it exits — the pattern in
  IMPROVEMENTS.md, *Suspend / resume*), *detach* (spawn in a new tmux window /
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
- [ ] **Bitbucket "tasks"** — checkable to-do items on a PR.
- [ ] **Assignees, milestones, projects** (reviewers + labels already shown).

### 6. Providers and platform

- [ ] **GitLab MR support** via `glab` (mirrors `gh` well).
- [ ] **Bitbucket Cloud client.**
- [ ] **Unified cross-provider list** with a provider icon per row.
- [ ] **Normalized "requirements to merge"** — GitLab approvals, Bitbucket
  default reviewers / merge checks, GitHub branch protection → one shared model.
- [ ] **Config** — repos / providers, default filters, keybindings, agent
  commands. *(theme is done: `~/.config/tuipr/config.toml` `theme = "…"`,
  overridden by `TUIPR_THEME`)*
- [ ] **No-panic audit** — audit fetch/parse paths so a bad response never
  panics (use `LoadState::Failed` / the popup everywhere instead of
  `unwrap`/`unreachable!`).
- [ ] **Empty / loading / error states** per view (use `LoadState` everywhere).
- [ ] **Release** — *partly done:* README, MIT license and a tag-driven
  release workflow for four targets exist (see RELEASING.md). Still open: a
  demo gif in the README, the first real release, a Homebrew formula,
  `cargo install tuipr` from crates.io, macOS notarization. Without a
  release the rest has no audience.

### Scope decision: authoring / management

tuipr is review-and-act focused. Authoring and PR *administration* belong in the
editor, the coding agent or the web UI, and are explicitly out of scope unless
a decision-path feature needs them:

- [ ] **Edit PR title / description** (the description is shown, not editable).
- [ ] **Edit labels** — add / remove (display + filter is planned).
- [ ] **Create a PR** — the agent or `gh pr create` does this.
- [ ] **Draft ↔ Ready** — borderline; revisit if agent-opened drafts become the
  norm and flipping them is part of triage.
