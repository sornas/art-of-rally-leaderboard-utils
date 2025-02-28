use crate::{FullTime, PartialTime, StageWithLeaderboard, format_delta, format_time};

pub fn stages(
    stages: &[StageWithLeaderboard],
    full_times: &[FullTime],
    partial_times: &[PartialTime],
    fastest_total: Option<usize>,
    fastest_stages: &[Option<usize>],
) -> (Vec<String>, Vec<Vec<[String; 4]>>) {
    let mut header = vec!["user".to_string(), "total".to_string()];
    header.extend(
        stages
            .iter()
            .map(|(stage, _group, weather)| format!("{} ({})", stage, weather)),
    );

    let mut drivers = Vec::new();
    for ft in full_times {
        let mut driver = vec![[
            ft.user_name.to_string(),
            String::new(),
            String::new(),
            String::new(),
        ]];
        driver.push([
            format_time(ft.total_time, true),
            format_delta(ft.total_time, fastest_total.unwrap(), true),
            String::new(),
            String::new(),
        ]);
        driver.extend(
            ft.stage_times
                .iter()
                .zip(&ft.local_rank)
                .zip(&ft.world_rank)
                .enumerate()
                .map(|(i, ((t, local_rank), world_rank))| {
                    [
                        format_time(*t, false),
                        format_delta(*t, fastest_stages[i].unwrap(), false),
                        art_of_rally_leaderboard_api::car_name(stages[i].1, ft.cars[i]).to_string(),
                        format!(
                            "{local_rank} | {}",
                            match world_rank {
                                Some(r) => r.to_string(),
                                None => "?".to_string(),
                            }
                        ),
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
        ]];
        driver.push([
            format!("* {}", format_time(pt.total_time, true)),
            String::new(),
            String::new(),
            String::new(),
        ]);
        driver.extend(pt.stage_times.iter().zip(&pt.world_rank).enumerate().map(
            |(i, (t, rank))| {
                match t {
                    Some(t) => [
                        format_time(*t, false),
                        format_delta(*t, fastest_stages[i].unwrap(), false),
                        art_of_rally_leaderboard_api::car_name(stages[i].1, pt.cars[i].unwrap())
                            .to_string(),
                        format!("world: {}", rank.unwrap()),
                    ],
                    None => ["-".to_string(), String::new(), String::new(), String::new()],
                }
            },
        ));
        drivers.push(driver);
    }

    (header, drivers)
}
