use super::layout::*;
use crate::geom::Rect;

const TOLERANCE: f32 = 1e-3;
const DOORWAY_MAX: f32 = 1.5;
const DOOR_HEIGHT: i32 = 1100;

#[derive(Clone, Debug, PartialEq)]
pub struct Door {
    pub name: String,
    pub wall: String,
    pub locked: bool,
    pub x: f32,
    pub z: f32,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub text: String,
    pub doors: Vec<Door>,
    pub walls: Vec<String>,
    pub caps: usize,
}

#[derive(Clone, Copy)]
struct Piece {
    a: f32,
    b: f32,
    locked: bool,
}

struct Run {
    vertical: bool,
    line: f32,
    start: f32,
    end: f32,
    doors: Vec<(f32, f32, bool)>,
}

fn pieces(solids: &[(Rect, Solid)]) -> Vec<(bool, f32, Piece)> {
    solids
        .iter()
        .filter(|(_, k)| *k != Solid::Thing)
        .map(|(r, k)| {
            let locked = *k == Solid::Locked;
            if r.h > r.w {
                (true, r.x + r.w / 2.0, Piece { a: r.y, b: r.y + r.h, locked })
            } else {
                (false, r.y + r.h / 2.0, Piece { a: r.x, b: r.x + r.w, locked })
            }
        })
        .collect()
}

fn runs(all: &[(bool, f32, Piece)]) -> Vec<Run> {
    let mut sorted: Vec<&(bool, f32, Piece)> = all.iter().collect();
    sorted.sort_by(|p, q| {
        (p.0, (p.1 * 1000.0).round() as i32, (p.2.a * 1000.0).round() as i32)
            .cmp(&(q.0, (q.1 * 1000.0).round() as i32, (q.2.a * 1000.0).round() as i32))
    });
    let mut out: Vec<Run> = vec![];
    let mut prev_locked = false;
    for (vertical, line, p) in sorted {
        let joins = out.last().is_some_and(|r| {
            r.vertical == *vertical && (r.line - line).abs() < TOLERANCE && {
                let gap = p.a - r.end;
                (gap.abs() < TOLERANCE && (p.locked || prev_locked))
                    || (gap > TOLERANCE && gap <= DOORWAY_MAX)
            }
        });
        if joins {
            let r = out.last_mut().unwrap();
            let gap = p.a - r.end;
            if p.locked {
                r.doors.push((p.a, p.b, true));
            } else if gap > TOLERANCE {
                r.doors.push((r.end, p.a, false));
            }
            r.end = p.b;
        } else {
            let mut r = Run {
                vertical: *vertical,
                line: *line,
                start: p.a,
                end: p.b,
                doors: vec![],
            };
            if p.locked {
                r.doors.push((p.a, p.b, true));
            }
            out.push(r);
        }
        prev_locked = p.locked;
    }
    out
}

fn door_name(r: &Run, centre: f32, end_z: f32) -> Option<String> {
    let near = |a: f32, b: f32| (a - b).abs() < 0.05;
    if r.vertical {
        for k in 0..MAX_HOTEL as usize * 2 {
            if near(r.line, room_side(k) * CORR) && near(centre, wing_z(k / 2)) {
                return Some(format!("room{k}"));
            }
        }
        if near(r.line, -HALL_W) && near(centre, SIDE_DOOR) {
            return Some("toilets".into());
        }
        if near(r.line, HALL_W) && near(centre, SIDE_DOOR) {
            return Some("cafe".into());
        }
    } else {
        if near(r.line, end_z) && centre.abs() < 0.05 {
            return Some("pool".into());
        }
        for w in 1..MAX_HOTEL as usize {
            if near(r.line, -(w as f32) * WING_D) && centre.abs() < 0.05 {
                return Some(format!("gate{w}"));
            }
        }
    }
    None
}

fn wall_name(r: &Run, doors: &[String], index: usize, end_z: f32) -> String {
    if let Some(d) = doors.first() {
        return format!("{d}_wall");
    }
    let mid = (r.start + r.end) / 2.0;
    if r.vertical && (r.line.abs() - HALL_W).abs() < 0.05 && mid < 0.0 {
        let w = (-mid / WING_D) as usize;
        let side = usize::from(r.line > 0.0);
        return format!("room{}_outer", w * 2 + side);
    }
    if !r.vertical && (r.line - end_z).abs() < 0.05 {
        return format!("end_wall{index}");
    }
    format!("wall{index}")
}

fn mm(v: f32) -> i32 {
    (v * 1000.0).round() as i32
}

pub fn plan(lay: &Layout, built: &dyn Fn(Build) -> bool) -> Plan {
    let solids = lay.solids(built);
    let all = pieces(&solids);
    let found = runs(&all);
    let end_z = lay.end_z();
    let thickness = mm(WALL_T);
    let mut text = String::new();
    text.push_str("units mm\n");
    text.push_str("level L0 0 \"Floor\"\n");
    text.push_str("level L1 1140 \"Wall\"\n");
    text.push_str("level L2 1200 \"Cap\"\n");
    text.push_str(&format!("wall_type wall block:{thickness}\n"));
    text.push_str(&format!("wall_type cap plaster:{thickness}\n"));
    text.push_str("building \"shell\"\n");
    let mut doors = vec![];
    let mut walls = vec![];
    let mut doorless = 0usize;
    for (i, r) in found.iter().enumerate() {
        let names: Vec<Option<String>> = r
            .doors
            .iter()
            .map(|(a, b, _)| door_name(r, (a + b) / 2.0, end_z))
            .collect();
        let named: Vec<String> = names.iter().flatten().cloned().collect();
        let label = wall_name(r, &named, i, end_z);
        if named.is_empty() {
            doorless += 1;
        }
        let point = |along: f32| {
            if r.vertical {
                (mm(r.line), -mm(along))
            } else {
                (mm(along), -mm(r.line))
            }
        };
        let (x0, y0) = point(r.start);
        let (x1, y1) = point(r.end);
        text.push_str(&format!(
            "{label}: wall {x0},{y0} {x1},{y1} type=wall base=L0 top=L1 justify=centre\n"
        ));
        walls.push(label.clone());
        for (k, (a, b, locked)) in r.doors.iter().enumerate() {
            let centre = (a + b) / 2.0;
            let name = names[k].clone().unwrap_or_else(|| format!("door{}", doors.len()));
            text.push_str(&format!(
                "{name}: door on={label} at={} w={} h={DOOR_HEIGHT}\n",
                mm(centre - r.start),
                mm(b - a)
            ));
            let (x, z) = if r.vertical { (r.line, centre) } else { (centre, r.line) };
            doors.push(Door {
                name,
                wall: label.clone(),
                locked: *locked,
                x,
                z,
                width: b - a,
            });
        }
    }
    let _ = doorless;
    let mut caps = 0;
    for (vertical, line, p) in all.iter().filter(|(_, _, p)| !p.locked) {
        let (x0, y0, x1, y1) = if *vertical {
            (mm(*line), -mm(p.a), mm(*line), -mm(p.b))
        } else {
            (mm(p.a), -mm(*line), mm(p.b), -mm(*line))
        };
        text.push_str(&format!(
            "cap{caps}: wall {x0},{y0} {x1},{y1} type=cap base=L1 top=L2 justify=centre\n"
        ));
        caps += 1;
    }
    Plan { text, doors, walls, caps }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_built(_: Build) -> bool {
        true
    }

    fn none_built(_: Build) -> bool {
        false
    }

    fn count(text: &str, op: &str) -> usize {
        text.lines()
            .filter(|l| l.split_once(": ").is_some_and(|(_, rest)| rest.starts_with(op)))
            .count()
    }

    #[test]
    fn every_hotel_has_a_door_for_each_room_and_side_room() {
        for n in 1..=MAX_HOTEL {
            let lay = Layout::new(n);
            for built in [&all_built as &dyn Fn(Build) -> bool, &none_built] {
                let p = plan(&lay, built);
                for r in 0..lay.rooms.len() {
                    let name = format!("room{r}");
                    assert_eq!(
                        p.doors.iter().filter(|d| d.name == name).count(),
                        1,
                        "hotel {n} {name}"
                    );
                }
                assert!(p.doors.iter().any(|d| d.name == "toilets"));
                assert_eq!(p.doors.iter().any(|d| d.name == "cafe"), lay.has_cafe);
            }
        }
    }

    #[test]
    fn wing_counts_give_two_to_four_wings_of_rooms() {
        let rooms: Vec<usize> = (1..=MAX_HOTEL)
            .map(|n| {
                let p = plan(&Layout::new(n), &all_built);
                p.doors.iter().filter(|d| d.name.starts_with("room")).count()
            })
            .collect();
        assert_eq!(rooms, vec![4, 6, 6, 8]);
    }

    #[test]
    fn locked_doors_match_the_game_locked_rects() {
        for n in 1..=MAX_HOTEL {
            let lay = Layout::new(n);
            for built in [&all_built as &dyn Fn(Build) -> bool, &none_built] {
                let locked = lay
                    .solids(built)
                    .iter()
                    .filter(|(_, k)| *k == Solid::Locked)
                    .count();
                let p = plan(&lay, built);
                assert_eq!(p.doors.iter().filter(|d| d.locked).count(), locked, "hotel {n}");
            }
        }
    }

    #[test]
    fn building_a_room_unlocks_its_door_and_a_gate_removes_its_door() {
        let lay = Layout::new(3);
        let locked_room = plan(&lay, &|b| b != Build::Room(3));
        let door = locked_room.doors.iter().find(|d| d.name == "room3").unwrap();
        assert!(door.locked);
        let open = plan(&lay, &|_| true);
        assert!(!open.doors.iter().find(|d| d.name == "room3").unwrap().locked);
        assert!(plan(&lay, &|b| b != Build::Gate(1)).doors.iter().any(|d| d.name == "gate1" && d.locked));
        assert!(!open.doors.iter().any(|d| d.name.starts_with("gate")));
    }

    #[test]
    fn toilets_cafe_and_pool_doors_follow_what_is_built() {
        let lay = Layout::new(3);
        let shut = plan(&lay, &|b| !matches!(b, Build::Toilets | Build::Cafe | Build::Pool));
        let locked: Vec<&str> = shut
            .doors
            .iter()
            .filter(|d| d.locked)
            .map(|d| d.name.as_str())
            .collect();
        assert!(locked.contains(&"toilets"));
        assert!(locked.contains(&"cafe"));
        assert!(locked.contains(&"pool"));
    }

    #[test]
    fn rooms_have_their_own_recolourable_wall_parts() {
        for n in 1..=MAX_HOTEL {
            let lay = Layout::new(n);
            let p = plan(&lay, &all_built);
            for r in 0..lay.rooms.len() {
                assert!(p.walls.contains(&format!("room{r}_wall")), "hotel {n} room {r}");
                assert!(p.walls.contains(&format!("room{r}_outer")), "hotel {n} room {r}");
            }
            let mut sorted = p.walls.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), p.walls.len());
        }
    }

    #[test]
    fn text_has_one_door_op_per_door_and_one_wall_op_per_wall() {
        for n in 1..=MAX_HOTEL {
            let lay = Layout::new(n);
            let p = plan(&lay, &none_built);
            assert_eq!(count(&p.text, "door "), p.doors.len());
            assert_eq!(count(&p.text, "wall "), p.walls.len() + p.caps);
            assert!(p.text.starts_with("units mm\n"));
        }
    }

    #[test]
    fn a_doorway_stays_inside_its_wall() {
        let lay = Layout::new(4);
        let p = plan(&lay, &all_built);
        for d in &p.doors {
            assert!(d.width > 1.3 && d.width < 3.3);
        }
    }
}
