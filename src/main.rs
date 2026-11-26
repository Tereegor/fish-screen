mod fish;

use anyhow::{Context, Result, ensure};
use clap::{Parser, ValueEnum};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Block,
};
use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    path::PathBuf,
    time::Instant,
};

#[derive(Clone, Copy, ValueEnum, PartialEq)]
enum Mode {
    Ascii,
    Original,
    Blocks,
}

fn color(text: &str) -> Result<[u8; 3], String> {
    let hex = match text {
        "black" => "000000",
        "red" => "ff0000",
        "green" => "00ff00",
        "yellow" => "ffff00",
        "blue" => "0000ff",
        "magenta" => "ff00ff",
        "cyan" => "00ffff",
        "white" => "ffffff",
        _ => text.strip_prefix('#').unwrap_or(""),
    };
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("expected a color name or #RRGGBB".into());
    }
    Ok([0, 2, 4].map(|pos| u8::from_str_radix(&hex[pos..pos + 2], 16).unwrap()))
}

#[derive(Parser)]
#[command(about = "A rotating fish for your terminal")]
struct Args {
    #[arg(long)]
    gif: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "ascii")]
    mode: Mode,
    #[arg(long,value_parser=color,default_value="white")]
    fg: [u8; 3],
    #[arg(long,value_parser=color,default_value="black")]
    bg: [u8; 3],
    #[arg(long, default_value_t=80,value_parser=clap::value_parser!(u16).range(1..=100))]
    size: u16,
    #[arg(long, default_value_t = 1.0)]
    speed: f64,
    #[arg(long,default_value_t=60,value_parser=clap::value_parser!(u16).range(1..=120))]
    fps: u16,
    #[arg(long,default_value_t=1,value_parser=clap::value_parser!(u16).range(1..=4))]
    glyph_size: u16,
}

fn rgb([red, green, blue]: [u8; 3]) -> Color {
    Color::Rgb(red, green, blue)
}

fn cell_size(size: Option<crossterm::terminal::WindowSize>) -> (f64, f64) {
    match size {
        Some(size)
            if [size.width, size.height, size.columns, size.rows]
                .iter()
                .all(|&n| n > 0) =>
        {
            (
                f64::from(size.width) / f64::from(size.columns),
                f64::from(size.height) / f64::from(size.rows),
            )
        }
        _ => (1., 2.),
    }
}

fn glyphs(
    first: &[u8],
    second: &[u8],
    cols: u16,
    zoom: u16,
    mix: f64,
    mode: Mode,
    mut put: impl FnMut(u16, u16, char),
) {
    let ascii = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
    let blocks = [' ', '░', '▒', '▓', '█'];
    let ramp = if mode == Mode::Blocks {
        &blocks[..]
    } else {
        &ascii[..]
    };
    let masks: [[u8; 5]; 10] = [
        [0; 5],
        [0, 0, 0, 0, 4],
        [0, 4, 0, 4, 0],
        [0, 0, 31, 0, 0],
        [0, 31, 0, 31, 0],
        [4, 4, 31, 4, 4],
        [17, 10, 31, 10, 17],
        [10, 31, 10, 31, 10],
        [25, 26, 4, 11, 19],
        [14, 17, 23, 21, 14],
    ];
    for (row_index, (row, next)) in first
        .chunks(cols.into())
        .zip(second.chunks(cols.into()))
        .enumerate()
    {
        for line in 0..zoom {
            for (column, (&start, &end)) in row.iter().zip(next).enumerate() {
                if start == 0 && end == 0 {
                    continue;
                }
                let shade = f64::from(start) + (f64::from(end) - f64::from(start)) * mix;
                let level = (shade * (ramp.len() - 1) as f64 / 255.).round() as usize;
                if level == 0 {
                    continue;
                }
                for col in 0..zoom {
                    let left = column as u16 * zoom + col;
                    let top = row_index as u16 * zoom + line;
                    if zoom == 1 || mode == Mode::Blocks {
                        put(left, top, ramp[level]);
                        continue;
                    }
                    let mut dots = 0;
                    for (bit, (dx, dy)) in [
                        (0, 0),
                        (0, 1),
                        (0, 2),
                        (1, 0),
                        (1, 1),
                        (1, 2),
                        (0, 3),
                        (1, 3),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let x = (2 * col + dx) * 5 / (2 * zoom);
                        let y = (4 * line + dy) * 5 / (4 * zoom);
                        if masks[level][usize::from(y)] & (16 >> x) != 0 {
                            dots |= 1 << bit;
                        }
                    }
                    put(left, top, char::from_u32(0x2800 + dots).unwrap());
                }
            }
        }
    }
}

#[test]
fn glyph_scale_and_blend() {
    let mut cells = Vec::new();
    glyphs(
        &[0, 0],
        &[255, 0],
        2,
        1,
        0.5,
        Mode::Blocks,
        |col, row, ch| cells.push((col, row, ch)),
    );
    assert_eq!(cells, [(0, 0, '▒')]);
    cells.clear();
    glyphs(&[255], &[255], 1, 3, 0., Mode::Ascii, |col, row, ch| {
        cells.push((col, row, ch))
    });
    assert_eq!(cells.len(), 9);
    assert!(cells.iter().all(|&(col, row, _)| col < 3 && row < 3));
}

fn clear() -> io::Result<()> {
    let mut out = io::stdout().lock();
    out.write_all(b"\x1b_Ga=d,d=I,i=7291,q=2;\x1b\\")?;
    out.flush()
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        (0.25..=4.).contains(&args.speed),
        "speed must be between 0.25 and 4"
    );
    let bytes = args
        .gif
        .as_ref()
        .map(fs::read)
        .transpose()
        .context("Cannot open GIF")?;
    let mut film = fish::load(
        bytes
            .as_deref()
            .unwrap_or(include_bytes!("../assets/fish.gif")),
    )
    .context("Cannot decode GIF")?;
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "Run in an interactive terminal"
    );
    let kitty = env::var("TERM").as_deref() == Ok("xterm-kitty") && env::var_os("TMUX").is_none();
    let mut term = ratatui::try_init()?;
    use event::{
        KeyboardEnhancementFlags as Flags, PopKeyboardEnhancementFlags as Pop,
        PushKeyboardEnhancementFlags as Push,
    };
    let enhanced =
        crossterm::execute!(io::stdout(), Push(Flags::DISAMBIGUATE_ESCAPE_CODES)).is_ok();
    let result = run(&mut term, &args, &mut film, kitty);
    if enhanced {
        let _ = crossterm::execute!(io::stdout(), Pop);
    }
    if kitty {
        let _ = clear();
    }
    ratatui::restore();
    result
}

fn run(
    term: &mut ratatui::DefaultTerminal,
    args: &Args,
    film: &mut [fish::Fish],
    kitty: bool,
) -> Result<()> {
    let mut mode = args.mode;
    let mut zoom = args.glyph_size;
    let mut speed = args.speed;
    let mut paused = false;
    let mut cache = Vec::new();
    let mut grid = (0, 0);
    let mut index = 0;
    let mut last = Instant::now();
    let mut phase = 0.;
    let style = Style::default().fg(rgb(args.fg)).bg(rgb(args.bg));
    let mut drawn = None;
    let mut cell = cell_size(crossterm::terminal::window_size().ok());
    loop {
        let now = Instant::now();
        if !paused {
            phase += now.duration_since(last).as_secs_f64() * speed;
        }
        last = now;
        while phase >= film[index].delay.as_secs_f64() {
            phase -= film[index].delay.as_secs_f64();
            index = (index + 1) % film.len();
        }
        let next = now + std::time::Duration::from_secs_f64(1. / f64::from(args.fps));
        let mut picture = None;
        term.draw(|frame| {
            let screen = frame.area();
            frame.render_widget(Block::default().style(style), screen);
            if screen.is_empty() {
                return;
            }
            let size =
                |length: u16| ((u32::from(length) * u32::from(args.size) / 100) as u16).max(1);
            let width = size(screen.width);
            let height = size(screen.height);
            let pixels = mode == Mode::Original && kitty;
            let step = if pixels {
                1
            } else {
                zoom.min(width).min(height)
            };
            let area = film[index].fit(
                Rect::new(
                    (screen.width - width) / 2,
                    (screen.height - height) / 2,
                    width,
                    height,
                ),
                cell,
                step,
            );
            if pixels {
                picture = Some((index, area));
            } else {
                let cols = area.width / step;
                let rows = area.height / step;
                if grid != (cols, rows) {
                    cache = vec![None; film.len()];
                    grid = (cols, rows);
                }
                let after = (index + 1) % film.len();
                for slot in [index, after] {
                    cache[slot].get_or_insert_with(|| film[slot].shades(cols, rows));
                }
                let mix = phase / film[index].delay.as_secs_f64();
                let buf = frame.buffer_mut();
                glyphs(
                    cache[index].as_ref().unwrap(),
                    cache[after].as_ref().unwrap(),
                    cols,
                    step,
                    mix,
                    mode,
                    |col, row, ch| {
                        buf[(area.x + col, area.y + row)].set_char(ch);
                    },
                );
            }
        })?;
        if drawn != picture {
            if let (Some((_, old)), Some((_, new))) = (drawn, picture)
                && old != new
            {
                clear()?;
            }
            if let Some((index, area)) = picture {
                film[index].show(&mut io::stdout().lock(), area, args.fg, args.bg)?;
            } else {
                clear()?;
            }
            drawn = picture;
        }
        if event::poll(next.saturating_duration_since(Instant::now()))? {
            let key = match event::read()? {
                Event::Resize(..) => {
                    cell = cell_size(crossterm::terminal::window_size().ok());
                    continue;
                }
                Event::Key(key) if key.kind != event::KeyEventKind::Release => key,
                _ => continue,
            };
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char('g') if kitty => {
                    mode = if mode == Mode::Original {
                        Mode::Ascii
                    } else {
                        Mode::Original
                    }
                }
                KeyCode::Char('+' | '=') => zoom = (zoom + 1).min(4),
                KeyCode::Char('-') => zoom = zoom.saturating_sub(1).max(1),
                KeyCode::Char('1') => speed = (speed * 1.25).min(4.),
                KeyCode::Char('2') => speed = (speed / 1.25).max(0.25),
                KeyCode::Char('3') => mode = Mode::Ascii,
                KeyCode::Char('4') => mode = Mode::Blocks,
                KeyCode::Char('5') => mode = Mode::Original,
                KeyCode::Char(' ') => paused = !paused,
                _ => {}
            }
        }
    }
    Ok(())
}

#[test]
fn palettes_and_frames() {
    assert_eq!(color("#ffbf69").unwrap(), [255, 191, 105]);
    assert!(color("#abc").is_err());
    let film = fish::load(include_bytes!("../assets/fish.gif")).unwrap();
    assert_eq!(film.len(), 36);
    assert!(film.iter().all(|frame| frame.delay.as_millis() == 90));
    assert_eq!(cell_size(None), (1., 2.));
    for (cell, bounds) in [
        ((8., 8.), (180, 124)),
        ((8., 16.), (180, 62)),
        ((10., 24.), (180, 51)),
    ] {
        let screen = Rect::new(0, 0, 180, 150);
        let area = film[0].fit(screen, cell, 1);
        assert_eq!((area.width, area.height), bounds);
        let big = film[0].fit(screen, cell, 4);
        assert_eq!((big.width % 4, big.height % 4), (0, 0));
        assert!(
            (f64::from(big.width) * cell.0 * 247. / 360. / cell.1 - f64::from(big.height)).abs()
                <= 2.
        );
    }
}
