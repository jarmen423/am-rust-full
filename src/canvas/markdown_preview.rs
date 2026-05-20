//! Lightweight markdown rendering for note card previews on the canvas.

use crate::theme::color32;
use crate::theme::palette;
use egui::{epaint, FontId, Painter, Pos2, Rect, Rounding, Stroke, TextFormat, Vec2};

const LINE_GAP: f32 = 3.0;
const CODE_PAD: f32 = 6.0;
const CODE_LINE_GAP: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeadingLevel {
    H1,
    H2,
    H3,
    H4,
}

#[derive(Debug, Clone)]
struct InlineSpan {
    text: String,
    bold: bool,
    italic: bool,
}

#[derive(Debug, Clone)]
struct PreviewLine {
    heading: Option<HeadingLevel>,
    spans: Vec<InlineSpan>,
}

#[derive(Debug, Clone)]
pub(crate) enum PreviewBlock {
    Line(PreviewLine),
    Bullet(String),
    Code {
        lang: Option<String>,
        lines: Vec<String>,
    },
}

/// Paint markdown body inside a card; returns the Y coordinate below the last painted block.
pub fn paint_markdown_preview(
    painter: &Painter,
    markdown: &str,
    top_left: Pos2,
    max_width: f32,
    max_bottom_y: f32,
    zoom: f32,
) -> f32 {
    let mut y = top_left.y;
    let blocks = parse_markdown_blocks(markdown);

    for block in blocks {
        if y >= max_bottom_y {
            break;
        }
        y = match block {
            PreviewBlock::Line(line) => {
                paint_line_block(painter, &line, top_left.x, y, max_width, max_bottom_y, zoom)
            }
            PreviewBlock::Bullet(text) => {
                paint_bullet_block(painter, &text, top_left.x, y, max_width, max_bottom_y, zoom)
            }
            PreviewBlock::Code { lang, lines } => {
                paint_code_block(
                    painter,
                    lang.as_deref(),
                    &lines,
                    top_left.x,
                    y,
                    max_width,
                    max_bottom_y,
                    zoom,
                )
            }
        };
    }

    y
}

fn paint_line_block(
    painter: &Painter,
    line: &PreviewLine,
    x: f32,
    y: f32,
    max_width: f32,
    max_bottom_y: f32,
    zoom: f32,
) -> f32 {
    let (base_size, bold_extra, color) = if let Some(level) = line.heading {
        let size = match level {
            HeadingLevel::H1 => 15.0,
            HeadingLevel::H2 => 13.5,
            HeadingLevel::H3 => 12.5,
            HeadingLevel::H4 => 12.0,
        } * zoom;
        (size.max(9.0), 0.0, color32(palette::TEXT_PRIMARY))
    } else {
        (
            (12.0 * zoom).max(7.0),
            1.0,
            color32(palette::TEXT_SECONDARY),
        )
    };

    let mut job = epaint::text::LayoutJob::default();
    job.wrap.max_width = max_width;

    for span in &line.spans {
        if span.text.is_empty() {
            continue;
        }
        let size = if span.bold {
            base_size + bold_extra
        } else {
            base_size
        };
        let mut format = TextFormat::simple(FontId::proportional(size), color);
        format.italics = span.italic;
        if span.bold {
            format.font_id.size = size;
            format.color = color32(palette::TEXT_PRIMARY);
        }
        job.append(&span.text, 0.0, format);
    }

    if job.sections.is_empty() {
        return y + base_size * 0.35;
    }

    let galley = painter.layout_job(job);
    let line_height = galley.size().y;
    if y + line_height > max_bottom_y {
        return y;
    }
    painter.galley(Pos2::new(x, y), galley, egui::Color32::WHITE);
    y + line_height + LINE_GAP * zoom
}

fn paint_bullet_block(
    painter: &Painter,
    text: &str,
    x: f32,
    y: f32,
    max_width: f32,
    max_bottom_y: f32,
    zoom: f32,
) -> f32 {
    let size = (12.0 * zoom).max(7.0);
    let bullet_indent = 14.0 * zoom;
    let mut job = epaint::text::LayoutJob::default();
    job.wrap.max_width = (max_width - bullet_indent).max(40.0);
    let format = TextFormat::simple(
        FontId::proportional(size),
        color32(palette::TEXT_SECONDARY),
    );
    job.append(&format!("• {text}"), 0.0, format);

    let galley = painter.layout_job(job);
    let line_height = galley.size().y;
    if y + line_height > max_bottom_y {
        return y;
    }
    painter.galley(Pos2::new(x, y), galley, egui::Color32::WHITE);
    y + line_height + LINE_GAP * zoom
}

fn paint_code_block(
    painter: &Painter,
    lang: Option<&str>,
    lines: &[String],
    x: f32,
    y: f32,
    max_width: f32,
    max_bottom_y: f32,
    zoom: f32,
) -> f32 {
    if lines.is_empty() {
        return y;
    }

    let pad = CODE_PAD * zoom;
    let font_size = (11.0 * zoom).max(7.0);
    let mono = FontId::monospace(font_size);
    let color = color32(palette::TEXT_PRIMARY);

    let mut content_height = pad;
    let inner_width = (max_width - pad * 2.0).max(40.0);
    let mut galleys = Vec::new();

    if let Some(lang) = lang.filter(|l| !l.is_empty()) {
        let label = format!("[{lang}]");
        let label_galley = painter.layout(
            label,
            FontId::proportional((10.0 * zoom).max(7.0)),
            color32(palette::TEXT_SECONDARY),
            inner_width,
        );
        content_height += label_galley.rect.height() + CODE_LINE_GAP * zoom;
        galleys.push((label_galley, true));
    }

    for line in lines {
        let galley = painter.layout(line.clone(), mono.clone(), color, inner_width);
        content_height += galley.rect.height() + CODE_LINE_GAP * zoom;
        galleys.push((galley, false));
    }
    content_height += pad;

    if y + content_height > max_bottom_y {
        return y;
    }

    let block_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(max_width, content_height));
    painter.rect_filled(block_rect, Rounding::same(4.0 * zoom), color32(palette::BG_DARK));
    painter.rect_stroke(
        block_rect,
        Rounding::same(4.0 * zoom),
        Stroke::new(1.0, color32(palette::BORDER)),
    );

    let mut inner_y = y + pad;
    for (galley, _is_label) in galleys {
        let h = galley.rect.height();
        if inner_y + h > max_bottom_y {
            break;
        }
        painter.galley(Pos2::new(x + pad, inner_y), galley, egui::Color32::WHITE);
        inner_y += h + CODE_LINE_GAP * zoom;
    }

    y + content_height + LINE_GAP * zoom
}

/// Walk markdown and emit display blocks (headings, bullets, fenced code).
pub(crate) fn parse_markdown_blocks(markdown: &str) -> Vec<PreviewBlock> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').trim();
            let lang = if lang.is_empty() {
                None
            } else {
                Some(lang.to_string())
            };
            i += 1;
            let mut code_lines = Vec::new();
            while i < lines.len() && lines[i].trim() != "```" {
                code_lines.push(lines[i].to_string());
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            blocks.push(PreviewBlock::Code {
                lang,
                lines: code_lines,
            });
            continue;
        }

        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        if let Some(item) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            blocks.push(PreviewBlock::Bullet(item.to_string()));
            i += 1;
            continue;
        }

        if let Some(line) = parse_markdown_line(trimmed) {
            blocks.push(PreviewBlock::Line(line));
        }
        i += 1;
    }

    blocks
}

fn parse_markdown_line(trimmed: &str) -> Option<PreviewLine> {
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("#### ") {
        return Some(PreviewLine {
            heading: Some(HeadingLevel::H4),
            spans: parse_inline_spans(rest),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("### ") {
        return Some(PreviewLine {
            heading: Some(HeadingLevel::H3),
            spans: parse_inline_spans(rest),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("## ") {
        return Some(PreviewLine {
            heading: Some(HeadingLevel::H2),
            spans: parse_inline_spans(rest),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("# ") {
        return Some(PreviewLine {
            heading: Some(HeadingLevel::H1),
            spans: parse_inline_spans(rest),
        });
    }

    Some(PreviewLine {
        heading: None,
        spans: parse_inline_spans(trimmed),
    })
}

/// Parse `**bold**`, `*italic*`, and `` `code` `` spans on one line.
fn parse_inline_spans(input: &str) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let mut rest = input;

    while !rest.is_empty() {
        if let Some(idx) = rest.find("**") {
            if idx > 0 {
                spans.push(plain_span(&rest[..idx]));
            }
            rest = &rest[idx + 2..];
            if let Some(end) = rest.find("**") {
                spans.push(InlineSpan {
                    text: rest[..end].to_string(),
                    bold: true,
                    italic: false,
                });
                rest = &rest[end + 2..];
            } else {
                spans.push(InlineSpan {
                    text: format!("**{rest}"),
                    bold: false,
                    italic: false,
                });
                break;
            }
            continue;
        }

        if let Some(idx) = rest.find('`') {
            if idx > 0 {
                spans.push(plain_span(&rest[..idx]));
            }
            rest = &rest[1..];
            if let Some(end) = rest.find('`') {
                spans.push(InlineSpan {
                    text: rest[..end].to_string(),
                    bold: false,
                    italic: true,
                });
                rest = &rest[end + 1..];
            } else {
                spans.push(plain_span(&format!("`{rest}")));
                break;
            }
            continue;
        }

        if let Some(idx) = rest.find('*') {
            if idx > 0 {
                spans.push(plain_span(&rest[..idx]));
            }
            rest = &rest[1..];
            if let Some(end) = rest.find('*') {
                spans.push(InlineSpan {
                    text: rest[..end].to_string(),
                    bold: false,
                    italic: true,
                });
                rest = &rest[end + 1..];
            } else {
                spans.push(plain_span(&format!("*{rest}")));
                break;
            }
            continue;
        }

        spans.push(plain_span(rest));
        break;
    }

    if spans.is_empty() {
        spans.push(plain_span(input));
    }

    spans
}

fn plain_span(text: &str) -> InlineSpan {
    InlineSpan {
        text: text.to_string(),
        bold: false,
        italic: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headings_and_bold() {
        let blocks = parse_markdown_blocks("# Note 17\n\n## This is h2\n\n**bold line**");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], PreviewBlock::Line(l) if l.heading == Some(HeadingLevel::H1)));
        assert!(matches!(&blocks[1], PreviewBlock::Line(l) if l.heading == Some(HeadingLevel::H2)));
        if let PreviewBlock::Line(l) = &blocks[2] {
            assert!(l.spans.iter().any(|s| s.bold && s.text == "bold line"));
        } else {
            panic!("expected line block");
        }
    }

    #[test]
    fn parses_fenced_code_block() {
        let md = "intro\n\n```python\nimport foo\nbar()\n```\n\nafter";
        let blocks = parse_markdown_blocks(md);
        let code_idx = blocks
            .iter()
            .position(|b| matches!(b, PreviewBlock::Code { .. }))
            .expect("code block");
        assert!(code_idx > 0, "code block should not be first");
        if let PreviewBlock::Code { lang, lines } = &blocks[code_idx] {
            assert_eq!(lang.as_deref(), Some("python"));
            assert_eq!(lines.len(), 2);
            assert_eq!(lines[0], "import foo");
        } else {
            panic!("expected code block");
        }
        assert!(blocks.len() > code_idx + 1, "content after code block");
    }

    #[test]
    fn long_note_includes_content_after_line_eight() {
        let mut md = String::new();
        for i in 1..=10 {
            md.push_str(&format!("line {i}\n"));
        }
        md.push_str("```rust\nfn main() {}\n```");
        let blocks = parse_markdown_blocks(&md);
        assert!(
            blocks.iter().any(|b| matches!(b, PreviewBlock::Code { .. })),
            "fenced code at end must be parsed"
        );
    }

    #[test]
    fn parses_bullet_items() {
        let blocks = parse_markdown_blocks("- first\n- second");
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], PreviewBlock::Bullet(s) if s == "first"));
    }
}
