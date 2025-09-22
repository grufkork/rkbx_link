use rusty_link::{AblLink, SessionState};

use crate::{config::Config, log::ScopedLogger, outputmodules::OutputModule};

use super::ModuleCreateOutput;

pub struct AbletonLink {
    link: AblLink,
    state: SessionState,
    last_num_links: u64,
    logger: ScopedLogger,
    last_beat: f32,
    cumulative_error: f32,
    cumulative_error_tolerance: f32,
}

impl AbletonLink {
    pub fn create(conf: Config, logger: ScopedLogger) -> ModuleCreateOutput {
        let link = AblLink::new(120.);
        link.enable(false);

        let mut state = SessionState::new();
        link.capture_app_session_state(&mut state);

        link.enable(true);

        Ok(Box::new(AbletonLink {
            link,
            state,
            last_num_links: 9999,
            logger,
            last_beat: 0.,
            cumulative_error: 0.0,
            cumulative_error_tolerance: conf.get_or_default("cumulative_error_tolerance", 0.05),
        }))
    }
}

impl OutputModule for AbletonLink {
    fn bpm_changed_master(&mut self, bpm: f32) {
        self.state.set_tempo(bpm as f64, self.link.clock_micros());
        self.link.commit_app_session_state(&self.state);
    }

    fn beat_update_master(&mut self, beat: f32) {
        // Let link free-wheel if not playing
        if self.last_beat == beat {
            return;
        }
        // let target_beat = (beat as f64) % 4.;

        let link_beat = self.state.beat_at_time(self.link.clock_micros(), 4.0) as f32;
        let diff = (link_beat - beat + 2.0) % 4.0 - 2.0;
        // println!("{diff}");
        self.cumulative_error += diff;
        // println!("cumerr {}", self.cumulative_error);
        if self.cumulative_error.abs() > self.cumulative_error_tolerance {
            self.cumulative_error = 0.0;
            // println!("SET -----------------------------------------------------");
            self.state
                .force_beat_at_time(beat.into(), self.link.clock_micros() as u64, 4.);
            self.link.commit_app_session_state(&self.state);
        }
        self.last_beat = beat;
    }

    fn slow_update(&mut self) {
        let num_links = self.link.num_peers();
        if num_links != self.last_num_links {
            self.last_num_links = num_links;
            self.logger.info(&format!("Link peers: {num_links}"));
        }
    }
}
