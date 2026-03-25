use std::marker::PhantomData;
use core::fmt;

use crate::log::ScopedLogger;

#[cfg(target_os = "windows")]
pub mod windows_memory;
#[cfg(target_os = "windows")]
pub type ActiveBackend = windows_memory::WindowsMem;

#[cfg(target_os = "macos")]
pub mod macos_memory;
#[cfg(target_os = "macos")]
pub type ActiveBackend = macos_memory::MacMemory;

pub trait MemBackend{
    // fn new() -> Result<Box<dyn MemBackend>, String>;
    fn get_base_offset(&self) -> usize;
    fn read<T>(&self, address: usize) -> Result<T, MemoryReadError>;
}

pub struct MemReader{
    backend: ActiveBackend,
    base: usize,
}

impl MemReader{
    pub fn new() -> Result<Self, MemoryReadError>{
        let backend = ActiveBackend::new()?;
        Ok(MemReader { base: backend.get_base_offset(), backend })
    }

    pub fn new_value<T>(&self, offsets: &Pointer) -> Result<Value<T>, MemoryReadError>{
        Value::new(self, offsets)
    } 
    pub fn new_values<T>(&self, pointers: &[Pointer]) -> Result<Vec<Value<T>>, MemoryReadError> {
        pointers.iter().map(|x| self.new_value(x)).collect()
    }

    pub fn new_pointerchain_value<T>(&self, offsets: Pointer) -> PointerChainValue<T>{
        PointerChainValue::new(self, offsets)
    }
    pub fn new_pointerchain_values<T>(
        &self,
        pointers: &[Pointer],
    ) -> Vec<PointerChainValue<T>> {
        pointers
            .iter()
            .map(|x| self.new_pointerchain_value(x.clone()))
            .collect()
    }

    pub fn read<T>(&self, address: usize) -> Result<T, MemoryReadError>{
        self.backend.read::<T>(address)
    }

}


pub struct Value<T> {
    address: usize,
    _marker: PhantomData<T>,
}

impl<T> Value<T> {
    fn new(mem: &MemReader, pointer: &Pointer) -> Result<Value<T>, MemoryReadError> {
        let mut address = mem.base;

        for offset in &pointer.offsets {
            address = mem.read::<usize>(address + offset).map_err(
                |mut e|{
                    e.pointer = Some(pointer.clone());
                    e
                }
            )?
        }
        address += pointer.final_offset;

        Ok(Value::<T> {
            address,
            _marker: PhantomData::<T>,
        })
    }

    pub fn read(&self, mem: &MemReader) -> Result<T, MemoryReadError> {
        mem.read::<T>(self.address)
    }
}




pub struct PointerChainValue<T> {
    pointer: Pointer,
    _marker: PhantomData<T>,
}

impl<T> PointerChainValue<T> {
    fn new(_mem: &MemReader, pointer: Pointer) -> PointerChainValue<T> {
        Self {
            pointer,
            _marker: PhantomData::<T>,
        }
    }



    pub fn read(&self, mem: &MemReader) -> Result<T, MemoryReadError> {
        Value::<T>::new(mem, &self.pointer)?.read(mem)
    }
}

#[derive(PartialEq, Clone)]
#[allow(dead_code)]
pub enum MemoryReadErrorType{
    ProcessNotFound,
    SnapshotFailed,
    ReadMemoryFailed,
    WriteMemoryFailed,
    ModuleNotFound,
}

#[derive(PartialEq, Clone)]
pub struct MemoryReadError {
    pub pointer: Option<Pointer>,
    pub address: usize,
    pub detail: Option<String>,
    pub error_type: MemoryReadErrorType,
}

#[derive(PartialEq, Clone, Debug)]
pub struct Pointer {
    pub offsets: Vec<usize>,
    pub final_offset: usize,
}

impl Pointer {
    pub fn new(offests: Vec<usize>, final_offset: usize) -> Pointer {
        Pointer {
            offsets: offests,
            final_offset,
        }
    }

    pub fn from_string(input: &str, logger: &ScopedLogger) -> Result<Self, String> {
        logger.debug(&format!("Parsing pointer: {input}"));
        let split = input
            .split(' ')
            .map(hexparse)
            .collect::<Result<Vec<usize>, String>>()?;
        let last = *split.last().ok_or("Last offset is missing")?;
        Ok(Self::new(split[0..split.len() - 1].to_vec(), last))
    }
}

impl fmt::Display for Pointer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut res = "[".to_string();
        for offset in &self.offsets {
            res += &format!("{offset:X}, ");
        }
        res += &format!("{:X}]", self.final_offset);

        write!(f, "{res}")
    }
}

fn hexparse(input: &str) -> Result<usize, String> {
    usize::from_str_radix(input, 16).map_err(|_| format!("Failed to parse hex value: {input}"))
}


