//! VT100/ANSI Escape Sequence Parser
//!
//! Parses PTY output and converts to terminal grid operations
//! Supports: SGR (colors, bold, italic), cursor movement, screen clear, true-color (24-bit RGB)
//! Per SRS §2.1.1 - Target: <2ms parse time per frame

/// VT100/ANSI sequence parser
pub struct VtParser {
    /// Pending parse state
    state: ParserState,
    /// Changes to apply to terminal grid
    changes: Vec<GridChange>,
    /// CSI parameter buffer
    params: Vec<u16>,
    /// Current parameter being accumulated
    current_param: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParserState {
    Ground,
    Escape,
    CsiEntry,
    CsiParam,
}

impl VtParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            changes: Vec::new(),
            params: Vec::new(),
            current_param: None,
        }
    }

    /// Feed PTY output bytes to parser
    /// Target: <2ms for typical terminal output
    pub fn feed(&mut self, data: &[u8]) {
        for &byte in data {
            match (self.state, byte) {
                // ESC sequence start
                (ParserState::Ground, 0x1B) => {
                    self.state = ParserState::Escape;
                }

                // CSI sequence start (ESC [)
                (ParserState::Escape, b'[') => {
                    self.state = ParserState::CsiEntry;
                    self.params.clear();
                    self.current_param = None;
                }

                // OSC sequence (ESC ]) - hyperlinks OSC 8
                (ParserState::Escape, b']') => {
                    // TODO: Parse OSC 8 hyperlinks (Phase 1.5)
                    self.state = ParserState::Ground;
                }

                // CSI Entry - first character after ESC [
                (ParserState::CsiEntry, b'0'..=b'9') => {
                    self.state = ParserState::CsiParam;
                    let digit = (byte - b'0') as u16;
                    self.current_param = Some(digit);
                }

                (ParserState::CsiEntry, b';') => {
                    self.state = ParserState::CsiParam;
                    self.params.push(0); // Empty parameter = 0
                    self.current_param = None;
                }

                // CSI parameters (accumulate numbers, split on semicolons)
                (ParserState::CsiParam, b'0'..=b'9') => {
                    let digit = (byte - b'0') as u16;
                    let param = self.current_param.unwrap_or(0);
                    self.current_param = Some(param * 10 + digit);
                }

                (ParserState::CsiParam, b';') => {
                    // Finish current parameter
                    self.params.push(self.current_param.unwrap_or(0));
                    self.current_param = None;
                }

                // CSI final byte - execute command
                (ParserState::CsiParam | ParserState::CsiEntry, b'm') => {
                    // SGR - Select Graphic Rendition (colors, bold, italic)
                    if let Some(param) = self.current_param {
                        self.params.push(param);
                    }
                    self.parse_sgr();
                    self.state = ParserState::Ground;
                }

                (ParserState::CsiParam | ParserState::CsiEntry, b'H') => {
                    // CUP - Cursor Position
                    if let Some(param) = self.current_param {
                        self.params.push(param);
                    }
                    let row = self.params.first().copied().unwrap_or(1).saturating_sub(1);
                    let col = self.params.get(1).copied().unwrap_or(1).saturating_sub(1);
                    self.changes.push(GridChange::CursorMove { row, col });
                    self.state = ParserState::Ground;
                }

                (ParserState::CsiParam | ParserState::CsiEntry, b'J') => {
                    // ED - Erase in Display
                    self.changes.push(GridChange::ClearScreen);
                    self.state = ParserState::Ground;
                }

                (ParserState::CsiParam | ParserState::CsiEntry, b'A') => {
                    // CUU - Cursor Up
                    if let Some(param) = self.current_param {
                        self.params.push(param);
                    }
                    let _n = self.params.first().copied().unwrap_or(1);
                    // TODO: Emit cursor up change
                    self.state = ParserState::Ground;
                }

                (ParserState::CsiParam | ParserState::CsiEntry, b'B') => {
                    // CUD - Cursor Down
                    if let Some(param) = self.current_param {
                        self.params.push(param);
                    }
                    let _n = self.params.first().copied().unwrap_or(1);
                    // TODO: Emit cursor down change
                    self.state = ParserState::Ground;
                }

                // Printable character
                (ParserState::Ground, 0x20..=0x7E) => {
                    let ch = byte as char;
                    self.changes.push(GridChange::PrintChar(ch));
                }

                // Newline
                (ParserState::Ground, b'\n') => {
                    self.changes.push(GridChange::Newline);
                }

                // Carriage return
                (ParserState::Ground, b'\r') => {
                    self.changes.push(GridChange::CarriageReturn);
                }

                // Tab
                (ParserState::Ground, b'\t') => {
                    self.changes.push(GridChange::Tab);
                }

                // Backspace
                (ParserState::Ground, 0x08) => {
                    self.changes.push(GridChange::Backspace);
                }

                // UTF-8 continuation bytes (0x80-0xBF)
                // TODO: Full UTF-8 parsing (Phase 1.5)
                (ParserState::Ground, 0x80..=0xBF) => {
                    // For now, replace with '?' - full UTF-8 in Phase 1.5
                    self.changes.push(GridChange::PrintChar('?'));
                }

                // Unknown or unimplemented
                _ => {
                    // Reset to ground state on unknown sequences
                    self.state = ParserState::Ground;
                }
            }
        }
    }

    /// Parse SGR (Select Graphic Rendition) parameters
    /// Handles ANSI colors (30-37, 40-47, 90-97, 100-107) and true-color (38;2;r;g;b)
    fn parse_sgr(&mut self) {
        // Start with current style (we'll modify it)
        let mut style = CellStyle::default(); // TODO: Track current style in parser state

        let mut i = 0;
        while i < self.params.len() {
            let param = self.params[i];

            match param {
                0 => {
                    // Reset all attributes
                    style = CellStyle::default();
                }
                1 => {
                    // Bold
                    style.bold = true;
                }
                3 => {
                    // Italic
                    style.italic = true;
                }
                4 => {
                    // Underline
                    style.underline = true;
                }
                22 => {
                    // Not bold
                    style.bold = false;
                }
                23 => {
                    // Not italic
                    style.italic = false;
                }
                24 => {
                    // Not underline
                    style.underline = false;
                }

                // Foreground colors (ANSI 30-37)
                30 => style.fg = Color::rgb(0, 0, 0),   // Black
                31 => style.fg = Color::rgb(205, 0, 0), // Red
                32 => style.fg = Color::rgb(0, 205, 0), // Green
                33 => style.fg = Color::rgb(205, 205, 0), // Yellow
                34 => style.fg = Color::rgb(0, 0, 205), // Blue
                35 => style.fg = Color::rgb(205, 0, 205), // Magenta
                36 => style.fg = Color::rgb(0, 205, 205), // Cyan
                37 => style.fg = Color::rgb(229, 229, 229), // White

                // Foreground 24-bit true-color (38;2;r;g;b)
                38 => {
                    if i + 4 < self.params.len() && self.params[i + 1] == 2 {
                        let r = self.params[i + 2].min(255) as u8;
                        let g = self.params[i + 3].min(255) as u8;
                        let b = self.params[i + 4].min(255) as u8;
                        style.fg = Color::rgb(r, g, b);
                        i += 4; // Skip r,g,b params
                    }
                }

                // Default foreground
                39 => style.fg = CellStyle::default().fg,

                // Background colors (ANSI 40-47)
                40 => style.bg = Color::rgb(0, 0, 0),
                41 => style.bg = Color::rgb(205, 0, 0),
                42 => style.bg = Color::rgb(0, 205, 0),
                43 => style.bg = Color::rgb(205, 205, 0),
                44 => style.bg = Color::rgb(0, 0, 205),
                45 => style.bg = Color::rgb(205, 0, 205),
                46 => style.bg = Color::rgb(0, 205, 205),
                47 => style.bg = Color::rgb(229, 229, 229),

                // Background 24-bit true-color (48;2;r;g;b)
                48 => {
                    if i + 4 < self.params.len() && self.params[i + 1] == 2 {
                        let r = self.params[i + 2].min(255) as u8;
                        let g = self.params[i + 3].min(255) as u8;
                        let b = self.params[i + 4].min(255) as u8;
                        style.bg = Color::rgb(r, g, b);
                        i += 4;
                    }
                }

                // Default background
                49 => style.bg = CellStyle::default().bg,

                // Bright foreground colors (ANSI 90-97)
                90 => style.fg = Color::rgb(127, 127, 127), // Bright black (gray)
                91 => style.fg = Color::rgb(255, 0, 0),
                92 => style.fg = Color::rgb(0, 255, 0),
                93 => style.fg = Color::rgb(255, 255, 0),
                94 => style.fg = Color::rgb(0, 0, 255),
                95 => style.fg = Color::rgb(255, 0, 255),
                96 => style.fg = Color::rgb(0, 255, 255),
                97 => style.fg = Color::rgb(255, 255, 255),

                // Bright background colors (ANSI 100-107)
                100 => style.bg = Color::rgb(127, 127, 127),
                101 => style.bg = Color::rgb(255, 0, 0),
                102 => style.bg = Color::rgb(0, 255, 0),
                103 => style.bg = Color::rgb(255, 255, 0),
                104 => style.bg = Color::rgb(0, 0, 255),
                105 => style.bg = Color::rgb(255, 0, 255),
                106 => style.bg = Color::rgb(0, 255, 255),
                107 => style.bg = Color::rgb(255, 255, 255),

                _ => {
                    // Unknown parameter, ignore
                }
            }

            i += 1;
        }

        // Emit style change
        self.changes.push(GridChange::StyleChange(style));
    }

    /// Drain parsed changes for terminal grid
    pub fn drain_changes(&mut self) -> Vec<GridChange> {
        std::mem::take(&mut self.changes)
    }

    /// Reset parser state
    pub fn reset(&mut self) {
        self.state = ParserState::Ground;
        self.changes.clear();
        self.params.clear();
        self.current_param = None;
    }
}

impl Default for VtParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Changes to apply to terminal grid
#[derive(Debug, Clone, PartialEq)]
pub enum GridChange {
    PrintChar(char),
    Newline,
    CarriageReturn,
    Tab,
    Backspace,
    ClearScreen,
    CursorMove { row: u16, col: u16 },
    StyleChange(CellStyle),
    ScrollUp,
    ScrollDown,
}

/// Cell style attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellStyle {
    /// Foreground color (24-bit true-color)
    pub fg: Color,
    /// Background color (24-bit true-color)
    pub bg: Color,
    /// Bold attribute
    pub bold: bool,
    /// Italic attribute
    pub italic: bool,
    /// Underline attribute
    pub underline: bool,
}

impl Default for CellStyle {
    fn default() -> Self {
        Self {
            fg: Color::rgb(192, 192, 192), // Light gray
            bg: Color::rgb(0, 0, 0),       // Black
            bold: false,
            italic: false,
            underline: false,
        }
    }
}

/// True-color (24-bit RGB) per SRS §2.1.1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_basic() {
        let mut parser = VtParser::new();
        parser.feed(b"Hello\n");

        let changes = parser.drain_changes();
        assert_eq!(changes.len(), 6); // H, e, l, l, o, \n
        assert_eq!(changes[0], GridChange::PrintChar('H'));
        assert_eq!(changes[5], GridChange::Newline);
    }

    #[test]
    fn test_parser_escape() {
        let mut parser = VtParser::new();
        parser.feed(b"\x1B[2J"); // Clear screen

        let changes = parser.drain_changes();
        assert!(changes.contains(&GridChange::ClearScreen));
    }
}
