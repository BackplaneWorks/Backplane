#!/bin/bash
# Agent CLIs a job installed on a Boat pool box go (Nov 21: a real `claude` in ~/.nvm on bp-build-1 made a hub test find it, and every landing
# failed until #1532). Run on the box at the start of every job (scripts/lib-boat.sh bo_run) and by `scripts/boat-pool.sh clean-agents`.
# Searched: the job's PATH and the login shell's (nvm and the like add theirs there). Only a command whose folder really is in the box user's
# home (its folder resolved, symlinks and .. followed) is removed, and with it its npm package when that is a known agent package whose folder
# is in the home too; each said on stderr. Everything else is left alone and named in one warning line: Boat installs its own agent server's
# CLIs on every box (/opt/ascii-agent; /usr/local/bin/{claude,codex} into /usr/local/lib/node_modules, and the opencode, pi and cursor-agent
# shims marked "ascii-harness-shim"), and they are not ours to touch. Exit 1 when a home one could not be removed: the job then does not run.
names="claude codex opencode gemini grok pi omp agy antigravity muse cursor-agent amp aider"
pkgs="@anthropic-ai/claude-code @openai/codex opencode-ai @google/gemini-cli @earendil-works/pi-coding-agent @sourcegraph/amp @vibe-kit/grok-cli"
home=$(cd -P "$HOME" 2>/dev/null && pwd) || { echo "boat: no home folder" >&2; exit 1; }
# the login PATH after a marker (a profile's banner does not mix in); the job wrapper's own folders too, for boat-pool.sh's manual run
PATH="$PATH:$HOME/.cargo/bin:/usr/local/cargo/bin:$HOME/.bun/bin:$(bash -lc 'printf "\n@@bp-path@@%s\n" "$PATH"' < /dev/null 2>/dev/null | sed -n 's/^@@bp-path@@//p' | tail -1)"
inhome() { case "$1/" in "$home"/*) return 0;; esac; return 1; }
rc=0; left=; seen=
while IFS= read -r dir; do
  case "$dir" in /*) ;; *) continue;; esac                        # a relative PATH entry: not a place a job installs to
  real=$(cd -P "$dir" 2>/dev/null && pwd) || continue
  case ":$seen:" in *":$real:"*) continue;; esac; seen="$seen:$real"
  for c in $names; do
    p=$real/$c; [ -f "$p" ] || [ -L "$p" ] || continue; [ -x "$p" ] || [ -L "$p" ] || continue
    if ! inhome "$real"; then left="${left:+$left, }$dir/$c"; continue; fi
    m=; r=$(readlink -f "$p" 2>/dev/null)
    for k in $pkgs; do case "$r" in */node_modules/"$k"/*) m=${r%%/node_modules/"$k"/*}/node_modules/$k;; esac; done
    if [ -n "$m" ]; then mr=$(cd -P "$m" 2>/dev/null && pwd); if [ -n "$mr" ] && inhome "$mr"; then m=$mr; else m=; fi; fi
    rm -f "$p"; [ -z "$m" ] || rm -rf "$m"
    if [ -e "$p" ] || [ -L "$p" ] || { [ -n "$m" ] && [ -e "$m" ]; }; then
      echo "boat: $(hostname): could not remove the agent CLI a job installed: $p${m:+ ($m)}: this box takes no job" >&2; rc=1; continue; fi
    echo "boat: $(hostname): removed the agent CLI a job installed: $c ($p${m:+, $m})" >&2
  done
done < <(printf '%s\n' "$PATH" | tr ':' '\n')
[ -z "$left" ] || echo "boat: $(hostname): agent CLIs left alone (outside the home: Boat's own or the base image's): $left" >&2
exit $rc
