use crate::engine::View;
use crate::font::measure;
use crate::geom::Rect;
use crate::safe::Insets;

pub const TOUCH_MM: f32 = 12.0;
pub const KEY_W_MM: f32 = 7.6;
pub const KEY_GAP_MM: f32 = 1.5;
pub const KID_MM: f32 = 15.0;
pub const GAP_MM: f32 = 2.0;
pub const EDGE_MM: f32 = 2.5;
pub const TEXT_MM: f32 = 4.2;
pub const BODY_SIZE: f32 = 26.0;
pub const SHEET_SHARE: f32 = 0.92;
pub const MAX_GLYPH: f32 = 220.0;
const LINE: f32 = 1.22;
const RIBBON_ASPECT: f32 = 250.0 / 1145.0;
const RIBBON_SIZE: f32 = 54.0;

impl View {
    pub fn short(&self) -> f32 {
        self.w.min(self.h)
    }

    pub fn portrait(&self) -> bool {
        self.h > self.w
    }

    pub fn from_mm(&self, mm: f32) -> f32 {
        mm * self.mm
    }

    pub fn to_mm(&self, units: f32) -> f32 {
        units / self.mm
    }

    pub fn touch(&self) -> f32 {
        self.from_mm(TOUCH_MM)
    }

    pub fn tap(&self, design: f32) -> f32 {
        design.max(self.touch())
    }

    pub fn kid_tap(&self, design: f32) -> f32 {
        design
            .max(self.from_mm(KID_MM).min(self.h * 0.15))
            .max(self.touch())
    }

    pub fn key_w(&self) -> f32 {
        self.from_mm(KEY_W_MM)
    }

    pub fn key_gap(&self) -> f32 {
        self.from_mm(KEY_GAP_MM).max(6.0)
    }

    pub fn gap(&self) -> f32 {
        self.from_mm(GAP_MM).max(12.0)
    }

    pub fn edge(&self) -> f32 {
        self.from_mm(EDGE_MM).max(14.0)
    }

    pub fn text_k(&self) -> f32 {
        (self.from_mm(TEXT_MM) / BODY_SIZE).max(1.0)
    }

    pub fn text(&self, design: f32) -> f32 {
        design * self.text_k()
    }

    pub fn sheet_w(&self, design: f32) -> f32 {
        let room = self.w - 2.0 * self.edge();
        let want = if self.portrait() {
            (design * self.text_k()).max(self.w * SHEET_SHARE)
        } else {
            design * self.text_k()
        };
        want.min(room)
    }

    pub fn safe(&self) -> Rect {
        self.rect()
    }

    pub fn top_button(&self, design: f32) -> f32 {
        self.tap(design)
    }

    pub fn home(&self) -> Rect {
        let d = self.tap(56.0);
        Rect::new(self.edge().min(14.0), 8.0, d, d)
    }

    pub fn top_right(&self, i: usize) -> Rect {
        let d = self.tap(56.0);
        let gap = self.gap().min(16.0);
        Rect::new(
            self.w - self.edge().min(14.0) - d - i as f32 * (d + gap),
            8.0,
            d,
            d,
        )
    }

    pub fn hud_left(&self) -> f32 {
        let h = self.home();
        h.x + h.w + self.gap().min(16.0)
    }

    pub fn hud_top(&self) -> f32 {
        let h = self.home();
        h.y + h.h + self.gap().min(16.0)
    }
}

pub fn fit_size(s: &str, size: f32, width: f32) -> f32 {
    let w = measure(s, size);
    if w <= width || w <= 0.0 {
        size
    } else {
        size * width / w
    }
}

pub fn wrap(s: &str, size: f32, width: f32) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut line = String::new();
    for word in s.split_whitespace() {
        let next = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if line.is_empty() || measure(&next, size) <= width {
            line = next;
        } else {
            out.push(std::mem::take(&mut line));
            line = word.to_string();
        }
    }
    if !line.is_empty() || out.is_empty() {
        out.push(line);
    }
    out
}

pub fn label_size(label: &str, r: Rect) -> f32 {
    fit_size(label, r.h * 0.46, r.w - r.h * 0.45)
}

pub fn ribbon_rect(view: &View, title: &str, cx: f32, cy: f32, k: f32) -> (Rect, f32) {
    let size = (RIBBON_SIZE * k).max(view.text(34.0));
    let tw = measure(title, size);
    let w = (tw + size * 4.8).min(view.w - 20.0);
    let h = w * RIBBON_ASPECT;
    let font = size.min(w * 0.7 / tw.max(1.0) * size);
    (Rect::new(cx - w / 2.0, cy - h / 2.0, w, h), font)
}

pub fn ribbon_h(view: &View, title: &str, k: f32) -> f32 {
    ribbon_rect(view, title, 0.0, 0.0, k).0.h
}

#[derive(Clone, Debug)]
enum Part {
    Space(f32),
    Text { size: f32, text: String },
    Block(f32, f32),
    Buttons { h: f32, labels: Vec<String> },
}

#[derive(Clone, Debug)]
pub struct Stack {
    view: View,
    design_w: f32,
    ribbon: Option<String>,
    rise: f32,
    parts: Vec<Part>,
}

#[derive(Clone, Debug, Default)]
pub struct Laid {
    pub rect: Rect,
    pub rects: Vec<Rect>,
    pub lines: Vec<String>,
    pub size: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Sheet {
    pub panel: Rect,
    pub ribbon: Option<(Rect, f32)>,
    pub k: f32,
    pub fits: bool,
    pub parts: Vec<Laid>,
}

impl Sheet {
    pub fn rect(&self, i: usize) -> Rect {
        self.parts[i].rect
    }

    pub fn button(&self, i: usize, j: usize) -> Rect {
        self.parts[i].rects[j]
    }

    pub fn buttons(&self) -> Vec<Rect> {
        self.parts.iter().flat_map(|p| p.rects.clone()).collect()
    }

    pub fn lines(&self, i: usize) -> (&[String], f32, Rect) {
        let p = &self.parts[i];
        (&p.lines, p.size, p.rect)
    }

    pub fn line_y(&self, i: usize, n: usize) -> f32 {
        let p = &self.parts[i];
        p.rect.y + p.size * LINE * (n as f32 + 0.5)
    }

    pub fn title_y(&self) -> f32 {
        self.panel.y + self.k * 8.0
    }
}

impl Stack {
    pub fn new(view: &View, design_w: f32) -> Self {
        Self {
            view: *view,
            design_w,
            ribbon: None,
            rise: 0.0,
            parts: vec![],
        }
    }

    pub fn ribbon(mut self, title: &str) -> Self {
        self.ribbon = Some(title.to_string());
        self
    }

    pub fn rise(mut self, units: f32) -> Self {
        self.rise = units;
        self
    }

    pub fn space(&mut self, design: f32) -> usize {
        self.parts.push(Part::Space(design));
        self.parts.len() - 1
    }

    pub fn text(&mut self, size: f32, s: &str) -> usize {
        self.parts.push(Part::Text {
            size,
            text: s.to_string(),
        });
        self.parts.len() - 1
    }

    pub fn block(&mut self, design: f32) -> usize {
        self.block_min(design, 0.0)
    }

    pub fn block_min(&mut self, design: f32, min: f32) -> usize {
        self.parts.push(Part::Block(design, min));
        self.parts.len() - 1
    }

    pub fn buttons(&mut self, design_h: f32, labels: &[&str]) -> usize {
        self.parts.push(Part::Buttons {
            h: design_h,
            labels: labels.iter().map(|s| s.to_string()).collect(),
        });
        self.parts.len() - 1
    }

    pub fn button(&mut self, design_h: f32, label: &str) -> usize {
        self.buttons(design_h, &[label])
    }

    fn pad(&self, k: f32) -> f32 {
        (34.0 * k).max(self.view.gap())
    }

    fn top_room(&self, k: f32) -> f32 {
        match &self.ribbon {
            Some(t) => ribbon_h(&self.view, t, k) * 0.5 + self.view.gap().max(12.0 * k),
            None => self.pad(k),
        }
    }

    fn row_fits(&self, inner: f32, h: f32, labels: &[String], force: bool) -> bool {
        if labels.len() < 2 {
            return true;
        }
        let gap = self.view.gap();
        let w = (inner - gap * (labels.len() as f32 - 1.0)) / labels.len() as f32;
        if force {
            return w >= self.view.touch();
        }
        let size = h * 0.46;
        w >= self.view.touch() * 1.6
            && labels
                .iter()
                .all(|l| measure(l, size * 0.8) <= w - h * 0.45)
    }

    fn heights(
        &self,
        k: f32,
        inner: f32,
        force_rows: bool,
    ) -> (f32, Vec<(f32, bool, f32, Vec<String>)>) {
        let v = &self.view;
        let gap = v.gap();
        let mut total = self.top_room(k) + self.pad(k);
        let mut out = vec![];
        for p in &self.parts {
            let item = match p {
                Part::Space(d) => (d * k, false, 0.0, vec![]),
                Part::Block(d, min) => ((d * k).max(*min), false, 0.0, vec![]),
                Part::Text { size, text } => {
                    let s = (size * k).max(v.text(*size)).min(size * k.max(v.text_k()));
                    let lines = wrap(text, s, inner);
                    let s = lines
                        .iter()
                        .map(|l| fit_size(l, s, inner))
                        .fold(s, f32::min);
                    (lines.len() as f32 * s * LINE, false, s, lines)
                }
                Part::Buttons { h, labels } => {
                    let bh = v.tap(h * k);
                    let row = self.row_fits(inner, bh, labels, force_rows);
                    let n = if row { 1.0 } else { labels.len() as f32 };
                    (n * bh + (n - 1.0) * gap, row, bh, labels.clone())
                }
            };
            total += item.0;
            out.push(item);
        }
        total += gap * (self.parts.len().saturating_sub(1)) as f32 * 0.5;
        (total, out)
    }

    pub fn layout(&self) -> Sheet {
        let v = &self.view;
        let panel_w = v.sheet_w(self.design_w);
        let k_max = (panel_w / self.design_w).max(0.4);
        let ribbon_top = |k: f32| {
            self.ribbon
                .as_ref()
                .map(|t| ribbon_h(v, t, k) * 0.5)
                .unwrap_or(0.0)
        };
        let margin = v.edge().min(20.0);
        let mut pick = None;
        for force in [false, true] {
            for step in 0..=16 {
                let k = k_max - (k_max - 0.4).max(0.0) * step as f32 / 16.0;
                let inner = panel_w - 2.0 * self.pad(k);
                let (total, items) = self.heights(k, inner, force);
                if total <= v.h - 2.0 * margin - ribbon_top(k) {
                    pick = Some((k, total, items));
                    break;
                }
            }
            if pick.is_some() {
                break;
            }
        }
        let (k, total, items, fits) = match pick {
            Some((k, t, i)) => (k, t, i, true),
            None => {
                let k = 0.4;
                let inner = panel_w - 2.0 * self.pad(k);
                let (t, i) = self.heights(k, inner, true);
                (k, t, i, false)
            }
        };
        let ribbon_top = ribbon_top(k);
        let pad = self.pad(k);
        let inner = panel_w - 2.0 * pad;
        let cy = (v.h / 2.0 + ribbon_top / 2.0 - self.rise)
            .max(ribbon_top + margin + total / 2.0)
            .min(v.h - margin - total / 2.0);
        let panel = Rect::new(v.w / 2.0 - panel_w / 2.0, cy - total / 2.0, panel_w, total);
        let gap = v.gap();
        let mut y = panel.y + self.top_room(k);
        let x = panel.x + pad;
        let mut parts = vec![];
        for (i, (h, row, size, lines)) in items.into_iter().enumerate() {
            let rect = Rect::new(x, y, inner, h);
            let mut laid = Laid {
                rect,
                rects: vec![],
                lines: vec![],
                size,
            };
            match &self.parts[i] {
                Part::Buttons { labels, .. } => {
                    let n = labels.len().max(1);
                    laid.rects = if row {
                        let w = (inner - gap * (n as f32 - 1.0)) / n as f32;
                        (0..n)
                            .map(|j| Rect::new(x + j as f32 * (w + gap), y, w, size))
                            .collect()
                    } else {
                        let w = if n == 1 {
                            inner.min(
                                (measure(&labels[0], size * 0.46) + size * 1.6).max(inner * 0.62),
                            )
                        } else {
                            inner
                        };
                        (0..n)
                            .map(|j| {
                                Rect::new(
                                    x + (inner - w) / 2.0,
                                    y + j as f32 * (size + gap),
                                    w,
                                    size,
                                )
                            })
                            .collect()
                    };
                    laid.lines = labels.clone();
                }
                Part::Text { .. } => laid.lines = lines,
                _ => {}
            }
            parts.push(laid);
            y += h + gap * 0.5;
        }
        let ribbon = self
            .ribbon
            .as_ref()
            .map(|t| ribbon_rect(v, t, panel.center().x, panel.y + 8.0 * k, k));
        Sheet {
            panel,
            ribbon,
            k,
            fits,
            parts,
        }
    }
}

pub fn board_shape(game: &str, rank: Option<usize>) -> (usize, bool) {
    if let Some(online) = crate::net::online_board(game) {
        return (online.len(), online.iter().any(|r| !r.mine));
    }
    let n = crate::profile::board(game).len().min(5);
    let n = if rank.is_some_and(|r| r >= 5) {
        n.max(5)
    } else {
        n
    };
    (n, false)
}

pub fn results_for(
    view: &View,
    game: &str,
    rank: Option<usize>,
    title: &str,
    line: &str,
    actions: &[&str],
) -> Results {
    let (rows, reportable) = board_shape(game, rank);
    results_layout(view, title, line, actions, rows, reportable)
}

pub struct Results {
    pub sheet: Sheet,
    pub line: usize,
    pub best: usize,
    pub board: usize,
    pub action: usize,
    pub rows: usize,
    pub row_h: f32,
}

impl Results {
    pub fn row(&self, i: usize) -> Rect {
        let b = self.sheet.rect(self.board);
        Rect::new(b.x, b.y + i as f32 * self.row_h, b.w, self.row_h - 4.0)
    }

    pub fn report_hit(&self, row: Rect, score_w: f32) -> Rect {
        let d = row.h.max(48.0 * self.sheet.k);
        Rect::new(
            row.x + row.w - score_w - d - 18.0 * self.sheet.k,
            row.y,
            d,
            row.h,
        )
    }

    pub fn buttons(&self) -> Vec<Rect> {
        self.sheet.parts[self.action].rects.clone()
    }
}

pub fn results_layout(
    view: &View,
    title: &str,
    line: &str,
    actions: &[&str],
    rows: usize,
    reportable: bool,
) -> Results {
    let mut n = rows;
    loop {
        let mut s = Stack::new(view, 580.0).ribbon(title);
        let line_i = s.text(34.0, line);
        let best = s.block(40.0);
        let k_guess = view.sheet_w(580.0) / 580.0;
        let min_row = if reportable { view.touch() + 4.0 } else { 0.0 };
        let row_h = (46.0 * k_guess.max(view.text_k()).min(2.4)).max(min_row);
        let board = s.block_min(n as f32 * 46.0, n as f32 * row_h);
        let action = s.buttons(78.0, actions);
        let sheet = s.layout();
        if sheet.fits || n == 0 {
            let row_h = if n == 0 {
                0.0
            } else {
                sheet.rect(board).h / n as f32
            };
            return Results {
                sheet,
                line: line_i,
                best,
                board,
                action,
                rows: n,
                row_h,
            };
        }
        n -= 1;
    }
}

pub fn panel_layout(view: &View, title: &str, lines: &[&str]) -> (Sheet, Vec<usize>) {
    let mut s = Stack::new(view, 560.0).ribbon(title);
    let mut ids = vec![];
    for (i, l) in lines.iter().enumerate() {
        let last = i + 1 == lines.len();
        ids.push(if last && lines.len() > 1 {
            s.button(78.0, l)
        } else if l.is_empty() {
            s.space(30.0)
        } else {
            s.text(34.0, l)
        });
    }
    (s.layout(), ids)
}

pub struct Keys {
    pub keys: Vec<Rect>,
    pub cols: usize,
    pub bottom: f32,
}

pub fn keypad(view: &View, area: Rect, n: usize, max_cols: usize, design_h: f32) -> Keys {
    let gap = view.key_gap();
    let min_w = view.key_w().max(28.0);
    let fit = ((area.w + gap) / (min_w + gap)).floor().max(1.0) as usize;
    let cols = fit.min(max_cols).max(1);
    let rows = n.div_ceil(cols);
    let kw = ((area.w - gap * (cols as f32 - 1.0)) / cols as f32)
        .min(design_h * 1.3)
        .max(min_w);
    let room = (area.h - gap * (rows as f32 - 1.0)) / rows as f32;
    let kh = room
        .min(kw.max(design_h))
        .min(design_h * 1.6)
        .max(view.touch())
        .max(28.0);
    let grid_w = cols as f32 * kw + (cols as f32 - 1.0) * gap;
    let left = area.x + (area.w - grid_w) / 2.0;
    let keys: Vec<Rect> = (0..n)
        .map(|i| {
            let (c, r) = (i % cols, i / cols);
            Rect::new(
                left + c as f32 * (kw + gap),
                area.y + r as f32 * (kh + gap),
                kw,
                kh,
            )
        })
        .collect();
    let bottom = keys.last().map_or(area.y, |k| k.y + k.h);
    Keys { keys, cols, bottom }
}

pub fn row(area: Rect, n: usize, gap: f32) -> Vec<Rect> {
    let n = n.max(1);
    let w = (area.w - gap * (n as f32 - 1.0)) / n as f32;
    (0..n)
        .map(|i| Rect::new(area.x + i as f32 * (w + gap), area.y, w, area.h))
        .collect()
}

pub fn phone_portrait() -> View {
    View::device(
        390.0,
        844.0,
        3.0,
        460.0,
        Insets {
            top: 47.0,
            bottom: 34.0,
            ..Default::default()
        },
    )
}

pub fn phone_landscape() -> View {
    View::device(
        844.0,
        390.0,
        3.0,
        460.0,
        Insets {
            left: 47.0,
            right: 47.0,
            bottom: 21.0,
            ..Default::default()
        },
    )
}

pub fn tablet_portrait() -> View {
    View::device(
        820.0,
        1180.0,
        2.0,
        264.0,
        Insets {
            top: 24.0,
            bottom: 20.0,
            ..Default::default()
        },
    )
}

pub fn tablet_landscape() -> View {
    View::device(
        1180.0,
        820.0,
        2.0,
        264.0,
        Insets {
            top: 24.0,
            bottom: 20.0,
            ..Default::default()
        },
    )
}

pub fn desktop() -> View {
    View::device(1920.0, 1080.0, 1.0, 96.0, Insets::default())
}

pub fn test_views() -> [(&'static str, View); 5] {
    [
        ("phone portrait", phone_portrait()),
        ("phone landscape", phone_landscape()),
        ("tablet portrait", tablet_portrait()),
        ("tablet landscape", tablet_landscape()),
        ("desktop", desktop()),
    ]
}

pub fn detail(view: &View, dpr: f32) -> f32 {
    view.scale * dpr
}

pub fn check_buttons(view: &View, what: &str, rects: &[Rect]) -> Result<(), String> {
    check_targets(view, what, rects, TOUCH_MM, TOUCH_MM)
}

pub fn check_keys(view: &View, what: &str, rects: &[Rect]) -> Result<(), String> {
    check_targets(view, what, rects, KEY_W_MM, TOUCH_MM)?;
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            let dx = (b.x - (a.x + a.w)).max(a.x - (b.x + b.w));
            let dy = (b.y - (a.y + a.h)).max(a.y - (b.y + b.h));
            let apart = view.to_mm(dx.max(dy));
            if apart < KEY_GAP_MM - 0.01 {
                return Err(format!(
                    "{what}: keys {a:?} and {b:?} only {apart:.2} mm apart"
                ));
            }
        }
    }
    Ok(())
}

fn check_targets(
    view: &View,
    what: &str,
    rects: &[Rect],
    w_mm: f32,
    h_mm: f32,
) -> Result<(), String> {
    for r in rects {
        let (w, h) = (view.to_mm(r.w), view.to_mm(r.h));
        if w < w_mm - 0.01 || h < h_mm - 0.01 {
            return Err(format!(
                "{what}: {r:?} is {w:.1}x{h:.1} mm, needs {w_mm}x{h_mm} mm"
            ));
        }
        check_inside(view, what, *r)?;
    }
    Ok(())
}

pub fn check_inside(view: &View, what: &str, r: Rect) -> Result<(), String> {
    let e = 0.5;
    if r.x < -e || r.y < -e || r.x + r.w > view.w + e || r.y + r.h > view.h + e {
        return Err(format!(
            "{what}: {r:?} leaves the safe area {}x{}",
            view.w, view.h
        ));
    }
    Ok(())
}

pub fn check_apart(what: &str, rects: &[Rect]) -> Result<(), String> {
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            let ox = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
            let oy = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
            if ox > 0.5 && oy > 0.5 {
                return Err(format!("{what}: {a:?} overlaps {b:?}"));
            }
        }
    }
    Ok(())
}

pub fn check_sheet(view: &View, what: &str, sheet: &Sheet) -> Result<(), String> {
    check_inside(view, what, sheet.panel)?;
    if let Some((r, _)) = sheet.ribbon {
        check_inside(view, &format!("{what} ribbon"), r)?;
    }
    let buttons = sheet.buttons();
    check_buttons(view, what, &buttons)?;
    check_apart(what, &buttons)?;
    for (i, p) in sheet.parts.iter().enumerate() {
        if p.rect.y < sheet.panel.y - 0.5
            || p.rect.y + p.rect.h > sheet.panel.y + sheet.panel.h + 0.5
        {
            return Err(format!(
                "{what}: part {i} {:?} spills out of {:?}",
                p.rect, sheet.panel
            ));
        }
        for r in &p.rects {
            if r.x < sheet.panel.x - 0.5 || r.x + r.w > sheet.panel.x + sheet.panel.w + 0.5 {
                return Err(format!(
                    "{what}: button {r:?} spills out of {:?}",
                    sheet.panel
                ));
            }
        }
        if !p.lines.is_empty() && p.size > 0.0 && p.rects.is_empty() {
            for l in &p.lines {
                if measure(l, p.size) > p.rect.w + 1.0 {
                    return Err(format!("{what}: line {l:?} is wider than its panel"));
                }
            }
        }
    }
    let parts: Vec<Rect> = sheet
        .parts
        .iter()
        .filter(|p| p.rect.h > 0.5)
        .map(|p| p.rect)
        .collect();
    check_apart(what, &parts)
}

pub fn check_text(view: &View, dpr: f32, what: &str, size: f32) -> Result<(), String> {
    let px = size * detail(view, dpr);
    if px > MAX_GLYPH {
        return Err(format!(
            "{what}: {size} units draws at {px:.0} px, past the {MAX_GLYPH} px glyph bucket"
        ));
    }
    Ok(())
}

pub fn check_sheet_text(view: &View, dpr: f32, what: &str, sheet: &Sheet) -> Result<(), String> {
    for p in &sheet.parts {
        if p.rects.is_empty() {
            check_text(view, dpr, what, p.size)?;
        } else {
            for (r, l) in p.rects.iter().zip(&p.lines) {
                check_text(view, dpr, what, label_size(l, *r))?;
            }
        }
    }
    if let Some((_, size)) = sheet.ribbon {
        check_text(view, dpr, what, size)?;
    }
    Ok(())
}

pub fn dpr_of(name: &str) -> f32 {
    match name {
        "phone portrait" | "phone landscape" => 3.0,
        "tablet portrait" | "tablet landscape" => 2.0,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_views_carry_their_real_density() {
        let p = phone_portrait();
        assert!((p.to_mm(p.w) - 390.0 / (460.0 / 25.4 / 3.0)).abs() < 0.1);
        let t = tablet_portrait();
        assert!((t.to_mm(t.w) - 820.0 / (264.0 / 25.4 / 2.0)).abs() < 0.1);
        for (name, v) in test_views() {
            assert!(v.touch() > 0.0, "{name}");
            assert!(v.to_mm(v.touch()) >= TOUCH_MM - 0.01, "{name}");
        }
    }

    #[test]
    fn a_portrait_sheet_takes_most_of_the_short_side() {
        for v in [phone_portrait(), tablet_portrait()] {
            let w = v.sheet_w(560.0);
            assert!(w >= v.short() * 0.85, "{w} of {}", v.short());
            assert!(w <= v.w - 2.0 * v.edge() + 0.01);
        }
    }

    #[test]
    fn a_three_button_sheet_fits_every_screen_with_twelve_mm_buttons() {
        for (name, v) in test_views() {
            let mut s = Stack::new(&v, 580.0).ribbon("Sky Jumper is locked");
            s.text(38.0, "Sky Jumper is locked");
            s.text(24.0, "Learn to earn 3 stars. You have 1.");
            s.text(22.0, "Get questions right, quickly, to earn stars.");
            s.buttons(
                78.0,
                &["Learn to unlock", "Not now", "Grown up? Hold to open all"],
            );
            let sheet = s.layout();
            check_sheet(&v, name, &sheet).unwrap();
            check_sheet_text(&v, dpr_of(name), name, &sheet).unwrap();
        }
    }

    #[test]
    fn side_by_side_buttons_stack_when_the_labels_do_not_fit() {
        let v = phone_portrait();
        let mut s = Stack::new(&v, 560.0);
        let i = s.buttons(78.0, &["Hide and report", "Cancel"]);
        let sheet = s.layout();
        let (a, b) = (sheet.button(i, 0), sheet.button(i, 1));
        assert!(b.y >= a.y + a.h, "{a:?} {b:?}");
        let v = tablet_landscape();
        let mut s = Stack::new(&v, 560.0);
        let i = s.buttons(78.0, &["Again", "Levels"]);
        let sheet = s.layout();
        assert_eq!(sheet.button(i, 0).y, sheet.button(i, 1).y);
    }

    #[test]
    fn results_and_panels_fit_every_screen() {
        for (name, v) in test_views() {
            for rows in [0, 3, 5] {
                for reportable in [false, true] {
                    let r = results_layout(
                        &v,
                        "Nice try!",
                        "Cleared 40 of 52 bricks",
                        &["Try again", "Levels"],
                        rows,
                        reportable,
                    );
                    let what = format!("{name} results {rows} rows");
                    check_sheet(&v, &what, &r.sheet).unwrap();
                    check_sheet_text(&v, dpr_of(name), &what, &r.sheet).unwrap();
                    if reportable && r.rows > 0 {
                        let row = r.row(0);
                        check_buttons(&v, &what, &[r.report_hit(row, 40.0)]).unwrap();
                    }
                    if v.portrait() {
                        assert!(r.sheet.panel.w >= v.short() * 0.85, "{what}");
                    }
                }
            }
            let (p, _) = panel_layout(&v, "Sky Jumper", &["Tap to jump", "", "Tap to start"]);
            check_sheet(&v, name, &p).unwrap();
            check_sheet_text(&v, dpr_of(name), name, &p).unwrap();
        }
    }

    #[test]
    fn long_lines_wrap_inside_the_panel() {
        let lines = wrap(
            "We hide them from your boards and tell a grown up.",
            50.0,
            500.0,
        );
        assert!(lines.len() >= 2);
        assert!(lines.iter().all(|l| measure(l, 50.0) <= 500.0));
        assert_eq!(wrap("", 30.0, 100.0), vec![String::new()]);
    }

    #[test]
    fn a_seven_column_keypad_fits_a_portrait_phone() {
        let v = phone_portrait();
        let area = Rect::new(0.0, 400.0, v.w, 900.0);
        let k = keypad(&v, area, 27, 10, 68.0);
        assert_eq!(k.cols, 7);
        check_keys(&v, "keypad", &k.keys).unwrap();
        for (name, v) in test_views() {
            let area = Rect::new(0.0, 0.0, v.w, v.h);
            let k = keypad(&v, area, 32, 10, 68.0);
            check_keys(&v, name, &k.keys).unwrap();
        }
    }

    #[test]
    fn corner_buttons_are_twelve_mm_on_every_screen() {
        for (name, v) in test_views() {
            let r = [v.home(), v.top_right(0), v.top_right(1)];
            check_buttons(&v, name, &r).unwrap();
            check_apart(name, &r).unwrap();
            assert!(v.hud_left() >= v.home().x + v.home().w);
        }
    }
}
