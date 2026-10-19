#!/bin/bash
set -euo pipefail
export PATH=/usr/bin:/bin:$PATH
cd "$(dirname "$0")"
exec ./backplane.exe "$@"
