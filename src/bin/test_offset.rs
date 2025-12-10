use std::thread;
use std::time::Duration;
use std::rc::Rc;
use std::io::Write;

// Copy the necessary modules from main
#[path = "../macos_memory.rs"]
mod macos_memory;
#[path = "../offsets.rs"]
mod offsets;
#[path = "../log.rs"]
mod log;

use macos_memory::{Process, read};
use offsets::{Pointer, RekordboxOffsets};
use log::{Logger, ScopedLogger};

fn main() {
    println!("=== Rekordbox Offset Tester ===\n");

    // Find Rekordbox process
    println!("Looking for Rekordbox...");
    let rb = match Process::from_process_name("rekordbox") {
        Ok(p) => {
            println!("✓ Found Rekordbox!");
            p
        }
        Err(e) => {
            eprintln!("✗ Failed to find Rekordbox: {:?}", e);
            eprintln!("\nMake sure:");
            eprintln!("  1. Rekordbox is running");
            eprintln!("  2. Rekordbox has been re-signed with get-task-allow");
            std::process::exit(1);
        }
    };

    // Load offsets
    println!("Loading offsets...");
    let logger = Rc::new(Logger::new(true)); // Enable debug mode
    let scoped_logger = ScopedLogger::new(&logger, "OffsetTest");
    let offsets_map = match RekordboxOffsets::from_file("data/offsets-macos", scoped_logger) {
        Ok(map) => {
            println!("✓ Loaded {} offset versions", map.len());
            map
        }
        Err(e) => {
            eprintln!("✗ Failed to load offsets: {}", e);
            std::process::exit(1);
        }
    };

    // Use first available offset set
    let offsets = offsets_map.values().next().expect("No offsets found");
    println!("✓ Using offsets for Rekordbox {}", offsets.rbversion);
    println!("\nStarting live monitoring (Ctrl+C to stop)...\n");

    // Read loop
    loop {
        // Read master deck index
        let master = match read_value::<u8>(&rb, &offsets.masterdeck_index) {
            Ok(v) => v,
            Err(_) => 255,
        };

        // Read BPM for all 4 decks
        let bpm1 = match read_value::<f32>(&rb, &offsets.current_bpm[0]) {
            Ok(v) => v,
            Err(_) => 0.0,
        };
        let bpm2 = match read_value::<f32>(&rb, &offsets.current_bpm[1]) {
            Ok(v) => v,
            Err(_) => 0.0,
        };
        let bpm3 = match read_value::<f32>(&rb, &offsets.current_bpm[2]) {
            Ok(v) => v,
            Err(_) => 0.0,
        };
        let bpm4 = match read_value::<f32>(&rb, &offsets.current_bpm[3]) {
            Ok(v) => v,
            Err(_) => 0.0,
        };

        // Read sample position for all 4 decks
        let pos1 = match read_value::<i64>(&rb, &offsets.sample_position[0]) {
            Ok(v) => v,
            Err(_) => 0,
        };
        let pos2 = match read_value::<i64>(&rb, &offsets.sample_position[1]) {
            Ok(v) => v,
            Err(_) => 0,
        };
        let pos3 = match read_value::<i64>(&rb, &offsets.sample_position[2]) {
            Ok(v) => v,
            Err(_) => 0,
        };
        let pos4 = match read_value::<i64>(&rb, &offsets.sample_position[3]) {
            Ok(v) => v,
            Err(_) => 0,
        };

        // Convert sample position to time (samples / 44100 = seconds)
        let time1 = pos1 as f64 / 44100.0;
        let time2 = pos2 as f64 / 44100.0;
        let time3 = pos3 as f64 / 44100.0;
        let time4 = pos4 as f64 / 44100.0;

        // Format times as mm:ss
        let format_time = |seconds: f64| {
            let mins = (seconds / 60.0).floor() as i32;
            let secs = seconds % 60.0;
            format!("{:2}:{:05.2}", mins, secs)
        };

        // Read track info for all 4 decks
        let track1 = read_track_info(&rb, &offsets.track_info[0]);
        let track2 = read_track_info(&rb, &offsets.track_info[1]);
        let track3 = read_track_info(&rb, &offsets.track_info[2]);
        let track4 = read_track_info(&rb, &offsets.track_info[3]);

        // Read ANLZ paths for all 4 decks
        let anlz1 = read_string(&rb, &offsets.anlz_path[0]);
        let anlz2 = read_string(&rb, &offsets.anlz_path[1]);
        let anlz3 = read_string(&rb, &offsets.anlz_path[2]);
        let anlz4 = read_string(&rb, &offsets.anlz_path[3]);

        // Helper to truncate strings
        let trunc = |s: &str, len: usize| {
            if s.len() > len {
                format!("{}...", &s[..len-3])
            } else {
                format!("{:<width$}", s, width = len)
            }
        };

        // Clear screen and move cursor to top
        print!("\x1b[2J\x1b[H");

        // Print master deck
        println!("Master Deck: {}\n", master + 1);

        // Print header
        println!("           Deck 1                Deck 2                Deck 3                Deck 4");
        println!("--------   --------------------  --------------------  --------------------  --------------------");

        // Print each row
        println!("Title      {}  {}  {}  {}",
                 trunc(&track1.0, 20), trunc(&track2.0, 20), trunc(&track3.0, 20), trunc(&track4.0, 20));
        println!("Artist     {}  {}  {}  {}",
                 trunc(&track1.1, 20), trunc(&track2.1, 20), trunc(&track3.1, 20), trunc(&track4.1, 20));
        println!("Album      {}  {}  {}  {}",
                 trunc(&track1.2, 20), trunc(&track2.2, 20), trunc(&track3.2, 20), trunc(&track4.2, 20));
        println!("BPM        {:20.2}  {:20.2}  {:20.2}  {:20.2}",
                 bpm1, bpm2, bpm3, bpm4);
        println!("Position   {:20}  {:20}  {:20}  {:20}",
                 format_time(time1), format_time(time2), format_time(time3), format_time(time4));

        println!("\nANLZ Paths:");
        println!("  Deck 1: {}", anlz1);
        println!("  Deck 2: {}", anlz2);
        println!("  Deck 3: {}", anlz3);
        println!("  Deck 4: {}", anlz4);

        std::io::stdout().flush().unwrap();

        thread::sleep(Duration::from_millis(500));
    }
}

fn read_value<T: Copy>(rb: &Process, pointer: &Pointer) -> Result<T, macos_memory::MemoryError> {
    let base = rb.get_module_base("rekordbox")?;
    let handle = &rb.process_handle;

    // Follow pointer chain
    let mut address = base;

    for offset in pointer.offsets.iter() {
        address = address + offset;
        address = read::<usize>(handle, address)?;
    }

    // Add final offset and read value
    address = address + pointer.final_offset;
    let value = read::<T>(handle, address)?;

    Ok(value)
}

fn read_value_with_addr<T: Copy>(rb: &Process, pointer: &Pointer) -> Result<(T, usize), macos_memory::MemoryError> {
    let base = rb.get_module_base("rekordbox")?;
    let handle = &rb.process_handle;

    // Follow pointer chain
    let mut address = base;

    for offset in pointer.offsets.iter() {
        address = address + offset;
        address = read::<usize>(handle, address)?;
    }

    // Add final offset and read value
    address = address + pointer.final_offset;
    let value = read::<T>(handle, address)?;

    Ok((value, address))
}

fn read_track_info(rb: &Process, pointer: &Pointer) -> (String, String, String) {
    // Read 200 bytes for track info string
    let bytes = match read_value::<[u8; 200]>(rb, pointer) {
        Ok(b) => b,
        Err(_) => return (String::new(), String::new(), String::new()),
    };

    // Take bytes until null terminator
    let raw: Vec<u8> = bytes.into_iter().take_while(|&x| x != 0x00).collect();

    // Convert to UTF-8 string
    let text = String::from_utf8(raw).unwrap_or_default();

    // Parse format: "Title: value\nArtist: value\nAlbum: value"
    let mut lines = text
        .lines()
        .map(|x| x.split_once(": ").unwrap_or(("", "")).1)
        .map(|x| x.to_string());

    let title = lines.next().unwrap_or_default();
    let artist = lines.next().unwrap_or_default();
    let album = lines.next().unwrap_or_default();

    (title, artist, album)
}

fn read_string(rb: &Process, pointer: &Pointer) -> String {
    // Read up to 512 bytes for path string
    let bytes = match read_value::<[u8; 512]>(rb, pointer) {
        Ok(b) => b,
        Err(_) => return String::from("(not loaded)"),
    };

    // Take bytes until null terminator
    let raw: Vec<u8> = bytes.into_iter().take_while(|&x| x != 0x00).collect();

    // Convert to UTF-8 string
    String::from_utf8(raw).unwrap_or_default()
}
