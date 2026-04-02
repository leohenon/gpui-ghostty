use ghostty_vt::Rgb;

#[derive(Clone, Copy, Debug)]
pub struct TerminalTheme {
    pub foreground: Option<Rgb>,
    pub background: Option<Rgb>,
    pub cursor: Option<Rgb>,
    pub selection_background: Option<Rgb>,
    pub selection_foreground: Option<Rgb>,
    pub palette: [Option<Rgb>; 256],
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self {
            foreground: None,
            background: None,
            cursor: None,
            selection_background: None,
            selection_foreground: None,
            palette: [None; 256],
        }
    }
}

impl TerminalTheme {
    pub fn with_ansi_palette(mut self, colors: [Rgb; 16]) -> Self {
        for (index, color) in colors.into_iter().enumerate() {
            self.palette[index] = Some(color);
        }
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalConfig {
    pub cols: u16,
    pub rows: u16,
    pub default_fg: Rgb,
    pub default_bg: Rgb,
    pub theme: TerminalTheme,
    pub update_window_title: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            default_fg: Rgb {
                r: 0xFF,
                g: 0xFF,
                b: 0xFF,
            },
            default_bg: Rgb {
                r: 0x00,
                g: 0x00,
                b: 0x00,
            },
            theme: TerminalTheme::default(),
            update_window_title: true,
        }
    }
}
