use std::time::{Duration, Instant};

use crate::{beatkeeper::TrackInfo, config::Config, log::ScopedLogger};

use super::{ModuleCreateOutput, OutputModule};

#[derive(Clone, Default)]
struct DeckState {
    track: TrackInfo,
    bpm: f32,
    position: i64,
    beat: f32,
}

pub struct Display {
    logger: ScopedLogger,
    interval: Duration,
    last_display: Instant,
    decks: Vec<DeckState>,
}

impl Display {
    pub fn create(conf: Config, logger: ScopedLogger) -> ModuleCreateOutput {
        let interval_secs: f32 = conf.get_or_default("interval", 1.);

        Ok(Box::new(Display {
            logger,
            interval: Duration::from_secs_f32(interval_secs),
            last_display: Instant::now(),
            decks: vec![DeckState::default(); 4],
        }))
    }

    fn display_info(&self) {
        // Helper to format time from samples
        let format_time = |samples: i64| {
            let seconds = samples as f64 / 44100.0;
            let mins = (seconds / 60.0).floor() as i32;
            let secs = seconds % 60.0;
            format!("{}:{:05.2}", mins, secs)
        };

        // Print each deck's status
        for (i, deck) in self.decks.iter().enumerate() {
            if !deck.track.title.is_empty() {
                self.logger.info(&format!(
                    "Deck {}: \"{}\" by {} | BPM: {:.1} | Pos: {} | Beat: {:.2}",
                    i + 1, deck.track.title, deck.track.artist,
                    deck.bpm, format_time(deck.position), deck.beat
                ));
            } else {
                self.logger.info(&format!("Deck {}: (no track)", i + 1));
            }
        }
    }
}

impl OutputModule for Display {
    fn pre_update(&mut self) {
        // Check if it's time to display
        if self.last_display.elapsed() >= self.interval {
            self.display_info();
            self.last_display = Instant::now();
        }
    }

    fn bpm_changed(&mut self, bpm: f32, deck: usize) {
        if deck < self.decks.len() {
            self.decks[deck].bpm = bpm;
        }
    }

    fn beat_update(&mut self, beat: f32, deck: usize) {
        if deck < self.decks.len() {
            self.decks[deck].beat = beat;
        }
    }

    fn time_update(&mut self, time: f32, deck: usize) {
        if deck < self.decks.len() {
            self.decks[deck].position = (time * 44100.0) as i64;
        }
    }

    fn track_changed(&mut self, track: &TrackInfo, deck: usize) {
        if deck < self.decks.len() {
            self.decks[deck].track = track.clone();
        }
    }
}
