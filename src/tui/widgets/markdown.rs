use ratatui::text::Line;

const GLAMOUR_MARGIN: usize = 2;

pub(in crate::tui) fn render(body: &str, width: u16) -> Vec<Line<'static>> {
    let source: Vec<_> = body.lines().collect();
    let mut output = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut fence: Option<&str> = None;
    while i < source.len() {
        let trimmed = source[i].trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker = &trimmed[..3];
            if fence == Some(marker) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(marker);
            }
        }
        if fence.is_none()
            && i + 1 < source.len()
            && source[i].contains('|')
            && table_separator(source[i + 1])
        {
            output.extend(render_glamour(
                &source[start..i].join("\n"),
                width,
                description_style(crate::tui::theme::current()),
            ));
            let table_start = i;
            i += 2;
            while i < source.len() && source[i].contains('|') && !source[i].trim().is_empty() {
                i += 1;
            }
            let mut columns = Vec::<usize>::new();
            for row in &source[table_start..i] {
                for (index, cell) in row.trim().trim_matches('|').split('|').enumerate() {
                    if columns.len() <= index {
                        columns.resize(index + 1, 0);
                    }
                    columns[index] = columns[index].max(ratatui::text::Span::raw(cell).width());
                }
            }
            let table_width = columns.iter().sum::<usize>() + columns.len() * 4 + 8;
            let render_width = width.max(table_width.min(u16::MAX as usize) as u16);
            output.extend(render_glamour(
                &source[table_start..i].join("\n"),
                render_width,
                description_style(crate::tui::theme::current()),
            ));
            start = i;
        } else {
            i += 1;
        }
    }
    output.extend(render_glamour(
        &source[start..].join("\n"),
        width,
        description_style(crate::tui::theme::current()),
    ));
    trim_blank_lines(output)
}

fn table_separator(line: &str) -> bool {
    let cells: Vec<_> = line.trim().trim_matches('|').split('|').collect();
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let cell = cell.trim().trim_matches(':');
            cell.len() >= 3 && cell.chars().all(|ch| ch == '-')
        })
}

pub(in crate::tui) fn render_no_margin(body: &str, width: u16) -> Vec<Line<'static>> {
    let lines = render_glamour(
        body,
        width.saturating_add(GLAMOUR_MARGIN as u16),
        glamour::Style::Dark.config(),
    );
    trim_blank_lines(strip_margin(lines, GLAMOUR_MARGIN))
}

fn render_glamour(body: &str, width: u16, style: glamour::StyleConfig) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::default()];
    }
    let rendered = std::panic::catch_unwind(|| {
        let ansi = glamour::Renderer::new()
            .with_style_config(style)
            .with_word_wrap(width as usize)
            .render(body);
        ansi_to_tui::IntoText::into_text(&ansi).map(|text| text.lines)
    });
    let lines = match rendered {
        Ok(Ok(lines)) => lines,
        _ => body.lines().map(|l| Line::raw(l.to_string())).collect(),
    };
    if lines.is_empty() {
        vec![Line::default()]
    } else {
        lines
    }
}

fn strip_margin(lines: Vec<Line<'static>>, n: usize) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|mut line| {
            if let Some(first) = line.spans.first_mut() {
                let trimmed: String = first.content.chars().skip(n).collect();
                first.content = trimmed.into();
            }
            line
        })
        .collect()
}

fn trim_blank_lines(mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    while lines.first().is_some_and(is_blank_line) {
        lines.remove(0);
    }
    while lines.last().is_some_and(is_blank_line) {
        lines.pop();
    }
    lines
}

fn is_blank_line(line: &Line<'static>) -> bool {
    line.spans.is_empty() || line.spans.iter().all(|s| s.content.trim().is_empty())
}

fn description_style(theme: &crate::tui::theme::Theme) -> glamour::StyleConfig {
    let mut style = glamour::Style::Dark.config();
    for block in [
        &mut style.document,
        &mut style.paragraph,
        &mut style.code_block.block,
    ] {
        block.style.color = ansi_color(theme.fg);
        block.style.background_color = None;
    }
    for block in [
        &mut style.heading,
        &mut style.h1,
        &mut style.h2,
        &mut style.h3,
        &mut style.h4,
        &mut style.h5,
        &mut style.h6,
    ] {
        block.style.color = ansi_color(theme.decorative);
        block.style.background_color = None;
        block.style.bold = Some(true);
    }
    style.code.style.color = ansi_color(theme.info);
    style.code.style.background_color = None;
    style.block_quote.style.color = ansi_color(theme.muted);
    style.horizontal_rule.color = ansi_color(theme.divider);
    for primitive in [&mut style.link, &mut style.link_text, &mut style.image] {
        primitive.color = ansi_color(theme.link);
    }
    style.image_text.color = ansi_color(theme.muted);
    style
}

fn ansi_color(color: ratatui::style::Color) -> Option<String> {
    use ratatui::style::Color;
    Some(match color {
        Color::Reset => return None,
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(index) => index.to_string(),
        Color::Black => "0".into(),
        Color::Red => "1".into(),
        Color::Green => "2".into(),
        Color::Yellow => "3".into(),
        Color::Blue => "4".into(),
        Color::Magenta => "5".into(),
        Color::Cyan => "6".into(),
        Color::Gray => "7".into(),
        Color::DarkGray => "8".into(),
        Color::LightRed => "9".into(),
        Color::LightGreen => "10".into(),
        Color::LightYellow => "11".into(),
        Color::LightBlue => "12".into(),
        Color::LightMagenta => "13".into(),
        Color::LightCyan => "14".into(),
        Color::White => "15".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::{CATPPUCCIN, GRAPHITE, GRUVBOX, SLATE, TERMINAL};
    use ratatui::style::Color;

    #[test]
    fn long_markdown_content_remains_accessible_in_narrow_views() {
        let sample = format!(
            "```\n{}END_CODE\n```\n\n| Key | Value |\n| --- | --- |\n| path | {}END_CELL |\n\n[Docs](https://example.com/{}END_LINK)",
            "x".repeat(120),
            "y".repeat(120),
            "z".repeat(120)
        );
        let lines = render(&sample, 38);
        let text = lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        for marker in ["END_CODE", "END_CELL", "END_LINK"] {
            assert!(text.contains(marker), "missing {marker}: {text}");
        }
    }

    #[test]
    fn description_uses_each_theme_without_colored_backgrounds() {
        let text = "# Review title\n\nRead [the docs](https://example.com) and `code`.\n\n> Context\n\n```\nlet value = 1;\n```";
        for theme in [&TERMINAL, &GRUVBOX, &CATPPUCCIN, &SLATE, &GRAPHITE] {
            for width in [40, 100] {
                let lines = render_glamour(text, width, description_style(theme));
                let rendered = lines
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(rendered.contains("Review title"));
                assert!(rendered.contains("let value = 1;"));
                for line in &lines {
                    assert!(line.style.bg.is_none_or(|bg| bg == Color::Reset));
                    assert!(
                        line.spans
                            .iter()
                            .all(|span| span.style.bg.is_none_or(|bg| bg == Color::Reset))
                    );
                }
            }
        }
    }
}
