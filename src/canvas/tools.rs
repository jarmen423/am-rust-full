//! Canvas tool enum — select, pan, connect, note.

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
}
