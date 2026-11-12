//! Dictation (docs/voice.md). `words`, `vdict`, `place`, `dictation`, `page`, `ring` and `ringbox`'s rules are pure; `session` is the IO: `bots.voice`, `/voice/in` and the helper; `resident` keeps the one recognizer loaded.
pub mod always;
pub mod cmds;
pub mod dictation;
pub mod eot;
pub mod jev;
pub mod page;
pub mod place;
pub mod resident;
pub mod ring;
pub mod ringbox;
pub mod ringpage;
pub mod session;
pub mod speaker;
pub mod vdict;
pub mod words;
