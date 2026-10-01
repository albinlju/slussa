pub use crate::app::reviews::CommentDraft;
use crate::{
    app::{
        action::{Action, EditorAction, Effect},
        reviews::CommentTarget,
    },
    tui::{
        component::{Component, saturating_u16},
        components::text_buffer::TextBuffer,
        theme,
    },
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

/// The comment being written on a PR, if there is one.
#[derive(Debug, Default)]
pub struct CommentEditor {
    editing: Option<Editing>,
}

#[derive(Debug)]
struct Editing {
    target: CommentTarget,
    buffer: TextBuffer,
    mode: EditMode,
    /// The draft was put aside or restored before it was shown again, and the
    /// title says so.
    resumed: bool,
    scroll: usize,
}

/// Where a draft is. One at a time: a draft that is put aside cannot also be
/// asking whether to discard it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditMode {
    /// Put aside with Esc or restored from disk: kept, and not on screen.
    Kept,
    Typing,
    ConfirmDiscard,
}

impl CommentEditor {
    /// Start a draft and show it.
    pub const fn start(target: CommentTarget, text: String) -> Self {
        Self {
            editing: Some(Editing {
                target,
                buffer: TextBuffer::new(text),
                mode: EditMode::Typing,
                resumed: false,
                scroll: 0,
            }),
        }
    }

    /// A draft from an earlier session: kept until it is asked for.
    pub fn restored(draft: CommentDraft) -> Self {
        Self {
            editing: Some(Editing {
                target: draft.target,
                buffer: TextBuffer::new(draft.text),
                mode: EditMode::Kept,
                resumed: true,
                scroll: 0,
            }),
        }
    }

    /// Show the draft that was put aside. False when there is none, and then
    /// the caller may start a new one.
    pub const fn resume(&mut self) -> bool {
        let Some(editing) = &mut self.editing else {
            return false;
        };
        editing.resumed = true;
        if matches!(editing.mode, EditMode::Kept) {
            editing.mode = EditMode::Typing;
        }
        true
    }

    /// The draft is gone: it was sent.
    pub fn clear(&mut self) {
        self.editing = None;
    }

    pub const fn is_open(&self) -> bool {
        match &self.editing {
            Some(editing) => !matches!(editing.mode, EditMode::Kept),
            None => false,
        }
    }

    pub const fn has_draft(&self) -> bool {
        self.editing.is_some()
    }

    pub fn target(&self) -> Option<&CommentTarget> {
        self.editing.as_ref().map(|editing| &editing.target)
    }

    #[cfg(test)]
    pub fn text(&self) -> Option<&str> {
        self.editing.as_ref().map(|editing| editing.buffer.as_str())
    }

    /// The draft as it is saved and sent.
    pub fn draft(&self) -> Option<CommentDraft> {
        self.editing.as_ref().map(|editing| CommentDraft {
            target: editing.target.clone(),
            text: editing.buffer.as_str().to_owned(),
        })
    }

    /// Typed or pasted text. Ignored unless the draft is being typed in.
    pub fn insert_text(&mut self, text: &str) {
        if let Some(editing) = &mut self.editing
            && editing.mode == EditMode::Typing
        {
            editing.buffer.insert(text);
        }
    }
}

/// What the editor's frame says besides the draft itself.
#[derive(Debug, Clone)]
pub struct EditorView {
    /// The draft is on its way to the server.
    pub sending: bool,
    /// A review is being drafted, so a line comment is added to it.
    pub review_active: bool,
    /// The comment a reply or an edit is about, as one line.
    pub context: Option<String>,
}

const fn typing_key(key: KeyEvent) -> Option<EditorAction> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    Some(match key.code {
        KeyCode::Char('s') if control => EditorAction::Submit,
        KeyCode::Char('x') if control => EditorAction::Discard,
        KeyCode::Char(c) if !control && !key.modifiers.contains(KeyModifiers::ALT) => {
            EditorAction::Type(c)
        }
        KeyCode::Enter => EditorAction::Type('\n'),
        KeyCode::Left => EditorAction::Move(-1),
        KeyCode::Right => EditorAction::Move(1),
        KeyCode::Up => EditorAction::Vertical(-1),
        KeyCode::Down => EditorAction::Vertical(1),
        KeyCode::Home => EditorAction::Home,
        KeyCode::End => EditorAction::End,
        KeyCode::Delete => EditorAction::Delete,
        KeyCode::Backspace => EditorAction::Backspace,
        KeyCode::Esc => EditorAction::Cancel,
        _ => return None,
    })
}

impl Component for CommentEditor {
    type Input<'a> = ();
    type View<'a> = EditorView;
    type Message = EditorAction;
    fn handle_key(&self, key: KeyEvent, (): &()) -> Option<Action> {
        let action = match self.editing.as_ref()?.mode {
            EditMode::Kept => return None,
            EditMode::Typing => typing_key(key)?,
            EditMode::ConfirmDiscard => match key.code {
                KeyCode::Enter => EditorAction::DiscardConfirm,
                KeyCode::Esc => EditorAction::Keep,
                _ => return None,
            },
        };
        Some(action.into())
    }
    fn update(&mut self, action: EditorAction, (): &()) -> Option<Effect> {
        let editing = self.editing.as_mut()?;
        match action {
            EditorAction::Type(c) => self.insert_text(&c.to_string()),
            EditorAction::Move(delta) => editing.buffer.move_horizontal(delta),
            EditorAction::Vertical(delta) => editing.buffer.move_vertical(delta),
            EditorAction::Home => editing.buffer.home(),
            EditorAction::End => editing.buffer.end(),
            EditorAction::Backspace => editing.buffer.backspace(),
            EditorAction::Delete => editing.buffer.delete(),
            // Esc keeps what was written; there is nothing to keep of an
            // empty draft.
            EditorAction::Cancel if editing.buffer.as_str().is_empty() => self.editing = None,
            EditorAction::Cancel => editing.mode = EditMode::Kept,
            EditorAction::Discard => editing.mode = EditMode::ConfirmDiscard,
            EditorAction::Keep => editing.mode = EditMode::Typing,
            EditorAction::DiscardConfirm => self.editing = None,
            // Sending is the screen's: it knows the PR the draft belongs to.
            EditorAction::Submit => {}
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, view: &EditorView) {
        let Some(editing) = &mut self.editing else {
            return;
        };
        let confirming = match editing.mode {
            EditMode::Kept => return,
            EditMode::Typing => false,
            EditMode::ConfirmDiscard => true,
        };
        let width = area.width.saturating_sub(4).min(88);
        let height = area.height.saturating_sub(2).min(20);
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let title = match &editing.target {
            CommentTarget::Line(a) => format!(" Comment {}:{} ", a.path, a.line),
            CommentTarget::Reply(id) => format!(" Reply to comment #{id} "),
            CommentTarget::Edit(comment) => format!(" Edit comment #{} ", comment.id),
            CommentTarget::Review { verdict } => format!(" {} review ", verdict.label()),
            CommentTarget::Pr => " PR comment ".into(),
        };
        let title = if editing.resumed {
            format!(" Resuming draft · {}", title.trim())
        } else {
            title
        };
        let theme = theme::current();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title)
            .border_style(Style::default().fg(theme.accent));
        let inner = block.inner(popup);
        frame.render_widget(Clear, popup);
        frame.render_widget(block, popup);
        let hints = footer_lines(
            inner.width.saturating_sub(2),
            view.sending,
            confirming,
            match &editing.target {
                CommentTarget::Line(_) if view.review_active => "add to review",
                CommentTarget::Review { .. } => "submit review",
                CommentTarget::Reply(_) => "post reply",
                CommentTarget::Edit(_) => "save changes",
                CommentTarget::Line(_) | CommentTarget::Pr => "post comment",
            },
        );
        let footer_height = saturating_u16(hints.len()).saturating_add(1);
        let mut body = Rect {
            height: inner.height.saturating_sub(footer_height),
            ..inner
        };
        if !confirming
            && body.height > 2
            && let Some(context) = &view.context
        {
            let spans = crate::tui::widgets::truncate_to_width(
                vec![Span::styled(
                    context.clone(),
                    Style::default().fg(theme.muted),
                )],
                body.width as usize,
            );
            frame.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect::new(body.x, body.y, body.width, 1),
            );
            body.y += 2;
            body.height -= 2;
        }
        if body.width == 0 || body.height == 0 {
            return;
        }
        if confirming {
            frame.render_widget(
                Paragraph::new("Discard this draft?").style(Style::default().fg(theme.fg)),
                body,
            );
        } else {
            let (lines, row, col) = editing.buffer.wrapped(body.width as usize);
            if row < editing.scroll {
                editing.scroll = row;
            }
            if row >= editing.scroll + body.height as usize {
                editing.scroll = row + 1 - body.height as usize;
            }
            frame.render_widget(
                Paragraph::new(lines)
                    .style(Style::default().fg(theme.fg))
                    .scroll((saturating_u16(editing.scroll), 0)),
                body,
            );
            if !view.sending {
                frame.set_cursor_position((
                    body.x.saturating_add(saturating_u16(col)),
                    body.y
                        .saturating_add(saturating_u16(row.saturating_sub(editing.scroll))),
                ));
            }
        }
        let footer = Rect::new(inner.x, body.y + body.height, inner.width, footer_height);
        let separator = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(theme.divider))
            .padding(ratatui::widgets::Padding::horizontal(1));
        frame.render_widget(Paragraph::new(hints).block(separator), footer);
    }
}

fn footer_lines(
    width: u16,
    sending: bool,
    discard: bool,
    submit_label: &str,
) -> Vec<Line<'static>> {
    let theme = theme::current();
    let muted = Style::default().fg(theme.muted);
    if sending {
        return vec![Line::from(vec![
            Span::styled("Sending…", Style::default().fg(theme.accent)),
            Span::styled("   Esc back", muted),
        ])];
    }
    let compact = width < 64;
    let editing = [
        (if compact { "^S" } else { "Ctrl+S" }, submit_label),
        ("Esc", "keep draft"),
        ("Enter", "newline"),
        (if compact { "^X" } else { "Ctrl+X" }, "discard"),
    ];
    let hints: &[(&str, &str)] = if discard {
        &[("Esc", "keep editing"), ("Enter", "discard")]
    } else {
        &editing
    };
    let mut lines = Vec::new();
    let mut line = Line::default();
    for (index, &(key, description)) in hints.iter().enumerate() {
        let size = key.len() + 1 + description.len();
        if !line.spans.is_empty() && line.width() + 3 + size > width as usize {
            lines.push(line);
            line = Line::default();
        }
        if !line.spans.is_empty() {
            line.spans.push(Span::styled(" · ", muted));
        }
        line.spans.push(Span::styled(
            key,
            Style::default().fg(if index == 0 { theme.accent } else { theme.fg }),
        ));
        line.spans
            .push(Span::styled(format!(" {description}"), muted));
    }
    lines.push(line);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::action::DetailAction, domain::comment::CommentId};
    fn editor(text: &str) -> CommentEditor {
        CommentEditor::start(CommentTarget::Pr, text.into())
    }
    fn key(editor: &CommentEditor, code: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
        editor.handle_key(KeyEvent::new(code, modifiers), &())
    }
    #[test]
    fn multiline_editing_and_paste_do_not_submit_or_trigger_shortcuts() {
        let mut e = editor("å🦀");
        e.update(EditorAction::Move(-1), &());
        e.insert_text("x\r\ny\tq\u{1b}");
        assert_eq!(e.text(), Some("åx\ny    q🦀"));
        e.update(EditorAction::Delete, &());
        e.update(EditorAction::Home, &());
        e.update(EditorAction::Vertical(-1), &());
        e.insert_text("A");
        assert_eq!(e.text(), Some("Aåx\ny    q"));
        assert!(matches!(
            key(&e, KeyCode::Enter, KeyModifiers::NONE),
            Some(Action::Detail(DetailAction::Editor(EditorAction::Type(
                '\n'
            ))))
        ));
        assert!(matches!(
            key(&e, KeyCode::Char('s'), KeyModifiers::CONTROL),
            Some(Action::Detail(DetailAction::Editor(EditorAction::Submit)))
        ));
    }
    #[test]
    fn escape_keeps_work_and_discard_requires_confirmation() {
        let mut e = editor("keep");
        e.update(EditorAction::Cancel, &());
        assert!(!e.is_open());
        assert_eq!(e.text(), Some("keep"));
        assert!(
            key(&e, KeyCode::Char('x'), KeyModifiers::NONE).is_none(),
            "a draft that is put aside takes no keys"
        );
        assert!(e.resume());
        e.update(EditorAction::Discard, &());
        e.insert_text("ignored");
        assert_eq!(e.text(), Some("keep"));
        assert!(matches!(
            key(&e, KeyCode::Esc, KeyModifiers::NONE),
            Some(Action::Detail(DetailAction::Editor(EditorAction::Keep)))
        ));
        e.update(EditorAction::Keep, &());
        assert!(e.is_open());
        e.update(EditorAction::Discard, &());
        e.update(EditorAction::DiscardConfirm, &());
        assert!(!e.has_draft());
        assert!(!e.resume(), "nothing is left to resume");
    }
    #[test]
    fn an_empty_draft_is_dropped_by_escape_and_a_restored_one_waits_to_be_asked_for() {
        let mut e = editor("");
        e.update(EditorAction::Cancel, &());
        assert!(!e.has_draft());

        let mut e = CommentEditor::restored(CommentDraft {
            target: CommentTarget::Reply(CommentId(7)),
            text: "from last time".into(),
        });
        assert!(e.has_draft() && !e.is_open());
        assert!(e.resume());
        assert!(e.is_open());
        e.insert_text("!");
        assert_eq!(e.draft().unwrap().text, "from last time!");
        assert!(matches!(
            e.target(),
            Some(CommentTarget::Reply(CommentId(7)))
        ));
        e.clear();
        assert!(!e.has_draft());
    }
}
