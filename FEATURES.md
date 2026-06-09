# Feature backlog for tuipr

**Where it stands today:** a read-only PR client over two providers — GitHub
(via the `gh` CLI) and Bitbucket Data Center (REST + PAT). 

---

## Done

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

---

## Backlog

### Redifined

- [ ] **Suggestions** — render GitHub/GitLab "suggested change" blocks specially.
- [ ] **Reactions** — show emoji reactions on comments (Bitbucket DC has a reactions API).
- [ ] **Fuzzy file finder / filter** in the diff tree (for big PRs).
- [ ] **Merge conflict / mergeability status** — can it merge? conflicts? N commits behind base? required checks gating.
- [ ] **Assignees, milestones, projects** (reviewers + labels are already shown).
- [ ] **PR list search** — free text on title / author.
- [ ] **Branch ahead / behind base** info.
- [ ] **Mergeability in the pr list** — conflict / behind-base indicators.
- [ ] **Labels in the list** — colored and filterable (shown in Overview today).
- [ ] **Errors visible in the UI** instead of panics when a fetch/parse fails.
- [ ] **Reply to / resolve a comment thread** — the cursor-focus groundwork is already in place.
- [ ] **Manual / auto refresh** (`r`) — re-fetch without restarting.
- [ ] **Build jobs auto update status** — Builds should re-fetch status automatically while open tab
- [ ] **Approve / Request changes / Comment** — needs a small text-input mode.

### Not redefined

- [ ] **Word-level (intra-line) diff** — highlight the changed words within a modified line.
- [ ] **Expand context** — unfold more lines above/below a hunk (needs a full-file fetch).
- [ ] **Side-by-side (split) diff** as an alternative to unified; jump between files/hunks.
- [ ] **Rename / move display** — "renamed from X" instead of delete + add.
- [ ] **Whitespace toggle** — ignore whitespace-only changes.
- [ ] **Binary / image files** — a clear "(binary file)" instead of a broken diff.
- [ ] **Jump to next / prev unresolved thread** (`]c` / `[c`).
- [ ] **Resolved / unresolved filter** in the Overview.
- [ ] **Outdated comments** — mark comments whose line changed since they were made.
- [ ] **Review as a group** — bundle a review's comments + summary + state (approved / changes requested), instead of loose timeline entries.
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
- [ ] **Help overlay** (`?`) listing all keys.
- [ ] **Status bar** — provider, repo, match count, loading spinner.
- [ ] **Config** — repos / providers, default filters, keybindings, theme.
- [ ] **Empty / loading / error states** per view (use `LoadState` everywhere).
- [ ] **Open in browser** (`o`) — the PR / focused file / line. Cheapest big payoff. *(write-adjacent)*
- [ ] **Copy** (`y`) — SHA / branch / PR URL / permalink to a line.
- [ ] **Check out PR locally**.
- [ ] **Merge / Squash / Rebase** — only the strategies the repo allows.
- [ ] **Draft ↔ Ready**.
- [ ] **GitLab MR support** via `glab` (mirrors `gh` well).
- [ ] **Unified cross-provider list** with a provider icon per row.
- [ ] **Normalized "requirements to merge"** — GitLab approvals, Bitbucket default reviewers / merge checks, GitHub branch protection → one shared model.

