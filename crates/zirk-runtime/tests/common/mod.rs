#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn process_peak_rss_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the supplied structure on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    let peak = unsafe { usage.assume_init().ru_maxrss };
    #[cfg(target_os = "macos")]
    let bytes = u64::try_from(peak).ok()?;
    #[cfg(target_os = "linux")]
    let bytes = u64::try_from(peak).ok()?.saturating_mul(1024);
    Some(bytes)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn process_peak_rss_bytes() -> Option<u64> {
    None
}

pub fn report_process_peak_rss(name: &str) {
    println!(
        "RESOURCE benchmark={name} process_peak_rss_bytes={} metric=process_lifetime_peak",
        process_peak_rss_bytes().map_or_else(|| "unavailable".into(), |bytes| bytes.to_string()),
    );
}
