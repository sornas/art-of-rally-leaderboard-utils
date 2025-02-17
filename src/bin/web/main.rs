use art_of_rally_leaderboard_api::{Area, Direction, Filter, Group, Leaderboard, Stage, Weather};
use art_of_rally_leaderboard_utils::{
    fastest_times, get_default_rallys, get_default_users, get_rally_results, split_times,
    table_utils,
};
use snafu::Whatever;

fn index(body: &str) -> String {
    let updated = chrono::Utc::now().format("%F %R %Z");
    format!(
        r#"
<!DOCTYPE html>
<html>

<head>
  <link rel="stylesheet" href="/style.css" />
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link
    href="https://fonts.googleapis.com/css2?family=Atkinson+Hyperlegible+Next:ital,wght@0,200..800;1,200..800&display=swap"
    rel="stylesheet">
  <link href="https://fonts.googleapis.com/css2?family=Ubuntu+Mono:ital,wght@0,400;0,700;1,400;1,700&display=swap"
    rel="stylesheet">
</head>


<body>
<h1>basvektorernas art of rally-leaderboard</h1>
{body}

<p>last updated: {updated}</p>
</body>
</html>
    "#
    )
}

fn rally(
    area: Area,
    group: Group,
    weather: Weather,
    header: Vec<String>,
    rows: Vec<Vec<[String; 3]>>,
) -> String {
    let header_cells =
        header
            .iter()
            .map(|s| format!("<th>{s}</th>\n"))
            .fold(String::new(), |mut cur, nxt| {
                cur += &nxt;
                cur
            });

    let mut driver_rows = String::new();
    for row in rows {
        driver_rows += "<tr class=\"times\">\n";
        for cell in row.iter().map(|[s, _, _]| s) {
            driver_rows += &format!("<td>{cell}</td>\n");
        }
        driver_rows += "</tr>\n";
        driver_rows += "<tr class=\"deltas\">\n";
        for cell in row.iter().map(|[_, s, _]| s) {
            driver_rows += &format!("<td>{cell}</td>\n");
        }
        driver_rows += "</tr>\n";
        driver_rows += "<tr class=\"cars\">\n";
        for cell in row.iter().map(|[_, _, s]| s) {
            driver_rows += &format!("<td>{cell}</td>\n");
        }
        driver_rows += "</tr>\n";
    }

    format!(
        r#"
<h2>{area:?} - {group:?} ({weather:?})</h2>
<table>
<thead>
{header_cells}
</thead>
{driver_rows}
</table>
    "#
    )
}

fn main() -> Result<(), Whatever> {
    let mut body = String::new();
    let rallys = get_default_rallys();
    let (platform, users) = get_default_users();
    for rally_settings in rallys {
        let leaderboards: Vec<_> = rally_settings
            .into_iter()
            .map(|(stage, group, weather)| Leaderboard {
                stage,
                weather,
                group,
                filter: Filter::Friends,
                platform,
            })
            .collect();
        let results = get_rally_results(&leaderboards, &users)?;
        let (full_times, partial_times) = split_times(&results);
        let (fastest_total, fastest_stages) = fastest_times(&full_times, &results);

        // TODO: handle different area/group for each stage
        let (
            Stage {
                area,
                stage_number: _,
                direction: _,
            },
            group,
            weather,
        ) = results.stages[0];
        let (header, rows) = table_utils::stages(
            &full_times,
            &partial_times,
            fastest_total,
            fastest_stages,
            group,
            area,
            Direction::Forward,
        );
        body += &rally(area, group, weather, header, rows);
    }
    let html = index(&body);
    println!("{html}");
    Ok(())
}
