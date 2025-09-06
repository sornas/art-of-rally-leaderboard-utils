use std::collections::BTreeMap;

use art_of_rally_leaderboard_api::{
    Area, Direction, Filter, Group, Leaderboard, Platform, Response, Stage, Weather,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use snafu::Whatever;

pub mod http;
pub mod secret;
pub mod table_utils;

pub type StageWithLeaderboard = (Stage, Group, Weather);

#[derive(Deserialize, Serialize)]
pub struct Rally {
    pub title: String,
    pub stages: Vec<StageWithLeaderboard>,
}

#[derive(Deserialize, Serialize)]
pub struct DriverResult {
    pub name: String,
    pub stages: Vec<Option<StageResult>>,
}

#[derive(Deserialize, Serialize)]
pub struct RallyResults {
    pub stages: Vec<StageWithLeaderboard>,
    pub driver_results: Vec<DriverResult>,
    pub stage_results: Vec<Vec<(String, StageResult)>>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct StageResult {
    pub car: usize,
    pub time_ms: usize,
    pub local_rank: usize,
    pub world_rank: Option<usize>,
}

pub fn get_default_rallys() -> Vec<Rally> {
    vec![
        Rally {
            title: "kenya - group b".to_string(),
            stages: [1, 2, 3, 4, 5, 6]
                .map(|stage_number| {
                    (
                        Stage {
                            area: Area::Kenya,
                            stage_number,
                            direction: Direction::Forward,
                        },
                        Group::GroupB,
                        Weather::Dry,
                    )
                })
                .to_vec(),
        },
        Rally {
            title: "norway - group 4".to_string(),
            stages: [1, 2, 3, 4, 5, 6]
                .map(|stage_number| {
                    (
                        Stage {
                            area: Area::Norway,
                            stage_number,
                            direction: Direction::Forward,
                        },
                        Group::Eighties,
                        Weather::Dry,
                    )
                })
                .to_vec(),
        },
    ]
}

pub fn get_rally_results(
    leaderboards: &[(StageWithLeaderboard, Platform)],
    user_ids: &[u64],
    user_names: &[&str],
) -> Result<RallyResults, Whatever> {
    let stages = leaderboards
        .iter()
        .copied()
        .map(|(stage, _)| stage)
        .collect_vec();
    let result_urls: Vec<_> = leaderboards
        .iter()
        .copied()
        .map(|((stage, group, weather), platform)| {
            (Leaderboard {
                stage,
                weather,
                group,
                platform,
                filter: Filter::Friends,
            })
            .as_url(user_ids[0], &user_ids[1..])
        })
        .collect();
    let leaderboard_results = http::download_all::<Response>(&result_urls);

    // TODO: only ask for rank of users who have a time
    let rank_urls: Vec<_> = user_ids
        .iter()
        .cartesian_product(leaderboards.iter().copied().map(
            |((stage, group, weather), platform)| Leaderboard {
                stage,
                weather,
                group,
                platform,
                filter: Filter::PlayerRank,
            },
        ))
        .map(|(user, leaderboard)| leaderboard.as_url(*user, &[]))
        .collect();

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct Rank {
        #[serde(rename = "result")]
        _result: i32,
        rank: usize,
    }

    // World rank, in the same order we asked for (so users x leaderboard: [(user1, board1), (user1, board2), ..., (user2, board1), ...])
    let ranks = http::download_all::<Rank>(&rank_urls);
    // If we chunk by number of leaderboards we get chunks per user.
    let world_rank_by_user: Vec<_> = ranks.chunks_exact(leaderboards.len()).collect();

    let mut driver_results: BTreeMap<String, Vec<Option<StageResult>>> = BTreeMap::new();
    for (stage_idx, leaderboard) in leaderboard_results.into_iter().enumerate() {
        let mut entries = leaderboard.unwrap().leaderboard;

        // We don't know which user id is which user! But we know the relative
        // ranking of usernames (LeaderboardEntry), and the world rank for each
        // user (world_rank_by_user). Sort the entries by local rank, and lookup
        // the name (and world rank) from the corresponding (ranking-relative)
        // world rank.

        entries.sort_by_key(|entry| entry.rank);

        let mut sorted_world_ranks = world_rank_by_user
            .iter()
            .flat_map(|user_ranks| user_ranks.get(stage_idx).unwrap())
            .zip(user_names)
            .map(|(rank, name)| (rank.rank, name))
            .sorted_by_key(|(rank, _name)| *rank);

        for entry in entries {
            let (world_rank, name) = sorted_world_ranks.next().unwrap();
            let entry_for_user = driver_results
                .entry(name.to_string())
                .or_insert_with(|| vec![Option::None; leaderboards.len()]);
            entry_for_user[stage_idx] = Some(StageResult {
                car: entry.car_id,
                time_ms: entry.score,
                local_rank: entry.rank,
                world_rank: Some(world_rank),
            })
        }
    }

    let mut stage_results = stages.iter().map(|_| Vec::new()).collect_vec();
    for (driver, driver_stage_results) in &driver_results {
        for (i, driver_stage_result) in driver_stage_results.iter().enumerate() {
            let Some(driver_stage_result) = driver_stage_result else {
                continue;
            };
            stage_results[i].push((driver.clone(), driver_stage_result.clone()));
        }
    }
    for stage_result in &mut stage_results {
        stage_result.sort_by_key(|(_, x)| x.time_ms);
    }

    Ok(RallyResults {
        stages,
        driver_results: driver_results
            .into_iter()
            .map(|(name, stages)| DriverResult { name, stages })
            .collect(),
        stage_results,
    })
}

#[derive(Debug)]
pub struct FullTime<'s> {
    pub total_time: usize,
    pub user_name: &'s str,
    pub stage_times: Vec<usize>,
    pub local_rank: Vec<usize>,
    pub world_rank: Vec<Option<usize>>,
    pub cars: Vec<usize>,
}

#[derive(Debug)]
pub struct PartialTime<'s> {
    pub finished_stages: usize,
    pub total_time: usize,
    pub user_name: &'s str,
    pub stage_times: Vec<Option<usize>>,
    pub local_rank: Vec<Option<usize>>,
    pub world_rank: Vec<Option<usize>>,
    pub cars: Vec<Option<usize>>,
}

pub fn split_times(rally: &RallyResults) -> (Vec<FullTime<'_>>, Vec<PartialTime<'_>>) {
    let mut full_times = Vec::new();
    let mut partial_times = Vec::new();

    for driver in rally.driver_results.iter() {
        let times = driver
            .stages
            .iter()
            .map(|o| o.as_ref().map(|stage| stage.time_ms));
        let cars = driver
            .stages
            .iter()
            .map(|o| o.as_ref().map(|stage| stage.car));
        let local_rank = driver
            .stages
            .iter()
            .map(|o| o.as_ref().map(|stage| stage.local_rank));
        let world_rank = driver
            .stages
            .iter()
            .map(|o| o.as_ref().map(|stage| stage.world_rank));
        let total_time: usize = times.clone().flatten().sum();
        let finished = driver.stages.iter().filter(|o| o.is_some()).count();
        let is_full = finished == driver.stages.len();
        if is_full {
            full_times.push(FullTime {
                total_time,
                user_name: driver.name.as_str(),
                stage_times: times.map(|o| o.unwrap()).collect(),
                local_rank: local_rank.map(|o| o.unwrap()).collect(),
                world_rank: world_rank.map(|o| o.unwrap()).collect(),
                cars: cars.map(|o| o.unwrap()).collect(),
            })
        } else {
            partial_times.push(PartialTime {
                finished_stages: finished,
                total_time,
                user_name: driver.name.as_str(),
                stage_times: times.collect(),
                local_rank: local_rank.collect(),
                world_rank: world_rank.flatten().collect(),
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
    rally: &RallyResults,
) -> (Option<usize>, Vec<Option<usize>>) {
    let fastest_total = full_times.iter().map(|ft| ft.total_time).min();
    let mut fastest_per_stage = vec![Option::<usize>::None; rally.stages.len()];
    for driver_result in &rally.driver_results {
        for (time, fastest_time) in driver_result
            .stages
            .iter()
            .zip(fastest_per_stage.iter_mut())
        {
            let time = time.as_ref().map(|u| u.time_ms);
            let fastest_ = fastest_time.as_ref().map(|u| *u);
            match (time, fastest_) {
                (Some(t), None) => *fastest_time = Some(t),
                (Some(t), Some(cur)) => *fastest_time = Some(cur.min(t)),
                (None, _) => {}
            }
        }
    }
    (fastest_total, fastest_per_stage)
}
