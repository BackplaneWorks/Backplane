# Clone from git

Add project has two ways in: **Open folder** (a folder on the hub's machine) and **Clone from git**. `/project <address> [first message]` in any
composer opens Clone from git on that address. The clone always runs in the hub (`native/host/src/commands/clone/`), on the hub's machine,
whichever client asked: a phone or the web asks; the hub clones.

## What the person does

1. **Repository.** An https or ssh URL (`https://github.com/o/r`, `git@github.com:o/r.git`, `ssh://...`), `host.tld/o/r`, or GitHub's
   `owner/repo`. Their GitHub repositories are suggested as they type: every account `gh` is signed in to, its own repositories and its
   organizations' (`gh repo list`), newest push first.
2. **Where.** The folder browser of Open folder picks the parent; the last part is the new folder's name (the repository's by default). The
   default parent is the one used last, else `~/Documents`, else the home folder; a default name taken by something else gets `-2`, `-3`...
   A folder that is there and holds anything is refused ("… already exists and isn't empty. Pick another folder or name."), except a clone of
   the same repository, which is added as it is. An empty folder is cloned into.
3. **Clone.** Progress (git's phase, percent, size and rate), Cancel (the whole process group ends; the folder it made goes). Failures are
   said in words; a refusal for access (`auth`) on GitHub offers **Sign in to GitHub**.
4. **After.** The folder is added as a project, the client goes to it, and its first thread is made unless the person turned that off. A
   KiCad or Altium project inside is found and said; when there are several, `.backplane.json` names the one Backplane opens (one named like
   the repository, or the one discovery takes). A `.backplane.json` the repository has is kept.

## Credentials

Nothing is ever asked for in a chat or passed through a client. git runs with `GIT_TERMINAL_PROMPT=0` (it never waits for a password), the
person's own credential helpers, and, for GitHub https, gh's (`credential.https://github.com.helper=!gh auth git-credential`, after theirs).
ssh runs with `BatchMode=yes` and `StrictHostKeyChecking=accept-new`, through the person's SSH agent (a hub started without
`SSH_AUTH_SOCK` finds the desktop's usual agent socket). `owner/repo` and `host/o/r` try https first, then ssh when https was refused for
access. A repository another signed-in gh account sees is cloned as `https://<account>@github.com/...`, which gh answers with that account.

**Sign in to GitHub** runs `gh auth login --web` on the hub's machine with no browser opened there; its one-time code and address
(`https://github.com/login/device`) are shown in the client, so the person signs in from whatever device they are on. When it is done the
repository list is read again.

## Large repositories and LFS

The clone is `git clone --progress --recurse-submodules` with `GIT_LFS_SKIP_SMUDGE=1`. A repository that uses LFS (`filter=lfs` in its
`.gitattributes`, or a `.lfsconfig`) then gets `git lfs pull`, with its own progress, when git-lfs is installed; when it is not, the project
is added and a note says the large files are placeholders until git-lfs is installed.

## Protocol

Desktop: the Add project picture (`shown.folders`, `present/picker.rs`, `commands/clone/view.rs`) carries `mode`, `tabs`, `icon`, `repo`
(the destination step's header), `job` (the clone, with `actions` and, when it is done with nothing to read, `open: {thread, project}`) and
`signin` (`{state, code, url, error}`). Actions: `proj-mode`, `proj-clone`, `proj-clone-go`, `proj-clone-cancel`, `proj-clone-retry`,
`proj-clone-back`, `proj-clone-thread`, `proj-gh-signin`, `proj-clone-open` (the client goes to the thread, else the project).

Every client (wire and data/1; feature `clone` in data/1's hello):

| Method | Params | Result |
|---|---|---|
| `clone.repos` | `q`, `limit`, `refresh` | `{repos: [{name, value, description, private, account}], more, state: ok\|nogh\|signedout\|error, loading, hint}`. `value` is what `clone.start` takes. |
| `clone.dest` | `repo`, and `dest` or `parent` + `name` (none: the default) | `{ok, error, same, repo, origin, name, dest, destShown, parent, folder, defaultParent, defaultParentShown}` |
| `clone.start` | `repo`, `dest` or `parent` + `name`, `thread` (default true), `text` (the first thread's first message), `id` (optional) | the job |
| `clone.status` | `id` (none: every job) | the job (`{jobs}`) |
| `clone.cancel` / `clone.retry` | `id` | the job (cancel: `cancelAccepted` false once it is being added; retry: a new job) |
| `clone.signin` | `restart`, `cancel` | `{state: starting\|waiting\|done\|failed\|cancelled, code, url, error}` |

A job: `{id, repo, origin, dest, destShown, name, state: running|done|failed|cancelled, phase, pct, detail, error, auth, signin, notes,
project, thread, started, ended}`. data/1 pushes `{"t":"d","k":"clone","clone":<job>}` on every change (progress at most five times a
second), `{"t":"d","k":"clone.repos"}` when the repository list was read again, and `{"t":"d","k":"clone.signin","signin":{...}}`.
`composer.run` answers `/project <address> [message]` with `{"kind":"clone","repo","text"}`: the app opens its Clone screen on it.
