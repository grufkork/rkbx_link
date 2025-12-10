// Show ANLZ files sorted by modification date

use std::path::PathBuf;
use std::fs;
use std::time::SystemTime;

fn main() {
    println!("=== ANLZ Files by Modification Date ===\n");

    let home = std::env::var("HOME").expect("HOME env variable not set");

    // Common ANLZ locations
    let anlz_paths = vec![
        format!("{}/Library/Application Support/Pioneer/rekordbox/share", home),
        format!("{}/Library/Application Support/Pioneer/rekordboxAgent/Storage", home),
        format!("{}/Library/Pioneer/rekordbox", home),
    ];

    println!("🔍 Scanning for ANLZ files...\n");

    let mut all_files = Vec::new();

    for base_path in &anlz_paths {
        let path = PathBuf::from(base_path);
        if path.exists() {
            println!("  Scanning: {}", base_path);
            find_anlz_files(&path, &mut all_files);
        }
    }

    if all_files.is_empty() {
        eprintln!("❌ No ANLZ files found!");
        eprintln!("\nSearched in:");
        for path in &anlz_paths {
            eprintln!("  {}", path);
        }
        return;
    }

    // Sort by modification time (most recent first)
    all_files.sort_by(|a, b| b.1.cmp(&a.1));

    println!("\n✓ Found {} ANLZ files\n", all_files.len());
    println!("Most recently modified ANLZ files:");
    println!("═══════════════════════════════════════════════════════════════════════════════\n");

    for (idx, (path, time)) in all_files.iter().take(20).enumerate() {
        // Format the time as "X seconds/minutes/hours ago"
        let elapsed = time.elapsed().unwrap_or_default();
        let time_str = if elapsed.as_secs() < 60 {
            format!("{} seconds ago", elapsed.as_secs())
        } else if elapsed.as_secs() < 3600 {
            format!("{} minutes ago", elapsed.as_secs() / 60)
        } else if elapsed.as_secs() < 86400 {
            format!("{} hours ago", elapsed.as_secs() / 3600)
        } else {
            format!("{} days ago", elapsed.as_secs() / 86400)
        };

        println!("{}. [{}]", idx + 1, time_str);
        println!("   {}", path.display());
        println!();
    }

    if all_files.len() > 20 {
        println!("... and {} more files (showing 20 most recent)\n", all_files.len() - 20);
    }

    println!("💡 How to use this:");
    println!("   1. Re-analyze a track in Rekordbox (right-click → Analyze)");
    println!("   2. Run this tool again");
    println!("   3. The top file should be the one you just analyzed");
    println!("   4. Copy that path and search for it in Cheat Engine");
}

fn find_anlz_files(dir: &PathBuf, results: &mut Vec<(PathBuf, SystemTime)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                find_anlz_files(&path, results);
            } else if let Some(ext) = path.extension() {
                if ext == "DAT" || ext == "EXT" {
                    if let Some(name) = path.file_name() {
                        if name.to_string_lossy().contains("ANLZ") {
                            if let Ok(metadata) = fs::metadata(&path) {
                                if let Ok(modified) = metadata.modified() {
                                    results.push((path, modified));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
