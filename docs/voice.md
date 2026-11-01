# Voice

Dictation into the composer, and a Pebble Index ring's voice notes, with a
personal dictionary, by one of two engines (Settings, Voice, Speech to
text: Auto, On this machine, OpenAI; setting `voice.engine`):

- **local**: NVIDIA Parakeet TDT 0.6B (int8, sherpa-onnx, Silero VAD) on the
  hub's machine. No key, no network, private, offline. Downloaded once
  from Settings (about 480 MB; `voice.model` parakeet-v2 English, or
  parakeet-v3, 25 languages). Auto uses it once it is downloaded.
- **cloud**: OpenAI's GPT-Live-Transcribe (`gpt-live-transcribe`), with an
  `sk-` key.

The design and why: `docs/voice-dictation-design.md`.

## The local engine

`backplane-voice live --engine local --local-dir <home>/voice/local`. The
runtime (sherpa-onnx's Node addon and ONNX Runtime, from npm, sha512
pinned) and the models (from sherpa-onnx's releases, sha256 pinned) are
fetched by `local-install` into `<home>/voice/local`, each checked and moved
into place, then `ready-<model>` is written (what the hub reads). Nothing is
bundled in the package; nothing loads in the hub. The helper loads the model
when a session starts (about 0.9 s, off the event loop; audio heard
meanwhile is kept) and exits with the session. One model does both the live
words and the final ones: while the VAD has a segment open, its audio so far
is decoded again every 300 ms (about 25 ms of CPU per second of speech on 4
threads) and sent as `hyp` with each word's start, end (centiseconds) and
probability (percent); the segment's last decode is its `done`.

## Live words, calmly

The hub (server/voice.bend's `Vo.step`) settles each hypothesis with
core/vdict.bend's `Vs.*`: words two hypotheses in a row agree on, ending
0.4 s or more before the audio so far, are committed; unsure ones (under
85 %) at the end wait for the next sure one. Committed words go through the
correction pass (`Vd.fix`) and out as `{"ev":"words", "first", "commit",
"tail", "firm", "fixes", "final", "unsure"}`. The client (client.bend's
`Voice.words`) takes the old tail out of the draft, puts the committed words
where the committed ones end (`@voice.pos`), and the tail after them
(`@voice.tl` its length): committed words never move or change; only the
tail, at the end, is rewritten. The cloud engine's deltas settle the same
way (untimed: never the last two words). The strip shows one word for the
state (Starting, Loading the model, Listening, Writing, Finishing) and the
level, never the words; after a segment its fixes are chips that undo.

## The dictionary

Settings (all clients see them; they live in the hub's log):

- `voice.e.<key>`: an entry as typed: `KiCad` (a term), `KiCad = key cad,
  kai cad` (a term and how it sounds), `jason -> JSON` (a replacement),
  `!um` (a word to drop). Settings, Voice, Dictionary takes many at once,
  one a line.
- `voice.l.<heard>`: a learned fix, `<written>\t<confirmed>\t<undone>`.
  Taking a guess or typing the right word confirms one; undoing a fix's
  chip undoes one. It applies by itself once confirmed twice and never
  undone; until then it is offered as a guess.

An empty value removes either. The correction pass (core/vdict.bend's
`Vd.fix`, laws `vd_*`): drop words, replacements and how terms sound
(exact phrases, longest first), terms written another way (`kicad`,
`stm 32`), then sound-alikes of terms (a rough sound key and edit
distance: "Kai CAD", "SBI" for SPI), applied by themselves only where the
engine was itself unsure (local engine), else offered as the guess. Words
to ask about: the guesses, words the engine heard at under 50 %, then near
misses and technical-looking words (`Voice.unsure`), at most five.
`dictionary.txt` (older hubs, bots' `voice_dictionary_add`) still adds
terms. Every `bots.voice` request carries the entries and settings
(bothub.bend's `Vreq.with`); the hub keeps the last as
`<home>/voice/prefs.json` for a ring's recordings.

## Another machine, a phone

The microphone is the one in front of the person; the engine is the hub's.
A page whose hub is on its own machine (127.0.0.1) lets the hub record its
microphone, as before. A page on another machine (the tailnet, a phone's
browser) records itself (`src/web/voice-mic.js`: getUserMedia, an
AudioWorklet, 16 kHz s16le) and starts with `src: "client"`; the reply names
`/voice/in?s=<key>`, where it POSTs ~200 ms pieces in order (`&end=1` last).
The hub copies each body straight into the helper's stdin (host.js's
`Sock.feed`, never a Bend list) and the helper converts it to the engine's
rate. The dictionary and learned fixes are the hub's settings, so every
client of that hub shares them.

## Pieces

- `tools/voice/` builds `backplane-voice` (Bun), the one part that is not
  Bend: it records the microphone (`pw-record`, `parecord` or `arecord`,
  24 kHz mono PCM), streams it to `wss://…/v1/realtime?intent=transcription`
  and prints NDJSON events. It also transcribes a finished recording
  (`file`: ffmpeg to PCM, then the same socket; `gpt-transcribe` over REST
  when ffmpeg is missing). Its README has the protocol.
- `src/server/voice.bend`: the key, the dictionary, live sessions, a ring's
  recording. `server.bend`'s `VoiceJob` answers `bots.voice` requests
  (`op`: status, key, unkey, dict, add, start, stop, devices).
- `src/core/voice.bend` (pure, laws `voice_*`): the dictionary, and which
  words of a transcript to ask about (`Voice.unsure`): a near miss of a
  dictionary term (one word, or two heard for one term: "key cad" for
  KiCad), or a technical-looking word nobody taught it (letters and digits
  mixed, all caps, inner capitals). At most five.
- `src/core/client.bend`'s `Voice`: the session as the client shows it,
  and the clarify actions. The window draws it in `layout.bend` (`Vx.*`,
  `Set.voice`), the web in `view.bend` (`View.voice`, `View.voice.set`).

## Storage

- The key: `<home>/secrets/openai.key` (0600). The helper reads the file
  (`--key-file`), so it is never in an argv, an environment, the event log
  or a message; clients see it masked (`sk-…WXYZ`).
- The dictionary: `<home>/voice/dictionary.txt`, a term a line; it is the
  helper's `keywords`.
- The microphone: the setting `voice.device` ("" for the default source).
- A live session's pid: `<home>/voice/live.pid`. A stop signals it only
  while `ps` says it is still `backplane-voice`.

## Dictating

The mic button beside the paperclip asks the hub to record its own
machine's microphone into the selected thread. The hub sends
`{"t":"voice","thread","ev",…}` to that client only: `live` (recorder,
device, label), `part` (words so far), `text` (a finished segment and its
`unsure` words), `level` (0..100, four times a second), `hint`
(`no-audio` 3 s without bytes, `silent` 4 s without speech, `ok` once
speech comes), `stopping`, `error`, and `idle` last. Nothing is sent until
the person sends it.

Each segment goes into the thread's draft where the caret is (the end, in
the web client or for a thread not on screen) and reads on from what is
there (`Voice.put`): a space before it unless the draft ends in a space, a
new line or a bracket, one after it when a word follows. Mid-sentence, its
first word loses the capital the transcriber gives every segment, unless it
is "I", not a plain word (USB, KiCad), or opens with a dictionary term (the
hub marks those `firm`); before a lower-case word its closing period goes.
The caret moves past it, ctrl+z takes it back out, and the window writes
the draft down as typing does.

The helper opens the recorder and the OpenAI session at once: `live` goes
out as soon as the microphone gives audio (waiting at most 150 ms more for
its name, and sent again with the name if that comes later), and what is
heard before the session opens is kept and sent the moment it does.

The words to clarify show one at a time above the composer: use the guess,
Fix… (type the right spelling: it replaces the word and joins the
dictionary), add the word as heard to the dictionary, or skip it (not asked
again this session).

## A Pebble ring

Set the ring's webhook (docs/bots.md) to send the recording (or both).
Forms may be 2 MB. A form with an `audio` part (M4A) that is let in (its
bearer token, or its Index signature, checked over the body's bytes) is
answered 202 at once; the recording is transcribed with the dictionary and
deleted, and the bot gets Backplane's words, with the words it may have
misheard, instead of the ring's own transcription. With no key, or no
words, the ring's transcription goes as before. A bot can add a term the
person confirms with `voice_dictionary_add`.

## Platforms

Recording uses PipeWire, PulseAudio or ALSA tools, so dictation works on
Linux. The ring's recordings work wherever the hub runs.

## Qualification (2026-11-01)

On six synthetic 16 kHz clips on Linux x64, with other builds running:
model load 1958 ms; first hypothesis median 573 ms (including the cold
first clip); final after the clip ends median 589 ms, p95 738 ms, including
VAD silence; word commit lag median 664 ms, p95 1833 ms. This small fixture
set is a latency sample, not an accuracy benchmark. It does not meet the
proposal's 250 ms first-word / 600 ms p95 final targets under this load.

Real browser demos using the hub capture path and Chromium's fake
microphone upload path retained the whole dictated draft through redraws.
They produced 49 and 37 text changes respectively, with zero replacements
starting more than three words before the old end. The upload demo used a
non-loopback hub address on this same machine; it is not physical
second-device acceptance. `test/tools/voice_browser_e2e.ts` makes this
fixture reproducible, including an undo/persistence assertion; its added
assertions still need a run on the final combined build.

Cloud live dictation and the ring webhook pass against a mock API; this
does not establish current live-service compatibility. iOS and Android
recorders compiled in their earlier qualification, but physical microphone
and separate-hub testing remains outstanding. The dictionary is shared by
clients of the same hub. Cross-hub dictionary replication, project-scoped
entries, project vocabulary extraction, edit-diff teaching, an optional
global hotkey and on-device phone inference remain design proposals.

The final full proof pass must be rerun on the combined integration source.
The controlled-heap attempt completed in 299 seconds with an incomplete
new `vd_learn_trusted` proof; the corrected five voice laws subsequently
passed in isolation. No existing law was weakened or removed.
