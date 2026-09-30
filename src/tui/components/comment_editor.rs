pub use crate::app::reviews::CommentDraft;
use crate::{
    app::{
        action::{Action, DetailAction},
        reviews::CommentTarget,
    },
    tui::{component::Component, theme},
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

#[derive(Debug, Default)]
pub struct CommentEditor {
    pub draft: Option<CommentDraft>,
    pub suspended: bool,
    pub resuming: bool,
    pub target_context: Option<String>,
    pub cursor: Option<usize>,
    pub scroll: usize,
    pub discard_confirm: bool,
}
impl CommentEditor {
    pub const fn is_open(&self) -> bool {
        self.draft.is_some() && !self.suspended
    }
    fn position(&self) -> usize {
        let text = self.draft.as_ref().map_or("", |d| d.text.as_str());
        let mut pos = self.cursor.unwrap_or(text.len()).min(text.len());
        while !text.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }
    pub fn insert_text(&mut self, text: &str) {
        if !self.is_open() || self.discard_confirm {
            return;
        }
        let text: String = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\t', "    ")
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect();
        let pos = self.position();
        if let Some(draft) = &mut self.draft {
            draft.text.insert_str(pos, &text);
            self.cursor = Some(pos + text.len());
        }
    }
    fn move_vertical(&mut self, delta: i16) {
        let pos = self.position();
        let Some(draft) = &self.draft else {
            return;
        };
        let text = &draft.text;
        let start = text[..pos].rfind('\n').map_or(0, |i| i + 1);
        let col = text[start..pos].chars().count();
        let target = if delta < 0 {
            if start == 0 {
                return;
            }
            let end = start - 1;
            (text[..end].rfind('\n').map_or(0, |i| i + 1), end)
        } else {
            let Some(end) = text[pos..].find('\n').map(|i| pos + i) else {
                return;
            };
            let next = end + 1;
            (
                next,
                text[next..].find('\n').map_or(text.len(), |i| next + i),
            )
        };
        self.cursor = Some(
            target.0
                + text[target.0..target.1]
                    .char_indices()
                    .nth(col)
                    .map_or(target.1 - target.0, |(i, _)| i),
        );
    }
}
impl Component for CommentEditor {
    type Context<'a> = bool;
    type Message = DetailAction;
    fn handle_key(&self, key: KeyEvent, _: &bool) -> Option<Action> {
        if !self.is_open() {
            return None;
        }
        if self.discard_confirm {
            return match key.code {
                KeyCode::Enter => Some(Action::Detail(DetailAction::CommentDiscardConfirm)),
                KeyCode::Esc => Some(Action::Detail(DetailAction::CommentKeep)),
                _ => None,
            };
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let action = match key.code {
            KeyCode::Char('s') if control => DetailAction::CommentSubmit,
            KeyCode::Char('x') if control => DetailAction::CommentDiscard,
            KeyCode::Char(c) if !control && !key.modifiers.contains(KeyModifiers::ALT) => {
                DetailAction::CommentType(c)
            }
            KeyCode::Enter => DetailAction::CommentType('\n'),
            KeyCode::Left => DetailAction::CommentMove(-1),
            KeyCode::Right => DetailAction::CommentMove(1),
            KeyCode::Up => DetailAction::CommentVertical(-1),
            KeyCode::Down => DetailAction::CommentVertical(1),
            KeyCode::Home => DetailAction::CommentHome,
            KeyCode::End => DetailAction::CommentEnd,
            KeyCode::Delete => DetailAction::CommentDelete,
            KeyCode::Backspace => DetailAction::CommentBackspace,
            KeyCode::Esc => DetailAction::CommentCancel,
            _ => return None,
        };
        Some(Action::Detail(action))
    }
    fn update(&mut self, action: DetailAction, _: &bool) -> Option<Action> {
        let pos = self.position();
        match action {
            DetailAction::CommentType(c) => self.insert_text(&c.to_string()),
            DetailAction::CommentMove(delta) => {
                if let Some(draft) = &self.draft {
                    self.cursor = Some(if delta < 0 {
                        draft.text[..pos]
                            .char_indices()
                            .next_back()
                            .map_or(0, |(i, _)| i)
                    } else {
                        pos + draft.text[pos..].chars().next().map_or(0, char::len_utf8)
                    });
                }
            }
            DetailAction::CommentVertical(delta) => self.move_vertical(delta),
            DetailAction::CommentHome => {
                if let Some(draft) = &self.draft {
                    self.cursor = Some(draft.text[..pos].rfind('\n').map_or(0, |i| i + 1));
                }
            }
            DetailAction::CommentEnd => {
                if let Some(draft) = &self.draft {
                    self.cursor = Some(
                        draft.text[pos..]
                            .find('\n')
                            .map_or(draft.text.len(), |i| pos + i),
                    );
                }
            }
            DetailAction::CommentBackspace => {
                if let Some(draft) = &mut self.draft
                    && let Some((prev, _)) = draft.text[..pos].char_indices().next_back()
                {
                    draft.text.drain(prev..pos);
                    self.cursor = Some(prev);
                }
            }
            DetailAction::CommentDelete => {
                if let Some(draft) = &mut self.draft
                    && pos < draft.text.len()
                {
                    draft.text.remove(pos);
                }
            }
            DetailAction::CommentCancel => {
                self.suspended = true;
                if self.draft.as_ref().is_some_and(|d| d.text.is_empty()) {
                    self.draft = None;
                }
            }
            DetailAction::CommentDiscard => self.discard_confirm = true,
            DetailAction::CommentKeep => self.discard_confirm = false,
            DetailAction::CommentDiscardConfirm => *self = Self::default(),
            other => return Some(Action::Detail(other)),
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, sending: &bool) {
        self.render_with_review(frame, area, *sending, false);
    }
}

impl CommentEditor {
    pub fn render_with_review(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        sending: bool,
        review_active: bool,
    ) {
        if !self.is_open() {
            return;
        }
        let Some(draft) = &self.draft else {
            return;
        };
        let width = area.width.saturating_sub(4).min(88);
        let height = area.height.saturating_sub(2).min(20);
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let title = match &draft.target {
            CommentTarget::Line(a) => format!(" Comment {}:{} ", a.path, a.line),
            CommentTarget::Reply(id) => format!(" Reply to comment #{id} "),
            CommentTarget::Edit { id, .. } => format!(" Edit comment #{id} "),
            CommentTarget::Review { verdict } => format!(" {} review ", verdict.label()),
            CommentTarget::Pr => " PR comment ".into(),
        };
        let title = if self.resuming {
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
            sending,
            self.discard_confirm,
            match &draft.target {
                CommentTarget::Line(_) if review_active => "add to review",
                CommentTarget::Review { .. } => "submit review",
                CommentTarget::Reply(_) => "post reply",
                CommentTarget::Edit { .. } => "save changes",
                _ => "post comment",
            },
        );
        let footer_height = hints.len() as u16 + 1;
        let mut body = Rect {
            height: inner.height.saturating_sub(footer_height),
            ..inner
        };
        if !self.discard_confirm
            && body.height > 2
            && let Some(context) = &self.target_context
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
        if self.discard_confirm {
            frame.render_widget(
                Paragraph::new("Discard this draft?").style(Style::default().fg(theme.fg)),
                body,
            );
        } else {
            let (lines, row, col) = visual_lines(&draft.text, self.position(), body.width as usize);
            if row < self.scroll {
                self.scroll = row;
            }
            if row >= self.scroll + body.height as usize {
                self.scroll = row + 1 - body.height as usize;
            }
            frame.render_widget(
                Paragraph::new(lines)
                    .style(Style::default().fg(theme.fg))
                    .scroll((self.scroll as u16, 0)),
                body,
            );
            if !sending {
                frame.set_cursor_position((
                    body.x + col as u16,
                    body.y + (row - self.scroll) as u16,
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

fn visual_lines(text: &str, cursor: usize, width: usize) -> (Vec<Line<'static>>, usize, usize) {
    let mut lines = vec![String::new()];
    let (mut row, mut col) = (0, 0);
    let mut caret = (0, 0);
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '\0')))
    {
        if c == '\n' {
            if i == cursor {
                caret = if col >= width {
                    (row + 1, 0)
                } else {
                    (row, col)
                };
            }
            lines.push(String::new());
            row += 1;
            col = 0;
            continue;
        }
        let size = Span::raw(c.to_string()).width();
        if col >= width || col + size > width {
            lines.push(String::new());
            row += 1;
            col = 0;
        }
        if i == cursor {
            caret = (row, col);
        }
        if i == text.len() {
            break;
        }
        lines[row].push(c);
        col += size;
    }
    (lines.into_iter().map(Line::raw).collect(), caret.0, caret.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn editor(text: &str) -> CommentEditor {
        CommentEditor {
            draft: Some(CommentDraft {
                target: CommentTarget::Pr,
                text: text.into(),
            }),
            ..CommentEditor::default()
        }
    }
    #[test]
    fn multiline_editing_and_paste_do_not_submit_or_trigger_shortcuts() {
        let mut e = editor("å🦀");
        e.update(DetailAction::CommentMove(-1), &false);
        e.insert_text("x\r\ny\tq\u{1b}");
        assert_eq!(e.draft.as_ref().unwrap().text, "åx\ny    q🦀");
        e.update(DetailAction::CommentDelete, &false);
        e.update(DetailAction::CommentHome, &false);
        e.update(DetailAction::CommentVertical(-1), &false);
        e.insert_text("A");
        assert_eq!(e.draft.as_ref().unwrap().text, "Aåx\ny    q");
        assert!(matches!(
            e.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &false),
            Some(Action::Detail(DetailAction::CommentType('\n')))
        ));
        assert!(matches!(
            e.handle_key(
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
                &false
            ),
            Some(Action::Detail(DetailAction::CommentSubmit))
        ));
    }
    #[test]
    fn escape_keeps_work_and_discard_requires_confirmation() {
        let mut e = editor("keep");
        e.update(DetailAction::CommentCancel, &false);
        assert!(!e.is_open());
        assert_eq!(e.draft.as_ref().unwrap().text, "keep");
        e.suspended = false;
        e.update(DetailAction::CommentDiscard, &false);
        e.insert_text("ignored");
        assert_eq!(e.draft.as_ref().unwrap().text, "keep");
        e.update(DetailAction::CommentKeep, &false);
        assert!(e.is_open());
        e.update(DetailAction::CommentDiscard, &false);
        e.update(DetailAction::CommentDiscardConfirm, &false);
        assert!(e.draft.is_none());
    }
    #[test]
    fn wrapped_unicode_cursor_remains_inside_viewport() {
        let (lines, row, col) = visual_lines("1234\nx", 6, 4);
        assert_eq!(lines.len(), 2);
        assert_eq!((row, col), (1, 1));
        for width in [2, 6, 30] {
            let text = "å🦀long text\nsecond line\n";
            for pos in text
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(text.len()))
            {
                let (lines, row, col) = visual_lines(text, pos, width);
                assert!(row < lines.len());
                assert!(col < width);
            }
        }
    }
}
