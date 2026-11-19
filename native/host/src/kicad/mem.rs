//! The machine's memory and a KiCad's, for the warm-up's rule (embed.rs `warm_decision`): free memory, total memory, memory pressure,
//! and a process's resident and peak memory. One function each; each OS's reader is small and the text it parses has a unit test.
//! Linux reads /proc; macOS the Mach host statistics, sysctl and proc_pid_rusage; Windows GlobalMemoryStatusEx and GetProcessMemoryInfo.
//! None: this platform cannot say (or the call failed).

/// A process's memory now and at its peak, in MB: (resident, peak). Linux VmRSS / VmHWM; macOS resident size / lifetime maximum
/// physical footprint (what Activity Monitor calls Memory, compressed pages included; it reads higher than `ps` RSS); Windows working set /
/// peak working set.
pub fn proc_mb(pid: u32) -> Option<(u64, u64)> {
    if pid == 0 { return None }
    #[cfg(target_os = "linux")]
    { let st = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
      return Some((kb_field(&st, "VmRSS:")? / 1024, kb_field(&st, "VmHWM:").or_else(|| kb_field(&st, "VmRSS:"))? / 1024)) }
    #[cfg(target_os = "macos")]
    unsafe {
        let mut ri: libc::rusage_info_v4 = std::mem::zeroed();
        if libc::proc_pid_rusage(pid as libc::c_int, libc::RUSAGE_INFO_V4, &mut ri as *mut _ as *mut libc::rusage_info_t) != 0 { return None }
        return Some((ri.ri_resident_size >> 20, ri.ri_lifetime_max_phys_footprint.max(ri.ri_phys_footprint) >> 20))
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{Foundation::CloseHandle, System::{ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS}, Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ}}};
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid); if h.is_null() { return None }
        let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed(); c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let ok = GetProcessMemoryInfo(h, &mut c, c.cb) != 0; CloseHandle(h);
        return ok.then(|| ((c.WorkingSetSize >> 20) as u64, (c.PeakWorkingSetSize >> 20) as u64))
    }
    #[allow(unreachable_code)]
    None
}

/// Memory a new program can have without the machine swapping, in MB: Linux MemAvailable; macOS free + inactive + purgeable pages, less
/// speculative ones (free_count includes them; they are file cache read ahead); Windows the available physical memory.
pub fn free_mb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    { return kb_field(&std::fs::read_to_string("/proc/meminfo").ok()?, "MemAvailable:").map(|kb| kb / 1024) }
    #[cfg(target_os = "macos")]
    unsafe {
        let mut v: libc::vm_statistics64 = std::mem::zeroed();
        let mut n = libc::HOST_VM_INFO64_COUNT;
        #[allow(deprecated)]
        let host = libc::mach_host_self();
        if libc::host_statistics64(host, libc::HOST_VM_INFO64, &mut v as *mut _ as libc::host_info64_t, &mut n) != libc::KERN_SUCCESS { return None }
        #[allow(deprecated)]
        let page = libc::vm_page_size as u64;
        return Some(mac_free_mb(v.free_count as u64, v.inactive_count as u64, v.purgeable_count as u64, v.speculative_count as u64, page))
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut m: MEMORYSTATUSEX = std::mem::zeroed(); m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        return (GlobalMemoryStatusEx(&mut m) != 0).then(|| m.ullAvailPhys >> 20)
    }
    #[allow(unreachable_code)]
    None
}

/// The machine's physical memory in MB.
pub fn total_mb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    { return kb_field(&std::fs::read_to_string("/proc/meminfo").ok()?, "MemTotal:").map(|kb| kb / 1024) }
    #[cfg(target_os = "macos")]
    unsafe {
        let mut b: u64 = 0; let mut len = std::mem::size_of::<u64>();
        if libc::sysctlbyname(b"hw.memsize\0".as_ptr() as *const libc::c_char, &mut b as *mut u64 as *mut libc::c_void, &mut len, std::ptr::null_mut(), 0) != 0 { return None }
        return Some(b >> 20)
    }
    #[cfg(windows)]
    { return crate::plat::proc::total_ram().map(|b| b >> 20) }
    #[allow(unreachable_code)]
    None
}

/// The share of the last 10 s in which every non-idle task waited for memory at once (Linux PSI `/proc/pressure/memory`, `full avg10`,
/// percent): the machine thrashing. Not `some`: system-wide it counts a task stalled inside a capped cgroup (a build slot reclaiming against
/// its own memory.max), which read 18 on h@backplane with 12.8 GB free. None elsewhere or unreadable (a kernel without PSI): no pressure.
pub fn pressure_avg10() -> Option<f64> {
    #[cfg(target_os = "linux")]
    { return psi_full_avg10(&std::fs::read_to_string("/proc/pressure/memory").ok()?) }
    #[allow(unreachable_code)]
    None
}

/// A `Key:   1234 kB` line's number (/proc/meminfo, /proc/<pid>/status).
pub fn kb_field(text: &str, key: &str) -> Option<u64> {
    text.lines().find(|l| l.starts_with(key))?.split_whitespace().nth(1)?.parse().ok()
}
/// `full avg10=…` of a PSI file.
pub fn psi_full_avg10(text: &str) -> Option<f64> {
    text.lines().find(|l| l.starts_with("full "))?.split_whitespace().find_map(|f| f.strip_prefix("avg10="))?.parse().ok()
}
/// macOS: what can be had without paging, from the VM page counts.
pub fn mac_free_mb(free: u64, inactive: u64, purgeable: u64, speculative: u64, page: u64) -> u64 {
    (free.saturating_sub(speculative) + inactive + purgeable) * page >> 20
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn parses_proc_and_psi_text() {
        let status = "Name:\tkicad\nVmPeak:\t 9000000 kB\nVmHWM:\t 1048576 kB\nVmRSS:\t  786432 kB\n";
        assert_eq!(kb_field(status, "VmHWM:"), Some(1048576));
        assert_eq!(kb_field(status, "VmRSS:"), Some(786432));
        assert_eq!(kb_field(status, "VmSwap:"), None);
        let meminfo = "MemTotal:       32617428 kB\nMemFree:         1000000 kB\nMemAvailable:   14336000 kB\n";
        assert_eq!(kb_field(meminfo, "MemAvailable:").map(|k| k / 1024), Some(14000));
        assert_eq!(kb_field(meminfo, "MemTotal:").map(|k| k / 1024), Some(31852));
        // h@backplane during a build, 12.8 GB free: some high (capped build slots), full low
        let busy = "some avg10=18.19 avg60=9.40 avg300=2.10 total=8732072\nfull avg10=0.31 avg60=0.20 avg300=0.46 total=8122749\n";
        assert_eq!(psi_full_avg10(busy), Some(0.31));
        let thrash = "some avg10=64.02 avg60=40.00 avg300=12.00 total=99999999\nfull avg10=37.50 avg60=20.00 avg300=6.00 total=88888888\n";
        assert_eq!(psi_full_avg10(thrash), Some(37.5));
        assert_eq!(psi_full_avg10("some avg10=4.00 avg60=1.00 avg300=0.20 total=1\n"), None, "no full line: unknown");
        assert_eq!(psi_full_avg10(""), None);
    }
    #[test] fn macos_free_counts_inactive_and_purgeable_not_speculative() {
        // 16 KiB pages: 1000 free of which 200 speculative, 3000 inactive, 100 purgeable -> 3900 pages = 60.9 MB
        assert_eq!(mac_free_mb(1000, 3000, 100, 200, 16384), (3900 * 16384) >> 20);
        assert_eq!(mac_free_mb(100, 0, 0, 500, 4096), 0, "speculative above free never underflows");
    }
    #[test] fn this_machine_reads_its_own_memory() {
        // the readers answer on the platforms they are for (each CI OS runs this)
        if cfg!(any(target_os = "linux", target_os = "macos", windows)) {
            let (rss, peak) = proc_mb(std::process::id()).expect("own memory");
            assert!(rss > 0 && peak > 0, "rss {rss} peak {peak}");
            let (free, total) = (free_mb().expect("free"), total_mb().expect("total"));
            assert!(total > 0 && free <= total, "free {free} total {total}");
        }
        assert_eq!(proc_mb(0), None);
    }
}
