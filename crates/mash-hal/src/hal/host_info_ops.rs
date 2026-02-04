//! Host information (read-only).
//!
//! This is "world-touching" (reads `/proc`, `/etc`) and belongs in the HAL.

use crate::HalResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsReleaseInfo {
    pub id: Option<String>,
    pub version_id: Option<String>,
}

pub trait HostInfoOps {
    fn hostname(&self) -> HalResult<Option<String>>;
    fn kernel_release(&self) -> HalResult<Option<String>>;
    fn os_release(&self) -> HalResult<OsReleaseInfo>;
    fn proc_cmdline(&self) -> HalResult<String>;
    fn proc_cpuinfo(&self) -> HalResult<String>;
    fn proc_meminfo(&self) -> HalResult<String>;
    fn proc_mounts(&self) -> HalResult<String>;
    fn proc_mountinfo(&self) -> HalResult<String>;
}
