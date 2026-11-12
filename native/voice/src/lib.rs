//! backplane-voice: speech to text for Backplane, by OpenAI or on this
//! machine. NDJSON events on stdout, diagnostics on stderr. See ../README.md.

pub mod args;
pub mod audio;
pub mod cmd;
pub mod devices;
pub mod live;
pub mod local;
pub mod multipart;
pub mod net;
pub mod openai;
pub mod out;
pub mod record;
pub mod recorder;
pub mod resident;
pub mod session;
pub mod sherpa;
pub mod stop;
pub mod turn;
pub mod util;
pub mod vad;
