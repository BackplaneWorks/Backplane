# silero_vad.onnx

The voice activity model the local engine uses (src/sherpa.rs), shipped in every release archive beside `backplane-voice`
(release.yml), so a new install needs the network only for the speech model it downloads. The helper takes this copy when it
matches the pin in src/local.rs (`Pins::pinned().vad`), and downloads only when it is missing or does not match (a dev build).

- What: Silero VAD v4.0 (https://github.com/snakers4/silero-vad/tree/v4.0), MIT, Copyright (c) 2020-present Silero Team:
  `silero_vad.LICENSE` is its licence, from that tag, and goes into the archives with it.
- Exported to ONNX by sherpa-onnx (https://github.com/k2-fsa/sherpa-onnx, Apache-2.0), `scripts/silero_vad/v4/export-onnx.py`,
  and published as `silero_vad.onnx` in its `asr-models` release; this is that file.
- 643854 bytes, sha256 9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6 (the same pin as 0.14's).
