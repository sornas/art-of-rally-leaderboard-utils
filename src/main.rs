use std::collections::BTreeMap;

use art_of_rally_leaderboard_api::car_name;
use art_of_rally_leaderboard_utils::{
    Rally, fastest_times, get_default_rallys, get_default_users, get_rally_results, split_times,
    table_utils::{format_delta, format_time},
};
use itertools::Itertools as _;
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

fn url_safe(s: &str) -> String {
    s.to_lowercase().replace(" ", "-")
}

fn main() -> Result<(), Whatever> {
    let rallys = get_default_rallys();
    let (platform, user_ids, user_names) = get_default_users();
    let mut interval_parts: Vec<PreEscaped<String>> = Vec::new();
    let mut absolute_parts: Vec<PreEscaped<String>> = Vec::new();
    let mut pages: BTreeMap<String, Vec<_>> = BTreeMap::new();

    interval_parts
        .push(html!(div { "interval time | " a href="/absolute.html" { "absolute time"} } ));
    absolute_parts.push(html!(div { a href="/index.html" {"interval time"} " | absolute time"}  ));

    for Rally { title, stages } in rallys {
        let leaderboards: Vec<_> = stages.iter().map(|stage| (*stage, platform)).collect();
        let results = get_rally_results(&leaderboards, &user_ids, &user_names)?;
        let (full_times, partial_times) = split_times(&results);
        let (fastest_total, fastest_stages) = fastest_times(&full_times, &results);

        interval_parts.push(html!(h2 { (title) }));
        // Total interval results table for each rally. (stages) x (drivers).
        interval_parts.push(html!(
            table class="rally" {
                thead {
                    th { "driver" }
                    th { }
                    th { "total" }
                    @for (stage, _group, weather) in &stages {
                        th { a href=(format!("/{}.html", url_safe(&format!("{stage} {weather}")))) { (stage) " (" (weather) ")" } }
                    }
                }
                @for ft in &full_times {
                    tr {
                        td { a href=(format!("/{}.html", url_safe(&ft.user_name))) { (ft.user_name) } }
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
                        td { a href=(format!("/{}.html", url_safe(&pt.user_name))) { (pt.user_name) } }
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

        absolute_parts.push(html!(h2 { (title) }));
        // Total absolute results table for each rally. (stages) x (drivers).
        absolute_parts.push(html!(
            table class="rally" {
                thead {
                    th { "driver" }
                    th { }
                    th { "total" }
                    @for (stage, _group, weather) in &stages {
                        th { a href=(format!("/{}.html", url_safe(&format!("{stage} {weather}")))) { (stage) " (" (weather) ")" } }
                    }
                }
                @for ft in &full_times {
                    tr {
                        td { a href=(format!("/{}.html", url_safe(&ft.user_name))) { (ft.user_name) } }
                        td { }
                        @let total = ft.total_time;
                        @let fastest_total = fastest_total.unwrap();
                        @if total == fastest_total {
                            td class="fastest" { (format_time(total, true)) }
                        } @else {
                            td { (format_time(total, true)) }
                        }
                        @for (i, time) in ft.stage_times.iter().copied().enumerate() {
                            @let fast = fastest_stages[i].unwrap();
                            @if time == fast {
                                td class="fastest" { (format_time(time, false)) }
                            } @else {
                                td { (format_time(time, false)) }
                            }
                        }
                    }
                }
                @for pt in &partial_times {
                    tr {
                        td { a href=(format!("/{}.html", url_safe(&pt.user_name))) { (pt.user_name) } }
                        td { "*" }
                        @let total = pt.total_time;
                        td { (format_time(total, true)) }
                        @for (i, time) in pt.stage_times.iter().copied().enumerate() {
                            @if let Some(time) = time {
                                @let fast = fastest_stages[i].unwrap();
                                @if time == fast {
                                    td class="fastest" { (format_time(time, false)) }
                                } @else {
                                    td { (format_time(time, false)) }
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
        for driver in &results.driver_results {
            pages.entry(driver.name.clone()).or_default().push(html!(
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
                    @for (i, ((stage, group, weather), stage_result)) in stages.iter().zip(&driver.stages).enumerate() {
                        @let Some(stage_result) = stage_result else { continue; };
                        @let time = stage_result.time_ms;
                        tr {
                            td { a href=(format!("/{}.html", url_safe(&format!("{stage} {weather}")))) { (stage) " (" (weather) ")" } }
                            td class="time" { (format_time(time, false)) }
                            @let fast = fastest_stages[i].unwrap();
                            @if time == fast {
                                td class="interval" { "-:--:---" }
                            } @else {
                                td class="interval" { (format_delta(time, fast, false)) }
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

        // For each stage, in-depth stats
        for (i, (stage, group, weather)) in stages.iter().enumerate() {
            let stage_name = &format!("{stage} {weather}");
            let Some(fast) = fastest_stages[i] else {
                continue;
            };
            struct S {
                name: String,
                time: usize,
                car: usize,
                world_rank: Option<usize>,
            }
            let times = full_times
                .iter()
                .map(|ft| S {
                    name: ft.user_name.to_string(),
                    time: ft.stage_times[i],
                    car: ft.cars[i],
                    world_rank: ft.world_rank[i],
                })
                .chain(partial_times.iter().filter_map(|pt| {
                    let time = pt.stage_times[i]?;
                    let car = pt.cars[i]?;
                    Some(S {
                        name: pt.user_name.to_string(),
                        time,
                        car,
                        world_rank: pt.world_rank.get(i).copied().flatten(),
                    })
                }))
                .sorted_by_key(|time| time.time);
            pages.entry(stage_name.clone()).or_default().push(html!(
                table class="stage" {
                    thead {
                        th { "driver" }
                        th { "time" }
                        th { "interval" }
                        th { "car" }
                        th { "world rank" }
                    }
                    @for time in times {
                        tr {
                            td { a href=(format!("/{}.html", url_safe(&time.name))) { (time.name) } }
                            td class="time" { (format_time(time.time, false)) }
                            @if time.time == fast {
                                td class="interval" { "-:--:---" }
                            } @else {
                                td class="interval" { (format_delta(time.time, fast, false)) }
                            }
                            td { (car_name(*group, time.car)) }
                            @if let Some(world_rank) = time.world_rank {
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
        html_page("basvektorernas art of rally-leaderboard", &interval_parts).into_string(),
    )
    .unwrap();
    std::fs::write(
        "public/absolute.html",
        html_page("basvektorernas art of rally-leaderboard", &absolute_parts).into_string(),
    )
    .unwrap();
    for (user, parts) in &pages {
        std::fs::write(
            format!("public/{}.html", url_safe(user)),
            html_page(user, parts).into_string(),
        )
        .unwrap();
    }
    Ok(())
}
