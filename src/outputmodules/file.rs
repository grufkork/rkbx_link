use std::fs;

use crate::{config::Config, log::ScopedLogger};

use super::{ModuleCreateOutput, OutputModule};

pub struct File {
    filename: String,
    logger: ScopedLogger,
}

impl File {
    pub fn create(conf: Config, logger: ScopedLogger) -> ModuleCreateOutput {
        Ok(Box::new(File {
            filename: conf.get_or_default("filename", "current_track.txt".to_string()),
            logger,
        }))
    }
}
impl OutputModule for File {
    fn track_changed_master(&mut self, track: &crate::beatkeeper::TrackInfo) {
        if let Err(e) = fs::write(
            &self.filename,
            format!("{}\n{}\n{}", track.title, track.artist, track.album),
        ) {
            self.logger.err(&format!("Failed to write to file: {e}"));
        }
    }
}
