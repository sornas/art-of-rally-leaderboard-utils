use art_of_rally_leaderboard_utils::{
    fastest_times, get_default_rallys, get_default_users, get_rally_results, split_times,
    table_utils,
};
use maud::{PreEscaped, html};
use snafu::Whatever;

fn index(body: &[PreEscaped<String>]) -> PreEscaped<String> {
    let updated = chrono::Utc::now().format("%F %R %Z");
    html!(
        (maud::DOCTYPE)
        html {
            head {
                link rel="stylesheet" href="/style.css";
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Atkinson+Hyperlegible+Next:ital,wght@0,200..800;1,200..800&display=swap";
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Ubuntu+Mono:ital,wght@0,400;0,700;1,400;1,700&display=swap";
            }

            body {
                h1 { "basvektorernas art of rally-leaderboard" }

                @for part in body {
                    (part)
                }

                p {
                    "last updated: " (updated)
                }
            }
        }
    )
}

fn rally(title: String, header: Vec<String>, rows: Vec<Vec<[String; 5]>>) -> PreEscaped<String> {
    html!(
        h2 { (title) }
        table {
            thead {
                @for h in header {
                    th { (h) }
                }
            }
            @for row in rows {
                tr class="times"         { @for cell in &row { td { (cell[0]) } } }
                tr class="deltas"        { @for cell in &row { td { (cell[1]) } } }
                tr class="cars"          { @for cell in &row { td { (cell[2]) } } }
                tr class="ranks"         { @for cell in &row { td { (cell[3]) } } }
                tr class="delta-percent" { @for cell in &row { td { (cell[4]) } } }
            }
        }
    )
}

fn main() -> Result<(), Whatever> {
    let mut body = Vec::new();
    let rallys = get_default_rallys();
    let (platform, user_ids, user_names) = get_default_users();
    for (title, rally_settings) in rallys {
        let leaderboards: Vec<_> = rally_settings
            .into_iter()
            .map(|stage| (stage, platform))
            .collect();
        let results = get_rally_results(&leaderboards, &user_ids, &user_names)?;
        let (full_times, partial_times) = split_times(&results);
        let (fastest_total, fastest_stages) = fastest_times(&full_times, &results);

        let (header, rows) = table_utils::stages(
            &results.stages,
            &full_times,
            &partial_times,
            fastest_total,
            &fastest_stages,
        );
        body.push(rally(title, header, rows));
    }
    let html = index(&body).into_string();
    println!("{html}");
    Ok(())
}
