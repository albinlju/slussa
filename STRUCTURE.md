# Folder structure: review and target

A working note from 2026-10-02, to be deleted. It reviews `src/` against
`docs/ROADMAP.md` and proposes a target structure. Nothing here has been
changed in the code.

## Verdict

The structure is clear and fits the product as it is today. It is built for
one front end, the TUI. The roadmap adds a second one, the agent-facing CLI,
which is part of what slussa is to be, and more locally saved state. The
structure has no place for either. The fix is a handful of moves, not a
rewrite, and each move waits for the feature that needs it.

## What it is worth

Said plainly, since most of this note is moves and not fixes:

| What | Worth |
|---|---|
| The import rule in `tests/repo_rules.rs` | An improvement today. The layers are held by a test instead of by review. |
| The key table with three outcomes | An improvement today. It removes a real source of errors: the help has already drifted from the keys. |
| Nesting `app` and `ui` under `tui/` | Nothing in behaviour or safety. It is what makes the top level readable, and it is the widest move. |
| The splits of `action.rs`, `store.rs`, `widgets/mod.rs`, `test_support.rs` | Forced by the size rule anyway. The note says where the cut goes. |
| The shape of `FetchKey`, the draft types to `domain`, `widgets/comment/` | Small and right. |

For reading the tree, the target is a clear improvement:

- **The top level reads as one sentence**: two front ends, what they share,
  and the core. Today `app` looks like the whole program, and nothing shows
  that there are two front ends.
- **"Where does this go?" is answered by the tree.** A new subcommand goes in
  `cli/`, new saved state in `local/`, connection and account in `session/`.
  Today the answer to all three is "somewhere in `app/`".
- **One non-negotiable is visible as directories**: `app` does I/O, `ui`
  draws.

What the tree does not show, today or in the target, is the features. It says
which layer something is in, not where "resolve thread" lives; see *Features
across the layers*.

## What works

- **The layers hold.** `domain` imports nothing else, and `providers` imports
  only `domain` and `git_url`. No import goes the wrong way in non-test code.
- **`tui/screens/` has exactly `pr_list` and `pr_detail`.** The two-views rule
  is visible in the tree, so a third view would be noticed at once.
- **The providers are symmetric.** Both have the same file names
  (`activities`, `builds`, `comments`, `commits`, `diff`, `prs`).
- **The test convention is followed.** A `tests/` directory has a file per
  concern and a `support.rs`.
- **Only `fetchers.rs` can start a provider call.** `spawn_read` and
  `spawn_write` are private to it, so the rule that provider calls leave the UI
  thread through one path is held by visibility.

## Weaknesses against the roadmap

1. **`app/` is both the TUI's application layer and a box for everything that
   is not UI.** `cli.rs` already imports `app::preflight` and `app::remote`.
   The planned subcommands (`list --json`, `blocked`, `threads`, `context`,
   `draft`, `review import`, `agent-instructions`) need `Session` and
   `Provider` but not `App`, and `cli.rs` is one file of 128 lines.
2. **Local state has no place of its own.** `app/drafts.rs` (448 lines) mixes
   file mechanics, the scope, the snapshot and `App::open`. The roadmap adds
   *Unread*, *Viewed files*, the remembered `s` choice and the proposal inbox.
   A headless process writes the inbox, so it cannot live in `app/`.
3. **The draft types sit in the TUI's engine.** `CommentTarget`,
   `PendingReview`, `PendingComment` and `CommentDraft` (`app/reviews.rs`) are
   the working model of a review: about ten files in `tui` use them, and so do
   `store`, `commands` and `fetchers`. The inbox and the CLI need them without
   `App`. `CommentAnchor`, which they carry, is already in `domain/review`.
4. **`app/action.rs` (466 lines) breaks at the next key.** It holds both the
   UI's messages (`Action` and its 14 sub-enums) and the application's work
   (`Effect`, `Command`, `TaskResult`, `Read`, `WriteError`). `App` only
   creates `Action::Paste` and passes the rest on, so `Action` belongs to the
   UI. `Action` wraps `Effect` (`Action::Effect`), so the UI's file imports the
   engine's and not the other way; the UI's imports of `Effect` and `Command`
   change path and stay.
5. **What the TUI and the CLI share has no home.** The context package for
   *Send to agent* and `slussa context` is to be built once, so it can live in
   neither `app/` nor `cli/`.
6. **`tests/provider_conformance.rs` cannot be written as the roadmap says.**
   The crate has no `lib.rs` and `test_support` is `#[cfg(test)]`, so an
   integration test reaches neither the providers nor `FakeGh`. The test
   belongs in `src/providers/`.
7. **`app` and `tui` are unclear as names.** `app` sounds like the whole
   program but is only the TUI's engine, and `tui` is only its surface. They
   are one unit in practice: `AppState` owns `Ui`, the event loop calls
   `tui::render` and `tui::key_to_action`, and `tui` reads `Store` and returns
   `Effect`. As siblings of `cli` at the top level they make the tree look as
   if it had three front ends.
8. **Nothing holds the direction of the imports.** It is one crate, so the
   compiler accepts an import from `domain` into `tui`. The layers hold today
   because review holds them.
9. **The same rule is written several times inside a layer.** When `R` is
   offered is said in four ways: `keys.rs` has no condition, `dialogs/help.rs`
   and `supports_action` in `view.rs` ask for the provider feature, and
   `footer.rs` asks twice for the feature and a focused thread that can be
   resolved. They have already drifted: the help says "(Overview)" for `a`,
   `m` and `x`, while `keys.rs` and `docs/KEYS.md` say they work from the
   Description too. The test compares the names of the keys, not where they
   work.
10. **A few files are touched by nearly every feature.** Of the 258 commits
    that touch `src/`, 59 touch `app/action.rs`, 43 `pr_detail/keys.rs` and 40
    `app/fetchers.rs`. They are the files at the size limit.
11. **Smaller things, all near the 500-line limit:**
    - `tui/widgets/mod.rs` (480 lines) implements instead of composing.
    - `comment.rs`, `comment_code.rs`, `comment_fold.rs`, `comment_frame.rs`
      and `comment_meta.rs` are a directory waiting to happen, and the AI
      features land there.
    - `app/store.rs` is 491 lines and `test_support.rs` 464.
    - "Remote" is in three places: `git_url.rs`, `app/remote.rs` and
      `bitbucket_dc/remote.rs`.
    - `TerminalGuard` is in `main.rs`, while suspend and resume is planned for
      `desktop.rs`, which is the browser and the clipboard.

## Target structure

`←` is moved from the place named, `+` is new when the feature is built, and
an unmarked line stays where it is.

```text
src/
├── main.rs                 logging, dispatch → cli or tui
├── logging.rs
├── config.rs               becomes config/ with agent commands and path rules
├── private_file.rs         leaf: used by logging and local
├── git_url.rs              leaf: used by providers, session and local
├── doc_contract.rs         test-only
│
├── cli/                    FRONT END 1: prints and exits
│   ├── mod.rs              arguments, Dispatch                    ← cli.rs
│   ├── auth.rs             `auth login`                           ← cli.rs
│   ├── list.rs             `list --json`                          +
│   ├── blocked.rs          `blocked <number>`                     +
│   ├── threads.rs          `threads <number>`, `--since`          +
│   ├── context.rs          `context <number>`                     +
│   ├── json.rs             output types, "schema": 1              +
│   ├── exit.rs             exit codes from kind()                 +
│   └── …                   a file per later subcommand (`draft`,
│                           `review import`, `agent-instructions`) +
│
├── tui/                    FRONT END 2: the interactive program
│   ├── mod.rs              run_tui                                ← main.rs
│   ├── app/                the engine: does I/O, never draws      ← src/app/
│   │   ├── mod.rs
│   │   ├── event_loop.rs   App, the event loop, effect dispatch
│   │   ├── state.rs        AppState (Store, Ui, Screen)
│   │   ├── store.rs        cache, LoadState, tickets, FetchKey
│   │   ├── notice.rs
│   │   ├── navigation.rs   Screen, open_pr
│   │   ├── effect.rs       Effect, TaskResult, Read, WriteError   ← action.rs
│   │   ├── commands.rs     Command, supported_by, execute         (Command ← action.rs)
│   │   ├── fetchers.rs     spawn_fetch, spawn_load_*, spawn_*: the only
│   │   │                   file that starts a provider call
│   │   ├── pr_groups.rs    reading the list: groups, pages,
│   │   │                   OpenChain, `L`          ← store.rs, fetchers.rs, loads.rs
│   │   ├── loads.rs        applies a TaskResult
│   │   ├── refresh.rs
│   │   ├── drafts.rs       App::open, restore, save, checkpoint   ← app/drafts.rs (the App part)
│   │   ├── desktop.rs      browser and clipboard
│   │   ├── terminal.rs     TerminalGuard, later run_in_terminal   ← main.rs
│   │   ├── tests/
│   │   └── flow_tests/
│   └── ui/                 the surface: draws, never starts I/O   ← src/tui/
│       ├── mod.rs          Ui, render, key_to_action
│       ├── action.rs       Action and its sub-enums               ← app/action.rs
│       ├── component.rs    the Component contract
│       ├── theme.rs, icons.rs, layout.rs, format.rs
│       ├── components/     with state of their own
│       │   ├── comment_editor.rs, text_buffer.rs
│       │   ├── search_input.rs, help_dialog.rs
│       │   └── diff_viewer/   viewer, keys, nav, render, tree, file_tree, pane, threads
│       ├── screens/
│       │   ├── pr_list/    screen, render, columns, filter, tests
│       │   └── pr_detail/
│       │       ├── screen.rs, interactions.rs, view.rs, render.rs
│       │       ├── bindings.rs   one row per key: key, where, offer,
│       │       │                 action, help, hint       ← keys.rs, dialogs/help.rs
│       │       ├── keys.rs       modal priority and routing to the children
│       │       ├── header.rs, footer.rs, build_status.rs, tests.rs
│       │       ├── dialogs/   confirm, review, merge, error, pr_summary
│       │       └── tabs/
│       │           ├── description.rs, commits.rs, builds.rs
│       │           └── overview/   timeline, blocks, hidden, sidebar
│       ├── widgets/        without state
│       │   ├── mod.rs      composition only
│       │   ├── comment/    mod, code, fold, frame, meta           ← comment*.rs
│       │   ├── text.rs     width, truncation, wrapping            ← mod.rs
│       │   ├── footer.rs   Hint, footer, search prompt            ← mod.rs
│       │   ├── panel.rs    frame, empty state, spinner, scrollbar ← mod.rs
│       │   └── dialog.rs, markdown.rs, table.rs
│       ├── regression_tests/
│       └── testdata/
│
├── session/                SHARED: who and where
│   ├── mod.rs              Session, connect                       ← app/preflight.rs
│   ├── preflight.rs        host classification, PreflightError    ← app/preflight.rs
│   └── remote.rs           origin_url, parse_host                 ← app/remote.rs
│
├── local/                  SHARED: what slussa keeps on disk
│   ├── scope.rs            provider/host/repo/account → directory ← app/drafts.rs
│   ├── file.rs             version, atomic write, lock            ← app/drafts.rs
│   ├── drafts.rs           Snapshot, DraftStorage, the v1 fixture ← app/drafts.rs
│   ├── seen.rs             Unread, Viewed files                   +
│   └── proposals.rs        the agent inbox                        +
│
├── handoff.rs              SHARED: the text package for an agent
│                           (TUI and `slussa context`)             +
│
├── domain/                 flat, no dependencies
│   ├── pr, comment, commit, diff, user, ci, event, activity
│   ├── review.rs           gains CommentTarget, PendingReview,
│   │                       PendingComment, CommentDraft           ← app/reviews.rs
│   ├── capabilities, attention, authorship
│   └── risk, findings, threads (thread assembly)                  +
│
├── providers/              domain + trait + leaves, as the roadmap says
│   ├── mod.rs              Provider
│   ├── error.rs, unified_diff.rs
│   ├── github/
│   ├── bitbucket_dc/
│   ├── transport_tests/
│   └── conformance_tests.rs                                       +
│
└── test_support/           gh.rs, http.rs, fixtures.rs            ← test_support.rs

tests/
└── repo_rules.rs           gains the rule for import directions   +
```

Dependencies go one way:

```text
domain ← providers ← session, local, handoff ← cli
                                             ← tui (app ↔ ui)
```

`git_url` and `private_file` are leaves: they import nothing of ours and may be
imported from anywhere.

Inside the shared level, `local` imports `session` and nothing goes the other
way: the scope of a saved file is built from `Session` (`drafts.rs` takes it
today), so `session/` has to exist before `local/` does.

`config` and `logging` stand outside the chain. `main` is the only one that
uses them today, `config` imports nothing of ours and `logging` only
`private_file`. They stay that way: a front end may use them, the shared level
and the core may not.

Everything left of the front ends is what a future `slussa-core` would
contain, so that split becomes a move of directories if it is ever needed.

## `tui/app` and `tui/ui`

`cli` and `tui` are the two front ends, and the top level should say so. With
`app` and `tui` as siblings of `cli`, two directories carry the same front end
and the tree has to be explained.

The boundary between the two halves is worth keeping, because it carries one
of the non-negotiables ("nothing started from rendering or key handling") as
directories:

- **`app`** does I/O and never draws: the event loop, `Store`, effects,
  fetchers.
- **`ui`** draws and takes keys but never starts I/O: components, screens,
  widgets.

The two halves import each other, today and in the target, so the boundary is
not a direction:

- **`app` uses `ui`** to own it and drive it: `Ui` in `AppState`, `render` and
  `key_to_action` in the event loop, `DetailView` and `ListContext` to hand
  the surface its state, and `DetailTab` in `navigation.rs`, `refresh.rs`,
  `store.rs` and `action.rs` to know what is on screen and so what to read.
- **`ui` uses `app`** to read and to answer: `Store`, `LoadState` and `Notice`
  to draw from, `Effect` and `Command` to return. Its imports of the draft
  types and of `Action` go away with the moves to `domain/review.rs` and
  `ui/action.rs`.

What holds the boundary is what `ui` may reach. In non-test code it imports
only `app` and `domain`, never `providers`, and that is already true today. It
cannot start a provider call without importing one, so this is the rule the
import test holds for it.

The names match the types that already exist (`App`, `AppState`, `Ui`), so no
type is renamed. `crate::tui::ui::widgets` reads a little doubled; that is the
price of not renaming. The tree is one level deeper.

The CLI needs nothing in `tui/` once `preflight` and `remote` have left for
`session/`: those two are all `cli.rs` imports from `app` today. The storage
half of `drafts.rs` can stay in `tui/app/` until `local/` is made, since only
the TUI uses it before the inbox exists.

## Decisions that differ from the first draft

- **`git_url.rs` stays a leaf.** `providers/bitbucket_dc/remote.rs` uses
  `git_url::split` and `web_base`. Folded into `session/remote.rs` it would
  make `providers` import `session`, against the direction above.
- **The draft types go to `domain/review.rs`, not to `local/`.** They are the
  model the editor, the store and the commands work on; in `local/` the UI
  would import its model from the storage layer. `local/drafts.rs` owns the
  file and its format only. This makes "a draft" a domain concept. If that
  turns out wrong, the alternative is a `review/` module of its own between
  `domain` and `local`.
- **`handoff` is one file.** The configured agent command belongs to `config`
  and running it to `tui/app/terminal.rs`; only the text package is shared. It
  becomes a directory when a second file is needed.
- **The import directions get a test.** `tests/repo_rules.rs` already stops a
  file over 600 lines; the same test file reads the `crate::` paths and fails
  an import that goes the wrong way. The rules it holds:
  - `domain` imports nothing of ours.
  - `providers` imports only `domain` and the leaves.
  - `ui` (`tui` today) imports only `app` and `domain`.
  - Once they exist: `session`, `local` and `handoff` import neither front
    end, and `cli` never imports `tui`.

  It is more than a search for `crate::name`. Most imports are grouped
  (`use crate::{app::…, tui::…}`), so the test reads the first segment of each
  branch of the tree, and paths written inline in a function body
  (`app/state.rs`) count too. `test_support` is allowed everywhere:
  `providers/github/cli.rs` and `logging.rs` use it under `#[cfg(test)]` in
  files that are not test files, and a rule that skips only test files would
  fail on them.

## `drafts.rs` in three parts

- The file mechanics (`DraftStorage`, `Snapshot`, the envelope, the scope, the
  version 1 fixture) go to `local/`.
- The types written to the file go to `domain/review.rs`.
- The `impl App` part (`App::open`, `restore`, `save_drafts`,
  `checkpoint_submission`) stays in `tui/app/drafts.rs`, or `local/` would
  have to know `App`.

## Features across the layers

The question was whether a feature is scattered over the project, as in MVC,
and whether it should be kept together instead.

**It is scattered, and the folders above do not change that.** "Resolve
thread" lives in 14 files: `domain` (2), `providers` (3), `app` (3) and `tui`
(6). The checklist for a new provider write in `docs/ARCHITECTURE.md` names
about ten places.

**Folders per feature do not fit here.** The features are not independent:
they are all operations on one PR in the same two views, and they share the
`Store`, the tickets and the key routing. The provider boundary is worth more
as a unit than the feature is, and the layer boundary carries a
non-negotiable. A feature needs a domain concept, two provider
implementations, a write and a key, so it cannot go under about ten files.
What can go is the same rule written several times inside one layer, and the
files every feature has to touch.

Three ideas were tried against the code, without writing any:

| Idea | Holds? | What the code showed |
|---|---|---|
| One row per key (`bindings.rs`) | Yes, for most of it | About 20 of the help's 28 rows are keys the PR screen routes itself. The rest (`j`/`k`, `enter`, `/`, `n`/`N` …) are routed by the child components and become rows without an action. |
| A file and a trait per write command | No | It fits 5 of 10 commands. `StartReview`, `AbandonReview` and `RemovePendingComment` are not writes; `SubmitComment` ends in three ways depending on the `Store`; `SubmitReview` reads the `Store` to build what it sends; and `Operation` has to travel with the ticket anyway. |
| A file and a trait per read resource | No | `Read` has to be one type for the task channel, and `FetchKey` a value that can be hashed, in three sets. A trait per resource needs `Box<dyn>` and loses the exhaustive match. |

What came out of it:

- **`bindings.rs`, with three outcomes.** A row says whether its key is
  *hidden* (the provider does not have it), *blocked* with a reason (the PR's
  state) or *offered*. That is *Show only what the provider supports* as a
  type; a `bool` cannot tell hidden from blocked, and the footer would still
  ask a second function. Key handling acts only on *offered*, the footer shows
  *offered* and dims *blocked* with its reason, and the help lists what is not
  *hidden*. `v`, `x` and `d` give a different action by state, so a row's offer
  and action are a function of the view. Two things stay as code: the modal
  priority at the top of `keys.rs`, and `footer.rs` deciding which hints each
  surface shows.
- **`pr_groups.rs`.** Reading the PR list (groups, pages, `OpenChain`, `L`)
  is a feature of its own, about 310 lines spread over `fetchers.rs`,
  `loads.rs` and `store.rs`. It moves together with no change of design,
  since `impl App` is already spread over several files. It is the split of
  `store.rs` by concern that the size rule asks for.
- **`Command` and `supported_by` go to `commands.rs`**, beside `execute`,
  when `action.rs` is split.
- **`FetchKey::Pr(resource, PrId)`** for the six per-PR resources.
  `store.rs` has two arms, `FetchKey::Prs(_) => false` in `start_loading` and
  `has_cached_data`, that cannot be reached because the function has already
  returned for that variant. With the new shape they cannot be written.

What was dropped:

- **The traits**, for the reasons in the table.
- **Moving the writes' `spawn_*` out of `fetchers.rs`.** `spawn_read` and
  `spawn_write` are private there, which is what holds the rule about leaving
  the UI thread. They stay.
- **Splitting `Command` into local and sent.** There is no state to make
  impossible. It may come back when the inbox adds more local commands.
- **One description for the six per-PR resources** (`resources.rs`). "Builds"
  is in 20 places in six files of the engine, 10 of them in `store.rs`, and
  six of the eight resources are the same code. It is the only idea that needs
  a design, so it waits for the next read resource; the shape of `FetchKey` is
  the part with a rule behind it.

With all of it, "resolve thread" goes from 14 files to 13. The gain is not in
the number of files: the rule for a key is written once instead of four
times, and `action.rs`, `keys.rs` and `store.rs` stop growing with every
feature.

## Order and triggers

Each move waits for the feature that needs it, as the roadmap's rule says. The
import rule is tooling, which the same rule says to do now.

| Move | Trigger |
|---|---|
| `handoff.rs`, `app/terminal.rs` | *Send to agent* |
| `providers/conformance_tests.rs` | The provider trait |

Nesting `app` and `ui` is mechanical but wide: nearly every file changes its
import paths (`crate::app::` → `crate::tui::app::`, `crate::tui::` →
`crate::tui::ui::`). It should be its own PR with no change in behaviour, done
together with `cli/`, and with `session/` if that has not moved already, when
the top level moves anyway.

`session/` has two triggers because `local/scope.rs` is built from `Session`.
*Unread / updated* is in the roadmap's first section and the headless commands
in its fourth, so `local/` is likely to come first. With `Session` still in
`app/preflight.rs`, `local` would import the TUI's engine, against the
direction above. The move is small: `preflight.rs` and `remote.rs`, 266 lines.

The splits are likely to come before the nesting too, since their triggers are
the next key and the size limit. Until `tui/app` and `tui/ui` exist they land
in today's paths: `tui/action.rs`, `app/effect.rs`, `app/pr_groups.rs`,
`tui/screens/pr_detail/bindings.rs`. Those files then move once more with the
nesting, which changes their paths and nothing in them.

## To keep in mind when moving

- **The draft file's spelling on disk does not change.** Only module paths
  move. The version 1 fixture follows the storage to `local/drafts.rs`, the
  types keep their serde attributes in `domain/review.rs`, and the fixture
  must keep passing.
- **The clippy rule moves with the module.** `wildcard_enum_match_arm` is set
  in `app/mod.rs`, `domain/mod.rs` and `providers/mod.rs`; `session/`,
  `local/` and `handoff.rs` should get it too.
- **`spawn_read` and `spawn_write` stay private to `fetchers.rs`.** A move
  that needs them from another file is the wrong move.
- **The help and `docs/KEYS.md` are held together by a test**
  (`src/doc_contract.rs`). It follows the table to `bindings.rs`, and can then
  compare where a key works as well as its name.
- **Docs name the directories.** `AGENTS.md` (the draft types in
  `src/app/reviews.rs`, the fixture in `src/app/drafts.rs`) and
  `docs/ARCHITECTURE.md` (the code map, the checklists, *Import types from
  their owners*) refer to `app` and `tui` by path and need updating in the
  same PR.
- **The roadmap names three things that change.** *Conformance test* says
  `tests/provider_conformance.rs`, which cannot reach the providers;
  *Suspend / resume* says `app/desktop.rs`; and *Keybinding table* has
  `gate: fn(..) -> bool`. They become `src/providers/conformance_tests.rs`,
  `tui/app/terminal.rs` and a gate with three outcomes.
- **This note is to be deleted.** The table above belongs under *Engineering*
  in `docs/ROADMAP.md`, each row as an item with its trigger.
