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
try.

### Kept loaded

While the audio features run on the local model (voice.engine resolves to
local and the model is downloaded: dictation from every client, the Pebble
ring and webhook recordings), the hub keeps **one** recognizer loaded from its
start: `backplane-voice serve` (`voice/resident.rs`), listening on
`<home>/voice/engine.sock` (0600). Every `live` and `file` helper gets
`--server <sock>` and decodes through it, so a session has its model in
~15 ms instead of ~1 s, and a ring recording costs ~50 MB instead of ~1.2 GB.
Each session is still its own helper process (recorder, VAD, its stdout), so a
crash ends one session. With the cloud engine nothing loads. The hub looks
again every 5 s and at once after a voice setting, an install or a remove; it
lets the recognizer go when no feature uses the local model, or when
MemAvailable drops under 1 GB (never mid-session), and loads it again with
2.5 GB available. A server that dies is started again after 5 s, doubling to
5 min. Settings shows it (`voice.resident` info) on the Local model row. When
it is not up, a helper builds its own recognizer, as before.

Measured on the desktop (32 cores, 2026-11-10, Parakeet v2 int8, 4 threads):

| | before | kept loaded |
|---|---|---|
| spawn to `loaded` (warm / page cache cold) | 1.09–1.15 s / 1.62 s | 13–17 ms |
| spawn to first `hyp` (speech from t=0) | 1.13–1.19 s / 1.66 s | 0.33 s (the VAD's, not the model's) |
| a ring recording (13 s) | 1.34 s, 1.17 GB peak | 0.32 s, 53 MB in the helper |
| the server | | ~700 MB after load, ~1.0 GB after decodes, ~1.2 GB plateau after 20 s segments (ONNX Runtime's arena); 0 CPU idle |

Building the recognizer is the whole wait (0.94 s warm, 1.39 s cold);
loading the runtime is 1–8 ms, the VAD 2–4 ms, a first decode of 1 s ~42 ms,
the microphone's first audio ~140 ms (pw-record). The C API offers no
session options (optimisation level, mmap'd weights), so it is kept loaded
instead; `malloc_trim` after the build gives back ~300 MB of parse buffers.

The helper without a server loads the model
when a session starts (audio heard
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

### Sending while dictating

Send (or Steer) while the microphone is on cuts the transcript there: every
word written so far, the tail included, goes with that message, and the next
draft starts empty with only what is said after them; no word is written
twice, none heard after them is lost. Each edit frame carries `cut` (the
session's sends so far) and `seq` (its number). On Send the client empties its
box, puts its mark at 0, counts the send and tells the hub with
`voice.act`/`act` `voice-clear` plus `cut` and `seq` (the last edit it wrote);
until the hub has it, edits keep coming with the old `cut` and the client
drops them. The hub (`Reader::cut`) counts as sent exactly the words up to
edit `seq` (`Settle::rewind`) and writes everything after into the new draft,
including words of edits the client dropped, and the rest of a segment that
finished in between. The desktop empties its box only when the hub took the
message, so it holds newer edits until then (3 s at most; past that the send
did not go and they are written at the end). Nov 11: a send mid-sentence put
the last word, "stuff.", at the start of the next draft, the uncommitted tail
being committed into the empty box.

## End of turn

`voice/eot.rs`, `voice/jev.rs`, the helper's `turn.rs`. When the person stops after a finished thought the
message sends itself; a thinking pause, "and…" or "um" keeps listening, however long.

1. At each pause that closes a segment (≥ 0.5 s of quiet) the helper's end-of-turn model gives a probability
   from the audio (`turn`), before the segment's last decode.
2. With that segment written, the hub asks **Jev** (TypeSafe's System One model) when Settings, Voice, End of
   turn: Jev is on and its key is in the vault (`typesafe-api-key`): one Noul, "The speaker has finished their
   message and is ready to send it", with the draft so far (only the current draft), the pause, the audio
   model's probability and the thread's last reply. The key is read from the vault when the request goes and
   passed to curl in a 0600 file that is removed after; nothing is cached. Over 400 ms, failing or off: the audio
   model decides alone. No model at all: a full stop and 2 s of silence.
3. At or over the threshold (0.65 by default; Eager 0.50 to Patient 0.85) the client shows "Sending…" with
   Keep talking for 1.5 s (a little longer when patient); new speech or Keep talking cancels it; then the hub
   cuts the transcript and the message goes with the usual 3 s undo window (`command`).
4. Learning, on the hub only: each end-of-turn send counts; an Undo within 15 s of one (`voice-undid`) makes the
   threshold 0.02 more patient (at most +0.15), each kept send 0.002 less (at most −0.05)
   (`voice.eot.learned`, Reset learning in Settings).

Frames: `{"ev":"countdown","ms":1500,"by":"jev|audio|silence"}` (`ms` 0: cancelled), then `command`. Client acts:
`voice-hold` (Keep talking), `voice-undid` (an Undo of a voice send).

Measured 2026-11-11 on this desktop:

| | accuracy | false sends | missed sends |
|---|---|---|---|
| Smart Turn v3.2 (Python reference), 873 English clips of smart-turn-data-v3.1-test (CC-BY-4.0), > 0.5 | 95.6 % | 5.9 % | 2.8 % |
| the same model through the helper (Rust features, the bundled onnxruntime 1.28) | 95.8 % | 5.4 % | 3.0 % |
| Parakeet's transcript alone (punctuation, trailing "and"/"um") | 83.4 % | 26.0 % | 7.0 % |
| Jev on the words and the pause, 200 clips, ≥ 0.5 | 83 % | 18 % | 16 % |
| Jev given the audio model's probability too, same 200 | 97.5 % | 3 % | 2 % |

Jev's round trip: 137 ms median, 169–174 ms at the 90th percentile (the hub's own smoke test: 120–149 ms). Smart Turn: ~20 ms of features and
~27 ms of model on one thread. LiveKit's turn-detector was not used: its licence allows it only inside LiveKit
Agents. On the person's own narrations (23 min, 250 pauses, the audio model alone at 0.65, kept on this machine)
3 of 58 mid-sentence pauses (5.2 %) would start a countdown, and 72 of 192 pauses after a full sentence (37.5 %),
where in a narration the next sentence usually starts and cancels it. The clips in native/voice/tests/turn
(Kokoro-82M, Apache-2.0) are the recorded-sample tests: a finished request 0.99, a thinking pause 0.03, "So I'll
send it to Kati, and then" 0.02.

## Hold to talk

The mic chip held (0.4 s or more) is hold-to-talk on the desktop, iOS and Android: dictation starts on the
press (on the phones when the hold is recognised), and letting go stops it; once its last words are in, the
draft goes with the same undo window as any voice send. A tap still starts or stops dictation.

## Voice commands

`voice/cmds.rs`. What is said at the very end of an utterance (the segment the
VAD closes after its pause) can be a command instead of words; mid-sentence it
never is ("send it to Kati" is words). Matching ignores case and punctuation and
takes the engine's endings of long words ("message completed"); short words
must be exact ("I sent it." is words).

- **Send** (on by default): "message complete", "send it", "send message", or
  the person's own phrases (`voice.cmd.send`, comma separated; `-` turns
  sending by voice off). The phrase is not written (or is taken back if it was
  already committed, an edit with `back`), the hub cuts the transcript as for a
  Send, and sends `{"ev":"command","cmd":"send","cut":n}`. The client takes
  the whole draft out of the box at once (dictation goes on into the empty
  box), flashes Send, shows "Sending by voice…" with Undo, and after 3 s sends
  it as the button does (turn.start, mode steer: it steers a running turn).
  Undo puts the text back in front of what was dictated since.
- Off until turned on: **scratch that** (`voice.cmd.scratch`: the last sentence
  goes), **new line** (`voice.cmd.newline`), **cancel message**
  (`voice.cmd.cancel`: the draft goes, with the same Undo), and **send after
  silence** (`voice.cmd.auto`: 3, 5 or 10 s with no words, no segment open and
  words written since the last send).
- A Pebble ring note loses a send phrase said at its end.

An edit's `back` (characters before the mark replaced too) exists only for
commands; every client applies it (`test/fixtures/voice-edits.json`).

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
