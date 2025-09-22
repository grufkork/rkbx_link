use crate::beatkeeper::TrackInfo;
use crate::config::Config;
use crate::log::ScopedLogger;
use crate::outputmodules::OutputModule;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use super::ModuleCreateOutput;

pub struct Setlist {
    start_time: u64,
    logger: ScopedLogger,
    stopped: bool,
    filename: String,
    separator: String,
}

impl Setlist {
    pub fn create(config: Config, logger: ScopedLogger) -> ModuleCreateOutput {
        let filename = config.get_or_default("filename", "setlist.txt".to_string());

        let mut setlist = Setlist {
            filename,
            separator: config.get_or_default("separator", " - ".to_string()),
            stopped: true,
            start_time: 0,
            logger: logger.clone(),
        };

        if let Ok(file) = File::open("setlist.txt") {
            let reader = io::BufReader::new(file);
            if let Some(Ok(line)) = reader.lines().next() {
                if let Ok(time) = line.parse::<u64>() {
                    setlist.stopped = false;
                    setlist.start_time = time;
                    setlist.logger.info(&format!(
                        "Continuing setlist started {} ago",
                        Setlist::to_timestamp(setlist.get_seconds() - time)
                    ));
                }
            }

            if setlist.stopped {
                logger.err("Failed to start: setlist file exists, but is invalid");
                return Err(());
            }
        } else {
            setlist
                .logger
                .info("No setlist file found, starting new setlist");
            setlist.start_time = setlist.get_seconds();
            match File::create(&setlist.filename) {
                Ok(mut file) => {
                    if let Err(e) = writeln!(file, "{}", setlist.start_time) {
                        logger.err(&format!("Failed to write to setlist file: {e}"));
                        return Err(());
                    } else {
                        setlist.stopped = false;
                    }
                }
                Err(e) => {
                    logger.err(&format!("Failed to create setlist file: {e}"));
                    return Err(());
                }
            }
        }

        Ok(Box::new(setlist))
    }

    fn get_seconds(&self) -> u64 {
        if let Ok(d) = SystemTime::now().duration_since(UNIX_EPOCH) {
            return d.as_secs();
        }
        self.logger.err("Time went backwards");
        0
    }

    fn to_timestamp(seconds: u64) -> String {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let seconds = seconds % 60;
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }
}

impl OutputModule for Setlist {
    fn track_changed_master(&mut self, track: &TrackInfo) {
        if self.stopped {
            return;
        }
        if let Ok(mut file) = OpenOptions::new()
            .read(false)
            .append(true)
            .open(&self.filename)
        {
            let elapsed_time = self.get_seconds() - self.start_time;

            writeln!(
                file,
                "{} {} {} {}",
                Self::to_timestamp(elapsed_time),
                track.artist,
                self.separator,
                track.title
            ).unwrap_or_else(|e| {
                self.logger.err(&format!("Failed to write to setlist file: {e}"));
            });
        } else {
            self.logger.err("Failed to open setlist file for writing!");
        }
    }
}
