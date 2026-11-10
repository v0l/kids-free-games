use super::{count, mathpop, spell, trains};
use crate::engine::View;
use crate::geom::Rect;
use crate::sizing::*;

fn targets(v: &View, what: &str, rects: &[Rect]) -> Vec<String> {
    let mut out = vec![];
    if let Err(e) = check_buttons(v, what, rects) {
        out.push(e);
    }
    if let Err(e) = check_apart(what, rects) {
        out.push(e);
    }
    out
}

fn panel(v: &View, what: &str, p: Rect, inside: &[Rect]) -> Vec<String> {
    let mut out = vec![];
    if let Err(e) = check_inside(v, what, p) {
        out.push(e);
    }
    for r in inside {
        if r.x < p.x - 0.5
            || r.y < p.y - 0.5
            || r.x + r.w > p.x + p.w + 0.5
            || r.y + r.h > p.y + p.h + 0.5
        {
            out.push(format!("{what}: {r:?} spills out of panel {p:?}"));
        }
    }
    if v.portrait() && p.w < v.short() * 0.85 {
        out.push(format!("{what}: panel {p:?} is narrow for portrait"));
    }
    out
}

fn all_problems(v: &View) -> Vec<String> {
    use super::*;
    let mut out = vec![];
    let mut t = |what: &str, r: &[Rect]| out.extend(targets(v, what, r));
    let b = breakout::level_tiles(v);
    t("breakout levels", &b);
    t("breakout serve", &[breakout::serve_levels_button(v)]);
    t("breakout lost", &[breakout::lost_levels_button(v, Some(0))]);
    t(
        "breakout cleared",
        &[breakout::next_button(v), breakout::levels_button(v)],
    );
    t("memory tiles", &memory::Memory::tiles(v));
    t(
        "memory play",
        &[
            memory::Memory::play_button(v),
            memory::Memory::levels_button(v),
        ],
    );
    t("memory done", &memory::Memory::done_buttons(v));
    let m = mathpop::levels_layout(v);
    let mut mr = m.tiles.clone();
    mr.push(m.back);
    t("mathpop levels", &mr);
    let d = mathpop::done_layout(v);
    t("mathpop done", &[d.next, d.again, d.levels]);
    t("mathpop levels button", &[mathpop::levels_button(v)]);
    let s = spell::levels_layout(v);
    let mut sr = s.tiles.clone();
    sr.push(s.back);
    t("spell levels", &sr);
    let sd = spell::done_layout(v);
    t("spell done", &[sd.next, sd.again, sd.levels]);
    t("spell levels button", &[spell::levels_button(v)]);
    let (a, bb) = merge::bottom_buttons(v);
    t("merge bottom", &[a, bb]);
    t("merge levels", &merge::level_rects(v));
    let (_, n, l) = merge::complete_layout(v);
    t("merge complete", &[n, l]);
    let bs = ballsort::buttons(v);
    t(
        "ballsort buttons",
        &[bs.undo, bs.restart, bs.hint, bs.levels],
    );
    for skip in [false, true] {
        let w = ballsort::win_layout(v, skip, Some(0));
        let r: Vec<Rect> = if skip {
            vec![w.skip, w.levels]
        } else {
            vec![w.levels]
        };
        t("ballsort win", &r);
    }
    let bsl = ballsort::select_layout(v);
    let mut cells = bsl.cells.clone();
    cells.push(bsl.play);
    t("ballsort select", &cells);
    let hs = helix::select_layout(v);
    let mut cells = hs.cells.clone();
    cells.push(hs.play);
    t("helix select", &cells);
    t("helix levels", &[helix::levels_button(v)]);
    let tl = clash::title_layout(v);
    t(
        "clash title",
        &[tl.prev, tl.next, tl.arena, tl.survival, tl.tower],
    );
    let tw = clash::tower_layout(v);
    t("clash tower", &[tw.prev, tw.next, tw.play, tw.back]);
    t("clash over", &[clash::over_buttons(v).1]);
    t("count levels", &count::level_tiles(v));
    t("count play", &[count::play_button(v)]);
    t("count levels button", &[count::levels_button(v)]);
    t("trains levels", &trains::level_tiles(v));
    t("trains play", &[trains::play_button(v)]);
    t("trains levels button", &[trains::levels_button(v)]);
    t("garden beds", &garden::bed_tiles(v));
    t("garden play", &[garden::play_button(v)]);
    t("garden beds button", &[garden::beds_button(v)]);
    t("stack picker", &stack::picker_layout(v));
    t(
        "stack buttons",
        &[stack::levels_button(v), stack::close_button(v)],
    );
    t("paper again", &[paper::play_again_button(v)]);
    t(
        "crossy checkpoint",
        &[crossy::Crossy::new().checkpoint_button(v)],
    );
    for n in [2, 3, 4] {
        t("count answers", &count::button_rects(v, n));
    }
    t("trains answers", &trains::button_rects(v));
    t("garden seeds", &garden::seed_rects(v));
    t("memory levels", &memory::Memory::tiles(v));
    let mut p = |what: &str, r: Rect, inside: &[Rect]| out.extend(panel(v, what, r, inside));
    p(
        "breakout cleared",
        breakout::panel_rect(v),
        &[breakout::next_button(v), breakout::levels_button(v)],
    );
    p(
        "memory done",
        memory::Memory::done_panel(v),
        &memory::Memory::done_buttons(v),
    );
    p("mathpop levels", m.sheet.panel, &mr);
    p("mathpop done", d.panel, &[d.next, d.again, d.levels]);
    p("spell levels", s.sheet.panel, &sr);
    p("spell done", sd.panel, &[sd.next, sd.again, sd.levels]);
    let (mp, n, l) = merge::complete_layout(v);
    p("merge complete", mp, &[n, l]);
    p("paper death", paper::death_panel(v), &[]);
    let mut s = |what: &str, sh: &Sheet| {
        if let Err(e) = check_sheet(v, what, sh) {
            out.push(e);
        }
    };
    s("helix win", &helix::win_sheet(v).sheet);
    s("merge complete sheet", &merge::complete_sheet(v).sheet);
    s("breakout cleared sheet", &breakout::cleared_sheet(v).sheet);
    s("mathpop done sheet", &mathpop::done_layout(v).done.sheet);
    s("spell done sheet", &spell::done_layout(v).done.sheet);
    s("memory done sheet", &memory::Memory::done_layout(v).sheet);
    s("mathpop levels sheet", &mathpop::levels_layout(v).sheet);
    for skip in [&ballsort::WIN_LABELS[..], &ballsort::WIN_SKIP_LABELS[..]] {
        s(
            "ballsort win sheet",
            &ballsort::win_results(v, skip, Some(0)).sheet,
        );
    }
    s(
        "breakout lost sheet",
        &results_for(
            v,
            breakout::KEY,
            Some(0),
            "Nice try!",
            "x",
            &breakout::LOST_LABELS,
        )
        .sheet,
    );
    let (panel, _) = panel_layout(
        v,
        "Lovely planting!",
        &["", "5 of 5 right first time  1.2 s each", "Keep growing"],
    );
    s("report panel", &panel);
    out
}

#[test]
fn every_learning_top_bar_fits_between_home_and_levels() {
    use crate::learn::{hud_problems, HudSpec, LevelsAt, UP_STREAK};
    let mathpop_titles: Vec<String> = mathpop::LEVEL_NAMES
        .iter()
        .enumerate()
        .map(|(i, n)| format!("Level {}: {}", i + 1, n))
        .collect();
    let spell_goals: Vec<String> = spell::LEVELS
        .iter()
        .map(|l| format!("{}: spell 10 words", l.name))
        .collect();
    let count_titles: Vec<String> = (0..count::RANGES.len()).map(count::level_name).collect();
    let count_goals: Vec<String> = (0..count::RANGES.len())
        .map(|i| {
            format!(
                "Get {UP_STREAK} right in a row to reach {}",
                count::level_name(i)
            )
        })
        .collect();
    let trains_goals: Vec<String> = (0..trains::LEVELS)
        .map(|i| {
            format!(
                "Get {UP_STREAK} right in a row to reach {}",
                trains::level_name(i)
            )
        })
        .collect();
    fn refs(v: &[String]) -> Vec<&str> {
        v.iter().map(|s| s.as_str()).collect()
    }
    let mut bad = vec![];
    for (name, v) in test_views() {
        for t in &mathpop_titles {
            let spec = HudSpec {
                title: t,
                levels: LevelsAt::Top,
                stars: 0,
                progress: 18.0,
                goal: "",
            };
            bad.extend(hud_problems(&v, &format!("{name} mathpop"), &spec, &[]));
        }
        let spec = HudSpec {
            title: "Level 10",
            levels: LevelsAt::Top,
            stars: 0,
            progress: 40.0,
            goal: spell::HUD_GOAL,
        };
        bad.extend(hud_problems(
            &v,
            &format!("{name} spell"),
            &spec,
            &refs(&spell_goals),
        ));
        for t in &count_titles {
            let spec = HudSpec {
                title: t,
                levels: LevelsAt::Goal,
                stars: count::RANGES.len() as u32,
                progress: 18.0,
                goal: count::HUD_GOAL,
            };
            bad.extend(hud_problems(
                &v,
                &format!("{name} count"),
                &spec,
                &refs(&count_goals),
            ));
        }
        let spec = HudSpec {
            title: "Level 8",
            levels: LevelsAt::Goal,
            stars: trains::LEVELS as u32,
            progress: 18.0,
            goal: trains::HUD_GOAL,
        };
        bad.extend(hud_problems(
            &v,
            &format!("{name} trains"),
            &spec,
            &refs(&trains_goals),
        ));
        let hb = [
            mathpop::hud(&v),
            spell::hud(&v),
            count::hud(&v),
            trains::hud(&v),
        ];
        for h in &hb {
            if let Some(l) = h.levels {
                if l.overlaps(&h.bar.title) {
                    bad.push(format!(
                        "{name}: Levels {l:?} overlaps the title room {:?}",
                        h.bar.title
                    ));
                }
            }
        }
        if mathpop::question_rect(&v).y < mathpop::hud(&v).bottom - 0.5 {
            bad.push(format!("{name}: mathpop question under the top bar"));
        }
        if spell::layout(&v, 5, 8).picture.y < spell::hud(&v).bottom - 0.5 {
            bad.push(format!("{name}: spell picture under the top bar"));
        }
        if count::prompt_y(&v) - count::prompt_size(&v) * 0.6 < count::hud(&v).bottom - 0.5 {
            bad.push(format!("{name}: count prompt under the top bar"));
        }
        if trains::prompt_rect(&v).y < trains::hud(&v).bottom - 0.5 {
            bad.push(format!("{name}: trains prompt under the top bar"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} problems:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn every_game_menu_and_modal_is_twelve_mm_and_fits() {
    crate::save::reset_for_test();
    crate::profile::create("Mia", 0);
    let mut bad = vec![];
    for (name, v) in test_views() {
        for e in all_problems(&v) {
            bad.push(format!("{name}: {e}"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} problems:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
