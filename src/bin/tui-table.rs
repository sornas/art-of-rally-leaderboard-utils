use std::collections::BTreeMap;

use art_of_rally_leaderboard_api::{Area, Direction, Group};
use art_of_rally_leaderboard_utils::{
    fastest_times, format_time, get_interesting_leaderboards, split_times, table_utils, FullTime,
    PartialTime, Rally, UserMap,
};
use comfy_table::{CellAlignment, Table};
use snafu::Whatever;

fn main() -> Result<(), Whatever> {
    let (rallys, name_to_idx) = get_interesting_leaderboards()?;

    for ((area, group), users) in &rallys {
        let (full_times, partial_times, none_times) = split_times(users, &name_to_idx);
        let (fastest_total, fastest_stages) = fastest_times(&full_times, users);

        println!();
        println!("{:?} ({:?})", area, group);
        stages(
            &full_times,
            &partial_times,
            &none_times,
            fastest_total,
            fastest_stages,
            *group,
            *area,
            Direction::Forward,
        );
    }

    println!();
    total_stages(&rallys, &name_to_idx);

    Ok(())
}

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
    let (header, rows) = table_utils::stages(
        full_times,
        partial_times,
        none_times,
        fastest_total,
        fastest_stages,
        group,
        area,
        direction,
    );

    let mut table = Table::new();
    table.load_preset(comfy_table::presets::ASCII_HORIZONTAL_ONLY);
    table.set_header(header);
    for column in table.column_iter_mut().skip(1) {
        column.set_cell_alignment(CellAlignment::Right);
    }
    table.add_rows(
        rows.iter()
            .map(|row| row.iter().map(|[s1, s2, s3]| format!("{s1}\n{s2}\n{s3}"))),
    );
    println!("{table}");
}

pub fn total_stages(rallys: &BTreeMap<(Area, Group), Rally<6>>, name_to_idx: &UserMap) {
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
