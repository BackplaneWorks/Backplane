# Dictation and the dictionary: design

Status: accepted 2026-11-01 (all five proposals), with one constraint added:
dictation must read as one calm stream (section 1b). What was built is in
`docs/voice.md`.

## What great dictation is

1. **You speak, the right text appears where you are, quickly.** Words show
   while you talk; the final words land within a few hundred ms of your last
   one. No key needed, nothing sent anywhere you did not choose.
2. **It already knows your words.** "KiCad", "STM32G431", "nRF54L15",
   "Backplane", your nets and part numbers come out spelled and cased right
   the first time, because the dictionary steers the model and a correction
   pass cleans up what it still gets wrong.
3. **A mistake is corrected once.** Fix "key cad" to "KiCad" once and it is
   remembered; the second time it is fixed for you (and you can undo it in
   one tap, which teaches it too).
4. **It asks only when it is unsure, without interrupting.** Doubtful words
   wait as one-tap chips; typing, sending or ignoring them never blocks.
5. **It is the same everywhere.** The window, the web page on a laptop and
   both phones dictate, with the same dictionary and the same corrections,
   whichever machine runs the hub.

## What Backplane has today

- One engine: OpenAI `gpt-live-transcribe` through `backplane-voice` (Bun),
  with an energy VAD; needs an `sk-` key.
- The microphone is always **the hub's own machine's**. Pressing the mic on a
  phone, or on a laptop whose page talks to a hub elsewhere, records the
  hub machine's microphone. The phones have no dictation at all.
- The dictionary is a list of terms in `<home>/voice/dictionary.txt`, sent as
  keywords; it lives on one hub and is not shared.
- `Voice.unsure` asks about near misses and technical-looking words. "Fix…"
  adds the right spelling to the dictionary but forgets *what was heard*, so
  the same mistake comes back.

## What Hex does (anomalyco/hex, read at 7e147c9)

Hex is now a Rust app (macOS, Windows, Linux; Swift only on iOS) on
transcribe.cpp, a ggml engine running GGUF models (Metal, Vulkan).

- Models: Parakeet "Unified English" 0.6B Q8 (~731 MB, the default, can
  stream on a GPU), Parakeet TDT v2/v3, Whisper large-v3-turbo, Qwen3-ASR and
  others; Silero VAD. Downloaded from Hugging Face at a pinned revision with
  `curl --continue-at`, size and SHA-256 checked, then moved into place.
  iOS uses Parakeet v2 through FluidAudio, or Apple Speech.
- Capture: 16 kHz mono; "keep ready" leaves the mic open and adds 450 ms of
  pre-roll, plus a 200 ms tail after release. A clip whose loudest 30 ms is
  under -50 dBFS, or where Silero finds no speech, never reaches the model.
- Hotkey: hold (default; under 300 ms is a tap and dropped), double-tap to
  lock, or toggle; modifier-only keys such as Option.
- Mostly batch: the whole clip is transcribed on release; streaming only on
  a GPU with a model that supports it, falling back to batch.
- Vocabulary `{term, sounds_like}`, up to 500: Whisper/Qwen get it as a
  prompt; Parakeet gets decoder biasing on 64 terms, behind a Labs switch
  that is off by default. Replacements are literal phrases, any case,
  word-bounded, longest first; an empty output removes the phrase. Filler
  words are a fixed list. Optional LLM rewrite last.
- Latency: the model stays loaded and is prewarmed with a silent run;
  unloading when idle is optional.
- Nothing is learned: every replacement is typed by hand.

**Taken from Hex**: Parakeet as the default, Silero gating before any
inference, pre-roll and tail, the tap threshold and double-tap lock for push
to talk, `{term, sounds_like}` entries, literal word-bounded replacements
where an empty output removes, pinned and hashed downloads, prewarming.
**Different here**: corrections are learned from the person's fixes and
undos, not only typed; the dictionary is shared by every device and engine;
the microphone may be on another machine than the engine; live words come
from a small streaming model on the CPU instead of needing a GPU; biasing is
on by default; text goes into Backplane's composer, not pasted into other
apps. We use sherpa-onnx rather than transcribe.cpp: prebuilt libraries for
every platform including Android and iOS, a C API that `bun:ffi` can load,
transducer hotwords, Silero and streaming Zipformer models in one library.

## The design

### 1. Engines

One session protocol (the helper's NDJSON events, unchanged), several
engines behind it. Setting `voice.engine`: `auto` (default), `local`,
`cloud`.

| engine  | where it runs                 | needs                     | biasing                       |
|---------|-------------------------------|---------------------------|-------------------------------|
| `local` | the hub machine, in the helper| the model downloaded once | hotwords + correction pass    |
| `cloud` | OpenAI                        | an API key, network       | keywords + prompt + correction|
| `device`| the phone itself (phase 2)    | iOS 26 / Android 13+      | contextual strings + correction|

`auto` picks local when its model is installed, else cloud when keyed, else
says what to set up.

**Local engine: sherpa-onnx** (as built: one model, see 1b; the two-pass
plan below was replaced). The helper `backplane-voice`
gains `--engine local` and loads sherpa-onnx's C library with `bun:ffi`
(the same way `tools/kipc` loads nng): no framework, no Python, one shared
library plus ONNX Runtime, prebuilt for linux-x64/arm64, macOS and Windows.

- *Live words*: a small streaming Zipformer transducer (about 70 MB int8)
  decodes every 100-200 ms, so `delta` events keep flowing as you speak.
- *Final words*: when the VAD ends a segment, NVIDIA Parakeet TDT 0.6B
  (int8, about 650 MB; v2 English, v3 25 European languages) decodes the
  whole segment and its text replaces the live words. Parakeet is more
  accurate than Whisper large-v3-turbo on English, much faster on a CPU, and
  does not hallucinate on silence the way Whisper does. Hex uses Parakeet
  too, through CoreML on macOS.
- *VAD*: Silero VAD (2 MB) replaces the energy VAD for local: 300 ms
  pre-roll, the segment ends after 600 ms of quiet (400 ms once the live
  words end in sentence punctuation), and is cut at 20 s.
- *Tiers* (Settings → Voice → Model): **Fast** (Zipformer only, ~70 MB, no
  second pass) and **Accurate** (both, ~720 MB, the default where the machine
  has 8 GB of memory or more).

Whisper (whisper.cpp) stays a later option for languages Parakeet lacks;
its only biasing is a 224-token `initial_prompt`, so it is not the default.

**Install and memory.** Neither model is in the package. Settings → Voice
→ "Download local model" fetches a pinned manifest (URL, size, SHA-256 per
file and platform, like Antigravity's `Agy.rels`) into
`<home>/voice/models/<id>/` and `<home>/voice/runtime/<version>/`, checks
the hashes and swaps it in by rename. Nothing loads into the hub: the helper
loads the model when a session starts, stays warm for ten minutes after the
last one (`voice.warm`), then exits and frees it. Pressing the mic (or the
hotkey going down) starts the load at once; audio recorded during the load
is buffered, so no words are lost.

### 1b. One calm stream (the person's constraint)

"Fine with two models, but it must be obvious what is happening and not a
lot of text flashing around." Measured on this desktop, Parakeet decodes a
second of speech in about 25 ms on 4 CPU threads, so the live words do not
need a second, weaker model at all: while a segment is open the helper
decodes its audio so far again every 300 ms with Parakeet itself, and the
segment's last decode is the final. One model, so the live words and the
final ones agree; there is no "draft model" to be overruled.

What the person sees:

- **Words appear once, in place, in the draft.** Each hypothesis carries
  every word's timing and confidence. Bend (`Vs.*`, Settle) commits a word
  once two hypotheses in a row agree on it and it ends at least 0.4 s
  before the audio so far; committed words are written into the draft and
  never move or change again. Unsure words (under 85 %) at the end wait for
  the next sure one, so "Kai CAD" is corrected to "KiCad" before it is ever
  shown.
- **Only the tail is ever rewritten**: the last one to three words, after
  the committed ones, at the end of what was dictated. No block is ever
  replaced, nothing above the caret reflows.
- **The final is not a swap.** When the segment ends, whatever was still
  tail is committed (corrected); the words already shown stay as they are.
- **One quiet word says where it is**: Starting, Loading the model (only
  the first press after the helper starts cold), Listening, Writing (words
  are settling), Finishing; with the level meter. The words themselves are
  never repeated in the strip.
- After a segment, the fixes it made sit as small chips under the strip
  ("KiCad ← Kai CAD ↶"); a click undoes one. Unsure words wait as before.

The hotwords spike: sherpa-onnx's hotword boosting had no effect on
Parakeet TDT (same audio, same output, boost 3 and 8), which matches Hex,
whose Parakeet biasing needs its own decoder fork and is off by default.
So the dictionary steers the local engine through the correction pass and
the engine's own word confidences, not decoder biasing.

### 2. Where the audio comes from, and where it is heard

The rule: **the microphone is the one in front of the person; inference
runs where the engine is.**

- *The window and the desktop app on the hub's machine*: the helper records
  the microphone itself (today's path).
- *A web page, or the desktop app, talking to a hub on another machine*:
  the page records (`getUserMedia` + an AudioWorklet, 16 kHz mono s16) and
  sends 100 ms frames to its hub over the WebSocket it already has
  (`voice.audio`, binary). The hub runs the helper with `--source stdin` and
  feeds it. The transcript comes back as the `voice` events of today.
- *A phone (phase 1)*: the same, recording natively (AVAudioEngine,
  AudioRecord) and streaming to the phone's focused hub.
- *A phone (phase 2, `device`)*: Apple's on-device SpeechAnalyzer or
  Android's on-device recognizer hears it on the phone, biased with the top
  dictionary terms as contextual strings; nothing leaves the phone. The
  text then goes through the same correction pass (Bend, in the phone's
  bridge), so results match the other clients.
- *The hub has no engine*: a paired machine of the owner's that has the
  local model can take the session over `/far/rpc` (later); otherwise cloud
  if a key is set; otherwise the mic says "Set up dictation" and opens
  Settings → Voice.

Audio is never stored; the hub drops each frame once the helper has it.
Frames from a client are accepted only on its own authenticated socket and
only for a session it started.

### 3. The dictionary

One store of **entries**, event-sourced in the hub's log, shared with the
owner's other machines.

| kind      | example                                     | effect                                              |
|-----------|---------------------------------------------|-----------------------------------------------------|
| `term`    | `KiCad` (sounds like: "key cad", "kai cad") | biases the model; written in this exact form        |
| `replace` | "jason" → "JSON", heard 3×, undone 0×       | a learned substitution, applied once trusted        |
| `drop`    | "um", "uh"                                  | removed from the output (Hex's word removal)        |

Each entry has an id, its text, optional spoken forms, a scope (everywhere,
or one project), counts (used, confirmed, undone) and when it was last used.

**Storage and sync.** Changes are ops (`add`, `edit`, `remove`, `count`)
logged as a new event `VoiceOp`, stamped with the hybrid stamps of
`ubop.bend` and folded into state: last writer wins per entry and field,
and counts are per-machine counters merged by maximum, so two machines that
both learn the same fix add up. Paired machines exchange ops the way the
uberbot's ops travel (`/far/rpc`, pulled from the last stamp seen). Phones
and pages read the dictionary from their hub; they never hold their own.
`dictionary.txt` stays as an import on first start and as an export the
helper reads.

### 4. Biasing the engine with the dictionary

Every engine gets a budget (cloud keywords 100, local hotwords ~1000,
Whisper 224 tokens, phone contextual strings 100), so terms are ranked:
the current project's scope first, then how often and how lately each was
used, then spoken forms. The hub writes the ranked list for each session.

- *Local*: sherpa-onnx hotwords (an Aho-Corasick boost on the transducer's
  beam search), per stream, each term with a boost score; a term used often
  gets a larger boost. Spoken forms are not hotwords (they would bias toward
  the wrong spelling); they serve the correction pass.
- *Cloud*: keywords as today, plus the prompt: the project's name and the
  last ~200 characters of the draft, so the model continues the sentence it
  is in.

### 5. The correction pass (Bend, every engine)

Pure, in `src/core/voice.bend`, with laws. Each final segment goes through
it in order, and each change keeps its span and rule so it can be shown and
undone:

1. **Drop words** (`drop` entries), word-bounded, any case.
2. **Learned substitutions** (`replace` entries that are trusted: confirmed
   at least twice and never undone, or marked "always"): longest phrase
   first, word-bounded, any case, keeping the case of the sentence start.
3. **Term matching**: windows of 1-3 heard words against each term and its
   spoken forms, scored by edit distance on a phonetic key (a Metaphone-like
   code written in Bend: "key cad" and "kicad" share `KKT`) and on letters.
   A high score with a joined form ("ki cad" → KiCad) is a rewrite; a
   middling one becomes an unsure word with that guess.
4. **Canonical form**: any match of a term ignoring case and spaces is
   written as the term ("kicad", "Ki Cad" → "KiCad"; "stm 32" → "STM32").
5. **Unsure words**: today's rules, plus the engine's own confidence where it
   gives one (sherpa's token log-probabilities), and the words step 3 left in
   the middle.

The segment then goes into the draft (`Voice.put`, unchanged).

### 6. Teaching it

**From a transcript.** The last dictated segments show in a strip above the
composer (as the unsure words do now), words that were changed underlined.
A tap on any word gives: the guesses, "Type the right word…", "Add to
dictionary", and, for a changed word, "Undo (was 'key cad')". Typing the
right word does three things: replaces it in the draft, adds the right
spelling as a `term` if new, and records `replace heard → right` (confirmed
once). Undo records the opposite (undone once), so a bad rule dies.

**From an edit.** If the person edits a dictated span by hand within a
minute (the draft is diffed against what was inserted), a word-for-word
substitution there becomes a suggestion: "Remember 'jason' → 'JSON'?" with
one tap. Nothing is learned without that tap.

**Unsure words.** Chips, one tap each: the guess, or keep it as heard (which
adds it as a term if it looks technical). "All fine" clears them. Skipping
is weak evidence against that guess; sending the draft clears them. They
never take the keyboard.

**Bulk.** Settings → Voice → Dictionary: a list with search, each entry's
kind, spoken forms, scope and counts, editable in place. Paste many lines at
once:

```
KiCad = key cad, kai cad
STM32G431
jason -> JSON
!um
```

(`=` spoken forms, `->` a substitution, `!` a drop word.) **From the
project**: "Suggest words from this project" reads the KiCad files'
symbol values, part numbers (MPN), net and sheet names and the intent file,
and offers the ones not already known, ticked by default. A hardware
project's vocabulary is mostly there.

**Agents.** `voice_dictionary_add` takes a kind and spoken forms; a bot can
offer "add these to your dictionary?" after a ring note.

### 7. Push to talk (optional, off by default)

Settings → Voice → Hotkey: off, or a key. In the desktop app it is a global
shortcut (Electron's `globalShortcut`, or uiohook for hold): hold to talk,
or double-tap to lock it on and press again to stop, dictating into the
thread on screen and bringing it forward. On the web it works only while
the page has the focus. A hold under 300 ms is a tap and records nothing.
While the hotkey is set the helper may keep the microphone open (a setting,
off by default) for 450 ms of pre-roll. Phones: none.

### 8. Speed

Targets on this desktop (CPU only): live words within 250 ms of being
spoken; the final words in the draft within 350 ms of the end of speech for
a 5 s utterance (p50), 600 ms (p95); a warm start within 150 ms of the
press; a cold start (model load) under 2 s with no words lost. The helper
stamps speech end and decode time on each `done`; the hub keeps perf
counters (`voice.final_ms`, `voice.decode_ms`, `voice.start_ms`); a bench
plays recorded WAVs through `--source stdin` in real time and prints the
percentiles, for the local and the cloud engine.

### 9. What gets built, in order

1. The dictionary store (`VoiceOp`, fold, sync), the correction pass and its
   laws, the strip and the teach actions on all four clients. Works with the
   cloud engine at once.
2. The local engine in the helper (sherpa-onnx over `bun:ffi`, Silero VAD,
   two passes, hotwords), the model manifest and installer, Settings.
3. Audio from the client: the web page and the desktop app over the socket,
   then both phones.
4. Latency bench and counters; the optional hotkey.
5. Phase 2: on-device phone engines.

## Questions for you

1. **Default model**: Parakeet v2 (English only, best accuracy) or v3
   (25 languages, slightly worse English)? I propose v2, with v3 offered.
2. **Phones, phase 1**: stream audio to the hub (works today on every hub
   with an engine), with on-device as phase 2. OK?
3. **Sync scope**: the dictionary follows the owner to every paired
   machine. Should project-scoped entries follow too (yes, proposed)?
4. **Learning threshold**: a substitution applies by itself after two
   confirmations and no undo. Stricter or looser?
5. **GPU**: sherpa-onnx can use CUDA here; CPU is fast enough for Parakeet,
   so CPU only to start (smaller download)?
