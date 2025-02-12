use std::{
    collections::{BTreeMap, HashMap},
    mem,
};

use art_of_rally_leaderboard_api::{
    Area, Direction, Filter, Group, IntoEnumIterator as _, Leaderboard, Platform, Response, Weather,
};
use color_eyre::Result;
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

fn download(multi: &mut Multi, token: usize, url: &str) -> Result<Easy2Handle<Collector>> {
    let version = curl::Version::get();
    let mut request = Easy2::new(Collector(Vec::new()));
    request.url(url)?;
    request.useragent(&format!("curl/{}", version.version()))?;

    let mut handle = multi.add2(request)?;
    handle.set_token(token)?;
    Ok(handle)
}

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
                    let status = handle.response_code().unwrap();
                    println!(
                        "transfer succeeded (status: {}) {} (download length: {})",
                        status,
                        urls[token].as_ref(),
                        handle.get_ref().0.len()
                    );
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

    // iter all pairs of (area, group) and look for ones where at least one person has driven all of them

    let users = [76561198230518420, 76561198087789780, 76561198062269100];
    let user_idx: BTreeMap<_, usize> = vec![("sornas", 0), ("jonais", 1), ("Gurka", 2)]
        .into_iter()
        .collect();

    // Area::iter().cartesian_product(Group::iter())
    let combinations = [(Area::Kenya, Group::GroupB)];
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

    let mut rallys: BTreeMap<(Area, Group), [[Option<usize>; 6]; 3]> = BTreeMap::new();
    for (area, group) in &combinations {
        rallys.insert((*area, *group), [[None; 6], [None; 6], [None; 6]]);
    }

    let responses = download_all::<Response>(&urls)?;
    for ((stage, (area, group)), response) in leaderboards.zip(responses.iter()) {
        let response = response.as_ref().unwrap().as_ref().unwrap();
        let entries = &response.leaderboard;
        for entry in entries.iter() {
            dbg!(&entry.user_name);
            let user = user_idx[entry.user_name.as_str()];
            rallys.get_mut(&(*area, *group)).unwrap()[user][stage] = Some(entry.score);
        }
    }

    for (area, group) in &combinations {
        dbg!(rallys[&(*area, *group)]);
    }

    Ok(())
}
