//! Canvas tool enum — select, pan, connect, note, text, and draw placeholders.

/// Active tool in the canvas toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CanvasTool {
    /// Select and drag cards.
    #[default]
    Select,
    /// Pan the viewport (also works with middle-mouse / space+drag).
    Pan,
    /// Drag from one card to another to create a connector.
    Connect,
    /// Click empty space to create a new note card.
    Note,
    /// Click empty space to place a text card (no new note file).
    Text,
    /// Shape tools — use Draw / Excalidraw bridge (not rendered on agentic_canvas).
    Rectangle,
    Circle,
    Diamond,
    Arrow,
}

impl CanvasTool {
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Pan => "Pan",
            Self::Connect => "Connect",
            Self::Note => "Note",
            Self::Text => "Text",
            Self::Rectangle => "Rect",
            Self::Circle => "Circle",
            Self::Diamond => "Diamond",
            Self::Arrow => "Arrow",
        }
    }

    /// Icon glyph for the floating toolbar (Unicode / simple).
    pub fn icon(self) -> &'static str {
        match self {
            Self::Select => "⬚",
            Self::Pan => "✥",
            Self::Connect => "↗",
            Self::Note => "▤",
            Self::Text => "T",
            Self::Rectangle => "▭",
            Self::Circle => "○",
            Self::Diamond => "◇",
            Self::Arrow => "➤",
        }
    }

    pub fn is_shape_placeholder(self) -> bool {
        matches!(
            self,
            Self::Rectangle | Self::Circle | Self::Diamond | Self::Arrow
        )
    }
}
