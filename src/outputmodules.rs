use crate::beatkeeper::TrackInfo;
use crate::config::Config;
use crate::log::ScopedLogger;

pub mod abletonlink;
pub mod file;
pub mod osc;
pub mod setlist;

pub trait OutputModule {
    fn bpm_changed(&mut self, _bpm: f32, _deck: usize) {}
    fn bpm_changed_master(&mut self, _bpm: f32) {}

    fn original_bpm_changed(&mut self, _bpm: f32, _deck: usize) {}
    fn original_bpm_changed_master(&mut self, _bpm: f32) {}

    fn beat_update(&mut self, _beat: f32, _deck: usize) {}
    fn beat_update_master(&mut self, _beat: f32) {}

    fn time_update(&mut self, _time: f32, _deck: usize) {}
    fn time_update_master(&mut self, _time: f32) {}

    fn track_changed(&mut self, _track: &TrackInfo, _deck: usize) {}
    fn track_changed_master(&mut self, _track: &TrackInfo) {}

    fn slow_update(&mut self) {}
}

pub struct ModuleDefinition {
    pub config_name: String,
    pub pretty_name: String,
    pub create: fn(Config, ScopedLogger) -> ModuleCreateOutput,
}

impl ModuleDefinition {
    pub fn new(
        confname: &str,
        prettyname: &str,
        create: fn(Config, ScopedLogger) -> ModuleCreateOutput,
    ) -> Self {
        ModuleDefinition {
            config_name: confname.to_string(),
            pretty_name: prettyname.to_string(),
            create,
        }
    }
}

pub type ModuleCreateOutput = Result<Box<dyn OutputModule>, ()>;
