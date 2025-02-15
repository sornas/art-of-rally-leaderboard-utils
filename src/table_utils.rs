use art_of_rally_leaderboard_api::{Area, Direction, Group, Stage};

use crate::{format_delta, format_time, FullTime, PartialTime};

pub fn stages(
    full_times: &[FullTime],
    partial_times: &[PartialTime],
    none_times: &[&str],
    fastest_total: Option<usize>,
    fastest_stages: [Option<usize>; 6],
    group: Group,
    area: Area,
    direction: Direction,
) -> (Vec<String>, Vec<Vec<[String; 3]>>) {
    let mut header = vec!["user".to_string(), "total".to_string()];
    header.extend((1..=6).map(|stage_number| {
        (Stage {
            area,
            stage_number,
            direction,
        })
        .to_string()
    }));

    let mut drivers = Vec::new();
    for ft in full_times {
        let mut driver = vec![[ft.user_name.to_string(), String::new(), String::new()]];
        driver.push([
            format_time(ft.total_time, true),
            format_delta(ft.total_time, fastest_total.unwrap(), true),
            String::new(),
        ]);
        driver.extend(ft.stage_times.iter().enumerate().map(|(i, t)| {
            [
                format_time(*t, false),
                format_delta(*t, fastest_stages[i].unwrap(), false),
                art_of_rally_leaderboard_api::car_name(group, ft.cars[i]).to_string(),
            ]
        }));
        drivers.push(driver);
    }
    for pt in partial_times {
        let mut driver = vec![[pt.user_name.to_string(), String::new(), String::new()]];
        driver.push([
            format!("* {}", format_time(pt.total_time, true)),
            String::new(),
            String::new(),
        ]);
        driver.extend(pt.stage_times.iter().enumerate().map(|(i, t)| match t {
            Some(t) => [
                format_time(*t, false),
                format_delta(*t, fastest_stages[i].unwrap(), false),
                art_of_rally_leaderboard_api::car_name(group, pt.cars[i].unwrap()).to_string(),
            ],
            None => ["-".to_string(), String::new(), String::new()],
        }));
        drivers.push(driver);
    }
    for name in none_times {
        let mut driver = vec![[name.to_string(), String::new(), String::new()]];
        driver.extend(std::iter::repeat(["-".to_string(), String::new(), String::new()]).take(7));
        drivers.push(driver)
    }

    (header, drivers)
}
