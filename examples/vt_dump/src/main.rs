fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;

    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;

    let terminal = ghostty_vt::Terminal::new(80, 24, 10_000)?;
    terminal.vt_write(&input);

    let mut render = ghostty_vt::RenderState::new()?;
    render.update(&terminal)?;

    let mut buf = Vec::new();
    let mut out = String::new();

    render.begin_row_iteration();
    while render.next_row() {
        render.begin_cell_iteration();
        while render.next_cell() {
            let len = render.cell_graphemes(&mut buf);
            if len == 0 {
                out.push(' ');
                continue;
            }

            for &cp in &buf[..len] {
                out.push(char::from_u32(cp).unwrap_or(' '));
            }
        }
        out.push('\n');
    }

    print!("{out}");
    Ok(())
}
