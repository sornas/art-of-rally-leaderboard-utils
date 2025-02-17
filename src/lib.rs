use std::collections::BTreeMap;

use art_of_rally_leaderboard_api::{
    Area, Direction, Group, Leaderboard, Platform, Response, Stage, Weather,
};
use snafu::Whatever;

pub mod http;
pub mod table_utils;

pub struct Rally {
    pub stages: Vec<(Stage, Group, Weather)>,
    pub results: Vec<(String, Vec<Option<StageResult>>)>,
}

#[derive(Clone)]
pub struct StageResult {
    pub car: usize,
    pub time_ms: usize,
}

pub fn get_default_rallys() -> Vec<Vec<(Stage, Group, Weather)>> {
    vec![
        [1, 2, 3, 4, 5, 6]
            .map(|stage_number| {
                (
                    {
                        Stage {
                            area: Area::Kenya,
                            stage_number,
                            direction: Direction::Forward,
                        }
                    },
                    Group::GroupB,
                    Weather::Dry,
                )
            })
            .to_vec(),
        [1, 2, 3, 4, 5, 6]
            .map(|stage_number| {
                (
                    {
                        Stage {
                            area: Area::Japan,
                            stage_number,
                            direction: Direction::Forward,
                        }
                    },
                    Group::GroupA,
                    Weather::Wet,
                )
            })
            .to_vec(),
    ]
}

pub fn get_default_users() -> (Platform, Vec<u64>) {
    (
        Platform::Steam,
        vec![76561198230518420, 76561198087789780, 76561198062269100],
    )
}

pub fn get_rally_results(leaderboards: &[Leaderboard], users: &[u64]) -> Result<Rally, Whatever> {
    let urls: Vec<_> = leaderboards
        .iter()
        .map(|l| l.as_url(users[0], &users[1..]))
        .collect();
    let responses = http::download_all::<Response>(&urls)?;

    let mut results: BTreeMap<String, Vec<Option<StageResult>>> = BTreeMap::new();
    for (i, response) in responses.into_iter().enumerate() {
        let response = response.unwrap().unwrap();
        let entries = response.leaderboard;
        for entry in entries {
            let for_user = results
                .entry(entry.user_name)
                .or_insert_with(|| vec![Option::None; leaderboards.len()]);
            for_user[i] = Some(StageResult {
                car: entry.car_id,
                time_ms: entry.score,
            })
        }
    }

    Ok(Rally {
        stages: leaderboards
            .iter()
            .map(|l| (l.stage, l.group, l.weather))
            .collect(),
        results: results.into_iter().collect(),
    })
}

pub struct FullTime<'s> {
    pub total_time: usize,
    pub user_name: &'s str,
    pub stage_times: Vec<usize>,
    pub cars: Vec<usize>,
}

pub struct PartialTime<'s> {
    pub finished_stages: usize,
    pub total_time: usize,
    pub user_name: &'s str,
    pub stage_times: Vec<Option<usize>>,
    pub cars: Vec<Option<usize>>,
}

pub fn split_times(rally: &Rally) -> (Vec<FullTime<'_>>, Vec<PartialTime<'_>>) {
    let mut full_times = Vec::new();
    let mut partial_times = Vec::new();

    for results in rally.results.iter() {
        let times = results
            .1
            .iter()
            .map(|o| o.as_ref().map(|stage| stage.time_ms));
        let cars = results.1.iter().map(|o| o.as_ref().map(|stage| stage.car));
        let total_time: usize = times.clone().flatten().sum();
        let finished = results.1.iter().filter(|o| o.is_some()).count();
        let is_full = finished == results.1.len();
        if is_full {
            full_times.push(FullTime {
                total_time,
                user_name: results.0.as_str(),
                stage_times: times.map(|o| o.unwrap()).collect(),
                cars: cars.map(|o| o.unwrap()).collect(),
            })
        } else {
            partial_times.push(PartialTime {
                finished_stages: finished,
                total_time,
                user_name: results.0.as_str(),
                stage_times: times.collect(),
                cars: cars.collect(),
            });
        }
    }
    full_times.sort_by_key(|ft| ft.total_time);
    // sort partialtimes first by amount of finished stages (largest first), then by total time (smallest first)
    partial_times.sort_by(|pt1, pt2| {
        pt2.finished_stages
            .cmp(&pt1.finished_stages)
            .then(pt1.total_time.cmp(&pt2.total_time))
    });
    (full_times, partial_times)
}

pub fn fastest_times(
    full_times: &[FullTime],
    rally: &Rally,
) -> (Option<usize>, [Option<usize>; 6]) {
    let fastest_total = full_times.iter().map(|ft| ft.total_time).min();
    let mut fastest_per_stage = [Option::<usize>::None; 6];
    for (_, times) in &rally.results {
        for (time, fastest) in times.iter().zip(fastest_per_stage.iter_mut()) {
            let time = time.as_ref().map(|u| u.time_ms);
            let fastest_ = fastest.as_ref().map(|u| *u);
            match (time, fastest_) {
                (Some(t), None) => *fastest = Some(t),
                (Some(t), Some(cur)) => *fastest = Some(cur.min(t)),
                (None, _) => {}
            }
        }
    }
    (fastest_total, fastest_per_stage)
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
