use std::mem;

use art_of_rally_leaderboard_api::{
    Area, Direction, Filter, Group, IntoEnumIterator as _, Leaderboard, Platform, Response, Weather,
};
use color_eyre::Result;
use curl::{
    easy::{Easy2, Handler, WriteError},
    multi::{Easy2Handle, Multi},
};
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
) -> Result<Vec<Option<T>>> {
    let mut multi = Multi::new();
    let mut handles = urls
        .iter()
        .enumerate()
        .map(|(token, url)| download(&mut multi, token, url.as_ref()))
        .collect::<Result<Vec<_>>>()?;
    let mut responses = vec![Option::None; handles.len()];

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
                    responses[token] = Some(resp);
                }
                Err(e) => {
                    eprintln!("error: {} - <{}>", e, urls[token].as_ref());
                }
            }
        });
    }

    Ok(responses)
}

fn main() -> Result<()> {
    color_eyre::install().unwrap();

    let urls: Vec<_> = (1..=6)
        .map(|stage| {
            (Leaderboard {
                area: Area::Finland,
                stage,
                direction: Direction::Forward,
                weather: Weather::Dry,
                group: Group::GroupA,
                filter: Filter::Friends,
                platform: Platform::Steam,
            })
            .as_url(76561198230518420, &[76561198087789780, 76561198062269100])
        })
        .collect();

    dbg!(&download_all::<Response>(&urls));

    Ok(())
}
