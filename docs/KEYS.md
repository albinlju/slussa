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
| `q` | quit; while an agent is reviewing a PR it asks first, since quitting stops the review and loses what it has found, and after a yes it waits with a spinner for the agent to stop before slussa closes |

The search also takes filters, written `key:value` among the words and combined with them: `author:name`, `review:approved` (also `changes`, `requested`, `none`) and `ci:failed` (also `pending`, `passing`), `merge:conflicts` for the PRs that cannot be merged for a conflict and `merge:clean` for those GitHub says have none (GitHub only; it says nothing of checks or reviews, and a PR whose conflict is not computed yet is in neither). A value can be shortened as long as it is the only one that begins so (`ci:f`), and a filter whose value is not complete yet leaves the list as it is. A word with any other key before its colon is searched for as text.

## A PR

| Key | What it does |
| --- | --- |
| `j` / `k` | scroll or move; in the Builds tab, move between the builds, and in a build's log scroll it; in the Overview, read on through a comment taller than the screen (a few rows at a time) before moving to the next one |
| `^d` / `^u` | half a page |
| `h` / `l` | previous / next tab, on every tab |
| `1`-`5` | select a tab |
| `[` / `]` | previous / next tab; while a commit is open, previous / next commit |
| `enter` | open or view; in the diff it moves from the file tree into the code; in the Builds tab it opens the log of the build the cursor is on (a GitHub Actions job; a check that is not one has no log, and the key does nothing) |
| `esc` | back; in the diff it moves from the code back to the file tree, and in a build's log back to the builds |
| `space` | toggle a fold (a folder in the file tree, a resolved thread); in the Overview, open or fold a long comment; in the diff's code pane, expand or collapse a resolved thread, or, on a long comment's fold row (`j`/`k` stop there), open or fold that comment |
| `/` | search in the diff files or the commits; `n` / `N` jump to the next / previous match; in a build's log, to the next / previous error line, round from the last to the first. The log opens on its first error with a few lines before it, and at its end when none is marked |
| `H` / `L` | pan a wide Description |
| `a` | submit a review verdict; tied to the commit you were shown (the head of the diff you opened, otherwise the list's), so an approval or a request for changes is refused if the branch has moved (withdrawing an approval is not), and the key is dimmed with `commit unknown` when none is known |
| `v` | start or finish a batched review; `V` discards it |
| `m` | merge, tied to the commit you were shown like `a`; the dialog lists what blocks it. On GitHub, while the PR waits on checks or reviews, `a` in the dialog makes `enter` merge it by itself when ready, and turns that off again. On GitHub, `d` in the dialog marks the PR's branch for deletion once it is merged (only a branch of the same repository, never the one merged into); if the branch cannot be deleted the merge still stands and a notice says so |
| `x` | close or decline the PR, or reopen a declined one |
| `w` | when the branch has moved since the diff you last had open (GitHub; an open PR), show only what is new, from that commit to the one now, in the Diff tab; the header says `↻ new since you read it` and the footer offers `w`. In it `w` or `esc` goes back to the whole diff, and no comment is made on it: its lines are not the PR's. Arriving at the PR's diff (opening the PR on it, or choosing the Diff tab) is what counts as read, so doing that first clears the mark; a refresh under you does not, whether it brings a newer diff or the same one again. Only the files the PR touches at the new commit, or touched at the one you read, are shown: one it has put back as the target has it is new too, and when GitHub cannot list all the files it touched before (it lists 300 at most) nothing is left out, so the files a merge of the target brought are then shown as well (the PR's diff of the new commit is read first, and a branch that moves again meanwhile says so), and what could not be read is asked for again by `w` or by `F` |
| `A` | ask the configured agent to review the PR (GitHub; an open PR; claude -p unless agent_review in the config says otherwise). A dialog names the command and what it is given (the title and description, the issues the PR closes, the repository's rules and the diff) and asks first. What it finds becomes proposals in the Diff tab, and nothing is posted. While a review runs the key stops it instead, after asking: what it has found so far is not kept, and leaving the PR does not stop it |
| `b` | in the Builds tab, run the failed builds again; offered when one has failed or was cancelled, and dimmed with the reason on a merged or declined PR (GitHub Actions) |
| `p` | ask those who asked for changes to review again (GitHub); named in the footer (`p: ask alice again`), hidden when nobody asked for changes, dimmed with the reason on a merged or declined PR |
| `i` | in the Overview, open the issue the PR closes in the browser (GitHub); the footer names it (`i: open #12`) and shows nothing when there is none. When the PR closes several, `i` asks which: `j`/`k` and `enter` in the picker, which names the repository of an issue in another one |
| `c` | comment; on an agent's proposal in the diff, take it: the editor opens with the proposal's words to edit and send as your own (a line comment joins the review in progress, as any other), and the proposal is not shown again |
| `r` | reply |
| `e` / `d` | edit / delete your own comment (`d` also removes a queued review comment, and on an agent's proposal in the diff discards it) |
| `R` | resolve or unresolve the thread |
| `f` | in the Overview, show all comments, only people's or only an AI agent's; offered when the PR has a comment by a bot account (GitHub) or one that starts with an `[ai] markers` line; what the filter hides stays as a dimmed line |
| `^j` / `^k` | step between comments in a thread |
| `u` / `U` | in the Overview, go to the next / previous review thread that is not resolved, round from the last to the first; the footer says how many there are (`u: unresolved (3)`) |
| `o` / `y` | open the PR in the browser / copy its link |
| `F` | refresh |
| `q` | quit; while an agent is reviewing a PR it asks first, since quitting stops the review and loses what it has found, and after a yes it waits with a spinner for the agent to stop before slussa closes |

`a`, `v`, `m`, `p` and `x` work from both the Description and the Overview tab. In the
diff, the arrow keys also move between the file tree and the code. A key that the
provider does not support is hidden; one that the PR's state blocks (for example
merging with conflicts) stays visible and says why.
