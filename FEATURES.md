# Feature ideas for tuipr

An idea backlog for where the app could grow, grounded in how GitHub, GitLab,
Bitbucket and others handle pull/merge requests. Ordered roughly by value/effort —
not a commitment, just a map to pick from. Check off `- [x]` as things land.

**Where the app stands today:** PR list + detail view with tabs (Conversation,
Commits, Checks*, Files*) backed by the `gh` CLI. `*` = still a placeholder. The
`providers` module + `domain::provider::ProviderKind` already hint that multiple
providers are intended. The search view is drafted in `SEARCH_SCREEN.md`.

---

## Tier 1 — Finish what's already started (high value, low effort)

Mostly completing tabs that already exist as placeholders.

- [x] **diff view.** `gh pr diff <n>`. File tree on the left, diff on the
  right, syntax highlighting.
- [ ] **Checks/CI status.** "Builds". Fetch via `gh pr checks <n>` →
  a list with green/red/yellow per check + link. Also show a summary icon in the PR list.
  [x] **Description** Should be above the tabs
- [x] **Overview** Today description is shown. Fetch review
  comments and threads (`gh pr view <n> --comments`). Show author + timestamp + body,
  ideally threaded.
- [x] **Reviewers/approvals in the list.** The reviewer span already exists in `pr_list`
  but the data isn't populated.

---

## Tier 2 — Go from reading to acting (what makes a TUI actually useful)

Every platform lets you act directly; a read-only view forces you back to the browser.

- [ ] **Open in browser** (`gh pr view <n> --web`). Cheapest possible action, huge payoff.
- [ ] **Check out PR locally** (`gh pr checkout <n>`). GitHub/GitLab/Bitbucket all have
  "checkout" instructions; here it becomes one keypress.
- [ ] **Approve / Request changes / Comment** (`gh pr review`). The core of a review loop.
  Needs a small text-input mode (same pattern as the search view).
- [ ] **Merge / Squash / Rebase** (`gh pr merge`). Platforms offer the three strategies +
  "delete branch after". Only show the strategies the repo allows.
- [ ] **Draft ↔ Ready** (`gh pr ready`). GitHub draft, GitLab/Bitbucket "WIP/Draft".
- [ ] **Comment threads: resolve.** GitLab/Bitbucket build the review flow around
  *resolving threads*. "N unresolved threads" is a strong mergeability signal to show.

---

## Tier 3 — Filtering, sorting and navigation (scales as the list grows)

Every platform has rich filters — that's how you find *your* PR among a hundred.

- [ ] **Search/filter** — see `SEARCH_SCREEN.md`. Then extend to structured filters:
  `author:`, `label:`, `review:approved`, `is:draft`, `status:open|merged|declined`.
- [ ] **Sorting** — recently updated, created, comment count, CI status.
- [ ] **"Mine" quick views** — GitHub: *Created*, *Assigned*, *Review requested*,
  *Mentioned*. These are the most-used views in practice. Worth their own keys/tabs.
- [ ] **Labels/tags** — show colored labels in the list; filter on them.
- [ ] **Mergeability in the list** — conflict? green/red. Behind the base branch?

---

## Tier 4 — Multiple providers (where `ProviderKind` points)

The domain is already provider-agnostic — that's a deliberate opening.

- [ ] **GitLab MR support** via the `glab` CLI (mirrors `gh` well). Map MR → your
  `PullRequest`.
- [ ] **Bitbucket PR support** via their API/CLI.
- [ ] **Unified view across repos/providers** — one list mixing GitHub + GitLab, with a
  small provider icon per row. Requires the list fetch to fan out.
- [ ] **Provider-specific concepts normalized** — GitLab "approvals required", Bitbucket
  "default reviewers"/"merge checks", GitHub "required reviews/branch protection" → one
  shared "requirements to merge" model.

---

## Tier 5 — Fresh data & notifications

- [ ] **Auto-refresh / manual refresh** (`r`). Today it loads once. A background tick that
  refetches the list (the pattern already exists via `spawn_load_*` + actions).
- [ ] **Unread/updated** — mark PRs with new activity since you last looked. GitHub
  "Unread"/notifications inbox is one of the most-used surfaces.
- [ ] **Notifications inbox view** — `gh api notifications`. A "what needs my attention
  now" screen (review requested, mentioned, CI failed on my PR).

---

## Tier 6 — The diff/review experience (where a good TUI can shine)

- [ ] **Syntax-highlighted diff** (e.g. `syntect`).
- [ ] **File tree + side-by-side diff**, jump between files/hunks.
- [ ] **Inline comments** — show comments anchored to the right line in the diff; add new
  ones. This is the heart of code review on all three platforms.
- [ ] **Review drafts** — collect several comments and "submit review" at once (GitHub's
  model), instead of one comment at a time.
- [ ] **Bitbucket "tasks"** — checkable to-do items on a PR. Simple and well-liked.

---

## Cross-cutting polish (ongoing)

- [ ] **Help overlay** (`?`) listing all keys — standard in every TUI.
- [ ] **Status bar** with context (current provider, repo, match count, loading spinner).
- [ ] **Errors visible in the UI** — today `.expect()`/panic if `gh` is missing or the
  response fails to parse. Show an error field instead of crashing the terminal.
- [ ] **Config** — which repos/providers, default filters, key bindings, theme.
- [ ] **Empty/loading/error states** per view (you have `LoadState` — use it everywhere).

---

## Suggested sequence

1. [ ] **Files diff + Checks + real comments** — finish the detail view (Tier 1).
2. [ ] **Open in browser + checkout + approve/merge** — make the app actionable (Tier 2).
3. [ ] **Search → structured filters + "review requested" view** (Tier 3).
4. [ ] **Refresh + unread** (Tier 5) — low effort, makes it feel alive.
5. [ ] **GitLab/Bitbucket** once the GitHub flow is solid (Tier 4).
6. [ ] **Diff polish + inline comments** as the big lift (Tier 6).

Everything above fits naturally into the existing TEA structure: new data → a new
`LoadState` field + `spawn_load_*`, new interaction → a new `Action` + arm in `apply`,
new view → a new `Screen` + module (see `SEARCH_SCREEN.md` for the template).
