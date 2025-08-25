use std::time::Duration;

use curl::{
    easy::{Easy2, Handler, WriteError},
    multi::{Easy2Handle, Multi},
};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt as _, Whatever};

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

enum CacheResult<T> {
    CacheHit(T),
    Miss,
}

fn try_get_cache<T>(url: &str) -> CacheResult<T>
where
    T: for<'a> Deserialize<'a>,
{
    let p = format!("cache/{:?}", md5::compute(url.as_bytes()));
    if !std::fs::exists(&p).unwrap() {
        return CacheResult::Miss;
    }
    CacheResult::CacheHit(serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap())
}

fn insert_cache<T>(url: &str, t: &T)
where
    T: Serialize,
{
    let p = format!("cache/{:?}", md5::compute(url.as_bytes()));
    std::fs::write(p, serde_json::to_string_pretty(t).unwrap()).unwrap();
}

/// Download and JSON-parse the results for some URLs.
pub fn download_all<T: for<'a> Deserialize<'a> + Serialize + Clone>(
    urls: &[impl AsRef<str>],
) -> Result<Vec<Option<Result<T, Whatever>>>, Whatever> {
    let cache = std::env::var("AOR_UTILS_CACHE").ok() == Some("1".to_string());
    if cache {
        std::fs::create_dir_all("cache").unwrap();
    }

    let progress_style = ProgressStyle::default_bar()
        .template("{bar} {msg} ({pos}/{len}) {elapsed}")
        .unwrap()
        .progress_chars("#|-");
    let progress = ProgressBar::new(urls.len() as _).with_style(progress_style);
    progress.enable_steady_tick(Duration::from_millis(100));

    let mut responses = Vec::new();
    for _ in 0..urls.len() {
        responses.push(Option::None);
    }

    let mut multi = Multi::new();
    multi.set_max_total_connections(1).unwrap();
    let mut handles = urls
        .iter()
        .map(|url| {
            if cache {
                (url, try_get_cache::<T>(url.as_ref()))
            } else {
                (url, CacheResult::Miss)
            }
        })
        .enumerate()
        .map(|(token, (url, cache_result))| match cache_result {
            CacheResult::CacheHit(x) => {
                progress.inc(1);
                responses[token] = Some(Ok(x));
                Ok(None)
            }
            CacheResult::Miss => Some(download(&mut multi, token, url.as_ref())).transpose(),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut running = true;
    while running {
        if multi.perform().whatever_context("error in multi handle")? == 0 {
            running = false;
        }
        multi.messages(|m| {
            let token = m.token().unwrap();
            let handle = handles.get_mut(token).unwrap().as_mut().unwrap();
            progress.inc(1);
            match m.result_for2(handle).unwrap() {
                Ok(()) => {
                    let _status = handle.response_code().unwrap();
                    let s = String::from_utf8(std::mem::take(&mut handle.get_mut().0)).unwrap();
                    responses[token] = match serde_json::from_str(&s) {
                        Ok(resp) => {
                            if cache {
                                insert_cache(urls[token].as_ref(), &resp);
                            }
                            Some(Ok(resp))
                        }
                        Err(e) => {
                            progress.println(format!("{}: {} ({:?})", urls[token].as_ref(), e, s));
                            None
                        }
                    }
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
