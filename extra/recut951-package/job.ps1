$ErrorActionPreference = 'Stop'
# Native Windows resource guard; executed on the disposable GitHub runner only.
# See Microsoft's JOBOBJECT_EXTENDED_LIMIT_INFORMATION and QueryInformationJobObject.
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class BP927Job {
  [StructLayout(LayoutKind.Sequential)] public struct Basic {
    public long processTime, jobTime; public uint flags;
    public UIntPtr minWorkingSet, maxWorkingSet; public uint activeProcesses;
    public UIntPtr affinity; public uint priority, scheduling;
  }
  [StructLayout(LayoutKind.Sequential)] public struct IO {
    public ulong readOps, writeOps, otherOps, readBytes, writeBytes, otherBytes;
  }
  [StructLayout(LayoutKind.Sequential)] public struct Limits {
    public Basic basic; public IO io; public UIntPtr processMemory, jobMemory, peakProcess, peakJob;
  }
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)] static extern IntPtr CreateJobObject(IntPtr sa, string name);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool SetInformationJobObject(IntPtr job, int kind, ref Limits value, uint bytes);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool QueryInformationJobObject(IntPtr job, int kind, out Limits value, uint bytes, IntPtr returned);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool IsProcessInJob(IntPtr process, IntPtr job, out bool member);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool DuplicateHandle(IntPtr sourceProcess, IntPtr source, IntPtr targetProcess, out IntPtr target, uint access, bool inherit, uint options);
  static IntPtr owned, query;
  static IntPtr ObservedJob() { return query!=IntPtr.Zero ? query : owned; }
  static void Check(bool ok) { if(!ok) throw new Win32Exception(Marshal.GetLastWin32Error()); }
  public static void CapSelf() {
    owned=CreateJobObject(IntPtr.Zero, null); if(owned==IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
    var value=new Limits(); value.basic.flags=0x200; value.jobMemory=new UIntPtr(3700000000UL);
    Check(SetInformationJobObject(owned,9,ref value,(uint)Marshal.SizeOf(typeof(Limits))));
    Check(AssignProcessToJobObject(owned,Process.GetCurrentProcess().Handle));
  }
  public static Limits Current() {
    var job=ObservedJob(); if(job==IntPtr.Zero) throw new InvalidOperationException("Explicit job query handle required");
    Limits value; Check(QueryInformationJobObject(job,9,out value,(uint)Marshal.SizeOf(typeof(Limits)),IntPtr.Zero)); return value;
  }
  public static bool Admit(IntPtr process) {
    if(owned==IntPtr.Zero) throw new InvalidOperationException("No owned capped job");
    Check(AssignProcessToJobObject(owned,process));
    bool member; Check(IsProcessInJob(process,owned,out member));
    if(!member) throw new InvalidOperationException("Child is outside the capped job");
    return member;
  }
  public static bool Member() {
    var job=ObservedJob(); if(job==IntPtr.Zero) throw new InvalidOperationException("Explicit job membership handle required");
    bool member; Check(IsProcessInJob(Process.GetCurrentProcess().Handle,job,out member)); return member;
  }
  public static bool ChildMember(IntPtr process) {
    var job=ObservedJob(); if(job==IntPtr.Zero) throw new InvalidOperationException("Explicit job query handle required");
    bool member; Check(IsProcessInJob(process,job,out member)); return member;
  }
  public static long DuplicateQueryHandle(IntPtr targetProcess) {
    if(owned==IntPtr.Zero) throw new InvalidOperationException("No owned capped job");
    IntPtr target; Check(DuplicateHandle(Process.GetCurrentProcess().Handle,owned,targetProcess,out target,0x4,false,0));
    return target.ToInt64(); // JOB_OBJECT_QUERY only; never SET_ATTRIBUTES.
  }
  public static void UseQueryHandle(long handle) { query=new IntPtr(handle); }
}
'@
function Get-BP927Resources {
    $limits = [BP927Job]::Current()
    $paging = @(Get-CimInstance Win32_PageFileUsage -ErrorAction Stop | Select-Object Name,AllocatedBaseSize,CurrentUsage,PeakUsage)
    $counters = Get-CimInstance Win32_PerfRawData_PerfOS_Memory -ErrorAction Stop |
        Select-Object PageReadsPersec,PageWritesPersec,PagesInputPersec,PagesOutputPersec,PageFaultsPersec,Timestamp_PerfTime,Frequency_PerfTime,AvailableBytes,CommittedBytes,CommitLimit
    @{
        jobMemoryLimitBytes = $limits.jobMemory.ToUInt64()
        peakJobMemoryBytes = $limits.peakJob.ToUInt64()
        peakProcessMemoryBytes = $limits.peakProcess.ToUInt64()
        limitFlags = $limits.basic.flags
        member = [BP927Job]::Member()
        pageFiles = $paging
        pagingCounters = $counters
        pagingCounterScope = 'Host-wide raw Windows memory counters; use timestamp/frequency deltas, not process attribution'
        pid = $PID
        observedAt = [DateTime]::UtcNow.ToString('o')
    }
}
function Assert-BP927Resources($resources, [bool]$AllowExistingPagefile=$false) {
    if (!$resources.member -or $resources.jobMemoryLimitBytes -le 0 -or $resources.jobMemoryLimitBytes -gt 3700000000 -or
        ($resources.limitFlags -band 0x200) -eq 0 -or ($resources.limitFlags -band 0x1800) -ne 0 -or
        (!$AllowExistingPagefile -and $resources.pageFiles.Count -ne 0) -or !$resources.pagingCounters) {
        throw 'BLOCKED: enforced <=3.7e9 B Windows packaging job, no breakaway and authorized measured paging policy required'
    }
}
