param([Parameter(Mandatory=$true)][string]$Source,[Parameter(Mandatory=$true)][string]$Tools,[Parameter(Mandatory=$true)][string]$Inputs,[Parameter(Mandatory=$true)][string]$ComponentSHA,[Parameter(Mandatory=$true)][string]$WindowsSHA,[Parameter(Mandatory=$true)][string]$Out)
$ErrorActionPreference='Stop'
. "$PSScriptRoot/job.ps1"
[BP927Job]::CapSelf()
$before=Get-BP927Resources
$before | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 "$env:RUNNER_TEMP/packaging-resource-before.json"
Assert-BP927Resources $before
if($before.jobMemoryLimitBytes -ne 3700000000){throw 'Exact packaging cap readback mismatch'}
try {
  & python "$PSScriptRoot/native-package.py" --source $Source --tools $Tools --target windows-x64 --authenticated-inputs $Inputs --components-sha256 $ComponentSHA --windows-sha256 $WindowsSHA --out $Out
  if($LASTEXITCODE -ne 0){throw "Native package failed: $LASTEXITCODE"}
} finally {
  $after=Get-BP927Resources
  Assert-BP927Resources $after
  $after | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 "$env:RUNNER_TEMP/packaging-resource-after.json"
}
