# tuipr — architecture

tuipr is a terminal pull-request client for GitHub (`gh`) and Bitbucket Data
Center (REST + PAT). It supports reading, commenting, reviewing, merging and
closing/declining PRs.

The application owns shared data and effects. Interactive UI components own
local state, input handling, updates and rendering. Presentation widgets are
ordinary rendering functions; they do not need the component interface.

## Product and interaction principles

tuipr should remain minimalist, easy to understand and comfortable for daily
use as its feature set grows. Polish comes from consistent behavior, clear
hierarchy, restrained styling and reliable feedback. These principles guide
future features and UI changes; they do not imply every current screen already
meets them.

- **Content first.** Give code, diffs and conversations the most space. Keep
  persistent controls and status indicators limited to what helps the current
  task. Adding a feature does not automatically justify another visible control.
- **Reveal actions in context.** Offer relevant actions for the focused item,
  such as replying to a comment or inspecting a build. Put less frequent actions
  behind a consistently placed, clearly labeled actions menu.
- **Keep features discoverable.** Provide a visible route to actions and help;
  shortcuts accelerate that route. Essential functions must not require users
  to guess an undocumented key. Keep contextual hints short and predictable.
- **Use dialogs for focused tasks.** A dialog can give a comment editor or merge
  choice room when needed. Avoid chains of popups and unnecessary confirmations
  that slow routine work. Opening and closing a dialog should preserve context.
- **Use restrained visual emphasis.** Reserve strong colors and emphasis for
  focus, meaningful changes and actionable problems. Use spacing and hierarchy
  to organize information; avoid competing badges, panels and indicators.
- **Keep interaction consistent.** Reuse navigation and selection behavior;
  Enter opens or selects and Esc returns or dismisses the current interaction.
  Text entry must clearly distinguish inserting a newline from sending text.
  Restore focus predictably and make sending, success and failure understandable.
- **Protect continuity.** Preserve work and reading position across ordinary
  interactions. Draft saving should happen automatically when implemented;
  failures should leave the user's work available for recovery.
- **Respect platform support.** Show optional functions only when the adapter
  supports them. Distinguish unsupported features from supported actions blocked
  by the current PR's state, with a concise reason for the latter.

When designing a feature, identify its entry point, what appears only after
interaction, and how the user returns to their work. Review both discoverability
and visual load, including narrow terminals and keyboard-only operation. Prefer
reusing an existing interaction over adding a new visual pattern. Detailed merge
requirements, for example, can open on demand while the main view keeps a brief
status; a longer editor can occupy space only while composing.

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
`app/reviews` (also re-exported by `components/comment_editor`). `app/state` is not a UI type re-export hub. Screens use
`DetailView` for read-only queries. `AppState::detail_view()` is a test helper;
application effects do not query UI state.

Run `cargo clippy --all-targets --locked -- -D warnings` alongside the tests.

## Provider flow verification

Local regression coverage is supplemented by a GitHub live run against
`albinlju/prtest`. See [PR_FLOW_VERIFICATION.md](PR_FLOW_VERIFICATION.md) for
passed flows, defects fixed during the run, evidence and remaining checks.
Bitbucket live verification is pending.
The payload changes follow the [GitHub review-comment API](https://docs.github.com/en/rest/pulls/comments),
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

Support is distinct from permission or PR state: a supported action can remain
visible but disabled with a reason (for example approving your own PR or
merging a closed PR). Profiles currently describe implemented adapter support;
repository permissions and server-version feature discovery are not probed.

## First UI polish pass

The PR list prioritizes the title and progressively omits secondary columns on
narrow terminals; omitted data remains available in PR details. Footer hints
are shown whole when they fit, with space reserved for `?: help`. Decorative
donation/version text no longer occupies the working footer. Help is reachable
from both screens, scrolls independently and closes without changing the
underlying selection or layout. Compact detail tabs show the active tab and
navigation hint when the complete tab bar will not fit. A pending review can
be finished from every tab where its footer advertises that action.

Further work remains: clearer merge requirements and build drilldown, and richer
contextual action discovery. Multiline editing and durable drafts are now implemented.
The subsequent polish pass below addresses compact headers and diff layouts.
These changes establish a calmer baseline without replacing existing review,
comment or lifecycle flows.

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
under `tuipr/drafts/`. Files are versioned and scoped by provider, remote host,
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
Linux uses `xdg-open` and an available `wl-copy`, `xclip` or `xsel`. Clipboard
helpers require an appropriate local graphical session. Failed helpers and
timeouts are reported; there is no unverified terminal clipboard fallback.
Each helper has a five-second deadline. Local tests cover URL mapping, input
routing, missing support and helper success/failure/timeouts without launching
a real browser or changing the user's clipboard. Linux and Windows desktop
integration have not been exercised live. File/line links and copying commit
SHAs or branch names remain separate future work.

## Existing UI polish: dialogs, compact layouts and feedback

- Review, merge, confirmation and filter dialogs share geometry and a separated
  keyboard footer through `widgets/dialog.rs`. Their selected option stays in
  view on short terminals. Review queues show a count and a bounded preview so
  a large queue cannot displace the verdict choices. Modal footers replace
  unrelated screen hints while the dialog is open.
- Compact PR details use two header rows, prioritize title/status/author and
  reclaim unnecessary vertical spacing. Full branch metadata remains in the
  roomy header. A compact tab bar advertises number keys, which actually select
  tabs in every detail context (h/l have other meanings in a diff).
- Diffs under 72 content columns show Files or Code according to focus, keeping
  the selected file when switching back. Wider layouts retain both panels with
  a bounded tree width. Panel titles and focused borders identify the active
  area; the code footer exposes `h: files`. Small panels use a shorter header.
- Context hints distinguish directories, code, replies, resolvable threads and
  queued comments. Thread folding is advertised only for resolved threads, and
  comment targets are cleared when their diff cannot be rendered. Optional
  provider support still gates operations and hints.
- Empty PR/file/commit searches explain how to clear the search or change a
  filter. Initial failures wrap their message and advertise refresh; unrequested
  data is distinct from an active load. Failed reloads retain cached data with
  a scoped warning until the affected resource succeeds. Error dialogs scroll
  long messages and close explicitly with Esc/Enter, preserving the underlying
  editor and selection. The Builds component now owns scrolling so long CI
  lists remain accessible.

Regression coverage includes 24x8 dialogs, 40x12 diff focus switching, large
review queues, long errors, empty searches, stale-target prevention, CI scrolling
and resource-specific refresh recovery. No server writes are required for this
polish verification.

### Visual comparison: original layout with clearer colors

The active presentation restores the original conversation layout from `6edd115`:
frames, timeline rail, spacing, labels and sidebar breakpoint. Focused comment
edges, timeline connectors and reply selection use accent. File locations use
link color separately from the surrounding metadata. Resolved fold status remains
success-colored when focused or expanded; disclosure and metadata use focus color.
Theme palettes and backgrounds remain unchanged.

Variant A is preserved in `docs/ui-examples/reference/variant-a.patch`, relative
to `6edd115`, with its tests and populated snapshot. The accompanying preview is
available for layout comparison. The earlier experiment is preserved separately.
Restore presentation changes selectively when comparing against later work.

### PR detail polish across tabs

Commit rows prioritize the hash and title at compact widths, retain age at medium
widths and show full metadata when there is room. The open-commit banner reserves
space for navigation. Build rows reserve status before duration; the summary drops
its progress bar in narrow views. Diff headers budget for the focused line as well
as stats before shortening the file path, using terminal column widths.

Overview comment rendering reports the selected comment's row range. Navigation
reveals that range once, rather than repeatedly scrolling to the end of a whole
thread. PageUp/PageDown (also Ctrl-u/Ctrl-d) scroll text independently; Ctrl-j/k
select individual comments. Tall comments are revealed from their start.

Description derives heading, link, quote and code colors from the app theme and
keeps the terminal background. This is scoped to Description; conversation
Markdown retains its existing presentation. Regression coverage includes narrow
rows, Unicode paths, long conversations and Description in all three themes.
