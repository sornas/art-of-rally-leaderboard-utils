use std::mem;
use std::{collections::BTreeMap, time::Duration};

use art_of_rally_leaderboard_api::{
    Area, Direction, Filter, Group, Leaderboard, Platform, Response, Stage, Weather,
};
use curl::{
    easy::{Easy2, Handler, WriteError},
    multi::{Easy2Handle, Multi},
};
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools as _;
use serde::Deserialize;
use snafu::{ResultExt, Whatever};

pub mod table_utils;

pub type Rally<const STAGES: usize> = [[Option<(usize, usize)>; STAGES]; 3];
pub type UserMap<'s> = BTreeMap<&'s str, usize>;

struct Collector(Vec<u8>);
impl Handler for Collector {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.0.extend_from_slice(data);
        Ok(data.len())
    }
}

/// Start a request on a [Multi] handle.
fn download(
    multi: &mut Multi,
    token: usize,
    url: &str,
) -> Result<Easy2Handle<Collector>, Whatever> {
    let mut request = Easy2::new(Collector(Vec::new()));
    request.url(url).whatever_context("invalid url")?;

    let mut handle = multi
        .add2(request)
        .whatever_context("could not add request to multi handle")?;
    handle.set_token(token).unwrap();
    Ok(handle)
}

/// Download and JSON-parse the results for some URLs.
pub fn download_all<T: for<'a> Deserialize<'a> + Clone>(
    urls: &[impl AsRef<str>],
) -> Result<Vec<Option<Result<T, Whatever>>>, Whatever> {
    let progress_style = ProgressStyle::default_bar()
        .template("{bar} {msg} ({pos}/{len}) {elapsed}")
        .unwrap()
        .progress_chars("#|-");
    let progress = ProgressBar::new(urls.len() as _).with_style(progress_style);
    progress.enable_steady_tick(Duration::from_millis(100));

    let mut multi = Multi::new();
    let mut handles = urls
        .iter()
        .enumerate()
        .map(|(token, url)| download(&mut multi, token, url.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut responses = Vec::new();
    for _ in 0..handles.len() {
        responses.push(Option::None);
    }

    let mut running = true;
    while running {
        if multi.perform().whatever_context("error in multi handle")? == 0 {
            running = false;
        }
        multi.messages(|m| {
            let token = m.token().unwrap();
            let handle = handles.get_mut(token).unwrap();
            // TODO: progress bar should update inside the collector and use content length header
            progress.inc(1);
            match m.result_for2(handle).unwrap() {
                Ok(()) => {
                    let _status = handle.response_code().unwrap();
                    let s = String::from_utf8(mem::take(&mut handle.get_mut().0)).unwrap();
                    let resp = serde_json::from_str(&s).unwrap();
                    responses[token] = Some(Ok(resp));
                }
                Err(e) => {
                    responses[token] = Some(Err(e).with_whatever_context(|_| {
                        format!("error in transfer for {}", urls[token].as_ref())
                    }));
                }
            }
        });
    }

    Ok(responses)
}

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
    let responses = download_all::<Response>(&urls)?;

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
