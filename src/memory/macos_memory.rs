// macOS process memory reading using mach directly

use sysinfo::{ProcessesToUpdate, System};
use std::mem;

use crate::memory::{MemBackend, MemoryReadError, MemoryReadErrorType};

// Mach kernel types
type MachPort = u32;
type KernReturn = i32;
type MachVmAddress = u64;
type MachVmSize = u64;

type Natural = u32;
const TASK_DYLD_INFO: u32 = 17;
const TASK_DYLD_INFO_COUNT: u32 = (mem::size_of::<TaskDyldInfo>() / mem::size_of::<Natural>()) as u32;

#[repr(C)]
struct TaskDyldInfo {
    all_image_info_addr: u64,
    all_image_info_size: u64,
    all_image_info_format: i32,
}

// External mach functions
extern "C" {
    fn mach_task_self() -> MachPort;
    fn task_for_pid(target_tport: MachPort, pid: i32, t: *mut MachPort) -> KernReturn;
    fn task_info(target_task: MachPort, flavor: u32, task_info_out: *mut TaskDyldInfo, task_info_count: *mut u32) -> KernReturn;
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
        sys.refresh_processes(ProcessesToUpdate::All, true);

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

        // Use TASK_DYLD_INFO to find the main binary's load address
        // without suspending the process (unlike vmmap)
        let base_address = MacMemory::discover_base_address(task)?;

        eprintln!("Discovered base address: 0x{:X}", base_address);

        Ok(MacMemory {
            process_handle,
            base_address,
        })
    }

    /// Discover the base address of the main binary via TASK_DYLD_INFO.
    /// Reads the dyld image list from the target process to find the
    /// first loaded image (the main executable) and returns its load address.
    fn discover_base_address(task: MachPort) -> Result<usize, MemoryError> {
        // 1. Get dyld_all_image_infos address from the task
        let mut dyld_info: TaskDyldInfo = unsafe { mem::zeroed() };
        let mut count = TASK_DYLD_INFO_COUNT;
        let result = unsafe {
            task_info(task, TASK_DYLD_INFO, &mut dyld_info, &mut count)
        };
        if result != 0 {
            eprintln!("task_info failed with error: {result}");
            return Err(MemoryError::ProcessNotFound);
        }

        let info_addr = dyld_info.all_image_info_addr;

        // 2. Read dyld_all_image_infos from the target process
        //    Layout: version (u32), infoArrayCount (u32), infoArray (u64 pointer)
        let mut header = [0u8; 16];
        let mut read_size: MachVmSize = 16;
        let result = unsafe {
            mach_vm_read_overwrite(
                task, info_addr, 16,
                header.as_mut_ptr() as MachVmAddress, &mut read_size,
            )
        };
        if result != 0 {
            eprintln!("Failed to read dyld_all_image_infos: mach error {result}");
            return Err(MemoryError::ProcessNotFound);
        }

        let info_array_ptr = u64::from_ne_bytes(header[8..16].try_into().unwrap());

        // 3. Read the first dyld_image_info entry
        //    Layout: imageLoadAddress (u64), imageFilePath (u64), imageFileModDate (u64)
        let mut first_entry = [0u8; 8];
        read_size = 8;
        let result = unsafe {
            mach_vm_read_overwrite(
                task, info_array_ptr, 8,
                first_entry.as_mut_ptr() as MachVmAddress, &mut read_size,
            )
        };
        if result != 0 {
            eprintln!("Failed to read dyld image info: mach error {result}");
            return Err(MemoryError::ProcessNotFound);
        }

        let base = u64::from_ne_bytes(first_entry) as usize;
        Ok(base)
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

