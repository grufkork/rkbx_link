// Memory browser - explores memory around known working offsets
// to find related values in the same data structure

use std::rc::Rc;
use std::collections::HashMap;
use std::io::{self, Write};

#[allow(dead_code)]
#[path = "../log.rs"]
mod log;
#[allow(dead_code)]
#[path = "../memory/mod.rs"]
mod memory;
#[allow(dead_code)]
#[path = "../offsets.rs"]
mod offsets;

use memory::{MemBackend, Pointer};
use memory::macos_memory::MacMemory;
use offsets::RekordboxOffsets;
use log::{Logger, ScopedLogger};

#[derive(Clone)]
struct MemorySnapshot {
    val_u64: u64,
    val_f32: f32,
    val_i64: i64,
    str_preview: String,
}

fn main() {
    println!("=== Memory Structure Browser ===\n");

    let rb = match MacMemory::from_process_name("rekordbox") {
        Ok(p) => {
            println!("✓ Found Rekordbox (base: 0x{:X})", p.base_address);
            p
        }
        Err(e) => {
            eprintln!("✗ Failed to find Rekordbox: {:?}", e);
            std::process::exit(1);
        }
    };

    // Load existing offsets
    let logger = Rc::new(Logger::new(true));
    let scoped_logger = ScopedLogger::new(&logger, "MemBrowser");
    let offsets_map = match RekordboxOffsets::from_file("data/offsets-macos", scoped_logger) {
        Ok(map) => map,
        Err(e) => {
            eprintln!("✗ Failed to load offsets: {}", e);
            std::process::exit(1);
        }
    };

    let offsets = offsets_map.values().next().expect("No offsets found");

    println!("\nFollowing BPM pointer chain for deck 1...");
    let bpm_struct_addr = follow_pointer_chain(&rb, &offsets.current_bpm[0]);

    if let Some(addr) = bpm_struct_addr {
        println!("✓ Found BPM structure at: 0x{:X}", addr);

        let bpm_final_offset = offsets.current_bpm[0].final_offset;
        println!("   BPM final offset: 0x{:X}", bpm_final_offset);

        loop {
            // Take initial snapshot
            println!("\n📸 Taking memory snapshot...");
            let snapshot1 = take_snapshot(&rb, addr);
            println!("   Captured {} memory locations", snapshot1.len());

            println!("\n⏸️  PAUSED - Now change something in Rekordbox:");
            println!("   - Change BPM (tempo slider)");
            println!("   - Seek to different position");
            println!("   - Load different track");
            println!("\nPress ENTER to rescan (or 'q' + ENTER to quit)...");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();

            if input.trim().eq_ignore_ascii_case("q") {
                println!("\n👋 Done!");
                break;
            }

            // Take second snapshot
            println!("📸 Taking second snapshot...");
            let snapshot2 = take_snapshot(&rb, addr);

            // Compare and show only changed values
            println!("\n🔍 Showing ONLY changed values:\n");
            println!("Offset    | Old Value        | New Value        | Change    | f32 change | i64 change    | Final Offset | Notes");
            println!("----------|------------------|------------------|-----------|------------|---------------|--------------|------------------");

            let mut changes = Vec::new();

            for (offset, snap1) in &snapshot1 {
                if let Some(snap2) = snapshot2.get(offset) {
                    if snap1.val_u64 != snap2.val_u64 {
                        changes.push((*offset, snap1.clone(), snap2.clone()));
                    }
                }
            }

            if changes.is_empty() {
                println!("\n⚠️  No changes detected! Make sure you changed something in Rekordbox.");
            } else {
                // Sort by offset for easier reading
                changes.sort_by_key(|(offset, _, _)| *offset);

                for (offset, snap1, snap2) in &changes {
                    let f32_change = snap2.val_f32 - snap1.val_f32;
                    let i64_change = snap2.val_i64 - snap1.val_i64;

                    // Calculate the actual final offset to use
                    let final_offset = (bpm_final_offset as i64 + *offset) as usize;

                    // Annotate what this might be
                    let note = if f32_change.abs() > 1.0 && f32_change.abs() < 100.0 {
                        "BPM?"
                    } else if i64_change.abs() > 100000 {
                        "sample pos?"
                    } else if snap1.str_preview != snap2.str_preview && !snap2.str_preview.is_empty() {
                        "text changed!"
                    } else if snap2.val_u64 > 0x600000000000 && snap2.val_u64 < 0x700000000000 {
                        "heap pointer?"
                    } else {
                        ""
                    };

                    println!("{:+5}({:03X}) | 0x{:016X} | 0x{:016X} | {:+9} | {:+10.2} | {:+13} | 0x{:X}       | {}",
                             *offset, offset.abs() as usize,
                             snap1.val_u64, snap2.val_u64,
                             snap2.val_u64 as i64 - snap1.val_u64 as i64,
                             f32_change, i64_change, final_offset, note);
                }

                println!("\n✓ Found {} changed values", changes.len());
                println!("\n💡 To use these offsets:");
                println!("   Use the 'Final Offset' column in your pointer chain");
                println!("   Your BPM pointer chain: {} {}",
                         offsets.current_bpm[0].offsets.iter()
                             .map(|o| format!("{:X}", o))
                             .collect::<Vec<_>>()
                             .join(" "),
                         format!("{:X}", bpm_final_offset));
                println!("   Replace the final offset ({:X}) with the Final Offset shown above", bpm_final_offset);
            }
        }

    } else {
        println!("✗ Failed to follow pointer chain");
    }
}

fn take_snapshot(rb: &MacMemory, base_addr: usize) -> HashMap<i64, MemorySnapshot> {
    let mut snapshot = HashMap::new();

    // Scan ±1024 bytes around the base address
    for offset in (-1024..1024).step_by(8) {
        let scan_addr = (base_addr as i64 + offset) as usize;

        if let Ok(val_u64) = rb.read::<u64>(scan_addr) {
            let val_f32 = rb.read::<f32>(scan_addr).unwrap_or(0.0);
            let val_i64 = rb.read::<i64>(scan_addr).unwrap_or(0);

            // Try to read as string
            let mut str_bytes = Vec::new();
            for i in 0..16 {
                if let Ok(byte) = rb.read::<u8>(scan_addr + i) {
                    if byte == 0 { break; }
                    if byte >= 32 && byte < 127 {
                        str_bytes.push(byte);
                    } else {
                        break;
                    }
                }
            }
            let str_preview = if str_bytes.len() > 3 {
                String::from_utf8_lossy(&str_bytes).to_string()
            } else {
                String::new()
            };

            snapshot.insert(offset, MemorySnapshot {
                val_u64,
                val_f32,
                val_i64,
                str_preview,
            });
        }
    }

    snapshot
}

fn follow_pointer_chain(rb: &MacMemory, pointer: &Pointer) -> Option<usize> {
    let mut address = rb.base_address;

    for offset in pointer.offsets.iter() {
        address = address + offset;
        address = rb.read::<usize>(address).ok()?;
    }

    address = address + pointer.final_offset;
    Some(address)
}
