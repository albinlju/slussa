# tuipr — architecture

tuipr is a terminal pull-request client for GitHub (`gh`) and Bitbucket Data
Center (REST + PAT). It supports reading, commenting, reviewing, merging and
closing/declining PRs.

The application owns shared data and effects. Interactive UI components own
local state, input handling, updates and rendering. Presentation widgets are
ordinary rendering functions; they do not need the component interface.

```text
src/
├── app/
│   ├── mod.rs             Event loop, task channel and effect dispatch
│   ├── state.rs           AppState composition (Store, Ui, Screen)
│   ├── store.rs           Cache, PR operations/errors, in-flight loads and reviews
│   ├── action.rs          UI messages, application requests and load results
│   ├── navigation.rs     Screen identity, open PR and initiate missing loads
│   ├── commands.rs       Execute resolved review/comment/lifecycle commands
│   ├── reviews.rs        Review drafts, comment targets and anchors
│   ├── fetchers.rs        Run providers off the UI thread
│   ├── loads.rs           Apply asynchronous results
│   ├── refresh.rs         Manual and periodic refresh
│   ├── preflight.rs       Provider detection and authentication checks
│   └── remote.rs          Local repository/remote detection
├── tui/
│   ├── mod.rs             UI composition, screen dispatch and input priority
│   ├── component.rs       Component contract and navigation helpers
│   ├── components/
│   │   ├── search_input.rs
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
│       ├── markdown.rs
│       └── table.rs
├── domain/               Provider-independent data models
└── providers/            Provider requests and payload mapping
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
Small visual pieces, including badges and the Builds display, stay render
functions. Tree and pane are internal parts of DiffViewer; its shared file
selection and focus are coordinated by that owner. PrDetailScreen owns optional dialog instances and manages opening/closing them;
each selectable dialog owns its private cursor. The screen resolves
selected verdicts, merge strategies and accepted confirmations into `Command`
payloads carrying the PR id. Application workflows never read dialog or editor
state.
Error and help displays remain simple presentation helpers. Sidebar implements
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

Review and editor drafts are in-memory session data, not persisted to disk.
Submission retains the draft and queued review comments until success. While a
mutation is pending, another mutation or editor change for that PR is blocked.
An error leaves the payload available for an explicit retry and is shown only
on its own PR. Errors capture input before the retained editor.

Provider writes still cannot guarantee exactly-once delivery: an ambiguous
network failure may follow a successful server write. Bitbucket full reviews
also submit multiple requests; a failed batch can have already posted some
comments. Retaining the payload prevents data loss but does not deduplicate
those posts on retry. There is no automatic retry.

## Input, updates and effects

```text
terminal key
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

The action channel and provider tasks remain centralized. Components never
start requests during rendering. Opening a commit emits a
`LoadCommitDiff { pr_id, oid }` request; the app checks the cache before spawning
work. Opening a PR similarly starts only missing initial loads.

## Startup and providers

`main` initializes logging and dispatches the synchronous CLI. Preflight detects
the repository host and authentication before the Tokio runtime starts.
GitHub delegates authentication and requests to `gh`; Bitbucket DC uses a PAT
from the OS keyring and blocking HTTP calls. `fetchers.rs` runs both providers
through `spawn_blocking` and returns results through the action channel.

Each provider keeps its DTOs and mapping private. `domain/` models what the UI
needs without provider-specific serialization details. Provider requests and
wire formats are unchanged by the component migration.

## Refresh and loading

The active Builds view refreshes every 15 seconds; other active data and PR
metadata refresh on a 60-second cadence. `F` requests an immediate refresh.
Existing data remains visible if a reload fails. Initial loading uses
`LoadState::{NotRequested, Loading, Loaded, Failed}`.

All fetch entry points register a resource key before spawning work and skip
an already-running fetch of that resource. Completion removes only its own
key; the refresh indicator remains active until the relevant loads settle.
This serializes requests per resource, preventing older overlapping reads from
replacing newer results. If a mutation finishes during an existing read, that
resource is marked for another fetch after the old read settles, so the change
is not missed. Successful mutations refresh activity, PR metadata and
mergeability. Read completions never acknowledge pending mutations.

UI modal detection is shared by keyboard routing and periodic/manual refresh,
including review and merge pickers. Complete provider pagination remains
separate follow-up work.

## Verification and adding behavior

`cargo test --locked` covers provider projections, parsing, component workflows
and rendering. The screen snapshots were captured before migration: all five
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
`components/comment_editor`. `app/state` is not a UI type re-export hub. Screens use
`DetailView` for read-only queries. `AppState::detail_view()` is a test helper;
application effects do not query UI state.

Run `cargo clippy --all-targets --locked -- -D warnings` alongside the tests.
