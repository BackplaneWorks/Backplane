param([string]$Source,[string]$Tools,[string]$Retained,[string]$Out)
$ErrorActionPreference='Stop'
. "$PSScriptRoot/job.ps1"
[BP927Job]::CapSelf()
$before=Get-BP927Resources
$before | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 "$env:RUNNER_TEMP/package-resource-before.json"
Assert-BP927Resources $before -AllowExistingPagefile $true
if($before.jobMemoryLimitBytes -ne 3699998720){throw 'Exact page-rounded packaging cap readback differs'}
$samples="$env:RUNNER_TEMP/package-paging-samples.jsonl"
$before | ConvertTo-Json -Depth 8 -Compress | Add-Content -Encoding utf8 $samples
$watch=[Diagnostics.Stopwatch]::StartNew()
$child=$null
$exitCode=$null
try {
  $info=[Diagnostics.ProcessStartInfo]::new()
  $info.FileName=(Get-Command python).Source
  $info.UseShellExecute=$false
  $info.WorkingDirectory=$Source
  foreach($arg in @("$PSScriptRoot/native_only.py",'--source',$Source,'--tools',$Tools,'--retained',$Retained,'--out',$Out)){
    $info.ArgumentList.Add($arg)
  }
  $child=[Diagnostics.Process]::Start($info)
  if(![BP927Job]::ChildMember($child.Handle)){throw 'Native packaging child did not inherit owned capped job'}
  while(!$child.WaitForExit(5000)) {
    $sample=Get-BP927Resources
    Assert-BP927Resources $sample -AllowExistingPagefile $true
    if($sample.jobMemoryLimitBytes -ne 3699998720){throw 'Owned job cap changed'}
    $sample | ConvertTo-Json -Depth 8 -Compress | Add-Content -Encoding utf8 $samples
  }
  $exitCode=$child.ExitCode
  if($exitCode -ne 0){throw "Native producer failed: $exitCode"}
} finally {
  $watch.Stop()
  $after=Get-BP927Resources
  $after | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 "$env:RUNNER_TEMP/package-resource-after.json"
  $after | ConvertTo-Json -Depth 8 -Compress | Add-Content -Encoding utf8 $samples
  @{
    sourceCommit='be904cd4cb21ad6febe17e674918c96945113d07'
    sourceTree='1034e9dc35fb517f1186ebcdb2d8f3351beb7a59'
    resourceDisposition='EXPLICIT_USER_APPROVED_EXISTING_HOST_PAGEFILE_WINDOWS_PACKAGING'
    requestedCapBytes=3700000000
    actualPageRoundedCapBytes=$after.jobMemoryLimitBytes
    member=$after.member
    limitFlags=$after.limitFlags
    peakJobMemoryBytes=$after.peakJobMemoryBytes
    wallSeconds=$watch.Elapsed.TotalSeconds
    childPID=$(if($null -ne $child){$child.Id}else{$null})
    nativeExitCode=$exitCode
    sampleIntervalSeconds=5
    pagingCounterScope='Host-wide pagefile and raw paging observations; not attributable to this job'
    before=$before
    after=$after
    noRelinkOrBend=$true
  } | ConvertTo-Json -Depth 12 | Set-Content -Encoding utf8 "$env:RUNNER_TEMP/package-resource-result.json"
  Assert-BP927Resources $after -AllowExistingPagefile $true
  if($after.jobMemoryLimitBytes -ne 3699998720){throw 'Final owned job cap differs'}
}
