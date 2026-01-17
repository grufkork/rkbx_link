use core::fmt;
use std::{collections::HashMap, fs::File, io::Read};
use crate::memory::Pointer;

use crate::log::ScopedLogger;

impl RekordboxOffsets {
    pub fn from_lines(lines: &[String], logger: &ScopedLogger) -> Result<RekordboxOffsets, String> {
        let mut rows = lines.iter().peekable();

        let rb_version = rows.next().ok_or("No lines left")?.to_string();

        logger.debug("Masterdeck index");
        let masterdeck_index = Pointer::from_string(
            rows.next().ok_or("Missing masterdeck index pointer")?,
            logger,
        )?;

        let mut sample_position = vec![];
        let mut current_bpm = vec![];
        let mut track_info = vec![];
        let mut anlz_path = vec![];

        while rows.peek().is_some() {
            logger.debug("Current BPM");
            current_bpm.push(Pointer::from_string(
                rows.next().ok_or("Missing BPM pointer")?,
                logger,
            )?);
            logger.debug("Sample position");
            sample_position.push(Pointer::from_string(
                rows.next().ok_or("Missing sample position pointer")?,
                logger,
            )?);
            logger.debug("Track info");
            track_info.push(Pointer::from_string(
                rows.next().ok_or("Missing track info pointer")?,
                logger,
            )?);
            logger.debug("ANLZ path");
            anlz_path.push(Pointer::from_string(
                rows.next().ok_or("Missing ANLZ path pointer")?,
                logger,
            )?);
        }

        Ok(RekordboxOffsets {
            rbversion: rb_version,
            sample_position,
            current_bpm,
            masterdeck_index,
            track_info,
            anlz_path,
        })
    }

    pub fn from_file(
        name: &str,
        logger: ScopedLogger,
    ) -> Result<HashMap<String, RekordboxOffsets>, String> {
        let Ok(mut file) = File::open(name) else {
            return Err(format!("Could not open offset file {name}"));
        };
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {
            return Err(format!("Could not read offset file {name}"));
        }
        drop(file);

        let mut empty_line_count = 0;

        let mut map = HashMap::new();

        let mut lines = vec![];
        for line in contents.lines() {
            if line.is_empty() {
                empty_line_count += 1;
                if empty_line_count >= 2 && !lines.is_empty() {
                    let offsets = RekordboxOffsets::from_lines(&lines, &logger)?;
                    map.insert(offsets.rbversion.clone(), offsets);
                    lines.clear();
                }
            } else {
                empty_line_count = 0;
                if !line.starts_with('#') {
                    lines.push(line.to_string());
                }
            }
        }

        Ok(map)
    }
}

#[derive(Clone, Debug)]
pub struct RekordboxOffsets {
    pub rbversion: String,
    pub masterdeck_index: Pointer,
    pub sample_position: Vec<Pointer>,
    pub current_bpm: Vec<Pointer>,
    pub track_info: Vec<Pointer>,
    pub anlz_path: Vec<Pointer>,
}




