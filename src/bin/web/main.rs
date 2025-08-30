use std::collections::BTreeMap;

use art_of_rally_leaderboard_api::car_name;
use art_of_rally_leaderboard_utils::{
    Rally, fastest_times, get_default_rallys, get_default_users, get_rally_results, split_times,
    table_utils::{format_delta, format_time},
};
use maud::{PreEscaped, html};
use snafu::Whatever;

fn html_page<'a>(
    header: &str,
    body: impl IntoIterator<Item = &'a PreEscaped<String>>,
) -> PreEscaped<String> {
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
                h1 { (header) }

                @for part in body {
                    (part)
                }

                p {
                    "last updated: " (chrono::Utc::now().format("%F %R %Z"))
                }
            }
        }
    )
}

fn main() -> Result<(), Whatever> {
    let rallys = get_default_rallys();
    let (platform, user_ids, user_names) = get_default_users();
    let mut parts = Vec::new();
    let mut pages: BTreeMap<String, Vec<_>> = BTreeMap::new();

    for Rally { title, stages } in rallys {
        let leaderboards: Vec<_> = stages.iter().map(|stage| (*stage, platform)).collect();
        let results = get_rally_results(&leaderboards, &user_ids, &user_names)?;
        let (full_times, partial_times) = split_times(&results);
        let (fastest_total, fastest_stages) = fastest_times(&full_times, &results);

        parts.push(html!(h2 { (title) }));
        // Total results table for each rally. (stages) x (drivers).
        parts.push(html!(
            table class="rally" {
                thead {
                    th { "driver" }
                    th { }
                    th { "total" }
                    @for (stage, _group, weather) in &stages {
                        th { (stage) " (" (weather) ")" }
                    }
                }
                @for ft in &full_times {
                    tr {
                        td {a href=(format!("/{}", ft.user_name.to_lowercase().replace(" ", "-"))) { (ft.user_name) } }
                        td { }
                        @let total = ft.total_time;
                        @let fastest_total = fastest_total.unwrap();
                        @if total == fastest_total {
                            td class="fastest" { (format_time(total, true)) }
                        } @else {
                            td { (format_delta(total, fastest_total, true)) }
                        }
                        @for (i, time) in ft.stage_times.iter().copied().enumerate() {
                            @let fast = fastest_stages[i].unwrap();
                            @if time == fast {
                                td class="fastest" { (format_time(time, false)) }
                            } @else {
                                td { (format_delta(time, fast, false)) }
                            }
                        }
                    }
                }
                @for pt in &partial_times {
                    tr {
                        td {a href=(format!("/{}", pt.user_name.to_lowercase().replace(" ", "-"))) { (pt.user_name) } }
                        td { "*" }
                        @let total = pt.total_time;
                        td { (format_time(total, true)) }
                        @for (i, time) in pt.stage_times.iter().copied().enumerate() {
                            @if let Some(time) = time {
                                @let fast = fastest_stages[i].unwrap();
                                @if time == fast {
                                    td class="fastest" { (format_time(time, false)) }
                                } @else {
                                    td { (format_delta(time, fast, false)) }
                                }
                            } @else {
                                td { }
                            }
                        }
                    }
                }
            }
        ));

        // For each driver, in-depth stats for each stage
        for driver in results.driver_results {
            pages.entry(driver.name).or_default().push(html!(
                h2 { (title) }
                table class="driver" {
                    thead {
                        th { "stage" }
                        th { "time" }
                        th { "interval" }
                        th { "car" }
                        th { "rank" }
                        th { "world rank" }
                    }
                    @for (i, ((stage, group, weather), stage_result)) in stages.iter().zip(driver.stages).enumerate() {
                        @let Some(stage_result) = stage_result else { continue; };
                        @let time = stage_result.time_ms;
                        tr {
                            td { (stage) " (" (weather) ")" }
                            td class="time" { (format_time(time, false)) }
                            @let fast = fastest_stages[i].unwrap();
                            @if time == fast {
                                td class="time" { "-:--:--" }
                            } @else {
                                td class="time" { (format_delta(time, fast, false)) }
                            }
                            td { (car_name(*group, stage_result.car)) }
                            td { (stage_result.local_rank) }
                            @if let Some(world_rank) = stage_result.world_rank {
                                td { (world_rank) }
                            } @else {
                                td { }
                            }
                        }
                    }
                }
            ));
        }
    }

    std::fs::write(
        "public/index.html",
        html_page("basvektorernas art of rally-leaderboard", &parts).into_string(),
    )
    .unwrap();
    for (user, parts) in &pages {
        std::fs::write(
            format!("public/{}.html", user.to_lowercase().replace(" ", "-")),
            html_page(user.as_str(), parts).into_string(),
        )
        .unwrap();
    }
    Ok(())
}
