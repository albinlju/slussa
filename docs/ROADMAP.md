# Roadmap for slussa

The product idea, what is planned in priority order, and how the code should get
better. What exists today is described in the [README](../README.md) and [KEYS.md](KEYS.md); how it is
built is in [ARCHITECTURE.md](ARCHITECTURE.md). History is in git.

## Where it stands today

A read-write PR client for GitHub (through `gh`) and Bitbucket Data Center (REST
and a personal access token). The list opens sorted by what needs you, and the PR
has its description, conversation, diff, commits and builds, with comment, review,
merge (also when ready, and deleting the branch), decline, reopen, asking again
for review and opening the issue it closes. What is left of the AI-specific
parts is below.

## Positioning: where it is going

slussa is meant to be the **human approval surface for AI-generated pull
requests**: the place where a reviewer decides, not the place where the code gets
read line by line.

**What it is chosen for: a decision that can be trusted.** A list sorted by what
needs the reader, every repository in it, a merge key and a build log are what any
pull request client has to have, and slussa has to have them too; none of them is
a reason to choose it. The reason is that what the reader approves is what they
read: an approval and a merge are bound to the commit that was shown, a branch
that has moved says what is new since it was read, what an AI wrote is marked as
AI, and an agent may put its findings beside the code but the reader sends them.
Nothing is judged for the reader and no model runs inside slussa. When agents
write the PRs and agents review them, the sign-off is the part that is left to a
person, and it has to mean that the person saw what was merged.

The assumption behind the backlog: agents write more of the code and open more of
the PRs, and AI reviewers do the line-level reading. What stays human is triage
("what needs me?"), intent-checking ("did the agent do what was asked, and what did
the AI review flag?"), and the sign-off itself (approve, merge, decline). Those are
overview-and-decision tasks, and a keyboard-driven terminal UI is better at them
than a web page. slussa should be the missing piece in a terminal workflow next to
an editor, a git client and a coding agent — never a competitor to the provider's
web UI.

**Simple to enter, deep to use.** The way in stays as small as it is: a list,
and a PR opened from it. That is the part to protect. It does not mean the
product stays small. What makes someone use a tool every day is that they never
have to leave it, and that comes from depth kept out of sight, as in the products
that look simple and are not. slussa has two views — the list and the PR — and
stays that way. Depth shows up as a better default, a column, a marker, a key that
appears when it matters, and a way to find the rest without a screen full of it
(a command palette), never as a new screen, a dashboard or a sidebar of widgets.
A feature earns its place by one test: *without it, does someone leave for the
web?* If so it belongs; if not, leave it out. It must be explainable in one
sentence, and reached with one key or found by name in the palette.

What this rules in (each item says how far it is):

- **A decision bound to what was read** *(built)*. An approval and a merge name
  the commit the reader was shown, and a push after that is refused instead of
  merged unseen.
- **Review in rounds** *(not built)*. An agent's PR is read several times. When
  the reader comes back, the PR says what is new since the head they read: the
  commits, the diff from then to now, the builds, and what became of their own
  threads.
- **Agents propose, the reader sends** *(not built)*. An agent's findings arrive
  as private proposals beside the lines they are about, marked as AI; the reader
  sends, edits or discards each. Nothing an agent wrote is posted without a
  person choosing it.
- **The list is the inbox** *(first version built)*. The PR list opens sorted by what needs the user —
  review requests, failed CI, new activity since last look — with a short reason
  on the row. No separate inbox screen; the plain list is the same view with the
  attention sort turned off.
- **The whole day in one list** *(not built)*. The PRs of every repository the
  reader works in, with the same reasons, so that the tool is open all day and
  not only inside one repository.
- **Never leave for the web** *(partly built)*. The small things people open a
  browser for: why a build failed, a checkout, marking a PR ready, a label or a
  reviewer. Each one that sends someone away makes them doubt the tool.
- **Reading it well** *(partly built)*. A diff that is hard to read sends people
  to the web too.
- **AI review is first-class** *(first version built)*. What an AI reviewer
  (a service, an agent, a team bot) did is marked as AI: `[AI]` on its comments and
  commits, a filter for them, and an `AI review` column in the list. Not built:
  the summary in the PR header, whether it commented or committed fixes: that a
  review happened, and against which commit. Running a review from slussa comes
  after handoff, since an automated review belongs before the human opens the PR.
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

- **Feature parity with the web UI for its own sake.** A feature is in when
  without it someone leaves for the web. Authoring and administration nobody
  opens a PR to do (settings, permissions, projects) are not the job (see
  *Scope decision* below).
- **Being a chat.** slussa is deterministic and immediate. It shows state and takes
  actions; it does not host a conversation with a model. The agent does the
  analysis, slussa is where the decision gets made.
- **Being a dashboard.** No third view, no configurable panes, no widget grid.
  The inbox is one list, across repositories; what a reader sets is how it sorts
  and filters, not which boxes it has. Defaults come first, so that nothing has to
  be configured to start.
- **Judging the PR for the reader.** No rules that flag a change by pattern and no
  model inside slussa. A line that says nothing is wrong reads as "safe", most of
  what such rules catch is already in the file list, and slussa is for the
  decision the reader makes.
- **Agents deciding.** An agent may read and propose through the CLI; approving
  and merging stay human.

Priority order for anything new: the decision that can be trusted (group 1: since
you read it, then agents propose, then a few readers who try it) → the whole day
in one list and not leaving for the web (groups 2 and 4) → reading it well
(group 6) → habits (a count for a prompt, the next PR) → depth that stays out of
sight (group 7) → AI-review integration → agent handoff → getting it into more
hands (the release gaps), which waits until the product is one someone will want →
everything else.

Why group 1 comes before every repository: the list across repositories is needed
for daily use, but it is the largest engineering cost here and it is what any
client has. *Since you read it* is small and builds on what exists. *Agents
propose* is not small, and it is the part that is slussa's own.

All upcoming features follow the [product and interaction principles](ARCHITECTURE.md#product-and-interaction-principles):
a calm default view, discoverable contextual actions, focused dialogs, consistent
keyboard behavior and optional features based on provider capabilities. Feature
scope includes how users find and leave the interaction, not only the API action.

---

## Features to build

Grouped by the priority order in *Positioning*. Within a group, items marked
*refined* have a settled design; the rest still need one.

### 1. A decision that can be trusted

What slussa is chosen for (see *Positioning*). The approval and the merge bound to
the commit that was read are built. These make that hold over several rounds, and
let an agent help without deciding.

- [ ] **Since you read it** (review in rounds). A marker in the header when the
  branch has moved since the diff that was read; then a key that shows only what
  is new, from the head that was read to now (a head that was force-pushed away
  may not answer); the builds shown for the head that was read, with a line when
  they are for another; and the reader's own threads, which of them were answered
  or resolved since. A merge or a verdict on a moved branch is already refused,
  and says to read what is new; this is how. Inside one session the list's head
  and the diff's are both in memory. The next day, which is when it matters, they
  are not: `local/seen.rs` keeps when a PR was looked at and how recently it had
  been updated then, not which commit, so the head that was read has to be saved
  with it. **Open:** what counts as read (the diff opened, or read to its end),
  and whether the list's row says it too.
- [ ] **Agents propose, the reader sends** *(first version built)*.
  `slussa propose import <PR>` reads one JSON document (the README gives it: the
  head the agent read, an optional summary, and line comments, each with a side and
  perhaps the agent's own finding id) and keeps it in a file of its own under
  `slussa/proposals`, one per scope. The import takes the file's lock for the moment
  it needs; the TUI reads the file without it, which is safe because a write replaces
  the whole file, when a PR is opened and on each refresh. In the Diff tab a proposal
  written against the commit the diff is of stands on its line, marked `[AI]`; `c`
  takes it into the comment editor with its words (so editing and sending is the
  ordinary path, into the review in progress when there is one) and `d` discards it.
  What the reader did is kept in the seen file (`local/seen.rs`, an optional list, so
  the version 1 file reads and writes as before), which also means it is forgotten
  with the PR after 90 days. A proposal for another commit is counted in the footer
  and not drawn. What is waiting is said in the footer on every tab, and the Overview's
  sidebar shows the latest summary (marked when it is of an older commit) with the count
  left. At most 1 000 per PR, 500 per document and 2 MiB. **Missing:** the summary is
  only the latest, cannot be dismissed and is not shown where the sidebar is hidden (a
  narrow terminal); a way to see the
  proposals for another commit (they are only counted); a mark in the PR list; proposals on the commit diffs and on *Since you read it*; `c` marks a
  proposal taken when the editor opens, not when the comment is sent, so a draft
  that is later thrown away loses the proposal (the draft is kept as any other, and
  the agent's file still has it, but nothing brings it back); undoing a discard; the
  proposals appearing without a refresh; and Bitbucket Data Center. **Not tried
  against** an agent that is not a script, or in a real terminal: the schema has
  only been written by hand and the Diff tab only through the test backend.
- [ ] **`slussa context <number>`** — one compact text package for an LLM (title,
  description, unresolved threads, CI, blockers). It is the package *Send to
  agent* needs too, so build it once; it needs a size rule for long threads and
  diffs.
- [ ] **`slussa agent-instructions`** — prints how an agent should use slussa, as a
  short snippet for AGENTS.md or a skill. An agent that is not told slussa is
  there never calls it, so this ships with the proposals and not after them. A
  CLI plus this is simpler than an MCP server for a local tool built on `gh` and
  `git`; revisit MCP later (it is also the trigger for a `slussa-core`).
- [ ] **Tried by a few readers.** When the two first items exist: a recording
  that shows them, and a handful of people who review agents' PRs asked to use
  slussa for a week. What they open the web for goes to group 4. This is how the
  direction is checked, before more is built on it. It is not the release gaps
  (a package, signing), which still wait.

### 2. The list: the whole day, and what needs you

Still one list, still one PR view. The list knows what needs the user, across
the repositories they work in, and says so on the row.

- [ ] **Every repository, without configuration.** Started outside a repository
  (or given a list of them in `config.toml`), the list is every PR that needs the
  reader, with one more column for the repository. GitHub only. This is the
  largest engineering cost in this roadmap: the session is fixed to one
  repository (`Provider::GitHub(GhRepo)`), a PR is known by its number alone, and
  the drafts and the read marks are filed per repository, so a PR needs a
  reference that names its repository (`PrRef`) wherever a number is used today.
  The search for "the PRs that need the viewer" was once declined because the
  search index lags; that has to be weighed again against reading each
  repository's list. Until then `slussa list --json` and `slussa count` can
  answer across repositories with none of it.
- [ ] **Next PR.** After an approval or a merge, one key goes to the next PR
  that needs the reader, and the footer says where in the queue it is (`3 of 12`).
- [ ] **Remember the reader's choices.** The sort, the filter and the status
  survive a restart, so the list opens the way it was left.
- [ ] **A list that is there at once.** The first page before the rest, then
  updated in place without the screen jumping; see *The first load is slow*
  under *Engineering*.

- [ ] **Attention reasons, the rest.** The list is sorted by what needs you,
  with a reason column (the README says how). Still open: a reason for
  mentions (they need the text of the comments), team review requests (GitHub
  counts people only) and opening the PR on the tab its reason points at. A reason for new comments was tried
  and dropped: the `●` before the number already says that something changed, and
  the reason said it a second time, at the cost of a column.
- [ ] **More search filters.** The search takes `author:`, `review:`, `ci:` and
  `merge:` (`conflicts`, `clean`; the README says how); the status is the `f` picker. Missing: `label:` (the list
  query leaves the labels out, which made a page twice as slow) and `is:agent` (see
  *AI authorship* below).
- [ ] **More sorts.** `s` opens a picker with needs you first, newest, recently updated and oldest. Comment count and CI status are not sorts: the Comments column and the `ci:` filter cover them. Oldest is the oldest of the PRs read, so in a view that says `recent` it is not the oldest there is.
- [ ] **Behind base in the PR list.** A conflict is shown in the `Status`
  column for any open PR (GitHub only; the list query reads `mergeable` for
  nothing). Whether
  the PR is behind its base needs `mergeStateStatus`, which doubled the time of
  the query (measured on cli/cli) and says `BLOCKED` for most PRs where branch
  protection is on, so it is left out.
- [ ] **Labels in the list** — colored and filterable (shown in Overview today).
- [ ] **Compact diff stats** (files / +/−) on list rows.
- [ ] **Search older PRs and show the total.** Merged and declined PRs are read a
  batch at a time and `L` reads older ones. Missing: searching older PRs at the
  provider, and showing how many exist in total.
- [ ] **Status bar** — provider, repo, match count, loading spinner. Only if it
  fits in the existing footer line; a second persistent bar is not wanted. The
  repository is the part that matters most now: slussa acts on the one `gh`
  places the directory in (see *Which repository*, below), and in a fork that
  can be the upstream or the fork, with nothing on screen saying which. **Open:**
  where it goes: the list's heading is the candidate, since it is there already.

### 3. AI review integration

The AI reviewer's output has to be as easy to read and act on as a human's, and
easier to tell apart.

An AI reviewer shows up in one of two ways, and slussa will meet both. One
*comments*: one batched review per head SHA, a first-line marker on every
comment, stable finding IDs carried across rounds, and a severity split where
only evidenced findings block. The other *commits* (see *From "Fixing the PR
Bottleneck"*): it fixes what it finds on the branch and comments only when it
is unsure, so its review is a set of commits and may leave no thread at all.
"AI actor" is therefore one notion, applied to the PR's author, to comments
and to commits.

- [ ] **Say more in the list's AI column.** The list has an `AI review`
  column on GitHub: whether a bot account has reviewed, and whether that was the head
  (`◆`, `◈`, `◇`, `✗`). It deliberately says nothing of what was found or
  handled: "handled" is no field (a fix without a reply or a resolve leaves no
  trace, and resolved is not fixed), so a count of open threads overstates, and
  one of answered threads claims too much. Missing: severity (`1 blocker`, where
  a reviewer exposes it with the evidence a blocker needs; see *What a
  commenting reviewer's review carries*), agents under a person's account (the
  list would have to read the first line of each thread's first comment),
  Bitbucket Data Center (one activity request per PR), a reviewer that is
  working now (GitHub has no such state; one bot shows it as a running check),
  and a reason in *Needs you* and the sort, so a PR with a stale or negative AI
  review rises.
- [ ] **Linked issues / cross-references** — the issues a PR closes are in the
  Overview's side panel under *Closes* (GitHub; read with the description and
  labels, so the list query is unchanged). `i` in the Overview opens it in the
  browser, or asks which when there are several. Missing: issues it only
  mentions, and Bitbucket Data Center, which has no such field.
  The linked issue is what was asked for, and checking the PR against it is the intent
  check the positioning promises.
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
- [ ] **Resolved / unresolved filter** in the Overview. (`u` / `U` already go between the unresolved threads there; a filter would also hide the rest.)
- [ ] **Outdated comments** — hide threads whose anchored line is gone from the
  diff; keep them in the Overview timeline.

What a commenting reviewer's review carries, to design the domain model
against:

- [ ] **One disposition per head SHA.** A review is one batched GitHub review
  (summary + inline comments) that names the head SHA it reviewed. Show
  *reviewed SHA vs. current head* on the AI summary line; a review of an older
  SHA is stale, not wrong.
- [ ] **Stable finding IDs across rounds.** Findings carry an ID such as
  `GATOR-<sha8>-<nn>` and are carried, resolved or waived across later
  commits; a maintainer's "won't fix" reply is a waiver, an author's "fixed"
  is a claim to verify. The open/fixed/waived state per finding is what a
  reviewer wants at a glance, and it is derivable from thread resolution +
  resolver identity + the marker.
- [ ] **Severity and evidence.** Findings are `Critical | Warning | Suggestion`,
  and only ones with a full evidence record (base behaviour, head behaviour,
  observable impact, reproducer, changed location) count as blockers; the rest
  are hypotheses. Suggestions never block. If slussa's own *run a review*
  command emits structured output, use this split: it gives the reviewer a
  defensible "N blockers, M suggestions" header instead of a wall of comments.
- [ ] **Concern format for the review prompt.** Every concern as "Before this
  PR, `<persona>` experienced `<old>`. With this PR, `<new>`, so `<impact>`."
  with file:line only as evidence. A good default prompt for slussa's
  run-a-review.
- [ ] **Convergence rules worth copying into the display.** After three
  finding-bearing rounds a reviewer may report critical findings only, and a
  rebase with the same patch (same patch-id) is not re-reviewed. Show the round
  count and "unchanged since last review" so a human knows when the AI has
  stopped adding value.

### 4. Act without leaving: the build, the branch and the small edits

Whatever sends the reader to the web for a minute belongs here. In the order the
test above ranks them:

- [ ] **Build logs** *(first version built)*. `enter` on a build in the Builds tab
  opens the log of its GitHub Actions job in the tab, on its first error (at its
  end when none is marked), and `n`/`N` step between the errors. The whole log is
  read when a build is opened, once, and the last 20 000 lines are kept. Missing:
  a log that is still being written (GitHub gives it when the job is done), a
  check that is not an Action (an external status has no log to read), the failing
  step alone, a re-read of a log, and Bitbucket Data Center. **Tried against a real
  log** (a failed job of a public repository, read with `gh` 2.98); not tried with
  an older `gh`, which has no `--allow-escape-sequences` and gets the call again
  without it (`FakeGh` covers that).
- [ ] **Check out the PR locally** and **open the focused file or line in
  `$EDITOR`** (moved from *Handoff*): preconditions for most handoffs, and
  useful alone.
- [ ] **Ready for review / back to draft.** Flipping an agent's draft is part of
  triage (moved from *Scope decision*).
- [ ] **Labels and assignees.** Add and remove, next to the reviewers `p` already
  asks again (moved from *Scope decision* and *Diff ergonomics*).
- [ ] **Repositories with a merge queue.** slussa merges through the REST endpoint
  that GitHub's own documentation says does not support merging with a merge
  queue, and recommends replacing with the asynchronous one (`merge-async`, whose
  `merge_action` can be `merge_queue`). What the old endpoint does where the queue
  is required, that page does not say: a refusal ("Changes must be made through
  the merge queue") is reported, and so is a direct merge that steps past the
  queue (a community report, not checked here). Both are wrong for slussa: a
  refusal leaves the reader with an error where the queue is the way, and a
  direct merge passes a gate the repository asked for. Done when the merge
  dialog knows the repository uses a queue and says so ("add to the merge queue"
  in place of "merge"), goes through the asynchronous endpoint, and keeps what the
  commit-bound merge gives today: check that the new endpoint takes the head
  (`sha`), otherwise compare it first. **Not verified:** what either endpoint does
  against a real repository with a queue, and whether it is the repository or the
  branch that decides; there is nothing like it on Bitbucket Data Center.
- [ ] **Approve and merge several at once** *(open question)*. For PRs by a bot the
  reader trusts and does not read, such as dependency updates: one decision for
  many, each PR keeping the head it was shown. **Open:** whether it belongs. It is
  a decision on code nobody read, which the rest of slussa is built against; a
  narrow form (only authors the reader has named) may be acceptable.
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
- [ ] **Re-run CI checks** — `b` in Builds runs the failed or cancelled GitHub Actions jobs again. Missing: one build at a time, all builds, and checks that are not Actions (external statuses).
- [ ] **Request / re-request reviewers** — `p` asks those who asked for
  changes to review again (GitHub; the footer names them, nothing is shown
  when nobody asked for changes). **Not tried against a real review:** it
  needs a changes-requested review by a second account, so only `FakeGh`
  covers the request and the reading again; try it on a PR where someone else
  asked for changes. Missing: asking someone who has not been
  asked, re-requesting an approval that a push made stale, team reviewers,
  and Bitbucket Data Center.
- [ ] **Delete the source branch after merge** — `d` in the merge dialog
  marks it (GitHub; only a branch of the same repository, never the target;
  not offered with *merge when ready*, where the repository's own setting
  decides). A branch that cannot be deleted leaves the merge done and says so.
  Missing: Bitbucket Data Center, a remembered choice, and a delete that is
  bound to the merged head. Today the delete goes by name, as GitHub's own
  button and `gh pr merge --delete-branch` do, so a push between the merge
  and the delete is lost with the branch (it can be recreated from its SHA).
  Closing that takes the head SHA on the PR, `sha` on the merge and a
  conditional delete (GraphQL `updateRefs` with `beforeOid`, not yet tried).
- [ ] **React to a comment** — add / remove your own emoji reaction.

### 5. Handoff to the coding agent

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
- [ ] **Run a review from slussa** *(first version built)*. `A` runs the
  configured command (`agent_review` in `config.toml`, a program and arguments;
  `claude -p` by default, `[]` for none) over the PR: the title, description and
  diff go to it on standard input with a built-in prompt that asks for the document
  of *Agents propose*, and what it answers becomes proposals the reader takes or
  discards, so nothing is posted. A dialog names the command and what it is given
  and asks first, each time. slussa fixes `head` to the commit whose diff the
  agent was given and drops comments on lines that are not in it. It runs off the
  UI thread (`spawn_fetch`), one at a time per PR, with a 10 minute deadline, and
  a failure is the PR's error. **Missing:** the prompt does not carry severity or
  the "Before this PR, `<who>` experienced…" format (the document has no field for
  either); the diff is cut at 150 000 bytes and the agent is told; the command
  cannot be given the files of the PR, which is *Check out the PR locally*; a
  second command for another kind of review; cancelling a review that runs (it
  ends at the deadline); the prompt cannot be changed in the config; a command
  that is not installed is found out when it runs, not before. **Not tried
  against** a real agent: the command has only been a script.
- [ ] **Copy additional references** — SHA / branch / permalink to a line.

#### The other direction: an agent calling slussa

Handoff above sends context *to* an agent. The reverse is an agent that triages
or checks a PR by calling slussa itself. `gh` already gives an agent raw data;
slussa adds what it computes: why a PR needs the human (the attention reason) and
why a merge is blocked, the same on both providers. The rule that keeps this in
step with the positioning: **agents may read and propose; only the human
decides.** No new view: these are non-interactive subcommands that print and exit.

- [ ] **`slussa <PR URL>`.** `slussa 44` starts the TUI on that PR of the
  repository you are in. Missing: a URL. It could name another repository, and
  the check that it is this one is provider specific (`owner/repo` on GitHub,
  `projects/…/repos/…` on Bitbucket), so it is left out.
- [ ] **`slussa list --json`** — open PRs in the TUI's order with the "Needs you"
  reason (reuses `domain::attention`). Output has `"schema": 1`, snake_case
  identifiers (`ci_failed`, not "CI failed") and flat usernames; errors go to
  stderr as JSON, never mixed into stdout. The JSON types are separate from the
  domain types, so internal changes do not change the output. A headless command
  never asks for input: it connects with `session::connect` and fails when the
  account is not logged in, instead of going through `cli::connect`, which starts
  the interactive `gh auth login`.
- [ ] **`slussa count`** — one number for a shell prompt, tmux or a status line:
  how many PRs need the reader (the attention reasons say which). It prints and
  exits, with no UI, and it is what makes the tool present without being open.
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

`slussa context`, the proposals an agent hands in and `slussa agent-instructions`
are in group 1, and are built first; what is said above about the output, the
errors and a command that never asks for input holds for them too.

Left out on purpose: `wait --ci` (`gh pr checks --watch` does it) and approve or
merge for agents. The JSON is a public contract, so keep it marked experimental
(`"schema": 1`) until it is used. GitHub first: Bitbucket is in maintenance, and
these commands go through the same provider code but are not planned to be tested
against it.

### 6. Reading it well

Using it every day means reading code in it, and a diff that is hard to read
sends people to the web. These rank with the rest, not below them: the reader may
read less of a diff than before, but what they do read has to be good. What is
new since the diff was read is *Since you read it*, in group 1.

- [ ] **Word-level (intra-line) diff.**
- [ ] **Expand context** — unfold above/below a hunk (needs a full-file fetch).
- [ ] **Side-by-side (split) diff** as an alternative to unified.
- [ ] **Rename / move display** — "renamed from X" instead of delete + add.
- [ ] **Whitespace toggle.**
- [ ] **Binary / image files** — a clear "(binary file)" instead of a broken diff.
- [ ] **Viewed-files tracking** — local "mark file reviewed", saved per PR.
- [ ] **Branch ahead / behind base** info.
- [ ] **Milestones and projects** (reviewers and labels already shown; assignees are in group 4).

### 7. Depth that stays out of sight

How a product with many features stays calm: the features are there, and the
surface does not show them until they matter.

- [ ] **A command palette.** One overlay (a dialog, not a view) that lists every
  action by name, filtered as the reader types; each action keeps its key, and the
  palette shows it. It is the help made searchable, and what lets the number of
  actions grow without the footer growing.
- [ ] **Config** — default filters and status, keybindings, agent commands (moved
  from *Providers and platform*). Defaults first: nothing has to be set to start.
- [ ] **Saved searches.** A search that is used every day gets a name and is one
  key away.

### 8. Providers and platform

- [ ] **GitHub Enterprise Server.** Only `github.com` is recognised as GitHub
  today; any other host is probed as a Bitbucket Data Center and refused if it is
  not one. `gh` works with a host (`gh auth login -h`), so most of this is
  recognising it in preflight, and carrying the host in the repository the calls
  are told to act on and in the drafts' name. It decides whether a company that
  runs its own GitHub can try slussa at all.
- [ ] **GitLab MR support** via `glab` (mirrors `gh` well). Reach more than anything else on this list: a team on GitLab cannot use slussa at all. After groups 1, 2, 4 and 6, since a second provider is also the trigger for the provider trait.
- [ ] **Windows support** — the browser and clipboard code has Windows paths,
  but CI builds only macOS and Linux and nothing has run them. Either add a
  Windows CI job and release target, or keep it stated as unsupported.
- [ ] **Bitbucket Cloud client.** (Not a successor to Data Center in this tool
  unless someone asks for it.)
- [ ] **Unified cross-provider list** with a provider icon per row.
- [ ] **Normalized "requirements to merge"** — GitLab approvals, Bitbucket
  default reviewers / merge checks, GitHub branch protection → one shared model.
- [ ] **Which repository, said.** slussa fixes the repository it acts on once
  (the one `gh` places the directory in, else `origin`'s) and tells every `gh`
  call, so nothing can go to two repositories. Missing, none of it a risk: the
  repository is not shown (see *Status bar*); a directory that `gh` could not
  place falls back to `origin` without saying so, where a line such as "acting
  on me/slussa; `gh repo set-default` chooses another" would; and it has not
  been tried in a real fork (what `gh repo view` answers there is read from its
  documentation, not run). **Open:** the wording, and whether the line is a
  notice at start or part of the heading.
- [ ] **Empty / loading / error states** per view (use `LoadState` everywhere).
- [ ] **Publish to crates.io from the release workflow.** Today `cargo publish` is
  run by hand from the tag, which needs a token on the publisher's machine and
  publishes whatever the working directory holds (a stray untracked file is
  packaged, as the dirty-tree check shows). The way to do it without a token is
  *Trusted Publishing*: crates.io is told which repository, workflow file and
  (optionally) environment may publish the crate; the job swaps its OIDC identity
  for a token that is revoked when it ends, so no secret is stored. A job after
  the release is created, in a protected GitHub environment that needs the
  maintainer's approval, keeps the one step that cannot be taken back under a human
  decision; `cargo publish --dry-run` also in the workflow's dry run, so that the
  packaging is tried without publishing. Setting it up on crates.io (the crate's
  settings) is done by the maintainer. Not before the attestation step has run on
  a real release, so that two new things do not meet in one.
- [ ] **Release gaps.** Deliberately late: getting slussa into more hands comes after
  it is a product people will want, not before. What is not there yet: a package
  for a package manager (an own Homebrew tap that the workflow updates, or waiting
  until the project is known enough for the main Homebrew repository, whose own
  automation then follows the releases); signing and notarizing the macOS binary
  (the README gives the `xattr` command); a check of "Review requested", which has
  only been seen against scripted `gh` output and needs a second account; and, when
  the time comes, a short demo recording for the README.

### Scope decision: authoring / management

slussa is review-and-act focused. Authoring and PR *administration* belong in the
editor, the coding agent or the web UI, and are out of scope unless a
decision-path feature needs them, or readers open a browser for them every day
(then they are in group 4):

- [ ] **Edit PR title / description** (the description is shown, not editable).
- [ ] **Create a PR** — the agent or `gh pr create` does this.

---

## Engineering

Rule of thumb for everything below: **do the refactors when a feature touches
the code anyway, do the tooling now.** Every open item has a trigger; do not do
it ahead of the feature that needs it.

### Code, with the next feature that touches the area

- [ ] **Keybinding table: the keys the children route.** The keys the PR
  screen routes itself are rows in `pr_detail/bindings/`: key, where, what the
  help says and what it does now (hidden, blocked with a reason, or offered),
  read by routing, the help and the footer's hints alike. What is left is the
  keys a child component routes (`j`/`k`, `enter`, `/`, `n`/`N` ...), which the
  help names on its own and `docs/KEYS.md` checks by name, not by where they
  work; as rows without an action they would let that test compare places too.
  `clippy::wildcard_enum_match_arm` still cannot cover `tui`: about 27 matches
  on crossterm's `KeyCode` end in a catch-all, and the lint does not tell a
  foreign enum from ours. *Trigger:* the next feature that adds a key to a
  child component.
- [ ] **Where the next features put their code.** The tree already has the
  places; this says which. `handoff.rs` at the top level holds the text package
  for an agent, built once for the TUI's *Send to agent* and `slussa context`;
  the configured command belongs to `config` and running it to
  `tui/app/terminal.rs`. A headless subcommand is a file in `cli/` (`list`,
  `blocked`, `threads`, `context`, and later `draft`, `review import`,
  `agent-instructions`), with `json.rs` for the output types (`"schema": 1`)
  and `exit.rs` for the exit codes; `cli` never imports `tui`. New saved state is
  a file in `local/` (`seen.rs`, which exists for *Unread* and where *Viewed files* would go, `proposals.rs`
  for the agent inbox) and imports `session`, never the reverse. Thread assembly
  and the findings rules go to `domain/` (`threads`, `findings`). `config.rs` becomes `config/` when it gains the agent commands.
  Each new module gets `wildcard_enum_match_arm`, as `session` and `local` have,
  and a row in the import rule of `tests/repo_rules.rs`. *Trigger:* the feature
  that needs each.
- [ ] **Provider trait instead of enum dispatch.** `providers/mod.rs` matches
  on `Provider` in every method; fine for two providers. Move to a
  `trait ProviderApi` (or keep the enum and implement it via the trait).
  Capabilities stay data. *Trigger:* a third provider.
- [ ] **Render-context structs for diff/thread rendering.** (`diff_viewer::render` now takes the `DiffContext`; the rest remain.) Five
  `too_many_arguments` allowances: `diff_viewer/pane.rs` (two),
  `widgets/comment/render.rs`, `widgets/diff_row.rs` and
  `tabs/overview/blocks.rs`. Bundle the per-render inputs (theme, focus, width,
  anchors, queued comments, current user) into one borrowed struct. *Trigger:*
  the next change that adds an argument to one of them; the AI marker did not
  need to.
- [ ] **Share thread assembly between providers.** `github/activities.rs` and
  `bitbucket_dc/activities.rs` both turn a flat event list into threads,
  replies and reactions. Move the assembly into `domain/` and have each
  provider produce flat `domain` events. *Trigger:* outdated-comment detection
  or review grouping, which need the same logic.
- [ ] **Suspend / resume for child processes.** Agent handoff and *open in
  `$EDITOR`* both need to hand the terminal to a child and take it back. The
  sequence: cancel
  background refreshes, pause the input reader, leave the alternate screen and
  raw mode, run the child with inherited stdio on `spawn_blocking`, restore raw
  mode and the alternate screen, clear and redraw, drain stale events, resume
  the reader and restart refreshes. slussa uses crossterm's `EventStream`,
  which has no pause: drop and recreate it around the child, or gate it with an
  `AtomicBool`. Put it in `tui/app/terminal.rs` as `run_in_terminal(cmd)`.
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
  `src/providers/conformance_tests.rs` (the crate has no `lib.rs` and
  `test_support` is `#[cfg(test)]`, so an integration test under `tests/` can
  reach neither the providers nor `FakeGh`) over a fake GitHub and a fake Bitbucket,
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
  lines, `mod.rs` composes; `tests/repo_rules.rs` fails a file over 600). Four
  `mod.rs` files still implement instead of composing: `tui/ui/`,
  `providers/`, `providers/github/` and `providers/bitbucket_dc/`; the two
  provider ones go with *Provider trait*. Not a refactor-only change. Three
  files are at about 500 (`tui/app/store.rs` with `Operation`, 523 since the build
  logs, and `PrResource`, `FetchKey` and the tickets the part to move out,
  `tui/ui/screens/pr_list/tests.rs`, `tui/ui/regression_tests/conversation.rs`):
  split by concern when one of them grows again.
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
  `pr_detail/keys.rs`. *Trigger:* *Render-context structs*, which splits
  them anyway.
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

- [ ] **Snapshots with realistic content.** `tui/ui/testdata/screens.txt` covers
  100x30 and 40x12 for the list and all five detail tabs, but with placeholder
  data: one PR, no diff, no threads. Add a snapshot with a diff with an inline
  thread, several commits and a failing build at both sizes, and one at 80x24.
  Consider `insta` then: one file per screen and a review step for changes,
  instead of one text file compared whole.
- [ ] **`cargo nextest`** in CI (parallel, per-test timeouts, clearer failure
  output). Local `cargo test` stays fine.
- [ ] **reqwest 0.13.** The crate is on 0.12; 0.13 is out. Not looked into.

### From "Fixing the PR Bottleneck" (Matt Pocock), for the AI features

A talk at AI Engineer Paris 2026 (<https://www.youtube.com/watch?v=LlgiOCmFG_w>)
on the same problem slussa is positioned against: agents open PRs faster than
humans can review them. Read from the automatic captions on 2026-10-01; the
slides were not seen. It is a talk about the speaker's own skills, from
experience and without data, so what is borrowed is the model, and each item
below says what it rests on.

- **Three layers before the decision.** Automated checks, automated review,
  human review. A green CI does not mean the code is ready; the two upper
  layers are there to catch checks that lie (a test that restates the
  implementation, one that reads the source as text, one that cannot fail).
  slussa is the surface for the third layer and shows what the first two did.
- **One-way and two-way doors.** Not every review matters equally: a merge
  that can be reverted needs little, one that cannot (a migration, data loss,
  something sent to users) is reviewed closely, together with how far a
  mistake reaches. His PRs carry this as a "merge danger" line at the bottom
  of the description. Not pursued: it asks for a convention that has not settled,
  and a marker that says nothing is wrong reads as safe (see *Judging the PR for
  the reader* under *What this rules out*).
- **The reviewer commits; comments are the exception.** A reviewing agent that
  comments gives the human more to read, so his fixes the code and comments
  only when unsure. It reads coding standards from a file of its own, kept out
  of AGENTS.md, and runs in its own context. This is why AI detection covers
  commits and why *Run a review from slussa* sits after *Send to agent*. That
  reviewers will commonly work this way is an assumption; the commenting
  reviewer under *AI review integration* is the other model, and both are kept.
- **Review the system, not only the code.** A human review comment should
  become a check or a standard, so that it is never written twice; his "retro"
  skill reads sessions and a period's PRs and reviews to propose them. It is
  the source of the retro export among the agent commands.
- **Not borrowed: diagrams in the description.** His PR descriptions lean on
  pseudo-code and diagrams (Mermaid, images) to show what changed.
  Pseudo-code reads well in a terminal; Mermaid is shown as its source and
  slussa does not render it, and `o` opens the PR in the browser. His PR skill
  was unreleased when this was written, so no convention for the description
  is parsed until one exists to read.
