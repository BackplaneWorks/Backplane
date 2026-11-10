use backplane_voice::args::{parse, USAGE};
use backplane_voice::{cmd, live, out, record};

fn main() {
    let argv: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    let a = match parse(&argv) {
        Ok(a) => a,
        Err(why) => {
            if let Some(w) = why {
                out::log(&w);
            }
            eprint!("{USAGE}");
            std::process::exit(2);
        }
    };
    // `record`'s stdout is audio: it never carries an event
    let pcm_out = a.cmd == "record";
    std::panic::set_hook(Box::new(move |info| {
        let m = info.to_string();
        let line = m.lines().next().unwrap_or("panic");
        out::log(line);
        if pcm_out {
            out::exit(1);
        }
        out::fail(line);
    }));
    if a.cmd == "serve" {
        backplane_voice::resident::serve(a);
    }
    if a.cmd.starts_with("local-") {
        cmd::local_cmd(a);
    }
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("a runtime");
    rt.block_on(async move {
        match a.cmd.as_str() {
            "live" => live::live(a).await,
            "file" => cmd::file(a).await,
            "record" => record::record(a).await,
            _ => cmd::devices().await,
        }
    });
}
