param([string]$Source,[string]$Tools,[string]$Retained,[string]$Out)
$ErrorActionPreference='Stop'
. "$PSScriptRoot/job.ps1"
[BP927Job]::CapSelf()
$before=Get-BP927Resources
$before | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 "$env:RUNNER_TEMP/package-resource-before.json"
Assert-BP927Resources $before
if($before.jobMemoryLimitBytes -ne 3700000000){throw 'Exact packaging cap readback differs'}
try {
  & python "$PSScriptRoot/native_driver.py" --source $Source --tools $Tools --retained $Retained --target windows-x64 --out $Out
  if($LASTEXITCODE -ne 0){throw "Native producer failed: $LASTEXITCODE"}
} finally {
  $after=Get-BP927Resources
  $after | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 "$env:RUNNER_TEMP/package-resource-after.json"
  Assert-BP927Resources $after
}
