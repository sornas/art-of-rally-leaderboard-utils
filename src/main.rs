use std::collections::BTreeMap;
use std::mem;

use art_of_rally_leaderboard_api::{
    Area, Direction, Filter, Group, Leaderboard, Platform, Response, Weather,
};
use color_eyre::Result;
use comfy_table::{CellAlignment, Table};
use curl::{
    easy::{Easy2, Handler, WriteError},
    multi::{Easy2Handle, Multi},
};
use itertools::Itertools;
use serde::Deserialize;

struct Collector(Vec<u8>);
impl Handler for Collector {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.0.extend_from_slice(data);
        Ok(data.len())
    }
}

/// Start a request on a [Multi] handle.
fn download(multi: &mut Multi, token: usize, url: &str) -> Result<Easy2Handle<Collector>> {
    let version = curl::Version::get();
    let mut request = Easy2::new(Collector(Vec::new()));
    request.url(url)?;
    request.useragent(&format!("curl/{}", version.version()))?;

    let mut handle = multi.add2(request)?;
    handle.set_token(token)?;
    Ok(handle)
}

/// Download and JSON-parse the results for some URLs.
fn download_all<T: for<'a> Deserialize<'a> + Clone>(
    urls: &[impl AsRef<str>],
) -> Result<Vec<Option<Result<T>>>> {
    let mut multi = Multi::new();
    let mut handles = urls
        .iter()
        .enumerate()
        .map(|(token, url)| download(&mut multi, token, url.as_ref()))
        .collect::<Result<Vec<_>>>()?;
    let mut responses = Vec::new();
    for _ in 0..handles.len() {
        responses.push(Option::None);
    }

    let mut running = true;
    while running {
        if multi.perform()? == 0 {
            running = false;
        }
        multi.messages(|m| {
            let token = m.token().unwrap();
            let handle = handles.get_mut(token).unwrap();
            match m.result_for2(handle).unwrap() {
                Ok(()) => {
                    let _status = handle.response_code().unwrap();
                    // TODO: report download progress
                    let s = String::from_utf8(mem::take(&mut handle.get_mut().0)).unwrap();
                    let resp = serde_json::from_str(&s).unwrap();
                    responses[token] = Some(Ok(resp));
                }
                Err(e) => {
                    responses[token] = Some(Err(e.into()));
                }
            }
        });
    }

    Ok(responses)
}

type FullTime<'s> = (usize, &'s str, [usize; 6]);
type PartialTime<'a> = (usize, usize, &'a str, [Option<usize>; 6]);

fn split_times<'s>(
    users: &[[Option<usize>; 6]; 3],
    name_to_idx: &BTreeMap<&'s str, usize>,
) -> (Vec<FullTime<'s>>, Vec<PartialTime<'s>>, Vec<&'s str>) {
    let mut full_times = Vec::new();
    let mut partial_times = Vec::new();
    let mut none_times = Vec::new();
    for (user_name, user_idx) in name_to_idx {
        let times = &users[*user_idx];
        let total_time: usize = times.iter().flatten().sum();
        let finished = times.iter().copied().filter(Option::is_some).count();
        let is_full = finished == times.len();
        let is_none = finished == 0;
        if is_full {
            full_times.push((
                total_time,
                *user_name,
                times
                    .iter()
                    .cloned()
                    .map(Option::unwrap)
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            ));
        } else if is_none {
            none_times.push(*user_name);
        } else {
            partial_times.push((finished, total_time, *user_name, *times));
        }
    }
    full_times.sort();
    // sort partialtimes first by amount of finished stages (largest first), then by total time (smallest first)
    partial_times.sort_by(|(finished1, time1, _, _), (finished2, time2, _, _)| {
        finished2.cmp(finished1).then(time1.cmp(time2))
    });
    (full_times, partial_times, none_times)
}

fn fastest_times(
    full_times: &[FullTime],
    users: &[[Option<usize>; 6]; 3],
) -> (Option<usize>, [Option<usize>; 6]) {
    let fastest_total = full_times.iter().map(|(t, _, _)| *t).max();
    let mut fastest_per_stage = [Option::<usize>::None; 6];
    for times in users {
        for (time, fastest) in times.iter().zip(fastest_per_stage.iter_mut()) {
            let time = time.as_ref().map(|u| *u);
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

fn text_table(
    full_times: &[FullTime],
    partial_times: &[PartialTime],
    none_times: &[&str],
    fastest_total: Option<usize>,
    fastest_stages: [Option<usize>; 6],
) {
    let mut table = Table::new();
    table.load_preset(comfy_table::presets::ASCII_HORIZONTAL_ONLY);
    // TODO: names of stages
    table.set_header(vec![
        "user", "total", "stage 1", "stage 2", "stage 3", "stage 4", "stage 5", "stage 6",
    ]);
    for column in table.column_iter_mut().skip(1) {
        column.set_cell_alignment(CellAlignment::Right);
    }
    for (total_time, name, times) in full_times {
        let mut row = vec![
            name.to_string(),
            format!(
                "{}\n{}",
                format_time(*total_time, true),
                format_delta(*total_time, fastest_total.unwrap(), true)
            ),
        ];
        row.extend(times.iter().map(|t| format_time(*t, false)));
        table.add_row(row);
    }
    for (_, total_time, name, times) in partial_times {
        let mut row = vec![
            name.to_string(),
            format!("* {}", format_time(*total_time, true)),
        ];
        row.extend(times.iter().enumerate().map(|(i, t)| match t {
            Some(t) => format!(
                "{}\n{}",
                format_time(*t, false),
                format_delta(*t, fastest_stages[i].unwrap(), false)
            ),
            None => "-".to_string(),
        }));
        table.add_row(row);
    }
    for name in none_times {
        let mut row = vec![name.to_string()];
        row.extend(std::iter::repeat("-".to_string()).take(7));
        table.add_row(row);
    }
    println!("{table}");
}

fn main() -> Result<()> {
    color_eyre::install().unwrap();

    let direction = Direction::Forward;
    let weather = Weather::Dry;
    let combinations = [(Area::Kenya, Group::GroupB)];

    let users = [76561198230518420, 76561198087789780, 76561198062269100];
    let name_to_idx: BTreeMap<_, usize> = vec![("sornas", 0), ("jonais", 1), ("Gurka", 2)]
        .into_iter()
        .collect();

    // generate API URLs for each leaderboard and download the leaderboards

    // let combinations = Area::iter().cartesian_product(Group::iter());
    let leaderboards = (1..=6).cartesian_product(combinations);
    let urls: Vec<_> = leaderboards
        .clone()
        .map(|(stage, (area, group))| {
            (Leaderboard {
                area,
                direction,
                weather,
                stage,
                group,
                filter: Filter::Friends,
                platform: Platform::Steam,
            })
            .as_url(users[0], &users[1..])
        })
        .collect();
    let responses = download_all::<Response>(&urls)?;

    // collect the responses

    let mut rallys: BTreeMap<(Area, Group), [[Option<usize>; 6]; 3]> = BTreeMap::new();
    for (area, group) in &combinations {
        rallys.insert((*area, *group), [[None; 6], [None; 6], [None; 6]]);
    }

    for ((stage, (area, group)), response) in leaderboards.zip(responses.iter()) {
        let response = response.as_ref().unwrap().as_ref().unwrap();
        let entries = &response.leaderboard;
        for entry in entries.iter() {
            let user = name_to_idx[entry.user_name.as_str()];
            rallys.get_mut(&(area, group)).unwrap()[user][stage] = Some(entry.score);
        }
    }

    // prepare table data: time in total and time per stage, split depending on
    // how many stages have been run by each driver

    for ((area, group), users) in &rallys {
        let (full_times, partial_times, none_times) = split_times(users, &name_to_idx);
        let (fastest_total, fastest_per_stage) = fastest_times(&full_times, users);

        // generate text table

        println!("{:?} ({:?})", area, group);
        text_table(
            &full_times,
            &partial_times,
            &none_times,
            fastest_total,
            fastest_per_stage,
        );
    }

    Ok(())
}

fn format_time(ms: usize, long: bool) -> String {
    let minutes = ms / 1000 / 60;
    let seconds = (ms / 1000) % 60;
    let millis = ms % 1000;
    if long {
        format!("{minutes:02}:{seconds:02}.{millis:03}")
    } else {
        format!("{minutes:01}:{seconds:02}.{millis:03}")
    }
}

fn format_delta(ms: usize, fast: usize, long: bool) -> String {
    assert!(ms >= fast);
    if ms == fast {
        "         ".to_string()
    } else {
        format!("+{}", format_time(ms - fast, long))
    }
}
