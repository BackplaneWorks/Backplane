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

`backplane-voice live --engine local --local-dir <home>/voice/local`.
`local-install` puts three things in `<home>/voice/local`, each checked
against its pin before it is moved into place, then writes `ready-<model>`
(what the hub reads). Every file is fsynced before its rename (a crash on
2026-11-09 kept the renames and lost the data: names in place, 0 bytes), the
unpacked runtime gets a manifest (`.files`: size, sha256, name), and all of it
is checked again where it landed before the mark is written. `local-status` and
`live` check what is there (sizes; the VAD and runtime by hash at install), not
only that names exist: a piece that is not whole is fetched again, and a
`ready-<model>` over broken files is dropped, so the page offers the download.

- the runtime: sherpa-onnx's C library and ONNX Runtime, from npm, sha512
  pinned;
- the model: from sherpa-onnx's releases, each file it holds pinned by size
  and sha256, with Hugging Face as the second source when the archive fails or
  does not match;
- the voice activity model, `silero_vad.onnx` (644 KB): the one piece the
  package carries, beside `backplane-voice` (release archives and dev cuts
  alike), because GitHub, its only source, fails for hours at a time. The copy
  is taken only when it matches its pin; an older package, or a copy that does
  not match, downloads it as before. `local-status` says `vadBundled` when a
  matching copy is there. Its notice is the archive's `licenses/silero-vad`;
  where it comes from is `native/voice/vad/README.md`.

Downloads that fail with a 5xx or a dropped connection are retried; a 4xx or
a checksum mismatch is final, and nothing half-checked is ever moved into
place. The details (pins, sources, retries) are `native/voice/README.md`,
"The local engine". The runtime and the models are not in the package (about
500 MB, and only for those who choose the local engine). Nothing loads in the
hub: the helper is its own process and opens the C library at run time, so a
crash there ends one session. The hub
publishes the download as info (`voice.job`, `voice.local`), so Settings
follows it wherever it is open, and a failure stays on the page until the next
try. The helper loads the model
when a session starts (about 0.9 s, off the event loop; audio heard
meanwhile is kept) and exits with the session. One model does both the live
words and the final ones: while the VAD has a segment open, its audio so far
is decoded again every 300 ms (about 25 ms of CPU per second of speech on 4
threads) and sent as `hyp` with each word's start, end (centiseconds) and
probability (percent); the segment's last decode is its `done`.

## Live words, calmly

The hub settles each hypothesis with `vdict::Settle`: words two hypotheses in
a row agree on, ending 0.4 s or more before the audio so far, are committed;
unsure ones (under 85 %) at the end wait for the next sure one. Committed
words go through the correction pass (`VDict::fix`). The hub keeps a shadow of
the draft (`place::Place`) and sends each step as one edit, `{"ev":"edit",
"del", "ins", "adv", "to"}`, relative to a mark the client keeps: committed
words never move or change; only the tail, at the end, is rewritten. All the
rules are the hub's; a client's whole part is `place::apply`. The cloud engine's deltas settle the same
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

An empty value removes either. The correction pass (`VDict::fix`): drop words, replacements and how terms sound
(exact phrases, longest first), terms written another way (`kicad`,
`stm 32`), then sound-alikes of terms (a rough sound key and edit
distance: "Kai CAD", "SBI" for SPI), applied by themselves only where the
engine was itself unsure (local engine), else offered as the guess. Words
to ask about: the guesses, words the engine heard at under 50 %, then near
misses and technical-looking words (`Voice.unsure`), at most five.
`dictionary.txt` (older hubs) is still read as more terms; a bot's
`voice_dictionary_add` writes a `voice.e.` entry. The hub reads the settings
itself, so a ring's recording needs no client.

## Another machine, a phone

The microphone is the one in front of the person; the engine is the hub's.
A window whose hub is on its own machine (127.0.0.1) lets the hub record its
microphone. A window on another machine records itself (`backplane-voice
record`) and a phone natively, 16 kHz s16le, and starts with `src: "client"`;
the hub names `/voice/in?s=<key>`, where it POSTs ~200 ms pieces in order
(`&end=1` last). The hub copies each body straight into the helper's stdin
and the helper converts it to the engine's rate. The dictionary and learned fixes are the hub's settings, so every
client of that hub shares them.

## Pieces

- `native/voice/` builds `backplane-voice` (Rust): it records the microphone
  (`pw-record`, `parecord` or `arecord`, 24 kHz mono PCM), runs the local engine, streams to
  `wss://…/v1/realtime?intent=transcription` and prints NDJSON events. It also
  transcribes a finished recording (`file`: ffmpeg to PCM, then the same
  socket; `gpt-transcribe` over REST when ffmpeg is missing). Its README has
  the protocol.
- `native/host/src/voice/`: `words` (the plain dictionary, `unsure`, `put`),
  `vdict` (entries, the correction pass, settle), `place` (the draft's shadow),
  `dictation` (a thread's state, chips, the helper's lines), `page` (Settings'
  rows) and `ring`, all pure; `session` is the IO: `bots.voice` (`op`: status,
  key, unkey, add, start, stop, view, act, page, page-act, install, remove, and
  the API keys' api-add, api-del) and `/voice/in`.
- `native/gpui/src/screens/voice.rs`, `voice_mic.rs`: the window. The phones:
  `docs/PHONE-DATA.md`, "Dictation".

## Storage

- The key: `<home>/secrets/openai.key` (0600). The helper reads the file
  (`--key-file`), so it is never in an argv, an environment, the event log
  or a message; clients see it masked (`sk-…WXYZ`).
- The dictionary: `<home>/voice/dictionary.txt`, a term a line; it is the
  helper's `keywords`.
- The microphone: the setting `voice.device` ("" for the default source).
- A live session is the hub's child: a stop closes its stdin.

## Dictating

The mic chip beside the paperclip starts a session in the thread on screen. The
hub sends `{"t":"voice","thread","ev",…}` to that client only: `view` (what to
draw: the strip's one word, the note, the fixes, the word to clarify), `level`
(0..100, four times a second), `edit` and `draft`. Nothing is sent to the agent
until the person sends it. The session ends with its client.

Each segment goes into the draft where the caret was and reads on from what is
there (`words::put`): a space before it unless the draft ends in a space, a new
line or a bracket, one after it when a word follows. Mid-sentence, its first
word loses the capital the transcriber gives every segment, unless it is "I",
not a plain word (USB, KiCad), or opens with a dictionary term; before a
lower-case word its closing period goes. The caret moves past it and ctrl+z
takes it back out.

The helper opens the recorder and the OpenAI session at once: `ready` goes out
as soon as the microphone gives audio, and what is heard before the session
opens is kept and sent the moment it does.

The words to clarify show one at a time above the composer: use the guess,
Fix… (type the right spelling: it replaces the word and joins the dictionary),
add the word as heard to the dictionary, or skip it (not asked again). Sending
the message clears them.

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

Recording on a desktop uses PipeWire, PulseAudio or ALSA tools, so it works on
Linux; the phones record natively. The ring's recordings work wherever the hub
runs.
