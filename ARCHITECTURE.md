# tuipr — architecture

A read-only terminal UI for pull requests, currently speaking two provider
dialects: GitHub (through the `gh` CLI) and Bitbucket Data Center (REST v1
with a PAT). This document walks the code the same way execution does:
from `main` to a frame on screen.

```
main.rs ── cli.rs ── app/preflight.rs        (startup: args, host detection, auth)
   │
   └─ run_tui ── app/mod.rs (event loop)
                    │
       key event ─► tui::key_to_action ─► Action ─► app/reducer/* ─► AppState
                    ▲                                      │
                    │                          spawns app/fetchers.rs
      spinner tick ─┤                                      │
       (while load) │                              providers/* (gh / REST)
       tui::render ◄┴── AppState ◄── LoadedAction ◄────────┘
```

The core idea is one-directional data flow, Elm-style: key presses become
`Action`s, a reducer applies them to a single `AppState`, and rendering is a
pure function of that state. Network results re-enter through the same
action channel, so there is exactly one place state changes.

## Startup

**`main.rs`** does two things: initialise logging (`logging.rs` — a file in
the platform data dir, filtered by `TUIPR_LOG`) and hand `std::env::args` to
`cli::dispatch`. It also owns the tokio runtime, built only when the TUI is
about to start — everything before that point (preflight, auth prompts) uses
blocking I/O (`reqwest::blocking`, `std::process::Command`), which would
panic inside an async context. Keeping the CLI side sync is what makes that
split safe.

**`cli.rs`** is the argument layer: the `-C <dir>` flag, the `auth` and
`keyring-test` subcommands, `--help`, and unknown-command errors. Its
contract with `main` is the `Dispatch` enum — either a subcommand ran to
completion (`Done(ExitCode)`) or the TUI should start against a detected
provider (`RunTui(Provider)`). Parsing is deliberately by hand; two
subcommands don't justify a CLI framework, and the boundary makes swapping
one in later a one-file change.

**`app/preflight.rs`** answers "which provider is this repo on, and are we
logged in?". The git-remote plumbing — `git remote get-url origin` and the
URL→host parsing — lives in `app/remote.rs`; preflight `classify_host`s the
host into a `HostKind` (recognition), and `run` decides what to do with each
(policy):

- `github.com` → check `gh` is installed and authenticated
  (`providers/github/auth.rs`). If not logged in, `cli::resolve_provider` launches
  the interactive `gh auth login` and retries preflight once.
- `bitbucket.org` → `HostKind::BitbucketCloud`, which `run` rejects as
  unsupported (on the roadmap).
- any other host → `bitbucket_dc::is_instance` probes
  `/rest/api/1.0/application-properties`
  (`providers/bitbucket_dc/probe.rs`) to detect a Data Center instance, then
  load its PAT from the OS keyring (`providers/bitbucket_dc/auth.rs`). No PAT →
  the error tells the user to run `tuipr auth login`, which prompts for a
  token, validates it against the same endpoint, and stores it.

Each provider owns its auth in its own module — `providers/github/auth.rs`
(gh delegation: is-installed / is-authenticated / launch-login) and
`providers/bitbucket_dc/auth.rs` (PAT lifecycle: keyring load/store, prompt,
validate). They share no logic — GitHub's credentials live in `gh`,
Bitbucket's in tuipr's keyring — so there's no unified `Provider::login()`;
the orchestration (which provider, resolve host, recover interactively) is
`preflight` + `cli`.

The result is a `providers::Provider` — the only value carried from startup
into the TUI.

## The event loop

**`app/mod.rs`** holds `App { state, provider, action_tx, action_rx }` and the
loop, a `tokio::select!` over three sources. Rendering is event-driven: a
frame is drawn once at startup, then only after something that can change the
view —

- the unbounded action channel → `Action`s, applied by the reducer, then a
  redraw. `Action::Quit` exits the loop; everything else mutates state. This
  is the path both key presses and fetch results funnel through, so it's the
  single redraw trigger for state changes.
- crossterm's `EventStream` → key presses, translated by
  `tui::key_to_action(&state, key)` into an `Action` (sent to the channel, no
  direct redraw); resize events redraw directly.
- a `SPINNER_INTERVAL` (100 ms) sleep, **guarded by `state.is_loading()`** so
  it's only armed while a fetch is in flight — it advances the loading
  spinner. When nothing is loading the branch is disabled and the loop blocks
  purely on events, so an idle tuipr wakes the CPU zero times per second.

Fetchers hold a clone of `action_tx`, which is how background work re-enters
the loop: a fetch completes, sends `Action::Loaded(...)`, the reducer stores
the result and the loop redraws. The UI thread never blocks on the network.

## State (`app/state.rs`)

`AppState` splits into three concerns:

- **`screen`** — where the user is: the PR list, or a PR's detail view with
  an active `DetailTab` (Description / Overview / Diff / Commits / Builds).
- **`cache`** — everything fetched: the PR list plus a `PrData` per opened
  PR (commits, diff, builds, activity, and lazily-fetched per-commit diffs).
  Every entry is a `LoadState<T>`: `NotRequested → Loading → Loaded |
  Failed`. `start_loading` flips to `Loading` only from `NotRequested` or
  `Failed`, which gives both at-most-once fetching and retry-on-reopen after
  an error. The cache is in-memory only and lives for the session — there is
  no refresh yet (it's in FEATURES.md).
- **`ui`** — cursor positions, scroll offsets, collapsed tree dirs, search
  boxes. Two patterns worth knowing:
  - *Viewport feedback*: the `*_viewport` and `pane_item_count`/`pane_matches`
    fields are written by the renderer each frame and read by key handlers
    and the reducer (half-page jump sizes, cursor clamping). The renderer is
    the only thing that knows how tall a view actually was.
  - *`SearchState`*: one generic `/`-search type shared by every searchable
    view (PR list, commit list, file tree, diff pane). Each view contributes
    only a matcher (`matches_pr`, `matches_commit`, …); the editing logic
    lives in one reducer and one key handler.

## Actions and the reducer (`app/action.rs`, `app/reducer/`)

`Action` is a small tree: `List`, `Detail`, `Diff`, `Commits`, `Search`,
`Loaded`, `Quit`. Cursor movements carry a signed delta (`MoveSelection(i16)`)
so one variant serves j/k (±1) and half-page jumps alike.

The reducer is split per concern (`list`, `detail`, `diff`, `commits`,
`search`, `loads`), all as `impl App` blocks. Two routing decisions matter:

- **Which diff view?** The Commits tab can drill into a single commit's
  diff, which reuses the whole Diff-tab machinery against a separate
  `DiffViewState` so the two views don't clobber each other's scroll.
  `reducer/diff.rs` resolves "the active diff view" in one place
  (`diff_view`/`diff_view_mut`) and every `DiffAction` flows through it.
- **Which search?** `Action::Search` carries no target. The reducer
  (`active_search_mut`) and the key handler (`tui::active_search`) both map
  the current screen/tab/focus to the right `SearchState` — the pane's
  search highlights matches (applied on Enter, stepped with `n`/`N`), every
  other search filters its list live.

`reducer/loads.rs` is the landing zone for fetch results: log the outcome,
store `LoadState::from_result` in the cache.

## Fetching (`app/fetchers.rs`)

One generic `spawn_fetch` wraps every call: `tokio::spawn` →
`task::spawn_blocking` (the providers are blocking) → map the result into a
`LoadedAction` and send it. Errors cross the boundary as `String`s — the UI
only ever displays them. Opening a PR kicks off four parallel fetches
(commits, diff, builds, activity); drilling into a commit lazily fetches
that commit's diff, keyed by oid.

## Providers (`providers/`)

`Provider` is an enum, not a trait — with two variants and unconditional
dispatch, `match` is simpler and keeps the fetch signatures honest. Each
provider module mirrors the same surface: `fetch_prs`, `fetch_commits`,
`fetch_diff`, `fetch_commit_diff`, `fetch_builds`, `fetch_activity`.

**Conventions shared by both providers:**

- Each fetch module owns its private serde DTOs and a `map_*` function into
  `domain` types. Wire shapes never leak past `providers/`.
- `fetch_activity` returns an `Activity` — comments, lifecycle events,
  and inline review threads from one conceptual fetch, because that's how
  the Overview consumes them.
- Pure projection functions (payload → domain) carry inline `#[cfg(test)]`
  tests against sample payloads.

**GitHub (`providers/github/`)** shells out to `gh` (`cli.rs` is the
spawn/parse wrapper), inheriting its auth and host handling. Listing and
events use `gh pr list`/`gh pr view`; comments and review threads use
GraphQL via `gh api graphql`, because only GraphQL exposes
`viewerHasReacted` (own-reaction highlighting), `isResolved`, and `diffSide`
(old-side thread anchors). The PR diff arrives as unified-diff text, parsed
by `providers/unified_diff.rs` — the only place raw diff text is interpreted.

**Bitbucket DC (`providers/bitbucket_dc/`)** talks REST v1 with a bearer PAT
(`http.rs` is the shared GET-JSON helper; 401/403 map to a
`NotAuthenticated` error that points the user back to `tuipr auth login`).
Its `/diff` endpoint returns structured JSON rather than diff text —
`json_diff.rs` projects it into the same `domain::Diff`. The whole activity
feed comes from one `/activities` call, split three ways in `activities.rs`.

## Domain (`domain/`)

Small, deliberately so: `PullRequest`, `Commit`, `Diff` (files → hunks →
added/removed/context lines), `Comment`/`ReviewThread`, `TimelineEvent`,
`Build`. The rule is *model what the UI renders* — fields no view reads
don't exist, and there are no serde derives here because nothing serialises
domain types. `comment::split_suggestions` lives here too: separating a
comment's prose from its ```suggestion``` blocks is a data concern both
renderers share.

## TUI (`tui/`)

`tui/mod.rs` exposes the two functions the event loop needs: `render`
(dispatch on `Screen`) and `key_to_action`. Key handling is layered: the
generic `/`-search interception runs first (typing mode captures characters;
`/` opens the active view's search), then the screen's own handler.

- **`screens/pr_list/`** — the table (fixed columns, flexing title), status
  filter picker modal, and list key handling.
- **`screens/pr_detail/`** — header + tab bar + one module per tab:
  `description` (markdown), `overview` (timeline + sidebar), `diff/`
  (file tree + pane), `commits` (list + drill-in banner reusing `diff/`),
  `checks` (builds). `key_to_action` here also resolves the Esc cascade
  (pane → tree → drill-in → list → PR list) and Ctrl+D/U routing.
- **`widgets.rs`** — shared rendering primitives: `boxed` (rounded-border
  boxes built as `Line`s so they can nest inside scrolling paragraphs),
  `framed_panel` (the rounded panel + header band shared by the diff tree
  and pane), `loaded_or_placeholder` (the loading/failed rows every
  fetch-backed tab shares), `empty` (the muted `(no …)` one-liner),
  `scrolled_paragraph` (clamp-scroll + scrollbar +
  viewport write-back), `loading` (the spinner row), `reactions_line`
  (powerline-capped pills; your own reactions get the accent-tinted
  background), search highlighting, diff row builders.
- **`table.rs`** — the declarative table engine for the PR-list grid:
  `Column { title, width: Fixed|Flex }` + `Table::{header, row}` measure,
  truncate, and pad cells so the screen only supplies content per column.
- **`markdown.rs`** — the charmed-glamour pipeline behind `catch_unwind`
  (falls back to raw text): `render` keeps glamour's document margins
  (Description), `render_no_margin` strips them so text sits flush inside
  comment boxes.
- **`format.rs`** — pure text formatting (`relative_age`,
  `truncate_ellipsis`).
- **`theme.rs`** — every color in one struct (`GRUVBOX` is the only theme so
  far). Semantic fields (`accent`, `suggestion`, `diff_added_bg`, …) rather
  than palette names, so views never hardcode a color.

Two rendering mechanics that aren't obvious from the outside:

- Markdown comes back from glamour with a fixed 2-column document margin;
  `markdown::render_no_margin` peels it so comment text sits flush inside boxes.
  Tabs that don't render through glamour are inset to match its indent.
- The diff pane returns its navigable-item count and search-match indices to
  `DiffViewState` *during render* — the reducer clamps and steps the cursor
  against whatever was actually drawn last frame. That's the price of
  keeping rendering stateless about layout; in practice one frame of lag is
  invisible.

## Adding a feature — the usual recipe

1. **New data?** Add the field to `domain` only if a view will render it.
   Extend the provider DTO + `map_*` in each `providers/` module (empty/`None`
   is fine where a provider can't supply it), and thread it through the
   relevant fetcher.
2. **New interaction?** Add an `Action` variant, map a key to it in the
   screen's `key_to_action`, handle it in the matching `reducer/` module,
   and read the new state in the renderer. Update the footer hints.
3. **New visual?** Prefer a `widgets.rs` helper if more than one view will
   want it; take colors from `theme`.
4. Put tests on the pure parts (payload projection, parsing, line
   building) — the render functions are checked by eye, the projections by
   `cargo test`.

FEATURES.md tracks what's done and what's next.
