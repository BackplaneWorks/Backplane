# backplane-voice

Speech to text for Backplane: live dictation and whole recordings (Pebble
Index ring notes), by OpenAI or on this machine with sherpa-onnx. One Rust
binary, a child process of the hub; **its stdout is the contract**. No Bun, no
Node.

```sh
scripts/build-voice.sh                              # -> dist/backplane-voice
cd native/voice && ../../scripts/cargo-q test --offline   # against a local mock API; never calls OpenAI
```

## What runs where

- **Linux only: recording.** `live` from a microphone and `record` drive
  `pw-record`, `parecord` or `arecord`, and `devices` asks `pactl`, `pw-dump`
  or `arecord -L`. On macOS and Windows there is no recorder: `live` from one
  fails with `no recorder found (...)`, and `devices` is an empty list and an
  error.
- **Everywhere:** `live --source stdin`, `file`, and the `local-*` commands.
  `file` wants `ffmpeg` on PATH for the local engine; the cloud path falls
  back to REST without it.
- The runtimes for macOS and Windows carry the same C API (checked in the
  tarballs), but only Linux x64 has been run.

## Commands

```
backplane-voice live --engine local --local-dir D [--model parakeet-v2] [--source ...] [--device NAME]
backplane-voice file PATH --engine local --local-dir D [--model ID] [--content-type CT]
backplane-voice local-status --local-dir D
backplane-voice local-install --local-dir D [--model ID]
backplane-voice local-remove --local-dir D [--model ID]
backplane-voice live [--keywords-file F] [--prompt P] [--language xx]... [--delay low]
                     [--source auto|pw-record|parecord|arecord|stdin] [--device NAME] [--key-file F]
backplane-voice file PATH [--content-type CT] [--keywords-file F] [--prompt P]
                     [--language xx]... [--delay low] [--key-file F]
backplane-voice serve --local-dir D [--model ID] --socket SOCK
backplane-voice devices
backplane-voice record [--rate 16000] [--source ...] [--device NAME]
```

No arguments, or bad ones: usage on stderr, exit 2.

- The key: `--key-file F` (read, trimmed), else `OPENAI_API_KEY`. Never on
  argv. `OPENAI_BASE_URL` (default `https://api.openai.com/v1`) moves the
  API, plain `http` included; the socket is the same URL with `http`→`ws`,
  plus `/realtime?intent=transcription`.
- `--keywords-file`: one term per line, trimmed, blank lines skipped, at
  most 100. `--language` repeats. `--delay`: `minimal|low|medium|high|xhigh`,
  default `low`.
- `--rate R` with `--source stdin`: the rate of the PCM on stdin; it is
  converted to the engine's (24 kHz cloud, 16 kHz local).

### devices

The microphones: `pactl -f json list sources`, else `pw-dump`, else
`arecord -L`; monitors of outputs are skipped. The default is `pactl
get-default-source`, or PipeWire's `default.audio.source`, or ALSA's
`default`. Nothing to ask gives an empty list and an error.

### live

Records s16le mono from the first of `pw-record --raw`, `parecord --raw`,
`arecord` found (or the one named; `stdin` reads raw PCM from stdin) from
`--device NAME` (`pw-record --target`, `parecord --device=`, `arecord -D`;
absent is the default source). A recorder alive for 500 ms with no audio yet
counts as started.

Cloud: 24 kHz, streamed to `gpt-live-transcribe` on the Realtime API, 100 ms
per append. That model has no turn detection, so the helper commits: a local
energy VAD holds audio until speech starts (300 ms of pre-roll, silence is
never sent), then commits after 700 ms of quiet, or every 15 s of continuous
speech. Under 200 ms of speech (a click) is cleared, not committed.

Local: 16 kHz. Silero VAD opens a segment (400 ms before it) and closes it
after half a second of quiet (250 ms kept), or at 20 s; speech under 250 ms
is a click. While a segment is open its audio so far is decoded again at most
every 300 ms, each decode a `hyp`; the segment's last decode is its `done`.
The model loads while the microphone already records: audio heard meanwhile is
held, then fed in.

While recording, `level` comes every 250 ms: the loudest 100 ms chunk's RMS,
-60 dBFS and below 0, full scale 100. Hints, each at most once: `no-audio`
when the recorder has given nothing 3 s after `ready`, `silent` when audio
comes but no speech in the first 4 s, then `ok` at the first speech after
either.

Stop: a line `stop` on stdin, stdin's end, SIGTERM or SIGINT (all alike),
or 10 minutes. It prints `stopping`, stops the recorder (giving it a second
to hand over what it has), commits what is buffered, waits up to 8 s for the
last transcripts, prints `end`, exit 0. With `--source stdin`, stdin's end is
the stop.

### file

PATH is an audio file (m4a, wav, mp3, ogg...), or with `--content-type
multipart/form-data; boundary=...` a whole multipart body whose file part
`audio` is the recording (else the error `no audio part`). With `ffmpeg` on
PATH it is decoded to PCM and sent through `gpt-live-transcribe`, one commit
per 15 s or less, the transcripts joined with a space; without ffmpeg, or if
that fails, it goes to REST `POST /audio/transcriptions` with `gpt-transcribe`
(`keywords[]`, `languages[]` fields; a 400 about them retries once without).
With `--engine local`: ffmpeg to 16 kHz PCM, then the VAD's segments one by
one. 90 s limit.

### serve, and --server

`serve` loads the model once (`malloc_trim` after, then a decode of silence so
the first real one is not the slow one), listens on the Unix socket SOCK
(0600) and prints `{"event":"loaded","ms":…,"rss_kb":…,"model":…}`; it ends,
removing the socket, when its stdin ends or a line `stop` comes. `live` and
`file` with `--engine local --server SOCK` decode through it instead of
building a recognizer (their `loaded` says `"resident":true`); when nothing
answers there, or it answers for another model, they build their own, as
without the flag, and a server lost mid-session is replaced the same way. One
server serves many clients, one decode at a time. On the socket: `bpv1
<model>\n`, answered `ok\n` (or `no …\n`); then frames of a u32 LE sample
count and f32 LE samples, each answered by a u32 LE length and sherpa-onnx's
JSON. Unix only.

### The end-of-turn model, and local-turn

`src/turn.rs`: Smart Turn v3.2 (pipecat-ai, BSD-2-Clause, `smart-turn-v3.2-cpu.onnx`, 8 MB, pinned by size and
sha256) hears from the last 8 s of audio whether the speaker has finished. Whisper's log-mel features are made
here (a 400-point mixed-radix FFT, Slaney mel bands; equal to transformers' to 2e-6), and the model runs through
onnxruntime's C API from the runtime already loaded (`OrtGetApiBase`, entries by their OrtApi index). One check
is ~20 ms of features and ~27 ms of model on one thread. In `live` with the local engine, every pause that closes
a segment asks it (the resident server answers when there is one: a frame whose count has bit 31 set) and prints
`turn`. `local-turn --local-dir D` fetches it alone, checked, without touching the speech model's ready mark;
`local-install` fetches it too (a failure there is said, not an error); `local-status` says `"turn"`. Without
it there are no `turn` events. Lab numbers and the choice against LiveKit's turn-detector: docs/voice.md.

### record

Raw s16le mono PCM on stdout at `--rate` (default 16000) until SIGTERM,
SIGINT, a `stop` line on stdin, stdin's end, or the reader closing stdout.
stdout is audio and nothing else; diagnostics go to stderr. Exit 0 when
stopped, 1 when no recorder could start or the recorder died, 2 for usage.
For a desktop client on another machine than its hub, capturing its own
microphone. Recorders are Linux-only; `--source stdin` copies stdin through.

## Events

NDJSON on stdout, one object a line, written at once; diagnostics on stderr.

```
{"event":"devices","default":"<name>","devices":[{"name":"<source>","label":"<description>"}]}
{"event":"ready","recorder":"pw-record","device":"<source>","label":"<description>"}
                                            live: recorder running (the session or the model comes
                                            up alongside; audio waits for it); device is --device
                                            or the default ("" unknown); sent again once if the
                                            name comes after 150 ms
{"event":"ready"}                           file: started
{"event":"loaded","ms":950,"resident":false} live, local: the model is up (resident: the server's, --server)
{"event":"level","v":0..100}                live, every 250 ms while recording
{"event":"hint","hint":"no-audio|silent|ok"}  live
{"event":"delta","item":"<id>","text":"<piece>"}         live, cloud
{"event":"hyp","item":"<id>","words":[["word",start,end,pct]],"end":120}
                                            live, local: the segment so far; start, end and the
                                            audio's end in centiseconds, pct the least sure piece
{"event":"done","item":"<id>","text":"<segment>"}        cloud; file: one, item "file"
{"event":"done","item":"<id>","text":"...","words":[...],"decode_ms":40,"final_ms":55}
                                            live, local: final_ms is from the VAD's end to here,
                                            0 for a segment cut by the stop
{"event":"turn","item":"<id>","p":0.93,"ms":30} live, local: a pause closed segment <id>; the end-of-turn model (turn.rs) says p
                                            that the speaker has finished; comes before that segment's `done`
{"event":"stopping"}                        live: a stop began
{"event":"error","error":"<one line>"}      no key, 401, no recorder, network, server errors
{"event":"local","platform":"linux-x64","supported":true,"runtime":true,"vad":true,"vadBundled":true,
 "models":[{"id":"parakeet-v2","label":"...","size":482440461,"installed":true}],"bytes":0}
                                            local-status, local-remove, local-install's last word
{"event":"progress","what":"<piece>","done":0,"total":0,"phase":"download|unpack"}
{"event":"end"}                             always last; exit 0, or 1 after any error
```

A 401 reads `OpenAI rejected the API key (401)`.

## The local engine

`<dir>` is `<home>/voice/local`:

```
runtime-2.0.1-<plat>/   sherpa-onnx's npm package: libsherpa-onnx-c-api.so and libonnxruntime.so
                        (.dylib on macOS; sherpa-onnx-c-api.dll and onnxruntime.dll on Windows)
silero_vad.onnx
sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8/   the model folder
ready-<id>              written last; what the hub reads
```

The same folders as 0.14, so an install from then keeps working with
nothing fetched again: the npm package always held the C library beside the
Node addon. Each piece streams into `.stage-<pid>`, its size and hash are
checked, it is unpacked in-process and renamed into place.

- The VAD ships in every release archive beside the helper (`vad/`, its
  licence and provenance in `vad/README.md`). An install copies that copy in
  when it matches the pin, and downloads it only when it is missing (dev builds).
- A model's archive is not pinned: upstream repacks it (2026-11-07, the same
  files in new archives). Its four files are, by size and sha256, checked after
  unpacking. When the archive fails or its files do not match, the same files
  come one by one from Hugging Face (`mirror`), each checked against the same pins.
- A 5xx or a dropped connection is tried again: four tries, 2, 6 and 18 s apart
  (`BACKPLANE_VOICE_RETRY_MS` for tests). A 4xx or a checksum is final.

`src/sherpa.rs` loads the C library with `libloading` and holds hand-written
`#[repr(C)]` structs for `sherpa/c-api.h`, the v2.0.1 header kept here. They
go by pointer, so a missing field is a silent misread: one test has the C
compiler print each struct's size and offsets from that header and compares
them, another compares the field names in order, and at run time
`SherpaOnnxGetVersionStr` must equal the pinned version before anything else
in the library is called. **Moving to another sherpa-onnx version means
replacing the header and the pins together.**

## Tests

Stand-ins only: `tests/common` is a Rust mock of the OpenAI API (only the key
`good` is accepted) and a file server for downloads; nothing here calls
OpenAI. The real engine runs only when both are set, so a plain `cargo test`
needs no download:

```sh
BACKPLANE_VOICE_LOCAL_DIR=<a folder local-install filled> \
BACKPLANE_VOICE_WAVS=<the model tarball's test_wavs/> \
  ../../scripts/cargo-q test --offline --test local_e2e -- --nocapture
```

## In Backplane

The hub keeps the key in `<home>/secrets/openai.key` and passes it with
`--key-file`, so it is in no child's argv or environment. The dictionary,
`<home>/voice/dictionary.txt`, is the `--keywords-file`. A live session is
stopped by SIGTERM to its PID.

Pebble Index ring voice notes arrive at a webhook as `multipart/form-data`:
an `audio` part (`audio/mp4`, M4A) and text parts `transcription` (the
ring's own), `recordedAt` and `client`. The hub saves the body and runs
`file BODY --content-type <the request's Content-Type>`.
