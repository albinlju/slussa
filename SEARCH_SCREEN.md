# Implementation plan: search/filter view

A step-by-step guide for adding a search screen to tuipr. None of this is implemented
yet — it's a blueprint to follow when you get started.

**Feature:** `/` opens the search screen, you type free text, the PR list filters live,
`Enter` opens the match, `Esc` goes back to the list.

The pattern is TEA (Model = `AppState`, Message = `Action`, Update = `App::apply`,
View = `render`). The point: add a `Screen` or `Action` variant and every `match`
becomes non-exhaustive → a compile error until you handle it. **The compiler is your
checklist** — follow the error messages and you can't miss a spot.

---

## Step 1 — `src/app/state.rs`: the variant + where the local state lives

Add `Search` as a unit variant. Keep `Screen` a cheap `Copy` discriminant (`String`
is not `Copy`, so do *not* embed the query string in the variant — that would force
`Copy` off and ripple through all code that moves `state.screen` around).

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    List,
    Detail { pr_id: u64, tab: DetailTab },
    Search,            // ← new
}
```

Screen-local mutable state lives in the model:

```rust
#[derive(Debug, Default)]
pub struct AppState {
    pub cache: Cache,
    pub selected: usize,
    pub screen: Screen,
    pub search_query: String,   // ← screen-local state lives in the model
}
```

If it grows to many fields per screen: gather them in a `struct SearchState { query, … }`.
A single string is enough here.

---

## Step 2 — `src/tui/mod.rs`: new `Action` variants

```rust
pub enum Action {
    // … existing …
    OpenSearch,
    SearchType(char),
    SearchBackspace,
}
```

`Back` already exists and is reused to leave the screen.

---

## Step 3 — new module `src/tui/search.rs`: render + key_to_action

```rust
use ratatui::{
    Frame, crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style}, text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use crate::{app::state::{AppState, LoadState}, domain::pr::PullRequest, tui::Action};

// shared filtering — used by both render and the Enter handling
fn filtered(state: &AppState) -> Vec<&PullRequest> {
    let q = state.search_query.to_lowercase();
    match &state.cache.prs {
        LoadState::Loaded(prs) => prs
            .iter()
            .filter(|p| p.title.to_lowercase().contains(&q))
            .collect(),
        _ => Vec::new(),
    }
}

pub fn render(frame: &mut Frame, state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let input = Paragraph::new(format!("  {}_", state.search_query))
        .block(Block::default().borders(Borders::ALL).title(" Search "));
    frame.render_widget(input, chunks[0]);

    let items: Vec<ListItem> = filtered(state)
        .iter()
        .map(|p| ListItem::new(Line::from(format!("  #{}  {}", p.id, p.title))))
        .collect();
    let list = List::new(items)
        .highlight_symbol("▶ ")
        .highlight_style(Style::default().fg(Color::Yellow));
    let mut ls = ListState::default();
    ls.select(Some(state.selected));
    frame.render_stateful_widget(list, chunks[1], &mut ls);
}

pub fn key_to_action(state: &AppState, key: KeyCode) -> Option<Action> {
    match key {
        KeyCode::Esc        => Some(Action::Back),
        KeyCode::Backspace  => Some(Action::SearchBackspace),
        KeyCode::Down       => Some(Action::NextPr),
        KeyCode::Up         => Some(Action::PrevPr),
        KeyCode::Enter      => filtered(state).get(state.selected).map(|p| Action::OpenPr(p.id)),
        KeyCode::Char(c)    => Some(Action::SearchType(c)),  // ← all text is captured here
        _ => None,
    }
}
```

**Important:** do *not* map `'q'` to `Quit` here. On a text-input screen, letters are
text — `q` should become a `q` in the query string. That's why `Esc` is the exit, and
navigation goes through the arrow keys (not `j`/`k`, since those become letters). This
is the whole reason `key_to_action` is *per screen* and not global.

---

## Step 4 — `src/tui/mod.rs`: wire it into the two dispatch matches

```rust
pub mod search;   // at the top

pub fn render(frame: &mut Frame, state: &AppState) {
    match state.screen {
        Screen::List   => pr_list::render(frame, state, frame.area()),
        Screen::Detail { pr_id, tab } => pr_detail::render(frame, state, pr_id, tab, frame.area()),
        Screen::Search => search::render(frame, state, frame.area()),   // ←
    }
}

pub fn key_to_action(state: &AppState, key: KeyCode) -> Option<Action> {
    match state.screen {
        Screen::List      => pr_list::key_to_action(state, key),
        Screen::Detail { .. } => pr_detail::key_to_action(state, key),
        Screen::Search    => search::key_to_action(state, key),         // ←
    }
}
```

---

## Step 5 — `src/app/mod.rs`: handle the new actions in `apply`

```rust
Action::OpenSearch => {
    self.state.screen = Screen::Search;
    self.state.search_query.clear();
    self.state.selected = 0;
}
Action::SearchType(c) => {
    self.state.search_query.push(c);
    self.state.selected = 0;     // new filtering → start back at the top
}
Action::SearchBackspace => {
    self.state.search_query.pop();
    self.state.selected = 0;
}
```

`Back` already exists and sets `screen = Screen::List` — reused as-is.
`NextPr`/`PrevPr` work unchanged (they only touch `self.state.selected`).

---

## Step 6 — the entry point: how you get *into* the screen

One line in `pr_list::key_to_action`:

```rust
KeyCode::Char('/') => Some(Action::OpenSearch),
```

---

## Step 7 — the punchline: the compiler led you all the way

Note you never had to *hunt* for the spots to change. `Screen::Search` → the compiler
pointed at `render` and `key_to_action`. Three new `Action` variants → the compiler
pointed at `apply`. An exhaustive checklist for free, and it's impossible to forget a
branch.

---

## Known edge to polish later

`selected` is shared across screens and now indexes a *filtered* list. The plan above
resets it on every search change, which is correct. But `NextPr`/`PrevPr` still clamp
against the **unfiltered** length (`cache.prs.len()` in `apply`). The fully correct fix
would clamp against the number of matches. Two clean paths:

- have `NextPr`/`PrevPr` in `apply` compute the length based on `state.screen`
  (in `Search` → `filtered(...).len()`), or
- give the search screen its own `NextMatch`/`PrevMatch` actions.

Not a blocking bug — just an edge to sand down.
