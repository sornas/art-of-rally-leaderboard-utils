use std::{collections::BTreeMap, mem};

use art_of_rally_leaderboard_api::{
    Area, Direction, Filter, Group, IntoEnumIterator as _, Leaderboard, Platform, Response, Weather,
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
    let leaderboards = (1..=6).cartesian_product(combinations.iter());
    let urls: Vec<_> = leaderboards
        .clone()
        .map(|(stage, (area, group))| {
            (Leaderboard {
                area: *area,
                direction,
                weather,
                stage,
                group: *group,
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
            rallys.get_mut(&(*area, *group)).unwrap()[user][stage] = Some(entry.score);
        }
    }

    // prepare table data: time in total and time per stage, split depending on
    // how many stages have been run by each driver

    for ((area, group), users) in &rallys {
        let mut fulltimes = Vec::new();
        let mut partialtimes = Vec::new();
        let mut nonetimes = Vec::new();
        for (user_name, user_idx) in &name_to_idx {
            let times = &users[*user_idx];
            let total_time: usize = times.iter().flatten().sum();
            let finished = times.iter().copied().filter(Option::is_some).count();
            let is_full = finished == times.len();
            let is_none = finished == 0;
            if is_full {
                fulltimes.push((
                    total_time,
                    user_name,
                    times
                        .iter()
                        .cloned()
                        .map(Option::unwrap)
                        .collect::<Vec<_>>(),
                ));
            } else if is_none {
                nonetimes.push(user_name);
            } else {
                partialtimes.push((finished, total_time, user_name, times));
            }
        }
        fulltimes.sort();
        // sort partialtimes first by amount of finished stages (largest first), then by total time (smallest first)
        partialtimes.sort_by(|(finished1, time1, _, _), (finished2, time2, _, _)| {
            finished2.cmp(finished1).then(time1.cmp(time2))
        });

        // generate text table

        let mut table = Table::new();
        table.load_preset(comfy_table::presets::ASCII_BORDERS_ONLY_CONDENSED);
        // TODO: names of stages
        table.set_header(vec![
            "user", "total", "stage 1", "stage 2", "stage 3", "stage 4", "stage 5", "stage 6",
        ]);
        for column in table.column_iter_mut().skip(1) {
            column.set_cell_alignment(CellAlignment::Right);
        }
        for (total_time, name, times) in &fulltimes {
            let mut row = vec![name.to_string(), format_time(*total_time, true)];
            row.extend(times.iter().map(|t| format_time(*t, false)));
            table.add_row(row);
        }
        for (_, total_time, name, times) in &partialtimes {
            let mut row = vec![
                name.to_string(),
                format!("* {}", format_time(*total_time, true)),
            ];
            row.extend(times.iter().map(|t| match t {
                Some(t) => format_time(*t, false),
                None => "-".to_string(),
            }));
            table.add_row(row);
        }
        for name in &nonetimes {
            let mut row = vec![name.to_string()];
            row.extend(std::iter::repeat("-".to_string()).take(7));
            table.add_row(row);
        }
        println!("{:?} ({:?})", area, group);
        println!("{table}");
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
