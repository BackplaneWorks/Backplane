# End-of-turn test clips

Spoken by Kokoro-82M (hexgrad, Apache-2.0; kokoro-onnx v1.0 model files), voice `af_heart`, resampled to 16 kHz mono PCM16, each followed by 1 s
of silence. Generated on 2026-11-11 for `tests/local_e2e.rs`; no person's voice.

- `finished.wav`: "Please add a reset button to the board and route it to pin P A zero." A finished request: should send.
- `midthought.wav`: "Please add a reset button to the board and" A thinking pause: must not send.
- `kati.wav`: "So I'll send it to Kati, and then" The words "send it" inside a sentence that goes on: must not send.

Smart Turn v3.2 through the helper (turn.rs) on this desktop: 0.99, 0.01 and 0.01.

Speaker check (`the_speaker_model_tells_the_enrolled_voice_from_others`), the same Kokoro-82M, no trailing silence:

- `speaker-a1..a3.wav` (voice `af_heart`): the enrolment; `speaker-a4.wav`: the same voice, another sentence: must match.
- `speaker-tv-m.wav` (`am_adam`, "The weather tonight will be cloudy with a chance of rain.") and `speaker-tv-f.wav` (`bf_emma`, "And now, back to
  the studio for the evening news."): other voices, must not.

TitaNet small through the helper on this desktop: 0.73 for a4, 0.24 and 0.43 for the others.
