//! `record`: raw s16le mono PCM to stdout until stopped, for a client whose
//! hub is on another machine. stdout carries nothing but audio; what there is
//! to say goes to stderr.

use crate::args::Args;
use crate::out::{exit, log};
use crate::recorder::{missing, open_source};
use crate::stop::{self, Stop};
use std::io::Write;
use std::time::Duration;

const LATE: Duration = Duration::from_millis(1000);
const DEFAULT_RATE: u32 = 16000;

pub async fn record(a: Args) -> ! {
    let rate = if a.rate > 0.0 { a.rate as u32 } else { DEFAULT_RATE };
    if let Some(why) = missing(&a.source) {
        log(&why);
        exit(1);
    }
    let stop = Stop::new(false);
    stop::on_signals(stop.clone());
    if a.source != "stdin" {
        stop::watch_stdin(stop.clone());
    }
    let mut src = match open_source(&a.source, &a.device, rate).await {
        Ok(s) => s,
        Err(m) => {
            log(&m);
            exit(1);
        }
    };
    log(&format!("recording from {} at {rate} Hz", src.name));
    // a reader that has stopped reading must not hold a stop up
    let mut last_call = stop.subscribe();
    tokio::spawn(async move {
        stop::stopped(&mut last_call).await;
        tokio::time::sleep(LATE + LATE / 2).await;
        exit(0);
    });
    let mut sr = stop.subscribe();
    let mut killed = false;
    let mut late = Box::pin(tokio::time::sleep(Duration::from_secs(86400)));
    let mut out = std::io::stdout();
    loop {
        tokio::select! {
            biased;
            r = src.next() => match r {
                Some(b) => {
                    if out.write_all(&b).and_then(|_| out.flush()).is_err() {
                        // the reader went away: nothing left to record for
                        src.kill();
                        exit(0);
                    }
                }
                None => {
                    if !stop.stopped() && !src.is_stdin() {
                        log(&format!("the recorder stopped ({})", src.why().await));
                        exit(1);
                    }
                    exit(0);
                }
            },
            _ = stop::stopped(&mut sr), if !killed => {
                killed = true;
                late.as_mut().reset(tokio::time::Instant::now() + LATE);
                src.kill();
                if src.is_stdin() {
                    exit(0);
                }
            }
            _ = &mut late, if killed => exit(0),
        }
    }
}
