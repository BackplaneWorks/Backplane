#!/bin/sh
# One-time host setup for building Backplane (Bend) locally. Run with sudo.
#   kicad     : kicad-cli for ERC/DRC/exports and viewer fixtures
#   xvfb      : headless browser tests
set -eu
apt-get update
apt-get install -y kicad xvfb
kicad-cli --version
