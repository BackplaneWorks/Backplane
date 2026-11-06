//! The command line.

use crate::openai::DELAYS;

pub const USAGE: &str = "usage:
  backplane-voice live --engine local --local-dir D [--model parakeet-v2] [--source ...] [--device NAME]
  backplane-voice file PATH --engine local --local-dir D [--model ID] [--content-type CT]
  backplane-voice local-status --local-dir D
  backplane-voice local-install --local-dir D [--model ID]
  backplane-voice local-remove --local-dir D [--model ID]
  backplane-voice live [--keywords-file F] [--prompt P] [--language xx]... [--delay low]
                       [--source auto|pw-record|parecord|arecord|stdin] [--device NAME] [--key-file F]
  backplane-voice file PATH [--content-type CT] [--keywords-file F] [--prompt P]
                       [--language xx]... [--delay low] [--key-file F]
  backplane-voice devices
  backplane-voice record [--rate 16000] [--source ...] [--device NAME]
The key comes from --key-file, else OPENAI_API_KEY. OPENAI_BASE_URL overrides the API base.
";

const COMMANDS: [&str; 7] = ["live", "file", "devices", "local-status", "local-install", "local-remove", "record"];
const SOURCES: [&str; 5] = ["auto", "pw-record", "parecord", "arecord", "stdin"];

#[derive(Clone, Debug, Default)]
pub struct Args {
    pub cmd: String,
    pub pos: Vec<String>,
    pub keywords_file: Option<String>,
    pub key_file: Option<String>,
    pub prompt: String,
    pub languages: Vec<String>,
    pub delay: Option<String>,
    pub source: String,
    pub device: String,
    pub content_type: String,
    pub engine: String,
    pub rate: f64,
    pub local_dir: String,
    pub model: String,
}

/// Err is the reason to print before the usage, if there is one.
pub fn parse(argv: &[String]) -> Result<Args, Option<String>> {
    let mut a = Args {
        cmd: argv.first().cloned().unwrap_or_default(),
        source: "auto".into(),
        engine: "openai".into(),
        model: "parakeet-v2".into(),
        ..Default::default()
    };
    if !COMMANDS.contains(&a.cmd.as_str()) {
        return Err(if a.cmd.is_empty() { None } else { Some(format!("unknown command: {}", a.cmd)) });
    }
    let mut rate_given = false;
    let mut i = 1;
    while i < argv.len() {
        let f = &argv[i];
        i += 1;
        if !f.starts_with("--") {
            a.pos.push(f.clone());
            continue;
        }
        let Some(v) = argv.get(i).cloned() else { return Err(Some(format!("{f} needs a value"))) };
        i += 1;
        match f.as_str() {
            "--keywords-file" => a.keywords_file = Some(v),
            "--key-file" => a.key_file = Some(v),
            "--prompt" => a.prompt = v,
            "--language" => a.languages.push(v),
            "--delay" => a.delay = Some(v),
            "--source" => a.source = v,
            "--device" => a.device = v,
            "--content-type" => a.content_type = v,
            "--engine" => a.engine = v,
            "--rate" => {
                rate_given = true;
                a.rate = v.trim().parse::<f64>().ok().filter(|r| r.is_finite()).unwrap_or(0.0);
            }
            "--local-dir" => a.local_dir = v,
            "--model" => a.model = v,
            _ => return Err(Some(format!("unknown flag: {f}"))),
        }
    }
    if let Some(d) = &a.delay {
        if !DELAYS.contains(&d.as_str()) {
            return Err(Some(format!("--delay is one of {}", DELAYS.join("|"))));
        }
    }
    if !SOURCES.contains(&a.source.as_str()) {
        return Err(Some(format!("bad --source: {}", a.source)));
    }
    if !["openai", "local"].contains(&a.engine.as_str()) {
        return Err(Some(format!("bad --engine: {}", a.engine)));
    }
    if (a.engine == "local" || a.cmd.starts_with("local-")) && a.local_dir.is_empty() {
        return Err(Some("--local-dir is needed".into()));
    }
    if a.cmd == "file" && a.pos.len() != 1 {
        return Err(Some("file needs one PATH".into()));
    }
    if a.cmd != "file" {
        if let Some(p) = a.pos.first() {
            return Err(Some(format!("unexpected argument: {p}")));
        }
    }
    if a.cmd == "record" && rate_given && (a.rate < 1.0 || a.rate.fract() != 0.0) {
        return Err(Some("bad --rate".into()));
    }
    Ok(a)
}

pub fn keywords_of(text: &str) -> Vec<String> {
    text.split('\n').map(|l| l.trim()).filter(|l| !l.is_empty()).take(100).map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Args, Option<String>> {
        parse(&s.split_whitespace().map(String::from).collect::<Vec<_>>())
    }
    fn why(s: &str) -> String {
        p(s).err().flatten().unwrap_or_default()
    }

    #[test]
    fn keywords_are_trimmed_blank_lines_skipped_at_most_100() {
        assert_eq!(keywords_of("  AC-42 \r\n\npremium plan\n"), vec!["AC-42", "premium plan"]);
        let many: String = (0..150).map(|i| format!("w{i}\n")).collect();
        let k = keywords_of(&many);
        assert_eq!(k.len(), 100);
        assert_eq!(k[99], "w99");
    }

    #[test]
    fn flags_and_defaults() {
        let a = p("live --source stdin --language en --language de --delay medium --rate 16000").unwrap();
        assert_eq!((a.source.as_str(), a.engine.as_str(), a.model.as_str()), ("stdin", "openai", "parakeet-v2"));
        assert_eq!(a.languages, vec!["en", "de"]);
        assert_eq!(a.rate, 16000.0);
        assert_eq!(p("live --rate nonsense").unwrap().rate, 0.0); // ignored, as Number() gives NaN
    }

    #[test]
    fn bad_arguments_say_why() {
        assert_eq!(p("").err(), Some(None));
        assert_eq!(why("nope"), "unknown command: nope");
        assert_eq!(why("live --device"), "--device needs a value");
        assert_eq!(why("live --what x"), "unknown flag: --what");
        assert_eq!(why("live --delay soon"), "--delay is one of minimal|low|medium|high|xhigh");
        assert_eq!(why("live --source mic"), "bad --source: mic");
        assert_eq!(why("live --engine x"), "bad --engine: x");
        assert_eq!(why("live --engine local"), "--local-dir is needed");
        assert_eq!(why("local-status"), "--local-dir is needed");
        assert_eq!(why("file"), "file needs one PATH");
        assert_eq!(why("file a b"), "file needs one PATH");
        assert_eq!(why("live a"), "unexpected argument: a");
        assert_eq!(why("record --rate 0"), "bad --rate");
        assert_eq!(why("record --rate 1.5"), "bad --rate");
        assert!(p("record --rate 8000 --source arecord --device hw:1").is_ok());
    }
}
