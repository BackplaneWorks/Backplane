#!/usr/bin/env bash
# Join an Azure Windows VM (no public IP) to the tailnet through a managed Run Command whose auth key is
# a protected parameter: the key goes from a file (TS_KEY_FILE, e.g. Backplane's secret_run files) into
# the request body, never onto a command line, and the run command is deleted afterwards.
#
#   TS_KEY_FILE=/path/to/key scripts/winvm/winvm-tailscale.sh bp-winbuild
set -euo pipefail
vm=${1:?usage: winvm-tailscale.sh VM}
rg=${WINVM_RG:-BP-WIN-RG}
: "${TS_KEY_FILE:?TS_KEY_FILE: a file holding the Tailscale auth key}"
id=$(az vm show -g "$rg" -n "$vm" --query id -o tsv)
body=$(mktemp); trap 'rm -f "$body"' EXIT; chmod 600 "$body"
VM="$vm" python3 -I - "$TS_KEY_FILE" > "$body" <<'EOF'
import json, os, sys
key = open(sys.argv[1]).read().strip()
script = r"""param([string]$AuthKey, [string]$Name)
$ErrorActionPreference = 'Stop'
$f = Join-Path $env:ProgramData 'bp-ts.key'
Set-Content -LiteralPath $f -Value $AuthKey -NoNewline -Encoding ascii
try { & 'C:\Program Files\Tailscale\tailscale.exe' up --unattended --hostname=$Name "--auth-key=file:$f" 2>&1 | Out-String }
finally { Remove-Item -Force $f }
& 'C:\Program Files\Tailscale\tailscale.exe' ip -4
"""
print(json.dumps({"location": "westus2", "properties": {
    "source": {"script": script}, "asyncExecution": False, "timeoutInSeconds": 300,
    "parameters": [{"name": "Name", "value": os.environ["VM"]}],
    "protectedParameters": [{"name": "AuthKey", "value": key}]}}))
EOF
base="https://management.azure.com$id/runCommands/bp-tailscale-join-$(date +%s)"; api='api-version=2024-07-01'
url="$base?$api"
trap 'rm -f "$body"; az rest --method delete --url "$url" -o none' EXIT
az rest --method put --url "$url" --body @"$body" -o none
for _ in $(seq 60); do
  st=$(az rest --method get --url "$base?\$expand=instanceView&$api" --query properties.instanceView.executionState -o tsv)
  case $st in Pending|Running|'') sleep 5 ;; *) break ;; esac
done
az rest --method get --url "$base?\$expand=instanceView&$api" --query "properties.instanceView.{state:executionState,out:output,err:error}" -o json
