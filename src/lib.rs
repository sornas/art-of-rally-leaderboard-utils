use std::collections::BTreeMap;

use art_of_rally_leaderboard_api::{
    Area, Direction, Filter, Group, Leaderboard, Platform, Response, Stage, Weather,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use snafu::Whatever;

pub mod http;
pub mod table_utils;

pub type StageWithLeaderboard = (Stage, Group, Weather);

pub struct Rally {
    pub stages: Vec<StageWithLeaderboard>,
    pub results: Vec<(String, Vec<Option<StageResult>>)>,
}

#[derive(Clone)]
pub struct StageResult {
    pub car: usize,
    pub time_ms: usize,
    pub local_rank: usize,
    pub world_rank: Option<usize>,
}

pub fn get_default_rallys() -> Vec<(String, Vec<StageWithLeaderboard>)> {
    vec![
        (
            "kenya - group b".to_string(),
            [1, 2, 3, 4, 5, 6]
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
        ),
        (
            "norway - group 4".to_string(),
            [1, 2, 3, 4, 5, 6]
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
        ),
    ]
}

pub fn get_default_users() -> (Platform, Vec<u64>, Vec<&'static str>) {
    (
        Platform::Steam,
        // TODO: map id to name some other way
        vec![
            76561198230518420,
            76561198087789780,
            76561198062269100,
            76561198207854185,
            76561198049071819,
            76561197996901884,
            76561198305558712,
            76561198052484118,
            76561198857520448
        ],
        vec!["Johan B", "Martin", "Gustav", "Beatrice", "Emil", "Frans", "Anton", "Leo", "Larssa"],
    )
}

pub fn get_rally_results(
    leaderboards: &[(StageWithLeaderboard, Platform)],
    user_ids: &[u64],
    user_names: &[&str],
) -> Result<Rally, Whatever> {
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
    let leaderboard_results = http::download_all::<Response>(&result_urls)?;

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
    let ranks = http::download_all::<Rank>(&rank_urls)?;
    // If we chunk by number of leaderboards we get chunks per user.
    let world_rank_by_user: Vec<_> = ranks.chunks_exact(leaderboards.len()).collect();

    let mut rally_results: BTreeMap<String, Vec<Option<StageResult>>> = BTreeMap::new();
    for (leaderboard_idx, leaderboard) in leaderboard_results.into_iter().enumerate() {
        let mut entries = leaderboard.unwrap().unwrap().leaderboard;

        // We don't know which user id is which user! But we know the relative
        // ranking of usernames (LeaderboardEntry), and the world rank for each
        // user (world_rank_by_user). Sort the entries by local rank, and lookup
        // the name (and world rank) from the corresponding (ranking-relative)
        // world rank.

        entries.sort_by_key(|entry| entry.rank);

        let mut sorted_world_ranks = world_rank_by_user
            .iter()
            .flat_map(|user_ranks| user_ranks.get(leaderboard_idx).unwrap())
            .zip(user_names)
            .filter_map(|(rank, name)| rank.as_ref().ok().map(|rank| (rank.rank, name)))
            .sorted_by_key(|(rank, _name)| *rank);

        for entry in entries {
            let (world_rank, name) = sorted_world_ranks.next().unwrap();
            let entry_for_user = rally_results
                .entry(name.to_string())
                .or_insert_with(|| vec![Option::None; leaderboards.len()]);
            entry_for_user[leaderboard_idx] = Some(StageResult {
                car: entry.car_id,
                time_ms: entry.score,
                local_rank: entry.rank,
                world_rank: Some(world_rank),
            })
        }
    }

    Ok(Rally {
        stages: leaderboards
            .iter()
            .copied()
            .map(|(stage, _)| stage)
            .collect(),
        results: rally_results.into_iter().collect(),
    })
}

pub struct FullTime<'s> {
    pub total_time: usize,
    pub user_name: &'s str,
    pub stage_times: Vec<usize>,
    pub local_rank: Vec<usize>,
    pub world_rank: Vec<Option<usize>>,
    pub cars: Vec<usize>,
}

pub struct PartialTime<'s> {
    pub finished_stages: usize,
    pub total_time: usize,
    pub user_name: &'s str,
    pub stage_times: Vec<Option<usize>>,
    pub local_rank: Vec<Option<usize>>,
    pub world_rank: Vec<Option<usize>>,
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
        let local_rank = results
            .1
            .iter()
            .map(|o| o.as_ref().map(|stage| stage.local_rank));
        let world_rank = results
            .1
            .iter()
            .map(|o| o.as_ref().map(|stage| stage.world_rank));
        let total_time: usize = times.clone().flatten().sum();
        let finished = results.1.iter().filter(|o| o.is_some()).count();
        let is_full = finished == results.1.len();
        if is_full {
            full_times.push(FullTime {
                total_time,
                user_name: results.0.as_str(),
                stage_times: times.map(|o| o.unwrap()).collect(),
                local_rank: local_rank.map(|o| o.unwrap()).collect(),
                world_rank: world_rank.map(|o| o.unwrap()).collect(),
                cars: cars.map(|o| o.unwrap()).collect(),
            })
        } else {
            partial_times.push(PartialTime {
                finished_stages: finished,
                total_time,
                user_name: results.0.as_str(),
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
    rally: &Rally,
) -> (Option<usize>, Vec<Option<usize>>) {
    let fastest_total = full_times.iter().map(|ft| ft.total_time).min();
    let mut fastest_per_stage = vec![Option::<usize>::None; rally.stages.len()];
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
