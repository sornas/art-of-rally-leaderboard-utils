use std::time::Duration;

use curl::{
    easy::{Easy2, Handler, WriteError},
    multi::{Easy2Handle, Multi},
};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
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
    multi.set_max_total_connections(1).unwrap();
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
                    let s = String::from_utf8(std::mem::take(&mut handle.get_mut().0)).unwrap();
                    responses[token] = match serde_json::from_str(&s) {
                        Ok(resp) => Some(Ok(resp)),
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
