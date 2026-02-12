// macOS process memory reading using mach directly

use sysinfo::{ProcessesToUpdate, System};
use std::mem;

use crate::memory::{MemBackend, MemoryReadError, MemoryReadErrorType};

// Mach kernel types
type MachPort = u32;
type KernReturn = i32;
type MachVmAddress = u64;
type MachVmSize = u64;

// External mach functions
extern "C" {
    fn mach_task_self() -> MachPort;
    fn task_for_pid(target_tport: MachPort, pid: i32, t: *mut MachPort) -> KernReturn;
    fn mach_vm_read_overwrite(
        target_task: MachPort,
        address: MachVmAddress,
        size: MachVmSize,
        data: MachVmAddress,
        out_size: *mut MachVmSize,
    ) -> KernReturn;
}

// Re-export types for compatibility
pub type Pid = i32;

#[derive(Clone, Copy)]
pub struct ProcessHandle {
    task: MachPort,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryError {
    ProcessNotFound,
    TaskAccessDenied,
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::ProcessNotFound => write!(f, "Process not found"),
            MemoryError::TaskAccessDenied => write!(f, "Task access denied (need sudo or entitlements)"),
        }
    }
}

pub struct MacMemory {
    pub process_handle: ProcessHandle,
    pub base_address: usize,
}

impl MacMemory {
    pub fn new() -> Result<MacMemory, MemoryReadError>{
        Ok(MacMemory::from_process_name("rekordbox").unwrap()) // FIX
    }

    /// Find a process by name and get its handle
    /// Process name should be something like "rekordbox" or "Rekordbox"
    pub fn from_process_name(name: &str) -> Result<Self, MemoryError> {
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All);

        // Find process by name (case insensitive)
        // Prefer exact match to avoid matching helper processes like "rekordboxAgent"
        let name_lower = name.to_lowercase();
        let process = sys
            .processes()
            .values()
            .find(|p| {
                p.name()
                    .to_str()
                    .map(|s| {
                        let pname = s.to_lowercase();
                        // Exact match or ends with the name (to match "/MacOS/rekordbox")
                        pname == name_lower || pname.ends_with(&format!("/{}", name_lower))
                    })
                    .unwrap_or(false)
            })
            .ok_or(MemoryError::ProcessNotFound)?;

        let pid = process.pid().as_u32() as Pid;
        eprintln!("Found Rekordbox process with PID: {}", pid);

        // Get task port for the process using task_for_pid
        let mut task: MachPort = 0;
        let result = unsafe {
            task_for_pid(
                mach_task_self(),
                pid,
                &mut task as *mut MachPort,
            )
        };

        if result != 0 {
            eprintln!("Failed to get task for PID {}: mach error code {}", pid, result);
            eprintln!();
            eprintln!("Make sure Rekordbox has been re-signed with get-task-allow!");
            eprintln!("Run: ./resign_rekordbox.sh");
            return Err(MemoryError::TaskAccessDenied);
        }

        eprintln!("Successfully got task port: {}", task);

        let process_handle = ProcessHandle { task };

        // For macOS, we need to discover the actual base address dynamically
        // since ASLR changes it every restart. We'll use a heuristic: find the
        // first executable region that looks like the main module
        let base_address = MacMemory::discover_base_address(pid)?;

        eprintln!("Discovered base address: 0x{:X}", base_address);

        Ok(MacMemory {
            process_handle,
            base_address,
        })
    }

    /// Discover the base address of a process by running vmmap
    fn discover_base_address(pid: Pid) -> Result<usize, MemoryError> {
        use std::process::Command;

        let output = Command::new("vmmap")
            .arg(pid.to_string())
            .output()
            .map_err(|_| MemoryError::ProcessNotFound)?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Look for "Load Address:    0xXXXXXXXXXX"
        for line in stdout.lines() {
            if line.contains("Load Address:") {
                if let Some(addr_str) = line.split_whitespace().last() {
                    if let Ok(addr) = usize::from_str_radix(addr_str.trim_start_matches("0x"), 16) {
                        return Ok(addr);
                    }
                }
            }
        }

        Err(MemoryError::ProcessNotFound)
    }

}



impl MemBackend for MacMemory{
    /// Read a value of type T from the process at the given address using mach_vm_read_overwrite
    fn read<T>(&self, address: usize) -> Result<T, MemoryReadError> {
        let mut value: T = unsafe { mem::zeroed() };
        let size = mem::size_of::<T>();
        let mut read_size: MachVmSize = size as MachVmSize;

        let result = unsafe {
            mach_vm_read_overwrite(
                self.process_handle.task,
                address as MachVmAddress,
                size as MachVmSize,
                &mut value as *mut T as MachVmAddress,
                &mut read_size,
            )
        };

        if result != 0 {
            return Err(MemoryReadError { pointer: None, address, detail: Some(format!("mach error: {result}")), error_type: MemoryReadErrorType::ReadMemoryFailed })
            
            // return Err(MemoryError::ReadFailed(format!(
            //             "address: 0x{:X}, mach error: {}",
            //             address, result
            // )));
        }

        Ok(value)
    }

    
    fn get_base_offset(&self) -> usize {
        self.base_address
    }
}

