#!/bin/sh
# The only way a test, an end-to-end run or a release check runs Codex on a paid OpenAI API key (the vault's `openai-test-api-key`).
# A paid key is for checking that the harness answers, on the cheapest model, never more: on Nov 19 a run that named no model let Codex
# take its default (gpt-6.2-sol) and spent about 130 requests on the person's key. So this wrapper:
#   - pins the model to CODEX_APIKEY_MODEL below (the one constant) and refuses to start with any other: -m/--model, -c model=..., a
#     profile (-p/--profile) or a local provider (--oss) that names another; with none named it adds the pin;
#   - writes its own CODEX_HOME config with the pin, low reasoning effort and low verbosity (Codex has no max-output-tokens setting);
#   - lets one turn run per CODEX_HOME (CODEX_APIKEY_TURNS, at most 3) and ends a turn after CODEX_APIKEY_SECONDS (default 120, at most 300).
# usage: CODEX_APIKEY_FILE=<key file> CODEX_APIKEY_HOME=<scratch dir> [CODEX_APIKEY_REAL=<codex>] scripts/codex-apikey.sh <codex args...>
#        scripts/codex-apikey.sh --model   prints the pinned model
# A harness puts it where `codex` is looked up (a stand-in folder's `codex` that execs it), so the hub's own arguments come through here.
# Exit 97: a model other than the pin was asked for; 98: the turn cap is reached; 2: a missing key file or home.
set -eu
CODEX_APIKEY_MODEL=gpt-5-nano

[ "${1:-}" = "--model" ] && [ $# -eq 1 ] && { echo "$CODEX_APIKEY_MODEL"; exit 0; }
die() { code=$1; shift; echo "codex-apikey: $*" >&2; exit "$code"; }

key=${CODEX_APIKEY_FILE:-}; home=${CODEX_APIKEY_HOME:-}
[ -n "$key" ] && [ -r "$key" ] || die 2 "CODEX_APIKEY_FILE must name a readable key file"
[ -n "$home" ] || die 2 "CODEX_APIKEY_HOME must name a scratch folder (Codex's own home for this run, never ~/.codex)"
case "$home" in "$HOME/.codex"|"$HOME/.codex/"*) die 2 "CODEX_APIKEY_HOME is the person's own Codex home: use a scratch folder";; esac

# every model the arguments name must be the pin
check() { [ "$1" = "$CODEX_APIKEY_MODEL" ] || die 97 "refused: model '$1' asked for; API-key runs use only $CODEX_APIKEY_MODEL"; }
named=0; prev=""
for a in "$@"; do
  case "$prev" in
    -m|--model) check "$a"; named=1 ;;
    -c|--config) case "$a" in model=*|model\ =*) v=${a#*=}; v=$(printf '%s' "$v" | tr -d "\"' "); check "$v"; named=1 ;;
                              model_provider=*|profile=*|oss_provider=*) die 97 "refused: '$a' could reach another model";; esac ;;
    -p|--profile|--local-provider) die 97 "refused: '$prev $a' could reach another model" ;;
  esac
  case "$a" in
    --model=*) check "${a#--model=}"; named=1 ;;
    --profile=*|--local-provider=*|--oss) die 97 "refused: '$a' could reach another model" ;;
  esac
  prev=$a
done

# one short turn per home (resume counts: it is another turn)
turns=${CODEX_APIKEY_TURNS:-1}; [ "$turns" -ge 1 ] 2>/dev/null && [ "$turns" -le 3 ] || die 2 "CODEX_APIKEY_TURNS is 1 to 3"
secs=${CODEX_APIKEY_SECONDS:-120}; [ "$secs" -ge 10 ] 2>/dev/null && [ "$secs" -le 300 ] || die 2 "CODEX_APIKEY_SECONDS is 10 to 300"
mkdir -p "$home"
case " $* " in *" exec "*|*" e "*)
  n=$(cat "$home/.turns" 2>/dev/null || echo 0)
  [ "$n" -lt "$turns" ] || die 98 "refused: this run already had $n turn(s) on the API key (CODEX_APIKEY_TURNS=$turns)"
  echo $((n + 1)) > "$home/.turns" ;;
esac

# the pin in Codex's own config too, so nothing Codex reads by itself names another model
cat > "$home/config.toml" <<EOF
model = "$CODEX_APIKEY_MODEL"
model_reasoning_effort = "low"
model_verbosity = "low"
EOF

real=${CODEX_APIKEY_REAL:-}
if [ -z "$real" ]; then real=$(command -v codex) || die 2 "no codex on PATH (set CODEX_APIKEY_REAL)"; fi
OPENAI_API_KEY=$(cat "$key"); CODEX_API_KEY=$OPENAI_API_KEY; CODEX_HOME=$home
export OPENAI_API_KEY CODEX_API_KEY CODEX_HOME
# with no model named, the pin goes first (a global -c applies to every subcommand)
if [ "$named" -eq 0 ]; then set -- -c "model=\"$CODEX_APIKEY_MODEL\"" "$@"; fi
if command -v timeout >/dev/null 2>&1; then exec timeout -k 5 "$secs" "$real" "$@"; fi
exec perl -e 'alarm shift; exec @ARGV or die "codex-apikey: cannot run $ARGV[0]: $!\n"' "$secs" "$real" "$@" # macOS has no timeout
