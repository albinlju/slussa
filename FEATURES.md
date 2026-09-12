# Feature backlog for tuipr

**Where it stands today:** a read-write PR client over two providers — GitHub
(via the `gh` CLI) and Bitbucket Data Center (REST + PAT). You can read, comment,
review, merge, and decline.

All upcoming features follow the [product and interaction principles](ARCHITECTURE.md#product-and-interaction-principles):
a calm default view, discoverable contextual actions, focused dialogs, consistent
keyboard behavior and optional features based on provider capabilities. Feature
scope includes how users find and leave the interaction, not only the API action.

---

## Done

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

### Refined

- [ ] **Apply suggestions** — apply a suggested change, and batch several into one
  commit. No provider exposes a clean "apply" API: it means fetch the file → replace
  the anchored line(s) → commit on the head branch. Realistically GitHub-only and
  same-repo to start (forks need the fork's coords + push access), and our `ThreadAnchor`
  only carries a single line, so multi-line suggestions aren't applicable yet. Needs a
  free key — `a` is the review menu, `b` is unused.
- [ ] **Mergeability detail** — beyond the basic mergeable/conflicts badge: *N commits
  behind base* and *required-checks gating* (both currently collapse to `Unknown`).
- [ ] **Repo-allowed merge strategies** — the picker offers Merge / Squash / Rebase on
  GitHub regardless of repo settings and lets the server reject; querying the repo would
  pre-filter the menu.
- [ ] **Assignees, milestones, projects** (reviewers + labels are already shown).
- [ ] **Branch ahead / behind base** info.
- [ ] **Mergeability in the pr list** — conflict / behind-base indicators (the detail
  header badge is done; this extends it to list rows).
- [ ] **Labels in the list** — colored and filterable (shown in Overview today).
- [ ] **No-panic audit** — the error popup + `FetchError::user_message()` are done; what
  remains is auditing fetch/parse paths so a bad response never panics (use
  `LoadState::Failed` / the popup everywhere instead of `unwrap`/`unreachable!`).
- [ ] **React to a comment** — add / remove your own emoji reaction (display is done but
  read-only). GitHub `addReaction` / `removeReaction`; Bitbucket DC reaction endpoints.
- [ ] **Request / re-request reviewers** — ask a user or team to review, and re-request
  after pushing changes. Today "Review requested" exists only as a list view, not an action.
- [ ] **Re-run CI checks** — re-trigger a failed (or all) check from the Builds tab.
- [ ] **Update / sync branch** — when the PR is behind base, merge or rebase base into it
  (pairs with the *behind base* indicator).
- [ ] **Reopen a closed PR** — the inverse of decline / close.
- [ ] **Multi-line (range) comments** — comment on a selected line range, not just a
  single line (the anchor model currently carries one line).
- [ ] **Implement Bitbucket Cloud client** 
- [ ] **Implement Gitlab client**

### Scope decision: authoring / management

tuipr is review-and-act focused (read, comment, review, merge, decline). Authoring and
PR *administration* are deliberately not built — decide whether they belong here at all
before treating them as gaps:

- [ ] **Edit PR title / description** (the description is shown, not editable).
- [ ] **Edit labels** — add / remove (only display + filter is planned).
- [ ] **Create a PR.**
- [ ] **Delete the source branch after merge** (a merge option).
- [ ] **Enable auto-merge** (GitHub).

### Not refined

- [ ] **Word-level (intra-line) diff** — highlight the changed words within a modified line.
- [ ] **Expand context** — unfold more lines above/below a hunk (needs a full-file fetch).
- [ ] **Side-by-side (split) diff** as an alternative to unified; jump between files/hunks.
- [ ] **Rename / move display** — "renamed from X" instead of delete + add.
- [ ] **Whitespace toggle** — ignore whitespace-only changes.
- [ ] **Binary / image files** — a clear "(binary file)" instead of a broken diff.
- [ ] **Jump to next / prev unresolved thread** (`]c` / `[c`).
- [ ] **Resolved / unresolved filter** in the Overview.
- [ ] **Outdated comments** — a thread whose anchored line no longer exists in the current diff is *outdated*: hide it from the diff (like GitHub) and show it only in the Overview timeline. Needs comparing each thread's anchor against the loaded diff.
- [ ] **Review as a group (display)** — in the timeline, render a review's comments +
  summary + state (approved / changes requested) as one grouped entry instead of loose
  entries. (Submitting a batched review is already done — this is the read side.)
- [ ] **Bitbucket "tasks"** — show the checkable to-do items on a PR.
- [ ] **Linked issues / cross-references** — "closes #123".
- [ ] **Compact diff stats** (files / +/−) on PR list rows.
- [ ] **Structured filters** — `author:`, `label:`, `review:approved`, `is:draft`, `status:`.
- [ ] **Sorting** — recently updated, created, comment count, CI status.
- [ ] **"Mine" quick views** — Created / Assigned / Review requested / Mentioned.
- [ ] **Pagination / load more** — the list is currently capped at 50.
- [ ] **Jump to PR by number** (`#123`).
- [ ] **Unread / updated** — flag PRs with new activity since you last looked.
- [ ] **Notifications inbox** — "what needs my attention" (review requested, mentioned, CI failed).
- [ ] **Viewed-files tracking** — local "mark file reviewed" (GitHub's *Viewed*), saved per PR.
- [ ] **Status bar** — provider, repo, match count, loading spinner.
- [ ] **Config** — repos / providers, default filters, keybindings. *(theme is done: `~/.config/tuipr/config.toml` `theme = "…"`, overridden by `TUIPR_THEME`)*
- [ ] **Empty / loading / error states** per view (use `LoadState` everywhere).
- [ ] **Open focused file / line in browser** — PR-level opening is implemented.
- [ ] **Copy additional references** — SHA / branch / permalink to a line. PR URL copying is implemented.
- [ ] **Check out PR locally**.
- [ ] **Draft ↔ Ready**.
- [ ] **GitLab MR support** via `glab` (mirrors `gh` well).
- [ ] **Unified cross-provider list** with a provider icon per row.
- [ ] **Normalized "requirements to merge"** — GitLab approvals, Bitbucket default reviewers / merge checks, GitHub branch protection → one shared model.
