# slussa — architecture

slussa is a terminal pull-request client for GitHub (`gh`) and Bitbucket Data
Center (REST + PAT). It supports reading, commenting, reviewing, merging and
closing/declining PRs.

The application owns shared data and effects. Interactive UI components own
local state, input handling, updates and rendering. Presentation widgets are
ordinary rendering functions; they do not need the component interface.

## Product and interaction principles

What slussa is for, what it rules out and the two-views rule are in
[ROADMAP.md](ROADMAP.md) (*Positioning*) and the non-negotiables in
`AGENTS.md`. The interaction principles below guide UI changes; they do not imply
every current screen already meets them.

- **Content first.** Give code, diffs and conversations the most space. Keep
  persistent controls and status indicators limited to what helps the current
  task; a new feature does not justify another visible control.
- **Actions in context, and discoverable.** Offer the relevant actions for the
  focused item (replying to a comment, inspecting a build) with a short,
  predictable footer hint; `?` lists the keys. An essential function must not
  need an undocumented key.
- **Dialogs for focused tasks.** A dialog gives a comment editor or a merge
  choice room when needed. Avoid chains of popups and unnecessary
  confirmations, and preserve context when one opens and closes.
- **Restrained emphasis.** Reserve strong colors for focus, meaningful changes
  and actionable problems; use spacing and hierarchy, not competing badges.
- **Consistent interaction.** Enter opens or selects and Esc returns or
  dismisses. Text entry must clearly distinguish inserting a newline from
  sending. Restore focus predictably and make sending, success and failure
  understandable.
- **Continuity.** Preserve work and reading position across ordinary
  interactions. Drafts are saved automatically; failures leave the user's work
  available for recovery.
- **Respect platform support.** Show optional functions only when the adapter
  supports them. Tell an unsupported feature from a supported action blocked by
  the PR's state, and give the reason for the latter.

When designing a feature, identify its entry point, what appears only after
interaction, and how the user returns to their work; check narrow terminals and
keyboard-only use, and prefer an existing interaction over a new visual pattern.

```text
src/
├── main.rs                Startup, logging and dispatch to `cli` or `tui`
├── cli/                   Arguments and the subcommands that print and exit (`auth login`, `-C`)
├── config.rs              config.toml: theme, sort and `[ai] markers`
├── logging.rs             Log file in the data directory (`SLUSSA_LOG`), for its owner only
├── private_file.rs        Files only their owner may read: the drafts and the log
├── git_url.rs             Splits a git remote into host and path; web base URL
├── test_support/          Test-only: `FakeGh`, `MockHttp` and the payloads they answer with
├── doc_contract.rs        Test-only: reads KEYS.md and the README for the tests that hold the code to them
├── tui/                   The interactive program
│   ├── mod.rs             Composes `app`, `ui` and `run`
│   ├── run.rs             `run`: starts the runtime, the application and the terminal
│   ├── app/               The engine: does I/O, never draws
│   │   ├── mod.rs             Composes the modules below
│   │   ├── event_loop.rs      `App`, the event loop, the task channel and effect dispatch
│   │   ├── state.rs           AppState composition (Store, Ui, Screen)
│   │   ├── store.rs           Cache, PR operations/errors, in-flight loads, reviews, tickets
│   │   ├── notice.rs          The one-line message over the footer
│   │   ├── effect.rs          `Effect` (work), `TaskResult` and `Read` (what came back)
│   │   ├── navigation.rs     Screen identity, open PR and initiate missing loads
│   │   ├── commands.rs       `Command`, what the provider supports of it, and its execution
│   │   ├── drafts.rs         Where drafts are kept; `App::open`, restore, save, journal
│   │   ├── seen.rs           Marks the PR on screen as seen; writes the file of looks
│   │   ├── desktop.rs        Browser and clipboard effects
│   │   ├── fetchers.rs        Run providers off the UI thread
│   │   ├── terminal.rs        `TerminalGuard`: the terminal while the TUI runs
│   │   ├── loads.rs           Apply asynchronous results
│   │   ├── pr_groups.rs       Reading the PR list: groups, pages, the open chain, `L`
│   │   └── refresh.rs         Manual and periodic refresh
│   └── ui/                The surface: draws and takes keys, never starts I/O
│       ├── action.rs          `Action` and the small enums a component consumes
│       ├── mod.rs             UI composition, screen dispatch and input priority
│       ├── component.rs       Component contract and navigation helpers
│       ├── theme.rs           The five palettes and theme lookup
│       ├── icons.rs, layout.rs, format.rs   Glyphs, layout helpers and text formatting
│       ├── components/
│       │   ├── search_input.rs
│       │   ├── help_dialog.rs   Scrollable help shared by list and detail
│       │   ├── comment_editor.rs  The draft being written and its mode
│       │   ├── text_buffer.rs     Text with a caret; wrapping
│       │   └── diff_viewer/
│       │       ├── viewer.rs  DiffViewer state and updates; what the pane drew last
│       │       ├── keys.rs    Tree/pane input
│       │       ├── render.rs  Composition; the comment counts per file
│       │       ├── tree.rs    Tree rendering, the file rows with their counts
│       │       ├── pane.rs    The code pane: diff lines, search and the cursor
│       │       ├── threads.rs One inline thread: its lines and the stops in it
│       │       ├── nav.rs     What the pane cursor can stand on (line, thread,
│       │       │              fold row, queued comment)
│       │       └── file_tree.rs  Visible tree projection
│       ├── screens/
│       │   ├── pr_list/
│       │   │   ├── screen.rs  PrListScreen: selection, overlay, filter and search
│       │   │   └── render.rs, columns.rs, filter.rs   Table, its columns, sort and status filter
│       │   └── pr_detail/
│       │       ├── screen.rs  PrDetailScreen: children, the surface shown and the overlay
│       │       ├── interactions.rs  Dialog/editor workflows and resolved commands
│       │       ├── bindings/  One row per key the screen routes itself: key, where, help,
│       │       │              and what it does now (hidden, blocked, offered)
│       │       ├── keys.rs    Modal priority, then the rows, then the focused child
│       │       ├── view.rs    Read-only component/store queries
│       │       ├── render.rs Screen layout and child rendering
│       │       ├── header.rs
│       │       ├── footer.rs
│       │       ├── dialogs/  Confirm, review, merge, error and help
│       │       │             Dialog components own selection, input and rendering
│       │       ├── build_status.rs
│       │       └── tabs/
│       │           ├── overview/
│       │           │   ├── timeline.rs  Interactive Timeline component: cursor,
│       │           │   │                filter, folds and reading through a tall item
│       │           │   ├── blocks.rs    The blocks of the timeline and the rail
│       │           │   ├── hidden.rs    The dimmed line for hidden comments
│       │           │   └── sidebar.rs   Stateless Ratatui Sidebar widget
│       │           └── ...             Description, CommitList and Builds
│       └── widgets/
│           ├── mod.rs        Composition only
│           ├── comment/      Comments and threads
│           │   ├── render.rs     Boxes and inline threads
│           │   ├── meta.rs       The author line, `[AI]`, reactions and `Reading`
│           │   ├── fold.rs       Folding a long comment and its fold row
│           │   ├── frame.rs      Header line, left rail and box around a comment
│           │   └── code.rs       A suggestion box and the diff around an anchor
│           ├── text.rs       Width, truncation, justification and wrapping
│           ├── footer.rs     The hints and the search prompt
│           ├── panel.rs      Frame, empty state, loading line, scrollbar
│           ├── diff_row.rs   One numbered row of a diff
│           ├── dialog.rs     Shared dialog geometry and footer
│           ├── markdown.rs
│           └── table.rs
├── local/                 What slussa keeps on disk, shared by the TUI and the subcommands
│   ├── scope.rs           Provider, host, the repository slussa acts on and account → the file's name
│   ├── file.rs            One file per scope: lock and atomic write
│   ├── drafts.rs          The drafts file, version 1, and its fixture
│   └── seen.rs            When each PR was last looked at (numbers and times only)
├── session/               Who and where, shared by the TUI and the subcommands
│   ├── mod.rs             `Session`, `connect`
│   ├── preflight.rs       Provider detection and authentication checks
│   └── remote.rs          Local repository/remote detection
├── domain/               Provider-independent data models, ids and rules (`review.rs` has the draft types);
│                         `authorship.rs` says who wrote a comment
└── providers/            Provider requests and payload mapping
    ├── github/           `gh` calls, GraphQL templates, pagination, threads
    ├── bitbucket_dc/     REST client, auth (keyring), probe, diff and activity mapping
    └── error.rs, unified_diff.rs
```

## Component contract

`Component` (`tui/ui/component.rs`) has two borrowed contexts and a typed message:

- `handle_key(&self, key, input) -> Option<Action>` translates input and
  changes nothing.
- `update(&mut self, message, input) -> Option<Effect>` changes local state. A
  returned `Effect` asks the application for work; `None` means it was handled
  locally.
- `render(&mut self, frame, area, view)` draws and records layout-derived
  values such as the viewport size and what the diff pane's cursor is on.

`Input` is what handling a key or a message needs to know; `View` is what
drawing needs. Most components need nothing to handle input (`Input = ()`) and
a good deal to draw, so the two are separate types and nobody builds a
placeholder context to call a method that ignores it.

Three message types keep the directions apart (`Action` in `tui/ui/action.rs`,
`Effect` and `TaskResult` in `tui/app/effect.rs`):

- **`Action`** is input: what a key or a paste becomes. It is grouped by the
  component that consumes it (`List`, `Detail`, `Diff`, `Commits`, `Search`),
  and `DetailAction` is grouped again by the part of the PR screen that handles
  it (`Nav`, `Timeline`, `Confirm`, `Review`, `Merge`, `Editor`, `Pr`, ...), so
  each part is handed only its own kind and matches it in full.
- **`Effect`** is work for the application (`Navigate`, `Refresh`, `OpenPr`,
  `Command`, `PrLink`, ...). Only these reach `App`, which matches them
  exhaustively. A component cannot hand its own message on: `Effect` has no
  variant for one.
- **`TaskResult`** is what work off the UI thread sends back: a `Read`, a
  finished write with its ticket, or a finished link action.

There is no shared mutable store inside a component and no
`Arc<Mutex<AppState>>`. Contexts borrow only the data a component needs. The PR
screen gets a `DetailContext`, built once per key or frame by
`DetailContext::new(store, pr_id, tab)`, which returns `None` unless the PR is
in the list: nothing on the screen asks again whether there is a PR.

The concrete implementations are `PrListScreen`, `PrDetailScreen`, `DiffViewer`,
`CommitList`, `Overview`, `Timeline`, `Description`, `SearchInput`,
`CommentEditor`, `ConfirmDialog`, `ReviewDialog`, `MergeDialog`, `ErrorDialog`
and `HelpDialog`. Small visual pieces, including badges, stay render functions.
Tree and pane are internal parts of DiffViewer; its shared file selection and
focus are coordinated by that owner. `PrDetailScreen` holds at most one dialog
(`Overlay`) and opens and closes it; each dialog owns its private cursor. The
screen resolves selected verdicts, merge strategies and accepted confirmations
into `Command` payloads carrying the PR id. Application workflows never read
dialog or editor state. Sidebar implements Ratatui `Widget` and borrows its
data without owning navigation state.

## State ownership

`AppState` composes `Store`, `Ui` and `Screen`:

- **Store** owns fetched PR data, user identity/provider capabilities and a map
  of review drafts keyed by PR id. Operations and action errors are also keyed
  by PR id; reads are tracked by resource (including PR id and commit oid).
  Reviews survive screen changes and cannot appear in or be submitted for a
  different PR.
- **Ui** owns the list and detail screen instances. Refresh indicators derive
  from the Store's in-flight resources for the visible screen.
- **PrListScreen** owns selection, status filter, sort, SearchInput and one
  optional `ListOverlay` (help, or the filter picker with its highlighted row).
- **PrDetailScreen** owns its child components, the editor and one optional
  `Overlay` (help, confirm, review or merge), so two dialogs cannot be open at
  once. `surface(tab)` says what the content area shows (`Surface`: a tab, and
  on Commits whether a commit is open); the keys, the footer and the comment
  targets all ask it. Its `open(pr_id)` lifecycle resets navigation and
  saves/restores editors by PR id. A submission acknowledgement only clears the
  corresponding PR's editor.
- **DiffViewer** owns file selection, tree expansion, focus, scroll and
  searches. What the pane drew last is one value, `PaneNav` (item count, search
  matches and what the cursor is on: a line, a thread or a queued comment). The
  renderer replaces it whole and empties it when no pane is drawn, so the keys
  never act on rows from an earlier frame. CommitList holds its own DiffViewer
  inside `CommitsView::Diff`, so drilling into a commit cannot change the PR
  diff's cursor or search, and there is no commit diff without an open commit.
- **Overview** composes Timeline and Sidebar; Timeline owns scroll, selected
  comments, reply/thread targets and Ctrl-j/k sub-navigation. Description owns
  its own scroll state.
- **CommentEditor** owns the draft being written: its target, a `TextBuffer`
  and one mode (`Kept` after Esc or a restore, `Typing`, `ConfirmDiscard`).

Review and editor drafts are persisted locally by `local/drafts.rs` (`tui/app/drafts.rs` holds them in the app), independently
of the provider APIs.
Submission retains the draft and queued review comments until success. While a
mutation is pending, another mutation or editor change for that PR is blocked.
`Esc` leaves the detail screen with the draft retained; `q` exits. Neither action
claims to cancel a server write. Runtime shutdown does not wait for provider
workers, and `gh` processes and their I/O have a 60-second deadline.
An error leaves the payload available for an explicit retry and is shown only
on its own PR. Errors capture input before the retained editor.

Provider writes still cannot guarantee exactly-once delivery: an ambiguous
network failure may follow a successful server write. Bitbucket full reviews
also submit multiple requests. Partial failures report acknowledged line comments
and summary posts; the app removes those lines from the remaining queue and
remembers a sent summary so an explicit retry skips it. A post whose response
was lost still needs server-side checking before retry. There is no automatic
retry.

## Input, updates and effects

```text
terminal key (applied synchronously before the next key)
  → editor / modal / search / active screen / focused child
  → Action
  → Ui.update
      → component.update → local state
      → optional Effect
          → App commands / navigation / refresh
          → provider task (needs a ticket)
          → TaskResult
          → Store + PR-scoped component acknowledgement
  → render
```

An open editor or modal captures input before the underlying search field.
Tab selection and its local resets are handled by PrDetailScreen, which returns
a `Navigate` effect for the application to store. Diff navigation and search
selection resets are delegated to the active DiffViewer; the root Ui only routes
to screens. List and commit search resets live in their respective components. The `Option<Action>` routing contract relies
on that explicit modal/focus priority: an ignored key in a modal does not fall
through to content behind it.

The task channel carries asynchronous results (`TaskResult`); local key actions are applied
immediately so rapid input cannot use stale selection or dialog state.
Provider tasks remain centralized. Components never
start requests during rendering. Opening a commit emits a
`LoadCommitDiff { pr_id, oid }` request; the app checks the cache before spawning
work. Opening a PR similarly starts only missing initial loads.

## Startup and providers

`main` initializes logging and dispatches the synchronous CLI. Preflight detects
the repository host and authentication before the Tokio runtime starts and
returns a `Session`: the provider together with the account it acts as. `App`
is built from a session, so there is no app without a known user, and a
`Username` is never empty. The theme is chosen once, by `theme::init`, before
anything is drawn. `TerminalGuard` in `main` restores the terminal when it is
dropped, on a normal exit, an early return or a panic.
GitHub delegates authentication and requests to `gh`; Bitbucket DC uses a PAT
from the OS keyring and blocking HTTP calls. `fetchers.rs` runs both providers
through `spawn_blocking` and returns results through the task channel.

The remote decides where a provider looks. An http(s) remote gives the web
address (scheme, host, port and the context path before `scm/`), so a Bitbucket
served over plain http or under `/bitbucket` works; an ssh remote carries no web
address and assumes `https://host` (`git_url::web_base`,
`bitbucket_dc::remote::base_url`). `github.com` and `bitbucket.org` are
recognised by name and any other host is probed for a Bitbucket Data Center.

Each provider keeps its DTOs and mapping private. `domain/` models what the UI
needs without provider-specific serialization details. `DiffRevision` records the
source/base of the displayed diff and travels with `CommentAnchor` (owned by
`domain/review`) into each queued/submitted comment. GitHub checks both PR refs
before and after loading a PR diff and sends the captured commit id. A GitHub
batch spanning multiple diff revisions fails explicitly instead of rebasing
comments onto the latest HEAD. Bitbucket uses the diff response hashes and its
COMMIT/EFFECTIVE anchor type. Unknown revisions cannot be posted.
Queued comments from another diff revision are not drawn on the current one.
Fetched threads also retain their revision. Diff placement and Overview code
excerpts require a matching revision; outdated comments remain visible without
a misleading excerpt.

## Refresh and loading

The active Builds view refreshes every 15 seconds; other active data and PR
metadata refresh on a 60-second cadence. `F` requests an immediate refresh.
Existing data remains visible if a reload fails. Initial loading uses
`LoadState::{NotRequested, Loading, Loaded, Failed}`.

The PR list is read per group (`PrGroup`: open, merged, declined), a page at a
time, and a `PrBatch` carries the provider's opaque continuation (GitHub's end
cursor, Bitbucket's offsets). The first page of open PRs is shown at once and
later pages are appended in arrival order; the attention order is applied once
when the reading ends. `OPEN_BATCH` (90) open PRs are read without being asked
and `L` reads 90 more. Merged and declined PRs are read only when their view is
first opened, a batch at a time, and `L` reads the next older batch. GitHub's
list query leaves out the body and labels; `fetch_info` reads them when a PR
opens (`Feature::PrInfo`, `PrResource::Info`). A refresh re-reads the open group
and the groups already opened, and keeps what was loaded.

All fetch entry points register a resource key before spawning work and skip
an already-running fetch of that resource. Completion removes only its own
key; the refresh indicator remains active until the relevant loads settle.
This serializes requests per resource, preventing older overlapping reads from
replacing newer results. If a mutation finishes during an existing read, that
resource is marked for another fetch after the old read settles, so the change
is not missed. Successful mutations refresh activity, PR metadata and
mergeability. Read completions never acknowledge pending mutations.

UI modal detection is shared by keyboard routing and periodic/manual refresh,
including review and merge pickers. Bitbucket collections follow `nextPageStart`
until `isLastPage`; GitHub connections follow GraphQL cursors, including nested
thread comments, labels and latest reviews. Nonadvancing/missing continuation
cursors fail the load rather than silently returning a partial collection.
GitHub build details load check runs and legacy statuses with REST pagination.
Bitbucket diffs explicitly marked truncated fail with an explanatory message.
Provider-side limits and server/version compatibility still require live checks.

## Rules for I/O and effects

- **Provider and process calls block, and run off the UI thread.** GitHub
  spawns `gh`; Bitbucket Data Center uses `reqwest::blocking`. Both run through
  `App::spawn_fetch`, which wraps them in `spawn_blocking` and sends the result
  back as a `TaskResult`. `gh` calls have a 60 second deadline. Do not add
  async HTTP or ad hoc threads; a new external call follows the same path.
- **Nothing starts I/O while rendering or handling a key.** Components return
  an `Effect`; `App` decides whether work starts.
- **A child process that needs the terminal** (an editor, an agent) cannot use
  this path. It needs the suspend and resume sequence described in
  ROADMAP.md (*Suspend / resume*), which is not built yet.

## Lifecycle: a read

1. A screen opens or a refresh ticks; `App` calls a `spawn_load_*` function in
   `tui/app/fetchers.rs`.
2. It asks `Store::begin_fetch(key)` for a `FetchTicket`. That registers the
   resource's `FetchKey` in `Store::fetches`, refuses a resource the provider
   does not have, and gives no ticket if the key is already there. `spawn_read`
   takes the ticket, so a read cannot start unregistered.
3. `spawn_fetch` runs the provider call on `spawn_blocking`.
4. The result returns as `TaskResult::Read(read)`. A `Read` names its own
   resource (`Read::key`), so `tui/app/loads.rs` settles the bookkeeping for every
   kind the same way and then stores the data with `LoadState::reload`, which
   keeps loaded data if the reload failed. It removes the key, records a
   refresh failure if needed, and starts a follow-up fetch if
   `reload_after_fetch` names the resource.
5. The next render reads the store.

## Lifecycle: a write

1. A key press becomes a `DetailAction`; the screen turns the finished dialog
   or editor into a `Command` with the PR id (`pr_detail/interactions.rs`).
   What a command carries is already checked: a comment's text is a `NonBlank`,
   an edit or a delete a `CommentKey`, a thread to resolve a `ThreadHandle`.
2. `App::execute` (`tui/app/commands.rs`) rejects commands the provider does not
   support (`Command::supported_by`).
3. `App::begin_write(pr_id, operation)` asks `Store::begin_write` for a
   `WriteTicket`, which records the `Operation` and gives none while another
   write is pending for that PR, and then calls `checkpoint_submission`, which
   saves drafts first. If that save fails, nothing is sent.
4. A `spawn_*` function takes the ticket and sends the write. A line comment
   becomes a `ReviewComment` in `fetchers::postable`, the one place that
   refuses a comment whose diff revision is unknown.
5. The result returns as `TaskResult::Written { ticket, result }`. On success
   the operation and the matching draft are cleared and activity, PR metadata
   and mergeability are refetched. On failure the error is stored under that
   PR, the draft stays, and the user can retry explicitly. The PR is marked
   "may have reached the server" only if `WriteError::may_have_reached_server`
   says so: a write refused locally, or by a stated server refusal, is not.

## Errors

A provider call returns `FetchError` (`providers/error.rs`), an enum that keeps
its cause. It travels as a value through `spawn_fetch`, `Read` and
`LoadState::Failed`, and is turned into text by `user_message()` only where it
is shown; the log gets its `Display`. That form leaves out what the server
answered with, since a failed GraphQL query still answers the fields it could
read: of a failed call's answer it holds the error messages and the size, and
of a parser's message not the values it quotes. A review that goes out as several
requests can end as `ReviewError::Partial`, which says how much arrived, and
becomes `WriteError::PartialReview` in the app. A worker that panicked is
`FetchError::WorkerPanicked`, not a lost result. Opening or copying a link
ends as `LinkDone` or `LinkError` (`tui/app/desktop.rs`), and the notice is worded
where it is shown.

## Types that carry the rules

A state that should not exist is made impossible to write, rather than checked
where it is used. Three shapes, in the order to reach for them:

- **An enum with the data in the variant it belongs to**, instead of a struct
  with a flag and optional fields. `Overlay`, `ListOverlay`, `Surface`,
  `CommitsView`, `EditMode`, `NavTarget` (a thread, and a fold row that stands
  for its thread), `Mergeability` (the reasons are in `Conflicts` and
  `Blocked`), `PrStatus` (being a draft and having a conflict are in `Open`,
  so a merged PR has neither; `Conflicts` says yes, no or unknown, since a
  provider does not always say), `ThreadHandle`, `LineRef`, `CommentKind`, `AccountKind` and
  `Authorship` (who wrote a comment), `AuthorFilter` (and `hides`, which says
  whose comments a filter leaves out), `Reveal` and `Folds`. A
  `match` on one of these names every variant, so a new one is a compile error
  wherever it has to be handled. Clippy's `wildcard_enum_match_arm` enforces
  that in `app`, `domain` and `providers`; `tui` matches crossterm's `KeyCode`
  in every key handler, where a catch-all is right, so there it is a review
  rule.
- **A newtype with one constructor**, for a value with a rule. `Username`
  (never empty), `NonBlank` (comment text), `AiMarkers` (a blank marker cannot
  be made, so none matches every comment), `WebUrl` (safe to hand to a
  browser), `ReviewComment` (its diff revision is known), `CommentKey` (the
  comment has an id), `Pat` (never printed). The ids `PrId`, `CommentId` and
  `CommitOid` have no rule; they exist so that two numbers or two strings
  cannot be passed in each other's place.
- **A ticket or a constructor that requires its data**, instead of a call order
  to remember. `FetchTicket`, `WriteTicket`, `Session`, `DetailContext`,
  `TerminalGuard`.

The types in `domain/review.rs`, with `CommentAnchor` and `DiffRevision`, are
written to the draft file, and a file that cannot be read stops slussa from
starting. The strong types sit on the UI and provider side of that boundary
and keep the file's spelling through serde (`#[serde(transparent)]` on the
ids, `review: bool` for `CommentKind`). The version 1 fixture in
`local/drafts.rs` pins the format.

## AI authorship, folds and reading

A comment is an AI agent's when its account is a bot's, or its first non-empty,
trimmed line starts with one of the `[ai] markers` in the config. The account is
known only to the
provider: GitHub answers the type of the author (`__typename`), and
`Comment::account` carries it as `AccountKind`; Bitbucket Data Center does not
say, so it is always `Person`. The markers are text, so they work on both and
for an agent that posts with a person's token. It is decided once, when the
activity arrives: `Store::judged` runs `AiMarkers::judge` (`domain/authorship.rs`)
over it, and `Store::set_ai_markers` judges what is already cached, so
`Comment::authorship` is current wherever it is read. A provider leaves it
`Human`, and the markers are private to the store, so no view can ask the
question a second way. Views read `Comment::is_ai`, `CommentThread::authorship`
and `Activity::has_ai`; with no bot and no marker, the filter and its key are
simply not offered. `Reading` carries the PR's author and the folds to the
comment widgets.

A commit is judged the same way, once, when it arrives (`Store::judged_commits`;
`set_ai_markers` judges the cached ones). It is an agent's when its account is a
bot's, when a line of its message is the trailer Claude Code adds
(`Co-Authored-By: … <noreply@anthropic.com>`, built in, since it is the one
convention an agent follows without being set up), or when any trimmed line of
it starts with a marker. GitHub gives a commit a git actor and
not an account, so a bot is told by the address `…[bot]@users.noreply.github.com`
(`github/commits.rs`); Bitbucket Data Center says nothing, so only markers apply.
`Commit` carries the whole `message` for this, and the Commits tab reads
`Commit::is_ai`.

A comment over 12 lines is folded to its first 8 and a dimmed row
(`widgets/comment/fold.rs`). `Folds::Open` is for a view with no key to open
them. The Overview keeps the opened ones in `Timeline::expanded`; the diff keeps
them in `DiffViewer::opened_comments`, where the fold row is a stop of its own
(`NavTarget::Fold`) so that `j`/`k` reach it and `space` there opens or folds
that comment, while `space` on the thread expands or collapses a resolved one.
`render_inline_thread` returns the rows of the folds along with the lines, and
`diff_viewer/threads.rs` turns them into stops.
The fold row says how many lines it hides and nothing about what they hold: a
count of what a bot found (issues, nitpicks) would mean reading one reviewer's
wording, and slussa does not do that for any one reviewer.

In the Overview `j` and `k` read on through an item taller than the screen
before they move to the next one: `Timeline` keeps the rows of the focused item
from the last draw, `TimelineAction::Move` decides between scrolling and
moving, and `Reveal` says what the next draw does about the scroll position.

## Checklists

**A new provider write.** Add the method to `Provider` and to both provider
modules. Add a `Feature` in `domain/capabilities` and set it in each provider's
capabilities. Add a `Command` variant and its `supported_by` arm, and an
`Operation` if it is a new kind. Add the message to the sub-enum of
`DetailAction` that owns it, with its key as a row in `pr_detail/bindings/rows.rs`
(the row gives the help entry and the footer hint, and `Offer` says when it is
hidden, blocked or offered), and resolve it to the `Command` in `interactions.rs`. Add the `execute` arm, which
gets a `WriteTicket` from `begin_write`, and a `spawn_*` function that takes
it. The compiler lists the matches that need the new variants. Write a
regression test that injects the result.

**A new read resource.** Add a `PrResource` (the key is `FetchKey::Pr(resource, id)`), a `Read` variant with its `key` and
`failure` arms, and a `LoadState` field on `PrData`. Extend `PrData::start_loading` and `has_loaded`
and `refresh_failed` in `tui/app/store.rs`, add a `spawn_load_*`
function, apply the `Read` in `loads.rs`, and choose its cadence in
`tui/app/refresh.rs`.

## Verification and adding behavior

`cargo test --locked` covers provider projections, parsing, component workflows
and rendering. The screen snapshots (`src/tui/testdata/screens.txt`) cover all five
detail tabs and the PR list at 100×30 and 40×12. They compare terminal text;
interactive tests additionally exercise search, filters, diff focus and match
wrapping, commit drilldown, review ownership, editor input and dialog priority.
Additional asynchronous regressions exercise draft recovery, review retention,
late acknowledgements across PR navigation, duplicate-submit blocking and
refresh bookkeeping. These use a current-thread runtime without yielding and
inject results directly; queued provider futures are cancelled without running.
These are local tests and do not submit anything to a provider.

For a local interaction, change the owning component's message handling and
rendering together. For shared data, extend Store/domain and the appropriate
provider/fetcher. For an external effect, return an action and implement the
application workflow. Add a regression test for observable behavior, especially
when navigation or asynchronous state is involved.

Import types from their owners: loading models from `tui/app/store`, review work
from `domain/review`, tab identities from `pr_detail/tabs`, and editor drafts from
`domain/review` (also re-exported by `components/comment_editor`). `tui/app/state` is not a UI type re-export hub. Screens use
`DetailView` for read-only queries. `AppState::detail_view()` is a test helper;
application effects do not query UI state. `tests/repo_rules.rs` fails a
`domain`, `providers`, `session`, `cli` or `tui/ui` file that imports from a layer it may not
(test code and `test_support` excepted). `cli` may use `local`, the files the subcommands
share with the TUI (`slussa propose import` writes the proposals file), and never `tui`.

Run `cargo clippy --all-targets --locked -- -D warnings` alongside the tests.

## Provider flow verification

GitHub writes (comment, review, resolve, merge, close, reopen) have been run
live against a throwaway repository, and blocked merges against a real ruleset.
Bitbucket Data Center has only been seen listing PRs on one real server; its
writes are tested against a mock of the documented API. The payload changes
follow the [GitHub review-comment API](https://docs.github.com/en/rest/pulls/comments),
[GitHub review API](https://docs.github.com/en/rest/pulls/reviews) and
[Bitbucket Data Center API](https://developer.atlassian.com/server/bitbucket/rest/v900/api-group-pull-requests/).

## Optional provider features

`domain/capabilities.rs` describes adapter support independently of provider
names. `Provider::capabilities()` supplies the profile stored in `Store`.
The default profile exposes only the core reading flows: PRs, descriptions,
activity, diffs and commits. Optional features include comment operations,
thread resolution, build details, mergeability, closing, review verdicts and
merge strategies. Unsupported features are hidden in tabs, footer hints and
help, blocked in keyboard/action handling, and checked again before commands
execute. Optional builds/mergeability fetches are skipped too.

Review support is one optional value, `Capabilities::review`
(`ReviewCaps`: the verdicts, the ones an author may give on their own PR, and
how a review is submitted). GitHub requires a single revision for an atomic
review; Bitbucket submits sequentially with partial-progress recovery. Unapprove is offered by the Bitbucket adapter only. Components use
capabilities rather than branching on the provider enum. New adapters can
expose their supported subset without adopting GitHub's complete feature set.

The Comments column counts conversation comments plus review threads on GitHub
and uses the server's `commentCount` on Bitbucket, which has not been checked
against a real server to include inline comments.

Support is distinct from permission or PR state: a supported action can remain
visible but disabled with a reason (for example approving your own PR or
merging a closed PR). Profiles currently describe implemented adapter support;
repository permissions and server-version feature discovery are not probed.

## Comment editing and local recovery

The comment editor opens as a focused dialog. Enter inserts a newline; Ctrl+S
submits (or queues an inline comment in an active review). Arrows and Home/End
move within the text; Delete and Backspace edit at the cursor. Bracketed paste
is handled as one text insertion, never as navigation or submit commands. Esc
keeps the draft and closes the editor. `c` resumes it on any detail tab. Ctrl+X
opens a discard confirmation. Existing drafts are resumed instead of silently
replaced when another comment action is selected.

`local/drafts.rs` persists draft targets, their captured diff revisions, review
queues and partial-submission receipts in the platform's local data directory
under `slussa/drafts/`. Files are versioned and scoped by provider, remote host,
repository path and authenticated account; no tokens are included. The snapshot
uses ordered maps, writes only when content changes, syncs a temporary file,
and renames it over the previous version. Every action is followed by a save
except a keystroke in the editor: a save is a file sync, so typed text is
written within half a second instead, and at once by the next other action
(a paste, closing the editor, sending) and on exit. Files use mode 0600 on Unix. A
[standard-library file lock](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)
prevents simultaneous writers for the same scope and is released on process exit.
Corrupt or incompatible files are preserved and reported at startup.

Recovery data is checkpointed before a server mutation starts. A failed
checkpoint blocks that submission and leaves the editor intact. Autosave
failures are visible in the footer. Successful acknowledgements remove the
corresponding drafts; partial reviews keep only the unacknowledged comments and
remember an already submitted summary. An interrupted or failed request is
marked for inspection after restart: its server outcome may be uncertain.
Restore never sends anything automatically. Recovered editors start closed,
with a contextual resume hint. The current model keeps one editor draft per PR,
plus the PR's review queue. Provider-side draft synchronization and an external
editor integration are not implemented.

## PR links

Providers populate the optional `PullRequest.url` using the server's web link:
GitHub's GraphQL `url` and Bitbucket DC's `links.self[].href`. Components do not
construct platform-specific routes. In either list or detail, `o` opens the PR
in the browser and `y` copies its URL; missing URLs suppress both actions and
their help entries. Search, editor and modal input retain precedence.

`Effect::PrLink` captures the target PR id. The app resolves the cached URL and
runs the desktop effect off the UI thread, independently of PR mutations and
draft recovery. A brief notice reports completion or failure, identifies the
PR, and leaves the selection unchanged. The URL is checked once, by
`WebUrl::parse` (HTTP(S), a host, no credentials or control characters), and
the helpers take only a `WebUrl`; they receive argument arrays or stdin, not
interpolated commands.

macOS uses `open`/`pbcopy`; Windows uses the URL handler and PowerShell clipboard;
Linux uses `xdg-open` and an available `wl-copy`, `xclip` or `xsel`. Over SSH,
or when no clipboard helper works, slussa writes an OSC 52 escape to the
terminal instead; the terminal never confirms it, so the notice says "sent to
terminal clipboard". Failed helpers and timeouts are reported. Each helper has
a five-second deadline. Local tests cover URL mapping, input
routing, missing support and helper success/failure/timeouts without launching
a real browser or changing the user's clipboard. Linux and Windows desktop
integration have not been exercised live. File/line links and copying commit
SHAs or branch names remain separate future work.

## UI behaviour rules

The rules the code relies on, kept short; the regression tests in
`tui/ui/regression_tests/` and `tui/app/tests/` pin them.

- **Dialogs.** Review, merge, confirmation and filter dialogs share geometry
  and a keyboard footer through `widgets/dialog.rs`; the selected option stays
  in view on short terminals and modal footers replace the screen's hints. A key
  a modal ignores does not fall through to what is behind it. Merge and close
  dialogs name the PR and the target branch, and close defaults to *No*.
  Discarding a populated review asks first, with *Keep reviewing* selected.
- **Editor labels follow the target.** A line comment says `add to review`, a
  PR comment `post comment`, a reply `post reply`, an edit `save changes` and a
  verdict summary `submit review`. The draft footer reads `v: finish draft (N)`,
  and Tab in the verdict dialog previews every queued comment. The labels never
  change what is published.
- **Unread.** `Seen` (`domain/seen.rs`) maps a PR number to what the list knew of
  it when the reader last looked: its `updated` and the time. A PR is unread when it
  has an entry and is updated after it, so a PR never opened is never unread and
  nothing is marked on the first run. A PR is marked seen when it is opened and
  whenever the list is read while it is on screen, at its newest, so what the
  reader does to it themself does not light it up. `local/seen.rs` keeps it in a
  file of its own per scope, holding no content, and entries unopened for 90 days
  are forgotten. It is a convenience: if another slussa has the file or it cannot
  be read, the marks last the run. The list draws `●` before the number (`#`
  keeps room for it, so the numbers do not move).
- **The AI review column** reads `PullRequest::ai_review` (`AiReview`: none, current,
  stale, changes requested). On GitHub `latestReviews`, which the list query
  already reads, also gives the type of each reviewer's account and the commit its
  review was made on, and `github/prs.rs` takes the bot accounts' reviews and
  compares that commit with the PR's `headRefOid`; the one that needs the reader
  most counts, and on a PR that is over any review is just current. It costs
  nothing against the query without it (1 rate-limit point a page, measured on
  cli/cli, 2026-10-03). A bot is told by the account type, not by `[ai] markers`,
  which would need the text of every comment. Bitbucket Data Center leaves it
  `None`, and the column is there only while some row has been reviewed.
- **Layout.** The list drops secondary columns as the terminal narrows; footer
  hints show whole when they fit, with room for `?: help`. The compact PR header
  uses two rows. A diff under 72 content columns shows Files or Code by focus,
  and panel titles and focused borders say which is active. Description
  renders pipe tables at full width and pans with `H`/`L`; a resize clamps the
  scroll.
- **Selection and scroll keep their identity.** Lists reconcile selection by PR
  id and commit oid; Overview by comment id and kind (a PR comment and a review
  comment can share an id), adjusting scroll so the selected comment keeps its
  height in the viewport. `PrDetailScreen` keeps each PR's tab components for
  the session, so search, scroll and focus survive leaving and returning;
  dialogs are transient and editors use the draft persistence above.
  `CommitList` owns its `ListState`, so opening a commit and returning keeps the
  viewport.
- **Empty and failed states.** An empty search says how to clear it; an initial
  failure wraps and advertises refresh; a failed reload keeps the cached data
  with a warning scoped to the resource until it succeeds. Error dialogs scroll
  long messages and close with Esc or Enter, leaving the editor and selection
  alone. The Builds tab owns its scrolling.
- **Colour.** Graphite is the default; the other themes are chosen with
  `theme` in the config or `SLUSSA_THEME`, and there is no theme picker. The
  four with a palette of their own are dark, and `terminal` takes its
  foreground and background from the terminal and so follows a light one;
  there is therefore no automatic light and dark, which is worth taking up
  again only with a light palette.
  Description takes heading, link, quote and code colours from the theme
  (inline code uses `orange`) and keeps the terminal background. Conversation
  Markdown still starts from the renderer's dark style; compare it visually
  before changing colours.
