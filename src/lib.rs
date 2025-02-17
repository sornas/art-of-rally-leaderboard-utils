use std::collections::BTreeMap;

use art_of_rally_leaderboard_api::{
    Area, Direction, Filter, Group, Leaderboard, Platform, Response, Stage, Weather,
};
use itertools::Itertools as _;
use snafu::Whatever;

pub mod http;
pub mod table_utils;

pub type Rally<const STAGES: usize> = [[Option<(usize, usize)>; STAGES]; 3];
pub type UserMap<'s> = BTreeMap<&'s str, usize>;

pub fn get_interesting_leaderboards(
) -> Result<(BTreeMap<(Area, Group, Weather), Rally<6>>, UserMap<'static>), Whatever> {
    let direction = Direction::Forward;
    let combinations = [
        (Area::Kenya, Group::GroupB, Weather::Dry),
        (Area::Japan, Group::GroupA, Weather::Wet),
    ];
    // let combinations = Area::iter().cartesian_product(Group::iter());

    let users = [76561198230518420, 76561198087789780, 76561198062269100];
    let name_to_idx: UserMap = vec![("sornas", 0), ("jonais", 1), ("Gurka", 2)]
        .into_iter()
        .collect();

    // generate API URLs for each leaderboard and download the leaderboards

    let leaderboards = (1..=6).cartesian_product(combinations.clone()).map(
        |(stage_number, (area, group, weather))| {
            (
                Stage {
                    area,
                    stage_number,
                    direction,
                },
                group,
                weather,
            )
        },
    );
    let urls: Vec<_> = leaderboards
        .clone()
        .map(|(stage, group, weather)| {
            (Leaderboard {
                stage,
                weather,
                group,
                filter: Filter::Friends,
                platform: Platform::Steam,
            })
            .as_url(users[0], &users[1..])
        })
        .collect();
    let responses = http::download_all::<Response>(&urls)?;

    // collect the responses

    let mut rallys: BTreeMap<(Area, Group, Weather), [[Option<(usize, usize)>; 6]; 3]> =
        BTreeMap::new();
    for (area, group, weather) in combinations {
        rallys.insert((area, group, weather), [[None; 6], [None; 6], [None; 6]]);
    }

    for ((stage, group, weather), response) in leaderboards.zip(responses.iter()) {
        let response = response.as_ref().unwrap().as_ref().unwrap();
        let entries = &response.leaderboard;
        for entry in entries.iter() {
            let user = name_to_idx[entry.user_name.as_str()];
            rallys.get_mut(&(stage.area, group, weather)).unwrap()[user][stage.stage_number - 1] =
                Some((entry.score, entry.car_id));
        }
    }

    Ok((rallys, name_to_idx))
}

pub struct FullTime<'s> {
    pub total_time: usize,
    pub user_name: &'s str,
    pub stage_times: [usize; 6],
    pub cars: [usize; 6],
}

pub struct PartialTime<'s> {
    pub finished_stages: usize,
    pub total_time: usize,
    pub user_name: &'s str,
    pub stage_times: [Option<usize>; 6],
    pub cars: [Option<usize>; 6],
}

pub fn split_times<'s>(
    users: &Rally<6>,
    name_to_idx: &UserMap<'s>,
) -> (Vec<FullTime<'s>>, Vec<PartialTime<'s>>, Vec<&'s str>) {
    let mut full_times = Vec::new();
    let mut partial_times = Vec::new();
    let mut none_times = Vec::new();
    for (user_name, user_idx) in name_to_idx {
        let times_cars = &users[*user_idx];
        let times = times_cars.map(|o| o.map(|(time, _)| time));
        let cars = times_cars.map(|o| o.map(|(_, car)| car));
        let total_time: usize = times.iter().flatten().sum();
        let finished = times_cars.iter().copied().filter(Option::is_some).count();
        let is_full = finished == times_cars.len();
        let is_none = finished == 0;
        if is_full {
            full_times.push(FullTime {
                total_time,
                user_name: *user_name,
                stage_times: times.map(Option::unwrap),
                cars: cars.map(Option::unwrap),
            });
        } else if is_none {
            none_times.push(*user_name);
        } else {
            partial_times.push(PartialTime {
                finished_stages: finished,
                total_time,
                user_name: *user_name,
                stage_times: times,
                cars,
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
    (full_times, partial_times, none_times)
}

pub fn fastest_times(
    full_times: &[FullTime],
    user_times: &[[Option<(usize, usize)>; 6]; 3],
) -> (Option<usize>, [Option<usize>; 6]) {
    let fastest_total = full_times.iter().map(|ft| ft.total_time).min();
    let mut fastest_per_stage = [Option::<usize>::None; 6];
    for times in user_times {
        for (time, fastest) in times.iter().zip(fastest_per_stage.iter_mut()) {
            let time = time.as_ref().map(|u| *u);
            let fastest_ = fastest.as_ref().map(|u| *u);
            match (time, fastest_) {
                (Some((t, _)), None) => *fastest = Some(t),
                (Some((t, _)), Some(cur)) => *fastest = Some(cur.min(t)),
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
