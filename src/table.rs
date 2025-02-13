use std::collections::BTreeMap;

use art_of_rally_leaderboard_api::{Area, Direction, Group, Stage};
use comfy_table::{CellAlignment, Table};

use crate::{split_times, FullTime, PartialTime};

pub fn stages(
    full_times: &[FullTime],
    partial_times: &[PartialTime],
    none_times: &[&str],
    fastest_total: Option<usize>,
    fastest_stages: [Option<usize>; 6],
    group: Group,
    area: Area,
    direction: Direction,
) {
    let mut table = Table::new();
    table.load_preset(comfy_table::presets::ASCII_HORIZONTAL_ONLY);
    let mut header = vec!["user".to_string(), "total".to_string()];
    header.extend((1..=6).map(|stage_number| {
        (Stage {
            area,
            stage_number,
            direction,
        })
        .to_string()
    }));
    table.set_header(header);
    for column in table.column_iter_mut().skip(1) {
        column.set_cell_alignment(CellAlignment::Right);
    }
    for ft in full_times {
        let mut row = vec![
            ft.user_name.to_string(),
            format!(
                "{}\n{}",
                format_time(ft.total_time, true),
                format_delta(ft.total_time, fastest_total.unwrap(), true),
            ),
        ];
        row.extend(ft.stage_times.iter().enumerate().map(|(i, t)| {
            format!(
                "{}\n{}\n{}",
                format_time(*t, false),
                format_delta(*t, fastest_stages[i].unwrap(), false),
                art_of_rally_leaderboard_api::car_name(group, ft.cars[i]),
            )
        }));
        table.add_row(row);
    }
    for pt in partial_times {
        let mut row = vec![
            pt.user_name.to_string(),
            format!("* {}", format_time(pt.total_time, true)),
        ];
        row.extend(pt.stage_times.iter().enumerate().map(|(i, t)| match t {
            Some(t) => format!(
                "{}\n{}\n{}",
                format_time(*t, false),
                format_delta(*t, fastest_stages[i].unwrap(), false),
                art_of_rally_leaderboard_api::car_name(group, pt.cars[i].unwrap()),
            ),
            None => "-".to_string(),
        }));
        table.add_row(row);
    }
    for name in none_times {
        let mut row = vec![name.to_string()];
        row.extend(std::iter::repeat("-".to_string()).take(7));
        table.add_row(row);
    }
    println!("{table}");
}

pub fn total_stages(
    rallys: &BTreeMap<(Area, Group), [[Option<(usize, usize)>; 6]; 3]>,
    name_to_idx: &BTreeMap<&str, usize>,
) {
    let mut table = Table::new();
    table.load_preset(comfy_table::presets::ASCII_FULL_CONDENSED);
    let mut header = vec!["area", "group", "total stages"];
    header.extend(name_to_idx.keys());
    table.set_header(header);
    for column in table.column_iter_mut().skip(2) {
        column.set_cell_alignment(CellAlignment::Right);
    }

    let mut rows = vec![];
    for ((area, group), users) in rallys {
        let (full_times, partial_times, _) = split_times(users, &name_to_idx);

        let stages = (full_times.len() * 6)
            + partial_times
                .iter()
                .map(|pt| pt.finished_stages)
                .sum::<usize>();
        let mut row = vec![
            format!("{area:?}"),
            format!("{group:?}"),
            format!("{stages}"),
        ];
        for user in name_to_idx.keys() {
            if let Some(t) = full_times
                .iter()
                .find_map(|ft| ft.user_name.eq(*user).then_some(ft.total_time))
            {
                row.push(format!("{} (6)", format_time(t, true)));
            } else if let Some((t, n)) = partial_times.iter().find_map(|pt| {
                pt.user_name
                    .eq(*user)
                    .then_some((pt.total_time, pt.finished_stages))
            }) {
                row.push(format!("{} ({})", format_time(t, true), n));
            } else {
                row.push("(0)".to_string());
            }
        }
        rows.push(row);
    }

    rows.sort_by(|e1, e2| e2[2].cmp(&e1[2]));
    for row in rows {
        table.add_row(row);
    }
    println!("{table}");
}

fn format_time(ms: usize, long: bool) -> String {
    let minutes = ms / 1000 / 60;
    let seconds = (ms / 1000) % 60;
    let millis = ms % 1000;
    if long {
        format!("{minutes:02}:{seconds:02}.{millis:03}")
    } else {
        format!("{minutes:01}:{seconds:02}.{millis:03}")
    }
}

fn format_delta(ms: usize, fast: usize, long: bool) -> String {
    assert!(ms >= fast);
    if ms == fast {
        "         ".to_string()
    } else {
        format!("+{}", format_time(ms - fast, long))
    }
}
