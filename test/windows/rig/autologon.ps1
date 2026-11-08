# Reads the password from stdin; stores it as the LSA secret DefaultPassword (not in the registry), turns on autologon for the user running it.
$ErrorActionPreference = 'Stop'
$pw = [Console]::In.ReadLine()
if (-not $pw) { throw 'no password on stdin' }
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class Lsa {
  [StructLayout(LayoutKind.Sequential)] public struct US { public ushort Length; public ushort MaximumLength; public IntPtr Buffer; }
  [StructLayout(LayoutKind.Sequential)] public struct OA { public int Length; public IntPtr RootDirectory; public IntPtr ObjectName; public uint Attributes; public IntPtr SecurityDescriptor; public IntPtr SecurityQualityOfService; }
  [DllImport("advapi32.dll")] static extern uint LsaOpenPolicy(IntPtr sys, ref OA oa, uint access, out IntPtr h);
  [DllImport("advapi32.dll")] static extern uint LsaStorePrivateData(IntPtr h, ref US key, ref US data);
  [DllImport("advapi32.dll")] static extern uint LsaClose(IntPtr h);
  [DllImport("advapi32.dll")] static extern int LsaNtStatusToWinError(uint s);
  static US Mk(string s) { var u = new US(); u.Buffer = Marshal.StringToHGlobalUni(s); u.Length = (ushort)(s.Length * 2); u.MaximumLength = (ushort)(s.Length * 2 + 2); return u; }
  public static int Store(string key, string val) {
    var oa = new OA(); IntPtr h; uint r = LsaOpenPolicy(IntPtr.Zero, ref oa, 0x00000020, out h); if (r != 0) return LsaNtStatusToWinError(r);
    var k = Mk(key); var d = Mk(val); r = LsaStorePrivateData(h, ref k, ref d); Marshal.FreeHGlobal(k.Buffer); Marshal.ZeroFreeGlobalAllocUnicode(d.Buffer); LsaClose(h);
    return r == 0 ? 0 : LsaNtStatusToWinError(r);
  }
}
'@
$e = [Lsa]::Store('DefaultPassword', $pw); $pw = $null
if ($e -ne 0) { throw "LsaStorePrivateData: $e" }
$k = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
Set-ItemProperty $k AutoAdminLogon '1'
Set-ItemProperty $k DefaultUserName $env:USERNAME
Set-ItemProperty $k DefaultDomainName $env:COMPUTERNAME
Remove-ItemProperty $k DefaultPassword -ErrorAction SilentlyContinue
# keep the session unlocked and quiet: no screen saver, no lock, no Server Manager at logon
New-Item -Force 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Personalization' | Out-Null
Set-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Personalization' NoLockScreen 1 -Type DWord
Set-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' InactivityTimeoutSecs 0 -Type DWord
New-Item -Force 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Control Panel\Desktop' | Out-Null
Set-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Control Panel\Desktop' ScreenSaveActive '0'
Get-ScheduledTask -TaskName ServerManager -ErrorAction SilentlyContinue | Disable-ScheduledTask | Out-Null
powercfg /change monitor-timeout-ac 0
'autologon on for $env:USERNAME (password in LSA)'
