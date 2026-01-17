use crate::memory::MemBackend;
use crate::memory::MemoryReadError;
use crate::memory::MemoryReadErrorType;
use crate::memory::Pointer;
use toy_arms::external::error::TAExternalError;
use toy_arms::external::{read, Process};
use winapi::ctypes::c_void;

pub struct WindowsMem {
    process_handle: *mut c_void,
    base: usize

}

impl WindowsMem{
    pub fn new() -> Result<Self, MemoryReadError> {
        let rb = match Process::from_process_name("rekordbox.exe") {
            Ok(p) => p,
            Err(e) => {
                return Err(WindowsMem::convert_error(None, 0, e))
            }
        };
        let process_handle = rb.process_handle;

        let base = match rb.get_module_base("rekordbox.exe") {
            Ok(b) => b,
            Err(e) => {
                {
                    return Err(WindowsMem::convert_error(None, 0, e))
                }
            }
        };

        return Ok(WindowsMem{
            process_handle,
            base
        })
    }

    fn convert_error(pointer: Option<Pointer>, address: usize, e: TAExternalError) -> MemoryReadError{
        let (detail, error_type) = match e{
            TAExternalError::SnapshotFailed(snapshot_failed_detail) => (Some(snapshot_failed_detail.to_string()), MemoryReadErrorType::SnapshotFailed),
            TAExternalError::ProcessNotFound => (None, MemoryReadErrorType::ProcessNotFound),
            TAExternalError::ModuleNotFound => (None, MemoryReadErrorType::ModuleNotFound),
            TAExternalError::ReadMemoryFailed(read_write_memory_failed_detail) => (Some(read_write_memory_failed_detail.to_string()), MemoryReadErrorType::ReadMemoryFailed),
            TAExternalError::WriteMemoryFailed(read_write_memory_failed_detail) => (Some(read_write_memory_failed_detail.to_string()), MemoryReadErrorType::WriteMemoryFailed),
        };

        MemoryReadError { pointer, address, detail, error_type }
    }
}

impl MemBackend for WindowsMem{
    
    fn read<T>(&self, address: usize) -> Result<T, MemoryReadError> {
        read(self.process_handle, address).map_err(|e| WindowsMem::convert_error(None, address, e))
    }

    fn get_base_offset(&self) -> usize {
        self.base
    }

}
