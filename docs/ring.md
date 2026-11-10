# The Pebble Index ring

Recordings from a Pebble Index 01 ring reach Backplane through the Pebble app's
webhook. The hub keeps each one with its audio and transcribes it with the
person's voice engine and dictionary. It then gives the note to the **Pebble** bot,
which decides what the note is and acts on it. 0.14 had the same flow (a
Pebble sub-bot, the #ring-requests room). The Rust hub rebuilt it on Nov 10, 2026.

## What the app sends

The source is Core Devices' `INDEX_WEBHOOK_API.md` (mobileapp repo,
`experimental/src/commonMain/kotlin/coredevices/ring/external/indexwebhook/`),
protocol version 1.

- `POST <URL>`, `multipart/form-data`, with every header the person configured
  (we use `Authorization: Bearer <token>`).
- Fields:
  - `audio`: `audio/mp4`, `<recordingId>.m4a`, AAC-LC, mono, 16 kHz. Sent when
    the app is set to send Recording or Both.
  - `transcription`: the app's own words. Sent when it is set to send
    Transcription or Both.
  - `recordedAt` (ms) and `client=ring`: always sent.
  - `test=true`: only on Send test event.
- Headers:
  - `X-Index-Webhook-Version: 1`.
  - `X-Index-Trigger`: `single-click-hold`, `double-click-hold` or `test-event`.
  - `X-Index-Delivery`: stable per recording.
  - `X-Audio-Size`.
  - With signing on: `X-Index-Timestamp` and `X-Index-Signature`, a hex
    HMAC-SHA256 of `v1\n<ts>\n<delivery>\n<trigger>\n<0|1>\n` followed by the raw body.
- Retries: the app retries network errors, 408, 429 and 5xx every minute, up
  to 10 times, also after it restarts. After that, Retry in the gesture's Recent
  runs sends it again. A recording made while the phone was offline arrives
  later, with its original `recordedAt`.
- Each gesture (Hold & talk, Double click & hold) has its own URL, headers and
  payload mode.

## The flow

1. **In** (`bots/hooks.rs`): `POST /hook/<id>`. The request must prove itself
   by the hook's bearer token or by the Index signature (the hook's secret,
   checked over the raw bytes, fresh within 300 s). Anything else gets a 401.
   A form with `client=ring` or the Index headers is a ring note.
   - Everything in it is untrusted. The parts are parsed as bytes
     (`voice/ringbox.rs`: at most 16 parts, a 2 MB body).
   - The delivery id is cut down to `[A-Za-z0-9.:_-]{,96}`. The note id is
     `n` plus 16 hex digits of sha256(hook, delivery), so nothing from the form
     ever names a path.
2. **Kept, de-duplicated**: `<home>/ring/<id>/audio.m4a` (as it came) and
   `note.json`, both 0600, before the answer.
   - A delivery seen before gets `200 already received`, and nothing is heard or
     sent again (the app's retries and Retry button).
   - A note that could not be written gets a 503, so the app tries again.
   - The test event gets a 200 and is shown on the Ring page. It is not kept.
3. **Heard** (`voice/session.rs` `hear_file`), one note at a time:
   - The engine is the one Settings, Voice chooses: the local Parakeet model
     when it is downloaded (Auto or On this machine), else GPT-Live-Transcribe
     with the OpenAI key. The dictionary corrects names and part numbers, and
     the words to ask about come along.
   - The local engine gives segments: start and end (centiseconds), and the
     percent of the least sure word. A note's confidence is its least sure
     segment.
   - With no engine, the app's own `transcription` is used (model `ring`).
4. **Given to Pebble**: a message in Pebble's thread at hop 0, as the person's
   own words:
   - `[voice note from your Pebble ring · ring note <id> · said <their local time> · Hold & talk · heard by parakeet-v2, 91% sure · came 3 h after it was said (the phone was away)]`
   - then the words, the words to ask about, and the app's own words when they differ.
   - The note records the thread and message (the Ring page's "What happened").
5. **Routed** by Pebble's persona (`bots/ring.rs` `PERSONA`):
   - work requests, errands and anything that contacts people go to the
     uberbot (`bot_send`, topic `ring-requests`, so they land in #ring-requests);
   - reminders: `remind_person`;
   - calendar asks: the Calendar bot if `bot_list` has one, else the uberbot;
   - todos: the task board;
   - notes: `memory_save`, tagged `ring`.

   Pebble posts one line in #ring-requests per note. It never sends email
   itself; an email is a draft. It sends `notify_person` only when something
   asked for is done.
6. **Backlog**: a minute after the hub starts, it takes up again any note it
   stopped on before the note reached Pebble: one said in the last 3 days,
   tried fewer than 5 times.

## The Ring page (Settings → Ring)

- **Setup**:
  - Set up (`ring.setup`) makes Pebble as a sub-bot of the uberbot, its
    webhook "Pebble Index" with a bearer token, and the #ring-requests room of
    the two. Whatever already exists is kept.
  - The row shows the URL and Copy URL / Copy header for the Pebble app.
  - The URL is `BACKPLANE_HOOK_URL` if set, else the HTTPS address `tailscale serve`
    gives the hub's port, else its listen address (the page warns that it is
    plain HTTP).
- **Search**: `ring.search {q}`. Every word must be in the note's hearings or
  the ring's words. The list every client shows follows the last search.
- **Each note**:
  - when it was said and the gesture; the words, and who heard them and how sure.
  - **Play**: `/ring/audio/<id>?k=<cap>`. The link's key is an HMAC with
    `<home>/secrets/ring.key`, and it plays that one recording only.
  - **Hear again (local / OpenAI)**: `ring.hear {note, engine}`. The engine is
    "", `local`, `local:<model>` or `openai`. The new hearing is kept next to the
    earlier ones, and the newest one with words is the one used.
  - **Send again**: `ring.send`.
  - **What happened**: opens Pebble's thread.
- **RPCs**: `ring.page`, `ring.page.act`, `ring.list {q}`, `ring.get {note}` (the whole note.json with its
  segments), `ring.search`, `ring.hear`, `ring.send`, `ring.setup`. Info for
  clients: `ring.notes`, `ring.setup`, `ring.busy`, `ring.test`.

The phones have the same page (Settings → Ring, feature `ring`). `ring.page`
gives rows, and `ring.page.act {action, value, fields}` gives the rows again
after the press. The rows come from one pure function, `voice/ringpage.rs`.
On the phones:

- Copy URL, Copy header, Copy both and **Share** (the share sheet) never
  leave the phone, so the person can paste into the Pebble app on the same phone.
- Play opens the recording's link.
- What happened opens Pebble's thread.

The **Test event** row says when the Pebble app's Send test event last
arrived. Check again reads the page again.

## When another hub owns the ring

Only one hub owns the ring: the Pebble bot, its webhook and token, and
`home/ring/`. After a move (the always-on hub, docs/BOAT-HUB.md), the old hub
gets the setting `ring.home` = `{"url": "<the owner's hook URL>", "machine": "<its name>"}`.
Its Ring page then:

- says "Ring is on <machine>", and Copy URL / Copy both give that URL;
- offers no Set up (`ring.setup` refuses) and shows no list;
- does not take up notes again after a restart.

The owning hub derives its own URL as above. The Authorization header does not
change, so in the Pebble app only the URL changes, followed by Send test event.

## Setting up the phone

1. Settings → Ring → Set up.
2. Pebble app → Index 01 Settings → Webhook → Hold & talk:
   - **URL**: Copy URL.
   - **Header**: name `Authorization`, value from Copy header (`Bearer …`).
   - **What to send**: Both.
   - **Save**, then **Send test event**. The Ring page shows when the test came.
3. Optionally do the same for Double click & hold.
4. The phone must be on the tailnet. Leave Sign requests off: the bearer token
   does the same job.
