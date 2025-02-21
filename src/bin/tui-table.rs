use art_of_rally_leaderboard_api::{Group, Stage, Weather};
use art_of_rally_leaderboard_utils::{
    fastest_times, get_default_rallys, get_default_users, get_rally_results, split_times,
    table_utils, FullTime, PartialTime,
};
use comfy_table::{CellAlignment, Table};
use snafu::Whatever;

fn main() -> Result<(), Whatever> {
    let rallys = get_default_rallys();
    let (platform, user_ids, user_names) = get_default_users();
    for (title, rally) in rallys {
        let leaderboards: Vec<_> = rally
            .into_iter()
            .map(|(stage, group, weather)| (stage, weather, group, platform))
            .collect();
        let results = get_rally_results(&leaderboards, &user_ids, &user_names)?;
        let (full_times, partial_times) = split_times(&results);
        let (fastest_total, fastest_stages) = fastest_times(&full_times, &results);

        println!("\n{title}");
        stages(
            &results.stages,
            &full_times,
            &partial_times,
            fastest_total,
            &fastest_stages,
        );
    }

    Ok(())
}

pub fn stages(
    stages: &[(Stage, Group, Weather)],
    full_times: &[FullTime],
    partial_times: &[PartialTime],
    fastest_total: Option<usize>,
    fastest_stages: &[Option<usize>],
) {
    let (header, rows) = table_utils::stages(
        stages,
        full_times,
        partial_times,
        fastest_total,
        fastest_stages,
    );

    let mut table = Table::new();
    table.load_preset(comfy_table::presets::ASCII_HORIZONTAL_ONLY);
    table.set_header(header);
    for column in table.column_iter_mut().skip(1) {
        column.set_cell_alignment(CellAlignment::Right);
    }
    table.add_rows(
        rows.iter()
            .map(|row| row.iter().map(|lines| lines.join("\n"))),
    );
    println!("{table}");
}
