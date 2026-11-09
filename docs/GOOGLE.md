# Google: Gmail and Calendar for bots

The hub signs in to Google accounts and gives bots Gmail and Calendar tools (`native/host/src/google/`), as 0.14 did (`src/server/google.bend`,
`src/core/google.bend`). Several accounts can be signed in at once; each bot is granted the accounts it may use. Nothing else (an ordinary thread, an agent's
shell, `secret_run`) can reach them.

## Sign-in

OAuth 2.0 for installed apps: PKCE (S256) and a loopback redirect `http://127.0.0.1:<port>/oauth/google` (RFC 8252). Google allows only the loopback redirect
for a Desktop-app client with Gmail scopes (its device flow takes neither Gmail nor Calendar). Scopes: `openid email gmail.modify calendar.events`, with
`access_type=offline` and `prompt=select_account consent`, so a refresh token comes back.

Where the browser comes back:
- **A client that listens itself** (the phones, the desktop client): it opens a listener on `127.0.0.1:0`, starts the sign-in with
  `redirect: "http://127.0.0.1:<its port>/oauth/google"`, opens the URL, catches the one `GET /oauth/google?...`, answers it a short page, and sends the hub the
  whole address it caught as the `code`. The hub takes a client's redirect only when it is exactly that shape (`oauth::loopback`).
- **The hub's own route**: without a client redirect the sign-in comes back to the hub's port on the machine the browser runs on. `GET /oauth/google` answers only
  a top-level navigation with no `Origin`, to `127.0.0.1`/`localhost`, from a loopback peer; the state and PKCE are checked there. A state that names another hub
  (`<nonce>.<base64url of its tailnet URL>`) is sent on to that hub once (`hop=1`), and only to a `https?://<name>.ts.net[:port]` address.
- **Paste**: the address the browser landed on (or the bare code) can always be pasted back (`code`).

A sign-in waits 15 minutes. Tokens: the refresh token goes in the vault as `hub.google.<id>` (`id` = the first 16 hex of the SHA-256 of the address in lower case),
with `{email, refresh, scopes, client, at, state, why}`; access tokens stay in memory and are refreshed a minute before they expire. A refresh that Google refuses
(`invalid_grant`: revoked, expired, a password change, or an app in testing past seven days) marks the account **sign-in expired**; its tools then say so and the
person signs in again. Remove revokes the refresh token at Google (best effort) and forgets it. Vault names starting `hub.` are the hub's own: `secret_run`,
`secret_request`, `secret_list` and the secret box never see them.

## The OAuth client

The client is, in order: `BACKPLANE_GOOGLE_CLIENT_ID` and `BACKPLANE_GOOGLE_CLIENT_SECRET` in the hub's environment; the pair a build was made with
(the same names at compile time, `option_env!`); the person's vault secrets `google-client-id` and `google-client-secret` (an agent asks for them with
`secret_request`). It is a Google Cloud OAuth client of type **Desktop app**. Until Google verifies the app, the consent page warns that it is unverified and at
most 100 test users can sign in; a project in **Testing** gives refresh tokens that end after 7 days (the account then reads sign-in expired).

## The `google` request (desktop; granted like `accounts`)

`google {op, ...}` → `{result: View}`; ops:
- `view`
- `signin {redirect?, account?}`: starts a sign-in (a new account, or `account` again); `redirect` a client's own loopback
- `code {attempt, code}`: the address the browser came back to, or the bare code
- `cancel {attempt}`
- `remove {account}`
- `grant {bot, account, on}`: may this bot use this account

`View` = `{client: {ready, line}, add, accounts: [Account], bots: [{id, name, accounts: [{account, email, on}]}], signin: Signin|null, note}`
- `Account` = `{id, email, scopes, state, line, tone, bots}`: `state` connected | expired, `line` its words ("connected", "sign-in expired"), `tone` ok | bad,
  `scopes` "Gmail, Calendar", `bots` the names of the bots allowed it ("" for none).
- `Signin` = `{attempt, url, redirect, state, text, error, paste, busy, done, email}`: `state` waiting | done | failed; `busy` while waiting; `url` opens in the browser;
  `paste` asks for the address it landed on.

## data/1 (phones; `accounts.page`, `account.view`, `account.act`)

Google is a third provider of `accounts.page`: `{provider: "google", title: "Google", add: "Add Google account", accounts: [...]}`, each account
`{id: "google:<id>", provider: "google", label: <email>, email, state, line, tone, limits: <scopes>, hold: ""}`. A sign-in not yet done is `google:new`.
`account.view {profile: "google:<id>"|"google:new"}` → `{account}` as for Claude and Codex, plus `loopback: true` (the app may catch the redirect itself):
`message` names the bots allowed it, `actions` are `signin` ("Sign in again"), one `bot:<bot id>` per bot ("Calendar: allowed" / "Calendar: not allowed", which
toggles it), and `remove`; `signin` is the attempt (`paste: true`).
`account.act {profile?, provider: "google", action, redirect?, code?}`: `add` and `signin` start a sign-in (`redirect` the app's own loopback); `code` sends the
caught address or a pasted one; `cancel`; `remove`; `bot:<id>` toggles a bot.

## Tools (a bot's threads, and runs working for a bot, granted at least one account)

`google_accounts`, `gmail_search {query, max}`, `gmail_read {id}`, `gmail_draft {to, subject, body, html, cc, bcc, reply_to, thread, attachments, draft}`,
`gmail_send` (the same, or `{draft}` alone), `calendar_events {from, to, max, query}`, `calendar_create`, `calendar_update`, `calendar_delete`; each takes
`account` (an address the bot is granted; default the first). `gmail_draft` saves at once and sends nothing. `gmail_send`, `calendar_create`,
`calendar_update` and `calendar_delete` open an approval with the whole email or the change in words, and run exactly what was shown once the person allows it
(Allow for this session skips the next ones in that thread). In a mailroom run the charter decides as for other tools (0.14's `Perm.run.google`): a risky tool
(`gmail_send`, `calendar_delete`) needs a `may: tool:<name>` line, the others run.

## Making a Google OAuth client (only when there is none to reuse)

0.14's builds baked one in from `backplane-bend/.env` (`BACKPLANE_GOOGLE_CLIENT_ID`, `_SECRET`); its accounts' refresh tokens work only with that same
client, so reuse it when it is a Desktop app client. Otherwise, in a browser signed in to the Google account that will own it:
1. Open https://console.cloud.google.com/projectcreate, name it "Backplane", Create, and select it in the top bar.
2. APIs & Services → Library: search "Gmail API", open it, Enable. Back, search "Google Calendar API", Enable.
3. Google Auth Platform (APIs & Services → OAuth consent screen) → Get started: App name "Backplane", your email as support email, Next; Audience
   **External**, Next; your email as contact, Next; agree, Create.
4. Audience → Test users → Add users: every Google account you will sign in with (up to 100), Save.
5. Data Access → Add or remove scopes: tick `.../auth/gmail.modify`, `.../auth/calendar.events`, `openid` and `.../auth/userinfo.email`
   (or paste them under "Manually add scopes"), Update, Save.
6. Clients → Create client: Application type **Desktop app**, name "Backplane hub", Create. Copy the Client ID and the Client secret from the dialog.
7. Give them to the hub only through the vault: ask an agent for `secret_request` `google-client-id` and `google-client-secret`, and paste each in its
   masked box. Never in a chat message, a file or a command line.
While the app is in Testing, Google shows "Google hasn't verified this app" (Continue) and a refresh token lasts 7 days, after which the account reads sign-in
expired. Publishing the app (Audience → Publish app) removes the 7-day limit; Gmail's restricted scope then needs Google's verification, which a personal
use under 100 users can leave unverified (the warning stays).
