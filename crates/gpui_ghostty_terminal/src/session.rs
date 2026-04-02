use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use ghostty_vt::{Error, KeyEncoder, RenderState, RenderStateColors, Rgb, Terminal};

use crate::TerminalConfig;

struct SessionUserdata {
    pty_writer: Option<Arc<Mutex<Box<dyn std::io::Write + Send>>>>,
    title_changed: bool,
}

pub struct TerminalSession {
    config: TerminalConfig,
    terminal: Terminal,
    render_state: RenderState,
    key_encoder: KeyEncoder,
    userdata: Box<SessionUserdata>,
}

unsafe extern "C" fn write_pty_callback(
    _terminal: ghostty_vt_sys::GhosttyTerminal,
    userdata: *mut c_void,
    data: *const u8,
    len: usize,
) {
    if userdata.is_null() || data.is_null() || len == 0 {
        return;
    }
    unsafe {
        let ud = &*(userdata as *const SessionUserdata);
        if let Some(ref writer) = ud.pty_writer {
            if let Ok(mut w) = writer.lock() {
                let _ = std::io::Write::write_all(&mut *w, std::slice::from_raw_parts(data, len));
                let _ = std::io::Write::flush(&mut *w);
            }
        }
    }
}

unsafe extern "C" fn title_changed_callback(
    _terminal: ghostty_vt_sys::GhosttyTerminal,
    userdata: *mut c_void,
) {
    if userdata.is_null() {
        return;
    }
    unsafe {
        let ud = &mut *(userdata as *mut SessionUserdata);
        ud.title_changed = true;
    }
}

unsafe extern "C" fn device_attributes_callback(
    _terminal: ghostty_vt_sys::GhosttyTerminal,
    _userdata: *mut c_void,
    out_attrs: *mut c_void,
) -> bool {
    if out_attrs.is_null() {
        return false;
    }
    unsafe {
        let attrs = &mut *(out_attrs as *mut ghostty_vt_sys::GhosttyDeviceAttributes);
        attrs.primary.conformance_level = 62;
        attrs.primary.features[0] = 22;
        attrs.primary.num_features = 1;
        attrs.secondary.device_type = 1;
        attrs.secondary.firmware_version = 10;
        attrs.secondary.rom_cartridge = 0;
        attrs.tertiary.unit_id = 0;
    }
    true
}

impl TerminalSession {
    pub fn new(config: TerminalConfig) -> Result<Self, Error> {
        let terminal = Terminal::new(config.cols, config.rows, 10000)?;
        let render_state = RenderState::new()?;
        let key_encoder = KeyEncoder::new()?;

        let userdata = Box::new(SessionUserdata {
            pty_writer: None,
            title_changed: false,
        });

        let ud_ptr = &*userdata as *const SessionUserdata as *mut c_void;
        terminal.set_userdata(ud_ptr);
        terminal.set_write_pty_callback(Some(write_pty_callback));
        terminal.set_title_changed_callback(Some(title_changed_callback));
        terminal.set_device_attributes_callback(Some(device_attributes_callback));

        apply_initial_theme(&terminal, &config);
        let _ = render_state.update(&terminal);

        Ok(Self {
            config,
            terminal,
            render_state,
            key_encoder,
            userdata,
        })
    }

    pub fn set_pty_writer(&mut self, writer: Arc<Mutex<Box<dyn std::io::Write + Send>>>) {
        self.userdata.pty_writer = Some(writer);
    }

    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    pub fn render_state(&mut self) -> &mut RenderState {
        &mut self.render_state
    }

    pub fn update_render_state(&mut self) {
        let _ = self.render_state.update(&self.terminal);
    }

    pub fn apply_config(&mut self, config: TerminalConfig) {
        self.config = config;
        apply_initial_theme(&self.terminal, &self.config);
        self.update_render_state();
    }

    pub fn cols(&self) -> u16 {
        self.terminal.cols()
    }

    pub fn rows(&self) -> u16 {
        self.terminal.rows()
    }

    pub fn default_foreground(&self) -> Rgb {
        self.render_state.foreground()
    }

    pub fn default_background(&self) -> Rgb {
        self.render_state.background()
    }

    pub fn cursor_color(&self) -> Option<Rgb> {
        self.render_state
            .colors()
            .cursor
            .or(self.config.theme.cursor)
    }

    pub fn selection_background(&self) -> Option<Rgb> {
        self.config.theme.selection_background
    }

    pub fn selection_foreground(&self) -> Option<Rgb> {
        self.config.theme.selection_foreground
    }

    pub fn colors(&self) -> RenderStateColors {
        self.render_state.colors()
    }

    pub fn bracketed_paste_enabled(&self) -> bool {
        self.terminal.get_mode(ghostty_vt::mode_new(2004, false))
    }

    pub fn mouse_reporting_enabled(&self) -> bool {
        self.terminal.mouse_tracking()
    }

    pub fn mouse_sgr_enabled(&self) -> bool {
        self.terminal.get_mode(ghostty_vt::mode_new(1006, false))
    }

    pub fn mouse_button_event_enabled(&self) -> bool {
        self.terminal.get_mode(ghostty_vt::mode_new(1002, false))
    }

    pub fn mouse_any_event_enabled(&self) -> bool {
        self.terminal.get_mode(ghostty_vt::mode_new(1003, false))
    }

    pub fn cursor_info(&self) -> ghostty_vt::CursorInfo {
        self.render_state.cursor_info()
    }

    pub fn cursor_position(&self) -> Option<(u16, u16)> {
        let info = self.render_state.cursor_info();
        if info.in_viewport {
            Some((info.x, info.y))
        } else {
            None
        }
    }

    pub fn take_title(&mut self) -> Option<String> {
        if self.userdata.title_changed {
            self.userdata.title_changed = false;
            self.terminal.title()
        } else {
            None
        }
    }

    pub(crate) fn window_title_updates_enabled(&self) -> bool {
        self.config.update_window_title
    }

    pub fn hyperlink_at(&self, _col: u16, _row: u16) -> Option<String> {
        None
    }

    pub fn take_clipboard_write(&mut self) -> Option<String> {
        None
    }

    pub fn encode_key(
        &self,
        key_name: &str,
        utf8: &str,
        shift: bool,
        ctrl: bool,
        alt: bool,
        is_repeat: bool,
    ) -> Option<Vec<u8>> {
        self.key_encoder.sync_from_terminal(&self.terminal);
        let key = ghostty_vt::map_key_name(key_name);
        let action = if is_repeat {
            ghostty_vt::KEY_ACTION_REPEAT
        } else {
            ghostty_vt::KEY_ACTION_PRESS
        };
        let mut mods: u16 = 0;
        if shift {
            mods |= ghostty_vt::KEY_MOD_SHIFT;
        }
        if ctrl {
            mods |= ghostty_vt::KEY_MOD_CTRL;
        }
        if alt {
            mods |= ghostty_vt::KEY_MOD_ALT;
        }
        let ucp = utf8
            .chars()
            .next()
            .map(|c| c as u32)
            .or_else(|| match key_name {
                "space" => Some(' ' as u32),
                _ if key_name.len() == 1 => key_name.chars().next().map(|c| c as u32),
                _ => None,
            })
            .unwrap_or(0);
        self.key_encoder.encode(key, action, mods, utf8, ucp)
    }

    pub fn vt_write(&self, bytes: &[u8]) {
        self.terminal.vt_write(bytes);
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.terminal.vt_write(bytes);
        Ok(())
    }

    pub fn scroll_viewport(&mut self, delta_lines: i32) -> Result<(), Error> {
        self.terminal.scroll_viewport_delta(delta_lines as isize);
        Ok(())
    }

    pub fn scroll_viewport_top(&mut self) -> Result<(), Error> {
        self.terminal.scroll_viewport_top();
        Ok(())
    }

    pub fn scroll_viewport_bottom(&mut self) -> Result<(), Error> {
        self.terminal.scroll_viewport_bottom();
        Ok(())
    }

    pub fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), Error> {
        self.config.cols = cols;
        self.config.rows = rows;
        self.terminal
            .resize(cols, rows, cell_width_px, cell_height_px)
    }
}

fn apply_initial_theme(terminal: &Terminal, config: &TerminalConfig) {
    for seq in initial_theme_sequences(config) {
        terminal.vt_write(&seq);
    }
}

fn initial_theme_sequences(config: &TerminalConfig) -> Vec<Vec<u8>> {
    let mut out = Vec::new();

    for (index, color) in config.theme.palette.iter().enumerate() {
        if let Some(color) = color {
            out.push(osc_palette(index as u16, *color));
        }
    }

    out.push(osc_special(
        10,
        config.theme.foreground.unwrap_or(config.default_fg),
    ));
    out.push(osc_special(
        11,
        config.theme.background.unwrap_or(config.default_bg),
    ));

    if let Some(color) = config.theme.cursor {
        out.push(osc_special(12, color));
    }

    out
}

fn osc_palette(index: u16, color: Rgb) -> Vec<u8> {
    format!(
        "\x1b]4;{index};rgb:{:02X}/{:02X}/{:02X}\x1b\\",
        color.r, color.g, color.b
    )
    .into_bytes()
}

fn osc_special(code: u16, color: Rgb) -> Vec<u8> {
    format!(
        "\x1b]{code};rgb:{:02X}/{:02X}/{:02X}\x1b\\",
        color.r, color.g, color.b
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::{initial_theme_sequences, osc_palette, osc_special};
    use crate::{TerminalConfig, TerminalTheme};
    use ghostty_vt::Rgb;

    #[test]
    fn initial_theme_sequences_emit_palette_defaults_and_cursor() {
        let mut config = TerminalConfig::default();
        config.default_fg = Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33,
        };
        config.default_bg = Rgb {
            r: 0x44,
            g: 0x55,
            b: 0x66,
        };
        config.theme = TerminalTheme {
            cursor: Some(Rgb {
                r: 0x77,
                g: 0x88,
                b: 0x99,
            }),
            palette: {
                let mut palette = [None; 256];
                palette[1] = Some(Rgb {
                    r: 0xAA,
                    g: 0xBB,
                    b: 0xCC,
                });
                palette
            },
            ..TerminalTheme::default()
        };

        let seq = initial_theme_sequences(&config);
        assert_eq!(seq[0], osc_palette(1, config.theme.palette[1].unwrap()));
        assert_eq!(seq[1], osc_special(10, config.default_fg));
        assert_eq!(seq[2], osc_special(11, config.default_bg));
        assert_eq!(seq[3], osc_special(12, config.theme.cursor.unwrap()));
    }

    #[test]
    fn initial_theme_sequences_prefer_theme_defaults() {
        let mut config = TerminalConfig::default();
        config.theme.foreground = Some(Rgb {
            r: 0xFE,
            g: 0xDC,
            b: 0xBA,
        });
        config.theme.background = Some(Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56,
        });

        let seq = initial_theme_sequences(&config);
        assert_eq!(seq[0], osc_special(10, config.theme.foreground.unwrap()));
        assert_eq!(seq[1], osc_special(11, config.theme.background.unwrap()));
    }
}
