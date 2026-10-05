# Keys

Press `?` in any view for the keys that apply right now. The help hides what the
connected provider does not support, so it can show fewer keys than this page.

## The list

| Key | What it does |
| --- | --- |
| `j` / `k`, arrows | move |
| `enter` | open the PR |
| `/` | search title and author; `esc` clears the search; a number after a hash (as in #44) and `enter` opens that PR, also one the list does not hold |
| `f` | filter by status (open, draft, merged, declined, all) |
| `s` | pick the sort order: needs you first, newest, recently updated or oldest |
| `L` | load more PRs, while the heading says more are unread; in the merged and declined views, the next older batch |
| `^d` / `^u` | half a page |
| `o` / `y` | open the PR in the browser / copy its link |
| `F` | refresh |
| `q` | quit |

The search also takes filters, written `key:value` among the words and combined with them: `author:name`, `review:approved` (also `changes`, `requested`, `none`) and `ci:failed` (also `pending`, `passing`), `merge:conflicts` for the PRs that cannot be merged for a conflict and `merge:clean` for those GitHub says have none (GitHub only; it says nothing of checks or reviews, and a PR whose conflict is not computed yet is in neither). A value can be shortened as long as it is the only one that begins so (`ci:f`), and a filter whose value is not complete yet leaves the list as it is. A word with any other key before its colon is searched for as text.

## A PR

| Key | What it does |
| --- | --- |
| `j` / `k` | scroll or move; in the Overview, read on through a comment taller than the screen (a few rows at a time) before moving to the next one |
| `^d` / `^u` | half a page |
| `h` / `l` | previous / next tab, on every tab |
| `1`-`5` | select a tab |
| `[` / `]` | previous / next tab; while a commit is open, previous / next commit |
| `enter` | open or view; in the diff it moves from the file tree into the code |
| `esc` | back; in the diff it moves from the code back to the file tree |
| `space` | toggle a fold (a folder in the file tree, a resolved thread); in the Overview, open or fold a long comment; in the diff's code pane, expand or collapse a resolved thread, or, on a long comment's fold row (`j`/`k` stop there), open or fold that comment |
| `/` | search in the diff files or the commits; `n` / `N` jump to the next / previous match |
| `H` / `L` | pan a wide Description |
| `a` | submit a review verdict |
| `v` | start or finish a batched review; `V` discards it |
| `m` | merge; the dialog lists what blocks it. On GitHub, while the PR waits on checks or reviews, `a` in the dialog makes `enter` merge it by itself when ready, and turns that off again. On GitHub, `d` in the dialog marks the PR's branch for deletion once it is merged (only a branch of the same repository, never the one merged into); if the branch cannot be deleted the merge still stands and a notice says so |
| `x` | close or decline the PR, or reopen a declined one |
| `b` | in the Builds tab, run the failed builds again; offered when one has failed or was cancelled, and dimmed with the reason on a merged or declined PR (GitHub Actions) |
| `p` | ask those who asked for changes to review again (GitHub); named in the footer (`p: ask alice again`), hidden when nobody asked for changes, dimmed with the reason on a merged or declined PR |
| `i` | in the Overview, open the issue the PR closes in the browser (GitHub); the footer names it (`i: open #12`, with the number of others after it when it closes more; the first is opened) and shows nothing when there is none |
| `c` | comment |
| `r` | reply |
| `e` / `d` | edit / delete your own comment (`d` also removes a queued review comment) |
| `R` | resolve or unresolve the thread |
| `f` | in the Overview, show all comments, only people's or only an AI agent's; offered when the PR has a comment by a bot account (GitHub) or one that starts with an `[ai] markers` line; what the filter hides stays as a dimmed line |
| `^j` / `^k` | step between comments in a thread |
| `u` / `U` | in the Overview, go to the next / previous review thread that is not resolved, round from the last to the first; the footer says how many there are (`u: unresolved (3)`) |
| `o` / `y` | open the PR in the browser / copy its link |
| `F` | refresh |
| `q` | quit |

`a`, `v`, `m`, `p` and `x` work from both the Description and the Overview tab. In the
diff, the arrow keys also move between the file tree and the code. A key that the
provider does not support is hidden; one that the PR's state blocks (for example
merging with conflicts) stays visible and says why.
