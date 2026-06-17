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
- [x] **File filter** (`/`) in the diff tree — case-insensitive substring match.
- [x] **PR list search** — free text on title / author.
- [x] **Reactions** — emoji reactions on comments (read-only), GitHub and
  Bitbucket DC. GitHub via `reactionGroups`; Bitbucket DC reads `properties.reactions`
  from the activities feed (emoji decoded from the twemoji URL codepoint).
- [x] **Suggestions (display)** — `suggestion` blocks in review comments render as
  a "◆ Suggested change" box with the anchored line as `−`, the proposed lines as
  `+`, and a `−1 +N` stat. Applying/batching is a separate backlog item.
- [x] **Comment actions** — reply (`r`), edit (`e`) / delete (`d`) your own comments
  (Ctrl-j/k sub-cursor to pick one in a thread), and resolve/unresolve (`R`). Resolved
  threads collapse to a one-line summary in the diff (`space` to expand); the Overview
  keeps the full thread.
---

## Backlog

### Refined

- [ ] **Apply suggestions** — `a` to apply a suggested change, `b` to batch several
  into one commit (display already done).
- [ ] **Merge conflict / mergeability status** — can it merge? conflicts? N commits behind base? required checks gating.
- [ ] **Assignees, milestones, projects** (reviewers + labels are already shown).
- [ ] **Branch ahead / behind base** info.
- [ ] **Mergeability in the pr list** — conflict / behind-base indicators.
- [ ] **Labels in the list** — colored and filterable (shown in Overview today).
- [ ] **Errors visible in the UI** instead of panics when a fetch/parse fails.
- [ ] **Optimistic comment insert** — show a just-posted line comment instantly (local insert into `activity`) instead of the ~3-5s wait for the refetch; needs current-user to attribute it correctly. ("posting…" indicator is already done.)
- [ ] **Manual / auto refresh** (`r`) — re-fetch without restarting.
- [ ] **Build jobs auto update status** — Builds should re-fetch status automatically while open tab
- [ ] **Approve / Request changes / Comment** — needs a small text-input mode.
- [ ] **Implement Bitbucket Cloud client** 
- [ ] **Implement Gitlab client**

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

