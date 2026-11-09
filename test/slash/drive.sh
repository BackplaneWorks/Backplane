#!/bin/zsh
# Drives every composer command and popup in the GPUI window of a running rig (test/slash/rig.sh), one screenshot each, in $SH (default ~/.cache/bp-tmp/slash-shots).
cd "$(dirname "$0")/../.." && export SH=${SH:-$HOME/.cache/bp-tmp/slash-shots} && . test/slash/d.sh
click thread:t2; xy 800 815
keys "/"; shot a1-slash-all; keys "s"; shot a2-slash-s; key Down; shot a3-down; key Escape; shot a4-esc
clear_draft; keys "/gi"; key Tab; keys ">"; key Down; key Tab; keys "see this"; shot b4-give-ready; key Return; sleep 2; shot c1-give-sent
for p in "hi @a" "in %b" "post #d" 'use $p' "&cl"; do clear_draft; keys "$p"; shot "pop-${p//[^a-z]/}"; key Tab; shot "pop-${p//[^a-z]/}-tab"; done
clear_draft; keys "/bt"; key Tab; keys "why is the rail 3v3"; key Return; sleep 4; shot d2-btw-answer
xy 800 815; clear_draft; keys "/sec"; key Tab; keys "TOK"; key Return; sleep 2; xy 800 714; keys "zzSECRET99"; shot e3-secret-masked; key Return; sleep 2; shot e4-secret-saved
xy 800 815; keys "/intent"; key Return; sleep 2; keys "Keep it small"; shot f3-intent-edit; click send; sleep 2; shot f4-intent-saved
xy 800 815; keys "also check the 5V rail"; key Return; sleep 2; shot g2-steered
keys "/compact"; key Return; sleep 2; shot g4-compact-sent
keys "/settle"; key Return; sleep 3; shot g6-settled
! grep -rl zzSECRET99 $R && echo "secret: nowhere in the rig's home"; cat $R/proj/p1/.backplane/intent.md
