use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools as _;
use serde::{Deserialize, Serialize};

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
) -> Vec<Option<T>> {
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

    let agent = ureq::agent();
    urls.iter()
        .map(|url| {
            let (url, cache_hit) = if cache {
                (url, try_get_cache::<T>(url.as_ref()))
            } else {
                (url, CacheResult::Miss)
            };
            match cache_hit {
                CacheResult::CacheHit(x) => {
                    progress.inc(1);
                    Some(x)
                }
                CacheResult::Miss => {
                    let resp = agent
                        .get(url.as_ref())
                        .call()
                        .ok()?
                        .body_mut()
                        .read_json()
                        .ok()?;
                    if cache {
                        insert_cache(url.as_ref(), &resp);
                    }
                    progress.inc(1);
                    Some(resp)
                }
            }
        })
        .collect_vec()
}
