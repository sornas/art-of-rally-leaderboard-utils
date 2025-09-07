use std::collections::{BTreeMap, HashMap};

use art_of_rally_leaderboard_api::{Platform, car_name};
use art_of_rally_leaderboard_utils::{
    Rally, RallyResults, fastest_times, get_default_rallys, get_rally_results, secret, split_times,
    table_utils::{format_delta, format_time},
};
use indexmap::IndexMap;
use itertools::Itertools as _;
use maud::{PreEscaped, html};
use serde::{Deserialize, Serialize};
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

type RallyName = String;
type StageName = String;

#[derive(Debug)]
enum Row {
    // Rendered as `> {rank} {name} {time}`
    FirstTime {
        rank: usize,
        name: String,
        time: usize,
    },
    // Rendered as `^ {rank} {name} {time} {delta}`
    TimeImprovedRankIncreased {
        rank: usize,
        name: String,
        time: usize,
        prev: usize,
    },
    // Rendered as `~ {rank} {name} {time} {delta}`
    TimeImproved {
        rank: usize,
        name: String,
        time: usize,
        prev: usize,
    },
    // Rendered as `v {rank} {name} {time} {delta}`
    TimeImprovedRankDecreased {
        rank: usize,
        name: String,
        time: usize,
        prev: usize,
    },
    // Rendered as `v {rank} {name} {time}`
    RankDecreased {
        rank: usize,
        name: String,
        time: usize,
    },
    // Rendered as `  {rank} {name} {time}` if active is true
    Unchanged {
        active: bool,
        rank: usize,
        name: String,
        time: usize,
    },
}

impl Row {
    fn rank(&self) -> usize {
        match self {
            Row::FirstTime { rank, .. } => *rank,
            Row::TimeImprovedRankIncreased { rank, .. } => *rank,
            Row::TimeImproved { rank, .. } => *rank,
            Row::TimeImprovedRankDecreased { rank, .. } => *rank,
            Row::RankDecreased { rank, .. } => *rank,
            Row::Unchanged { rank, .. } => *rank,
        }
    }

    fn name(&self) -> &str {
        match self {
            Row::FirstTime { name, .. } => name,
            Row::TimeImprovedRankIncreased { name, .. } => name,
            Row::TimeImproved { name, .. } => name,
            Row::TimeImprovedRankDecreased { name, .. } => name,
            Row::RankDecreased { name, .. } => name,
            Row::Unchanged { name, .. } => name,
        }
    }

    fn is_unchanged(&self) -> bool {
        matches!(self, Row::Unchanged { .. })
    }
}

type NotificationTable = IndexMap<RallyName, IndexMap<StageName, Vec<Row>>>;

fn send_notification(notifications: &NotificationTable) {
    let mut message = "```".to_string();
    for (rally, stages) in notifications {
        // Skip rallys where all rows are unchanged
        if stages.values().flatten().all(Row::is_unchanged) {
            continue;
        }
        message += &format!("\n{rally}\n");
        for (stage, rows) in stages {
            // Skip stages where all rows are unchanged
            if rows.iter().all(Row::is_unchanged) {
                continue;
            }
            message += &format!("  {stage}\n");
            for row in rows {
                let name_width = rows.iter().map(|row| row.name().len()).max().unwrap();
                message += &match row {
                    Row::FirstTime { rank, name, time } => {
                        format!(
                            "    > {}.  {:name_width$}  {}",
                            rank,
                            name,
                            format_time(*time, false),
                            name_width = name_width,
                        )
                    }
                    Row::TimeImprovedRankIncreased {
                        rank,
                        name,
                        time,
                        prev,
                    } => format!(
                        "    ^ {}.  {:name_width$}  {}  {}",
                        rank,
                        name,
                        format_time(*time, false),
                        format_delta(*time, *prev, false),
                        name_width = name_width,
                    ),
                    Row::TimeImproved {
                        rank,
                        name,
                        time,
                        prev,
                    } => format!(
                        "    ~ {}.  {:name_width$}  {}  {}",
                        rank,
                        name,
                        format_time(*time, false),
                        format_delta(*time, *prev, false),
                        name_width = name_width,
                    ),
                    Row::TimeImprovedRankDecreased {
                        rank,
                        name,
                        time,
                        prev,
                    } => format!(
                        "    v {}.  {:name_width$}  {}  {}",
                        rank,
                        name,
                        format_time(*time, false),
                        format_delta(*time, *prev, false),
                        name_width = name_width,
                    ),
                    Row::RankDecreased { rank, name, time } => {
                        format!(
                            "    v {}.  {:name_width$}  {}",
                            rank,
                            name,
                            format_time(*time, false),
                            name_width = name_width
                        )
                    }
                    Row::Unchanged {
                        active: true,
                        rank,
                        name,
                        time,
                    } => {
                        format!(
                            "      {}.  {:name_width$}  {}",
                            rank,
                            name,
                            format_time(*time, false),
                            name_width = name_width
                        )
                    }
                    Row::Unchanged { active: false, .. } => continue,
                };
                message += "\n";
            }
        }
    }
    message += "```";

    #[derive(Serialize)]
    struct WebhookMessage {
        content: String,
        allowed_mentions: HashMap<String, Vec<String>>,
    }

    println!("{message}");
    println!("sending notification...");
    match ureq::post(secret::WEBHOOK_URL).send_json(&WebhookMessage {
        content: message,
        allowed_mentions: [("parse".to_string(), vec![])].into_iter().collect(),
    }) {
        Ok(mut r) => println!("{:?}: {:?}", r.status(), r.body_mut().read_to_string()),
        Err(e) => println!("{e:?}"),
    }
}

#[derive(Deserialize, Serialize)]
struct Db {
    rallys: Vec<Rally>,
    results: Vec<RallyResults>,
    platform: Platform,
    user_ids: Vec<u64>,
    user_names: Vec<String>,
    discord_ids: Vec<String>,
}

fn download(
    rallys: Vec<Rally>,
    platform: Platform,
    user_ids: Vec<u64>,
    user_names: Vec<&'static str>,
    discord_ids: Vec<&'static str>,
) -> Result<Db, Whatever> {
    let mut results = Vec::new();
    for rally in &rallys {
        let leaderboards = rally
            .stages
            .iter()
            .copied()
            .map(|stage| (stage, platform))
            .collect_vec();
        results.push(get_rally_results(&leaderboards, &user_ids, &user_names)?);
    }

    Ok(Db {
        rallys,
        results,
        platform,
        user_ids,
        user_names: user_names.into_iter().map(str::to_string).collect(),
        discord_ids: discord_ids.into_iter().map(str::to_string).collect(),
    })
}

fn report(db: Db, prev: Option<Db>) {
    let mut parts = Vec::new();
    let mut pages: BTreeMap<String, Vec<_>> = Default::default();
    let mut table: NotificationTable = Default::default();

    for (rally, results) in db.rallys.iter().zip(db.results.iter()) {
        // Find corresponding previous rally by name
        let prev_results = prev.as_ref().and_then(|prev| {
            prev.rallys
                .iter()
                .zip(prev.results.iter())
                .find_map(|(prev_rally, prev_results)| {
                    prev_rally.title.eq(&rally.title).then_some(prev_results)
                })
        });

        // For each user, if they drove a new record, add it to the notification table
        for driver in &results.driver_results {
            for (stage_idx, ((stage, _group, weather), stage_results)) in
                rally.stages.iter().zip(driver.stages.iter()).enumerate()
            {
                let stage_name = format!("{stage} {weather}");
                let Some(stage_results) = stage_results else {
                    continue;
                };
                let (time, rank) = (stage_results.time_ms, stage_results.local_rank);

                // Try to find the previous time
                let prev_stage_result = prev_results
                    .and_then(|r| {
                        r.driver_results
                            .iter()
                            .find_map(|d| d.name.eq(&driver.name).then_some(&d.stages))
                    })
                    .and_then(|stages| stages[stage_idx].as_ref());
                let prev_time = prev_stage_result.as_ref().map(|r| r.time_ms);
                let prev_rank = prev_stage_result.as_ref().map(|r| r.local_rank);
                let time_increased = prev_time.is_some_and(|prev_time| time < prev_time);
                let rank_increased = prev_rank.is_some_and(|prev_rank| rank < prev_rank);
                let rank_same = prev_rank.is_some_and(|prev_rank| rank == prev_rank);
                let rank_decreased = prev_rank.is_some_and(|prev_rank| rank > prev_rank);

                dbg!((
                    &driver.name,
                    &stage_name,
                    (time, rank),
                    (prev_time, prev_rank)
                ));

                let name = driver.name.clone();
                let mut add_row = |x| {
                    table
                        .entry(rally.title.clone())
                        .or_default()
                        .entry(stage_name.clone())
                        .or_default()
                        .push(x);
                };
                if prev_stage_result.is_none() {
                    add_row(Row::FirstTime { rank, name, time });
                } else if time_increased && rank_increased {
                    add_row(Row::TimeImprovedRankIncreased {
                        rank,
                        name,
                        time,
                        prev: prev_time.unwrap(),
                    });
                } else if time_increased && rank_same {
                    add_row(Row::TimeImproved {
                        rank,
                        name,
                        time,
                        prev: prev_time.unwrap(),
                    });
                } else if time_increased && rank_decreased {
                    add_row(Row::TimeImprovedRankDecreased {
                        rank,
                        name,
                        time,
                        prev: prev_time.unwrap(),
                    });
                } else if rank_decreased {
                    add_row(Row::RankDecreased { rank, name, time });
                } else {
                    add_row(Row::Unchanged {
                        active: false,
                        rank,
                        name,
                        time,
                    })
                }
            }
        }
    }

    for stages in table.values_mut() {
        for rows in stages.values_mut() {
            rows.sort_by_key(Row::rank);

            for i in 0..rows.len() - 1 {
                let (head, tail) = rows.split_at_mut(i + 1);
                if let Row::Unchanged { active, .. } = &mut head[i]
                    && !matches!(tail[0], Row::Unchanged { .. })
                {
                    *active = true;
                }
            }
        }
    }

    for (rally, results) in db.rallys.iter().zip(db.results.iter()) {
        let (full_times, partial_times) = split_times(results);
        let (fastest_total, fastest_stages) = fastest_times(&full_times, results);

        parts.push(html!(h2 { (rally.title) }));
        // Total results table for each rally. (stages) x (drivers).
        parts.push(html!(
            table class="rally" {
                thead {
                    th { "driver" }
                    th { }
                    th { "total" }
                    @for (stage, _group, weather) in &rally.stages {
                        th { a href=(format!("/{}.html", url_safe(&format!("{stage} {weather}")))) { (stage) " (" (weather) ")" } }
                    }
                }
                @for ft in &full_times {
                    tr {
                        td { a href=(format!("/{}.html", url_safe(ft.user_name))) { (ft.user_name) } }
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
                        td { a href=(format!("/{}.html", url_safe(pt.user_name))) { (pt.user_name) } }
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
        for driver in &results.driver_results {
            pages.entry(driver.name.clone()).or_default().push(html!(
                h2 { (rally.title) }
                table class="driver" {
                    thead {
                        th { "stage" }
                        th { "time" }
                        th { "interval" }
                        th { "car" }
                        th { "rank" }
                        th { "world rank" }
                    }
                    @for (i, ((stage, group, weather), stage_result)) in rally.stages.iter().zip(&driver.stages).enumerate() {
                        @let Some(stage_result) = stage_result else { continue; };
                        @let time = stage_result.time_ms;
                        tr {
                            td { a href=(format!("/{}.html", url_safe(&format!("{stage} {weather}")))) { (stage) " (" (weather) ")" } }
                            td class="time" { (format_time(time, false)) }
                            @let fast = fastest_stages[i].unwrap();
                            @if time == fast {
                                td class="interval" { "-:--.---" }
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
        for (i, (stage, group, weather)) in rally.stages.iter().enumerate() {
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
                                td class="interval" { "-:--.---" }
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

    dbg!(&table);

    if prev.is_some() && !table.is_empty() {
        send_notification(&table);
    }

    std::fs::write(
        "public/index.html",
        html_page("basvektorernas art of rally-leaderboard", &parts).into_string(),
    )
    .unwrap();
    for (user, parts) in &pages {
        std::fs::write(
            format!("public/{}.html", url_safe(user)),
            html_page(user, parts).into_string(),
        )
        .unwrap();
    }
}

fn main() -> Result<(), Whatever> {
    std::fs::create_dir_all("data").unwrap();

    let rallys = get_default_rallys();
    let (platform, user_ids, user_names, discord_ids) = secret::users();

    let db = download(rallys, platform, user_ids, user_names, discord_ids)?;
    let ts = chrono::Utc::now().timestamp();

    let prev = std::fs::read_dir("data")
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .sorted()
        .last()
        .map(|path| ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap());

    std::fs::write(format!("data/{ts}.ron"), ron::to_string(&db).unwrap()).unwrap();

    // let db =
    //     ron::from_str(&std::fs::read_to_string(std::env::args().nth(2).unwrap()).unwrap()).unwrap();
    // let prev = Some(
    //     ron::from_str(&std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap()).unwrap(),
    // );

    report(db, prev);

    Ok(())
}
