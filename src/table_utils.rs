use crate::{FullTime, PartialTime, StageWithLeaderboard};

pub fn stages(
    stages: &[StageWithLeaderboard],
    full_times: &[FullTime],
    partial_times: &[PartialTime],
    fastest_total: Option<usize>,
    fastest_stages: &[Option<usize>],
) -> (Vec<String>, Vec<Vec<[String; 5]>>) {
    let mut header = vec!["user".to_string(), "total".to_string()];
    header.extend(
        stages
            .iter()
            .map(|(stage, _group, weather)| format!("{stage} ({weather})")),
    );
    let num_cols = header.len();

    let mut drivers = Vec::new();
    for ft in full_times {
        let mut driver = vec![[
            ft.user_name.to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ]];
        let fastest_total = fastest_total.unwrap();
        driver.push([
            format_time(ft.total_time, true),
            format_delta(ft.total_time, fastest_total, true),
            String::new(),
            String::new(),
            format_percent(ft.total_time, fastest_total),
        ]);
        driver.extend(
            ft.stage_times
                .iter()
                .zip(&ft.local_rank)
                .zip(&ft.world_rank)
                .enumerate()
                .map(|(i, ((t, local_rank), world_rank))| {
                    let fastest = fastest_stages[i].unwrap();
                    [
                        format_time(*t, false),
                        format_delta(*t, fastest, false),
                        art_of_rally_leaderboard_api::car_name(stages[i].1, ft.cars[i]).to_string(),
                        format!(
                            "{local_rank} | {}",
                            match world_rank {
                                Some(r) => r.to_string(),
                                None => "?".to_string(),
                            }
                        ),
                        format_percent(*t, fastest),
                    ]
                }),
        );
        drivers.push(driver);
    }
    for pt in partial_times {
        let mut driver = vec![[
            pt.user_name.to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ]];
        driver.push([
            format!("* {}", format_time(pt.total_time, true)),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ]);
        driver.extend(pt.stage_times.iter().zip(&pt.world_rank).enumerate().map(
            |(i, (t, rank))| match t {
                Some(t) => {
                    let fastest = fastest_stages[i].unwrap();
                    [
                        format_time(*t, false),
                        format_delta(*t, fastest, false),
                        art_of_rally_leaderboard_api::car_name(stages[i].1, pt.cars[i].unwrap())
                            .to_string(),
                        format!("world: {}", rank.unwrap()),
                        format_percent(*t, fastest),
                    ]
                }
                None => [
                    "-".to_string(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ],
            },
        ));
        driver.extend(std::iter::repeat_n(
            [
                "-".to_string(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
            num_cols - driver.len(),
        ));
        drivers.push(driver);
    }

    (header, drivers)
}

pub fn format_time(ms: usize, long: bool) -> String {
    let minutes = ms / 1000 / 60;
    let seconds = (ms / 1000) % 60;
    let millis = ms % 1000;
    if long {
        format!("{minutes:02}:{seconds:02}.{millis:03}")
    } else {
        format!("{minutes:01}:{seconds:02}.{millis:03}")
    }
}

pub fn format_delta(ms: usize, fast: usize, long: bool) -> String {
    assert!(ms >= fast);
    if ms == fast {
        "         ".to_string()
    } else {
        format!("+{}", format_time(ms - fast, long))
    }
}

pub fn format_percent(ms: usize, fast: usize) -> String {
    assert!(ms >= fast);
    if ms == fast {
        "      ".to_string()
    } else {
        format!("{:.2}%", (ms as f32 * 100.0) / fast as f32)
    }
}
