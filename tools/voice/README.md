# backplane-voice

Speech to text with OpenAI for Backplane: live dictation from the desktop
microphone, and whole recordings (Pebble Index ring notes). A thin Bun host
adapter with no dependencies; what to do with the text stays in .

```sh
scripts/build-voice.sh                     # -> dist/backplane-voice (bun --compile)
cd tools/voice && bun test                 # against a local mock API; never calls OpenAI
BACKPLANE_VOICE=$PWD/../../dist/backplane-voice bun test   # the same, on the build
```

## Commands

```
backplane-voice live [--keywords-file F] [--prompt P] [--language xx]... [--delay low]
                     [--source auto|pw-record|parecord|arecord|stdin] [--device NAME] [--key-file F]
backplane-voice file PATH [--content-type CT] [--keywords-file F] [--prompt P]
                     [--language xx]... [--delay low] [--key-file F]
backplane-voice devices
```

No arguments, or bad ones: usage on stderr, exit 2.

- The key: `--key-file F` (read, trimmed), else `OPENAI_API_KEY`. Never on
  argv. `OPENAI_BASE_URL` (default `https://api.openai.com/v1`) moves the
  API; the socket is the same URL with `http`→`ws`, plus
  `/realtime?intent=transcription`.
- `--keywords-file`: one term per line, trimmed, blank lines skipped, at
  most 100. `--language` repeats. `--delay`: `minimal|low|medium|high|xhigh`,
  default `low`.

### devices

The microphones: `pactl -f json list sources`, else `pw-dump`, else
`arecord -L`; monitors of outputs are skipped. The default is `pactl
get-default-source`, or PipeWire's `default.audio.source`, or ALSA's
`default`. Nothing to ask gives an empty list and an error.

### live

Records s16le mono 24 kHz from the first of `pw-record --raw`, `parecord
--raw`, `arecord` found (or the one named; `stdin` reads raw PCM from stdin)
from `--device NAME` (`pw-record --target`, `parecord --device=`,
`arecord -D`; absent is the default source)
and streams it to `gpt-live-transcribe` on the Realtime API, 100 ms per
append. That model has no turn detection, so the helper commits: a local
energy VAD holds audio until speech starts (300 ms of pre-roll, silence is
never sent), then commits after 700 ms of quiet, or every 15 s of
continuous speech. Under 200 ms of speech (a click) is cleared, not
committed.

While recording, `level` comes every 250 ms: the loudest 100 ms chunk's RMS,
-60 dBFS and below 0, full scale 100. Hints, each at most once: `no-audio`
when the recorder has given nothing 3 s after `ready`, `silent` when audio
comes but no speech in the first 4 s, then `ok` at the first speech after
either.

Stop: a line `stop` on stdin, stdin's end, SIGTERM or SIGINT (all alike),
or 10 minutes. It prints `stopping`, stops the recorder, commits what is
buffered, waits up to 8 s for the last transcripts, prints `end`, exit 0.
With `--source stdin`, stdin's end is the stop.

### file

PATH is an audio file (m4a, wav, mp3, ogg...), or with `--content-type
multipart/form-data; boundary=...` a whole multipart body whose part
`audio` is the recording (else the error `no audio part`). With `ffmpeg` on
PATH it is decoded to PCM and sent through `gpt-live-transcribe`, one
commit per 15 s or less, the transcripts joined with a space; without
ffmpeg, or if that fails, it goes to REST `POST /audio/transcriptions` with
`gpt-transcribe` (`keywords[]`, `languages[]` fields; a 400 about them
retries once without). 90 s limit.

## Events

NDJSON on stdout, one object a line, written at once; diagnostics on stderr.

```
{"event":"devices","default":"<name>","devices":[{"name":"<source>","label":"<description>"}]}
{"event":"ready","recorder":"pw-record","device":"<source>","label":"<description>"}
                                            live: recorder running (the OpenAI session opens
                                            alongside; audio waits for it); device is --device
                                            or the default ("" unknown); sent again once if the
                                            name comes after 150 ms
{"event":"ready"}                           file: started
{"event":"level","v":0..100}                live, every 250 ms while recording
{"event":"hint","hint":"no-audio|silent|ok"}  live
{"event":"delta","item":"<id>","text":"<piece>"}         live only
{"event":"done","item":"<id>","text":"<segment>"}        file: one, item "file"
{"event":"stopping"}                        live: a stop began
{"event":"error","error":"<one line>"}      no key, 401, no recorder, network, server errors
{"event":"end"}                             always last; exit 0, or 1 after any error
```

A 401 reads `OpenAI rejected the API key (401)`.

## In Backplane

The hub keeps the key in `<home>/secrets/openai.key` and passes it with
`--key-file`, so it is in no child's argv or environment. The dictionary,
`<home>/voice/dictionary.txt`, is the `--keywords-file`. A live session is
stopped by SIGTERM to its PID.

Pebble Index ring voice notes arrive at a webhook as `multipart/form-data`:
an `audio` part (`audio/mp4`, M4A) and text parts `transcription` (the
ring's own), `recordedAt` and `client`. The hub saves the body and runs
`file BODY --content-type <the request's Content-Type>`.
