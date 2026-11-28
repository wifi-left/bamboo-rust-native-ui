// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Process metrics for benchmarks and the future debug form.

/// Private commit in bytes (`PagefileUsage`), excluding shared DLLs.
#[cfg(windows)]
pub fn private_bytes() -> Option<u64> {
    use windows::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    let mut counters = PROCESS_MEMORY_COUNTERS::default();
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    unsafe {
        GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb).ok()?;
    }
    Some(counters.PagefileUsage as u64)
}

#[cfg(not(windows))]
pub fn private_bytes() -> Option<u64> {
    None
}
