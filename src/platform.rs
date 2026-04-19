/// Returns `Some(true)` if the filesystem at `path` is on a rotational (HDD) disk,
/// `Some(false)` for SSD/NVMe, or `None` if detection is unavailable.
///
/// On Linux this reads `/sys/block/<dev>/queue/rotational`.
/// On other platforms it always returns `None`.
pub fn is_rotational(path: &std::path::Path) -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        linux_is_rotational(path)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        None
    }
}

#[cfg(target_os = "linux")]
fn linux_is_rotational(path: &std::path::Path) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;

    let meta = std::fs::metadata(path).ok()?;
    let dev = meta.dev();

    // Standard Linux major/minor extraction from dev_t (64-bit).
    let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff_u64);
    let minor = (dev & 0xff) | ((dev >> 12) & 0xffff_ff00);

    let sysfs = format!("/sys/dev/block/{}:{}", major, minor);
    let sysfs_path = std::path::Path::new(&sysfs);

    // Try direct queue/rotational (whole-disk device).
    let direct = sysfs_path.join("queue/rotational");
    if direct.exists() {
        return read_flag(&direct);
    }

    // Follow symlink for partition devices (e.g. sda1 → sda).
    let link = std::fs::read_link(sysfs_path).ok()?;
    // link is relative, e.g. "../../block/sda/sda1" — go up one level to get the disk.
    let parent_rel = link.parent()?;
    let parent_abs = std::path::Path::new("/sys/dev/block")
        .join(parent_rel)
        .canonicalize()
        .ok()?;
    read_flag(&parent_abs.join("queue/rotational"))
}

#[cfg(target_os = "linux")]
fn read_flag(path: &std::path::Path) -> Option<bool> {
    let val = std::fs::read_to_string(path).ok()?;
    Some(val.trim() == "1")
}
