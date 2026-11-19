#!/bin/sh
# The pictures graphic.html shows, cropped from the captures in shots/ (capture.sh). Crops only: nothing is retouched or scaled up.
# Every crop ends above the composer's agent row, which carries the agent's own mark (a third-party logo: not in our graphics).
set -eu
cd "$(dirname "$0")"
convert shots/kicad.png    -crop 1904x1356+0+0   +repage img/kicad.png      # the whole window: chat, then KiCad in the pane
convert shots/altium.png   -crop 1904x1356+0+0   +repage img/altium.png     # the whole window: chat, then the Altium sheet
convert shots/terminal.png -crop 1904x1356+0+0   +repage img/terminal.png   # the whole window with the chat's terminal open
convert shots/restore.png  -crop 1232x906+530+450 +repage img/restore.png   # the chat column
convert shots/copy.png     -crop 1232x906+530+450 +repage img/copy.png      # the chat column
convert shots/kicad-wide.png -crop 2944x1076+0+0 +repage img/kicad-wide.png  # the whole 16:9 window, for the landscape hero and the header banner
