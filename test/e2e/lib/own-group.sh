# Sourced by the e2e scripts: what a run starts is ended through its own process groups, never by a name or a pattern.
# (`pkill -f -- "$T/"` with an empty $T matches every process of the user, the person's live hub included; a pattern can also match a process that is not the run's.)
#   own CMD [ARGS...] [> LOG]   start CMD in the background in a process group of its own; $! is its pid and its group's id, kept in OWN_GROUPS
#   own_stop                    TERM to every group started with own, then KILL to what is left after 2 s
OWN_GROUPS=()
own() { setsid "$@" & OWN_GROUPS+=("$!"); }
own_stop() {
  local g left=
  for g in "${OWN_GROUPS[@]}"; do kill -TERM -- "-$g" 2>/dev/null && left=1; done
  [ -z "$left" ] || sleep 2
  for g in "${OWN_GROUPS[@]}"; do kill -KILL -- "-$g" 2>/dev/null; done
  OWN_GROUPS=()
}
