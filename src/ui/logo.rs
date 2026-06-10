//! Animated "tuira" splash logo in the ANSI-Shadow block style.
//!
//! Each letter is drawn raised, standing above its own drop-shadow — the
//! box-drawing edges (`╗╝╚═║`) of the font *are* that shadow. The animation
//! presses a letter down onto its shadow so the edges vanish and the letter
//! goes flat (solid blocks only), then lets it pop back out. The press ripples
//! across the word, staggered letter by letter.
//!
//! Loop shape: starting all-flat, the word ripples up then back down, twice in
//! a row, then holds for a short pause — repeating forever. The stagger across
//! letters follows a bell-curve (ease-in-out) easing, and the press itself is a
//! true sub-cell vertical glide built from eighth-block characters.

use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ui::theme::Theme;

const LETTER_HEIGHT: usize = 6;

/// One letter sprite. `'█'` is a solid block (the raised face); the box-drawing
/// characters are the shadow; `' '` is empty. Letters are placed adjacent — the
/// font carries its own spacing.
struct Glyph {
    rows: [&'static str; LETTER_HEIGHT],
}

const T: Glyph = Glyph {
    rows: [
        "████████╗",
        "╚══██╔══╝",
        "   ██║   ",
        "   ██║   ",
        "   ██║   ",
        "   ╚═╝   ",
    ],
};

const U: Glyph = Glyph {
    rows: [
        "██╗   ██╗",
        "██║   ██║",
        "██║   ██║",
        "██║   ██║",
        "╚██████╔╝",
        " ╚═════╝ ",
    ],
};

const I: Glyph = Glyph {
    rows: ["██╗", "██║", "██║", "██║", "██║", "╚═╝"],
};

const R: Glyph = Glyph {
    rows: [
        "██████╗ ",
        "██╔══██╗",
        "██████╔╝",
        "██╔══██╗",
        "██║  ██║",
        "╚═╝  ╚═╝",
    ],
};

const A: Glyph = Glyph {
    rows: [
        " █████╗ ",
        "██╔══██╗",
        "███████║",
        "██╔══██║",
        "██║  ██║",
        "╚═╝  ╚═╝",
    ],
};

const WORD: [&Glyph; 5] = [&T, &U, &I, &R, &A];

// --- Animation timing -------------------------------------------------------

/// Stagger between adjacent letters, and the time a letter rests fully up
/// before the fall wave begins. Tuned a further 50% faster than the last pass.
///
/// `SPREAD` is the time the rise (and fall) wave takes to sweep all letters;
/// `HOLD` is the extra all-up rest before the fall begins; `TAIL` is a short
/// all-flat beat closing each iteration; `PAUSE` is the longer rest after a
/// burst of two iterations.
const SPREAD: f64 = 0.243;
const HOLD: f64 = 0.057;
const TAIL: f64 = 0.1;
const PAUSE: f64 = 0.8;
/// Ease window: how long a letter takes to glide between fully pressed and
/// fully raised. `TAIL` is sized to fit it within an iteration.
const TRANS: f64 = 0.091;

/// One up-then-down ripple: rise sweep, hold, fall sweep, tail.
const ITERATION: f64 = SPREAD + HOLD + SPREAD + TAIL;
const ITERATIONS_PER_BURST: f64 = 2.0;
const BURST: f64 = ITERATIONS_PER_BURST * ITERATION;
const LOOP: f64 = BURST + PAUSE;

/// Bell-shaped (ease-in-out) easing: maps a letter's normalized position
/// (0..=1) to its normalized trigger time within a sweep.
fn smoothstep(p: f64) -> f64 {
    p * p * (3.0 - 2.0 * p)
}

/// The position-eased trigger offset for the letter at `index`.
fn rise_offset(index: usize) -> f64 {
    let position = index as f64 / (WORD.len() - 1) as f64;
    SPREAD * smoothstep(position)
}

/// Continuous raise amount in `0.0..=1.0` (0 pressed, 1 raised) for the letter
/// at `index`, easing over `TRANS` at each edge.
fn raise_amount(t: f64, index: usize) -> f64 {
    let loop_t = t.rem_euclid(LOOP);
    if loop_t >= BURST {
        return 0.0;
    }
    let iter_t = loop_t.rem_euclid(ITERATION);
    let offset = rise_offset(index);
    let rise_at = offset;
    let fall_at = SPREAD + HOLD + offset;
    if iter_t < rise_at {
        0.0
    } else if iter_t < rise_at + TRANS {
        smoothstep((iter_t - rise_at) / TRANS)
    } else if iter_t < fall_at {
        1.0
    } else if iter_t < fall_at + TRANS {
        1.0 - smoothstep((iter_t - fall_at) / TRANS)
    } else {
        0.0
    }
}

/// How far the font's own shadow sits down-and-right.
const DEPTH: usize = 1;
/// Extra blank columns between letters (the chosen spacing).
const GAP: usize = 2;
/// Bottom-anchored partial block glyphs, 1/8 through 8/8 filled.
const EIGHTHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// One resolved grid cell: glyph plus optional foreground/background colors.
#[derive(Clone, Copy, PartialEq)]
struct CellData {
    ch: char,
    fg: Option<Color>,
    bg: Option<Color>,
}

const EMPTY_CELL: CellData = CellData {
    ch: ' ',
    fg: None,
    bg: None,
};

/// Renders the animated logo (sub-cell glide) centered in `area`.
pub fn render(frame: &mut Frame<'_>, area: Rect, elapsed: Duration, theme: &Theme) {
    let t = elapsed.as_secs_f64();
    let amounts: [f64; 5] = std::array::from_fn(|index| raise_amount(t, index));

    let logo_height = LETTER_HEIGHT as u16;
    let top = area.height.saturating_sub(logo_height) / 2;
    let logo_area = Rect {
        x: area.x,
        y: area.y + top,
        width: area.width,
        height: logo_height.min(area.height),
    };
    frame.render_widget(
        Paragraph::new(compose_lines(&amounts, theme)).alignment(Alignment::Center),
        logo_area,
    );
}

/// Builds the colored block art for one glide frame onto a fixed-width grid.
fn compose_lines(amounts: &[f64; 5], theme: &Theme) -> Vec<Line<'static>> {
    let word_width: usize = WORD.iter().map(glyph_width).sum();
    let width = word_width + GAP * (WORD.len() - 1) + DEPTH;
    let mut grid = vec![vec![EMPTY_CELL; width]; LETTER_HEIGHT];

    let mut base = 0;
    for (letter_index, glyph) in WORD.iter().enumerate() {
        stamp_glide(&mut grid, glyph, base, amounts[letter_index], theme);
        base += glyph_width(glyph) + GAP;
    }

    grid.into_iter().map(|row| row_to_line(&row)).collect()
}

fn put(grid: &mut [Vec<CellData>], r: usize, c: usize, cell: CellData) {
    if let Some(slot) = grid.get_mut(r).and_then(|row| row.get_mut(c)) {
        *slot = cell;
    }
}

/// Stamps a letter with genuine sub-cell vertical motion: the block face glides
/// down by `1 - amount` cells using eighth-block characters (foreground/
/// background swapping handles partial cells either side of a boundary), while a
/// solid block shadow sits offset down-right and fades out as the letter drops.
fn stamp_glide(grid: &mut [Vec<CellData>], glyph: &Glyph, base: usize, amount: f64, theme: &Theme) {
    let bg = theme.status_bar_bg();
    let face = theme.accent_fg();
    let shadow = lerp(bg, theme.border_fg(), amount);
    let dy = 1.0 - amount;

    for (r, line) in glyph.rows.iter().enumerate() {
        for (c, ch) in line.chars().enumerate() {
            if ch == '█' {
                put(grid, r + DEPTH, base + c + DEPTH, solid('█', shadow));
            }
        }
    }

    let height = grid.len();
    let mut bottom = vec![0.0_f64; height];
    let mut top = vec![0.0_f64; height];
    let columns = glyph.rows[0].chars().count();
    for col in 0..columns {
        bottom.iter_mut().for_each(|v| *v = 0.0);
        top.iter_mut().for_each(|v| *v = 0.0);
        for (r, line) in glyph.rows.iter().enumerate() {
            if line.chars().nth(col) == Some('█') {
                bottom[r] += 1.0 - dy;
                if r + 1 < height {
                    top[r + 1] += dy;
                }
            }
        }
        for row in 0..height {
            let cell = glide_cell(bottom[row], top[row], face, bg);
            if let Some(cell) = cell {
                put(grid, row, base + col, cell);
            }
        }
    }
}

/// Resolves one glided column cell from its bottom-anchored and top-anchored
/// fill fractions into a partial-block glyph (or `None` if effectively empty).
fn glide_cell(bottom: f64, top: f64, face: Color, bg: Color) -> Option<CellData> {
    let total = bottom + top;
    if total < 0.06 {
        return None;
    }
    if total >= 0.94 {
        return Some(solid('█', face));
    }
    if top <= 0.06 {
        return Some(CellData {
            ch: eighth(bottom),
            fg: Some(face),
            bg: Some(bg),
        });
    }
    if bottom <= 0.06 {
        // Fill the top of the cell by drawing the empty bottom in `bg` over a
        // `face` background.
        return Some(CellData {
            ch: eighth(1.0 - top),
            fg: Some(bg),
            bg: Some(face),
        });
    }
    Some(solid('█', face))
}

fn eighth(fraction: f64) -> char {
    let index = (fraction * 8.0).round().clamp(1.0, 8.0) as usize;
    EIGHTHS[index - 1]
}

fn solid(ch: char, fg: Color) -> CellData {
    CellData {
        ch,
        fg: Some(fg),
        bg: None,
    }
}

fn lerp(from: Color, to: Color, factor: f64) -> Color {
    let factor = factor.clamp(0.0, 1.0);
    match (from, to) {
        (Color::Rgb(fr, fg, fb), Color::Rgb(tr, tg, tb)) => Color::Rgb(
            lerp_channel(fr, tr, factor),
            lerp_channel(fg, tg, factor),
            lerp_channel(fb, tb, factor),
        ),
        _ if factor >= 0.5 => to,
        _ => from,
    }
}

fn lerp_channel(from: u8, to: u8, factor: f64) -> u8 {
    (f64::from(from) + (f64::from(to) - f64::from(from)) * factor).round() as u8
}

fn row_to_line(row: &[CellData]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_fg: Option<Color> = None;
    let mut run_bg: Option<Color> = None;

    for cell in row {
        if !run.is_empty() && (cell.fg != run_fg || cell.bg != run_bg) {
            spans.push(span_for(std::mem::take(&mut run), run_fg, run_bg));
        }
        run_fg = cell.fg;
        run_bg = cell.bg;
        run.push(cell.ch);
    }
    if !run.is_empty() {
        spans.push(span_for(run, run_fg, run_bg));
    }
    Line::from(spans)
}

fn span_for(text: String, fg: Option<Color>, bg: Option<Color>) -> Span<'static> {
    let mut style = Style::default();
    if let Some(fg) = fg {
        style = style.fg(fg);
    }
    if let Some(bg) = bg {
        style = style.bg(bg);
    }
    Span::styled(text, style)
}

fn glyph_width(glyph: &&Glyph) -> usize {
    glyph.rows[0].chars().count()
}
