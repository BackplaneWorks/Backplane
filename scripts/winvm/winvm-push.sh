#!/usr/bin/env bash
# Install or update the idle watchdog on a Windows VM over ssh (an ssh Host from ~/.ssh/config).
#
#   scripts/winvm/winvm-push.sh bp-winbuild 20     # deallocate after 20 idle minutes
#   scripts/winvm/winvm-push.sh bp-win 30 -JobDir 'C:\bp\sess'
#   scripts/winvm/winvm-push.sh bp-win 30 -WhatIf  # a dry run: log, never deallocate
set -euo pipefail
host=${1:?usage: winvm-push.sh SSH_HOST IDLE_MINUTES [-WhatIf]}
minutes=${2:?idle minutes}
shift 2; extra="$*"
here=$(cd "$(dirname "$0")" && pwd)
ssh "$host" 'powershell -NoProfile -Command "New-Item -ItemType Directory -Force C:\bp-tools\winvm | Out-Null"'
scp -q "$here/bp-idle-watchdog.ps1" "$here/install-idle-watchdog.ps1" "$host:C:/bp-tools/winvm/"
ssh "$host" "powershell -NoProfile -ExecutionPolicy Bypass -File C:\\bp-tools\\winvm\\install-idle-watchdog.ps1 -IdleMinutes $minutes $extra"
