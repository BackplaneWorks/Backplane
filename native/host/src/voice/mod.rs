//! Dictation (docs/voice.md). `words`, `vdict`, `place`, `dictation`, `page`, `ring` and `ringbox`'s rules are pure; `session` is the IO: `bots.voice`, `/voice/in` and the helper.
pub mod dictation;
pub mod page;
pub mod place;
pub mod ring;
pub mod ringbox;
pub mod session;
pub mod vdict;
pub mod words;
