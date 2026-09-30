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
`CLAUDE.md`. The interaction principles below guide UI changes; they do not imply
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
├── main.rs, cli.rs        Startup, logging and CLI dispatch (`auth login`, `-C`)
├── config.rs              config.toml: theme and sort
├── logging.rs             Log file in the data directory (`SLUSSA_LOG`)
├── git_url.rs             Splits a git remote into host and path; web base URL
├── test_support.rs        Test-only: `FakeGh` and `MockHttp`
├── app/
│   ├── mod.rs             Event loop, task channel and effect dispatch
│   ├── state.rs           AppState composition (Store, Ui, Screen)
│   ├── store.rs           Cache, PR operations/errors, in-flight loads and reviews
│   ├── action.rs          UI messages, application requests and load results
│   ├── navigation.rs     Screen identity, open PR and initiate missing loads
│   ├── commands.rs       Execute resolved review/comment/lifecycle commands
│   ├── reviews.rs        Review drafts, comment targets and anchors
│   ├── drafts.rs         Scoped, atomic local draft recovery
│   ├── desktop.rs        Browser and clipboard effects
│   ├── fetchers.rs        Run providers off the UI thread
│   ├── loads.rs           Apply asynchronous results
│   ├── refresh.rs         Manual and periodic refresh
│   ├── preflight.rs       Provider detection and authentication checks
│   └── remote.rs          Local repository/remote detection
├── tui/
│   ├── mod.rs             UI composition, screen dispatch and input priority
│   ├── component.rs       Component contract and navigation helpers
│   ├── theme.rs           The five palettes and theme lookup
│   ├── icons.rs, layout.rs, format.rs   Glyphs, layout helpers and text formatting
│   ├── components/
│   │   ├── search_input.rs
│   │   ├── help_dialog.rs   Scrollable help shared by list and detail
│   │   ├── comment_editor.rs
│   │   └── diff_viewer/
│   │       ├── mod.rs     DiffViewer state and updates
│   │       ├── keys.rs    Tree/pane input
│   │       ├── render.rs  Composition
│   │       ├── tree.rs    Tree rendering
│   │       ├── pane.rs    Diff and inline-thread rendering
│   │       └── file_tree.rs  Visible tree projection
│   ├── screens/
│   │   ├── pr_list/       PrListScreen: table, filter and search
│   │   └── pr_detail/
│   │       ├── mod.rs     PrDetailScreen: children and local dialog state
│   │       ├── interactions.rs  Dialog/editor workflows and resolved commands
│   │       ├── keys.rs    Modal, screen and focused-child routing
│   │       ├── view.rs    Read-only component/store queries
│   │       ├── render.rs Screen layout and child rendering
│   │       ├── header.rs
│   │       ├── footer.rs
│   │       ├── dialogs/  Confirm, review, merge, error and help
│   │       │             Dialog components own selection, input and rendering
│   │       ├── build_status.rs
│   │       └── tabs/
│   │           ├── overview/
│   │           │   ├── mod.rs       Layout and child delegation
│   │           │   ├── timeline.rs  Interactive Timeline component
│   │           │   └── sidebar.rs   Stateless Ratatui Sidebar widget
│   │           └── ...             Description, CommitList and Builds
│   └── widgets/
│       ├── mod.rs        Shared presentation primitives
│       ├── comment.rs    Comments, threads, reactions and suggestions
│       ├── dialog.rs     Shared dialog geometry and footer
│       ├── markdown.rs
│       └── table.rs
├── domain/               Provider-independent data models and rules
└── providers/            Provider requests and payload mapping
    ├── github/           `gh` calls, GraphQL templates, pagination, threads
    ├── bitbucket_dc/     REST client, auth (keyring), probe, diff and activity mapping
    └── error.rs, unified_diff.rs
```

## Component contract

`Component` has an associated borrowed `Context` and a typed `Message`:

- `handle_key(&self, key, context) -> Option<Action>` translates input.
- `update(&mut self, message, context) -> Option<Action>` changes local state.
  A returned action requests application-level work; `None` means it was handled
  locally.
- `render(&mut self, frame, area, context)` draws and updates layout-derived
  values such as viewport size and visible diff anchors.

There is no shared mutable store inside a component and no `Arc<Mutex<AppState>>`.
Contexts borrow only the data a component needs. The list receives PR data and a
refresh indicator; the diff receives diff data, threads, queued comments and an
author. The detail screen receives a read-only Store and navigation context.

The concrete implementations are `PrListScreen`, `PrDetailScreen`, `DiffViewer`,
`CommitList`, `Overview`, `Timeline`, `Description`, `SearchInput`, `CommentEditor`,
`ConfirmDialog`, `ReviewDialog` and `MergeDialog`.
Small visual pieces, including badges, stay render
functions. Tree and pane are internal parts of DiffViewer; its shared file
selection and focus are coordinated by that owner. PrDetailScreen owns optional dialog instances and manages opening/closing them;
each selectable dialog owns its private cursor. The screen resolves
selected verdicts, merge strategies and accepted confirmations into `Command`
payloads carrying the PR id. Application workflows never read dialog or editor
state.
`ErrorDialog` owns message scrolling and input capture. A shared `HelpDialog` component
owns help scrolling; each screen owns opening/closing it and its help entries. Sidebar implements
Ratatui `Widget` and borrows its data without owning navigation state.

## State ownership

`AppState` composes `Store`, `Ui` and `Screen`:

- **Store** owns fetched PR data, user identity/provider capabilities and a map
  of review drafts keyed by PR id. Operations and action errors are also keyed
  by PR id; reads are tracked by resource (including PR id and commit oid).
  Reviews survive screen changes and cannot appear in or be submitted for a
  different PR.
- **Ui** owns the list and detail screen instances. Refresh indicators derive
  from the Store's in-flight resources for the visible screen.
- **PrListScreen** owns selection, status filter, filter picker and SearchInput.
- **PrDetailScreen** owns its child components, editor and dialog state. Its
  `open(pr_id)` lifecycle resets navigation and saves/restores editors by PR id.
  A submission acknowledgement only clears the corresponding PR's editor.
- **DiffViewer** owns file selection, tree expansion, focus, scroll, searches and
  rendered line/thread anchors. CommitList owns a second DiffViewer instance,
  so drilling into a commit cannot change the PR diff's cursor or search.
- **Overview** composes Timeline and Sidebar; Timeline owns scroll, selected
  comments, reply/thread targets and Ctrl-j/k sub-navigation. Description owns
  its own scroll state.
- **CommentEditor** owns the active text draft and editing behavior.

Review and editor drafts are persisted locally by `app/drafts.rs`, independently
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
      → optional application effect
          → App commands / navigation / refresh
          → provider task
          → Loaded action
          → Store + PR-scoped component acknowledgement
  → render
```

An open editor or modal captures input before the underlying search field.
Tab selection and its local resets are handled by PrDetailScreen, which returns
a `Navigate` effect for the application to store. Diff navigation and search
selection resets are delegated to the active DiffViewer; the root Ui only routes
to screens. List and commit search resets live in their respective components. The current `Option<Action>` routing contract relies
on that explicit modal/focus priority: an ignored key in a modal does not fall
through to content behind it.

The action channel carries asynchronous results; local key actions are applied
immediately so rapid input cannot use stale selection or dialog state.
Provider tasks remain centralized. Components never
start requests during rendering. Opening a commit emits a
`LoadCommitDiff { pr_id, oid }` request; the app checks the cache before spawning
work. Opening a PR similarly starts only missing initial loads.

## Startup and providers

`main` initializes logging and dispatches the synchronous CLI. Preflight detects
the repository host and authentication before the Tokio runtime starts.
GitHub delegates authentication and requests to `gh`; Bitbucket DC uses a PAT
from the OS keyring and blocking HTTP calls. `fetchers.rs` runs both providers
through `spawn_blocking` and returns results through the action channel.

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
opens (`Feature::PrInfo`, `FetchKey::Info`). A refresh re-reads the open group
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
  back as an `Action::Loaded`. `gh` calls have a 60 second deadline. Do not add
  async HTTP or ad hoc threads; a new external call follows the same path.
- **Nothing starts I/O while rendering or handling a key.** Components return
  an `Action`; `App` decides whether work starts.
- **A child process that needs the terminal** (an editor, an agent) cannot use
  this path. It needs the suspend and resume sequence described in
  ROADMAP.md (*Suspend / resume*), which is not built yet.

## Lifecycle: a read

1. A screen opens or a refresh ticks; `App` calls a `spawn_load_*` function in
   `app/fetchers.rs`.
2. It inserts the resource's `FetchKey` into `Store::fetches`. If the key is
   already there, it returns.
3. `spawn_fetch` runs the provider call on `spawn_blocking`.
4. The result returns as `Action::Loaded(...)` and `app/loads.rs` applies it to
   the `Cache` with `LoadState::reload`, which keeps loaded data if the reload
   failed. It removes the key, records a refresh failure if needed, and starts
   a follow-up fetch if `reload_after_fetch` names the resource.
5. The next render reads the store.

## Lifecycle: a write

1. A key press becomes a `DetailAction`; the screen turns the finished dialog
   or editor into a `Command` with the PR id (`pr_detail/interactions.rs`).
2. `App::execute` (`app/commands.rs`) rejects commands the provider does not
   support (`Command::supported_by`) and ignores a second write while one is
   pending for that PR.
3. It records an `Operation` for the PR and calls `checkpoint_submission`,
   which saves drafts first. If that save fails, nothing is sent.
4. A `spawn_*` function sends the write and returns a `LoadedAction`.
5. On success the operation and the matching draft are cleared and activity,
   PR metadata and mergeability are refetched. On failure the error is stored
   under that PR, the draft stays, and the user can retry explicitly.

## Checklists

**A new provider write.** Add the method to `Provider` and to both provider
modules. Add a `Feature` in `domain/capabilities` and set it in each provider's
capabilities. Add a `Command` variant and its `supported_by` arm, and an
`Operation` if it is a new kind. Add the `DetailAction`, key, help entry and
footer hint, and resolve it to the `Command` in `interactions.rs`. Add the
`execute` arm and a `spawn_*` function, handle the `LoadedAction` in `loads.rs`,
and write a regression test that injects the result.

**A new read resource.** Add a `FetchKey` and a `LoadState` field on `PrData`.
Extend `has_cached_data`, `refreshing` and `refresh_failed` in `app/store.rs`,
add a `spawn_load_*` function and a `LoadedAction`, apply it in `loads.rs`, and
choose its cadence in `app/refresh.rs`.

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

Import types from their owners: loading models from `app/store`, review work
from `app/reviews`, tab identities from `pr_detail/tabs`, and editor drafts from
`app/reviews` (also re-exported by `components/comment_editor`). `app/state` is not a UI type re-export hub. Screens use
`DetailView` for read-only queries. `AppState::detail_view()` is a test helper;
application effects do not query UI state.

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

Review submission semantics are explicit: GitHub requires a single revision
for an atomic review; Bitbucket submits sequentially with partial-progress
recovery. Unapprove is offered by the Bitbucket adapter only. Components use
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

`app/drafts.rs` persists draft targets, their captured diff revisions, review
queues and partial-submission receipts in the platform's local data directory
under `slussa/drafts/`. Files are versioned and scoped by provider, remote host,
repository path and authenticated account; no tokens are included. The snapshot
uses ordered maps, writes only when content changes, syncs a temporary file,
and renames it over the previous version. Files use mode 0600 on Unix. A
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

`Action::PrLink` captures the target PR id. The app resolves the cached URL and
runs the desktop effect off the UI thread, independently of PR mutations and
draft recovery. A brief notice reports completion or failure, identifies the
PR, and leaves the selection unchanged. HTTP(S) URLs are validated before use;
helper processes receive argument arrays or stdin, not interpolated commands.

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
`tui/regression_tests.rs` and `app/tests.rs` pin them.

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
  `theme` in the config or `SLUSSA_THEME`, and there is no theme picker.
  Description takes heading, link, quote and code colours from the theme
  (inline code uses `orange`) and keeps the terminal background. Conversation
  Markdown still starts from the renderer's dark style; compare it visually
  before changing colours.
