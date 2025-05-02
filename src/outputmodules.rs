use crate::config::Config;
use crate::beatkeeper::TrackInfo;
use crate::log::ScopedLogger;

pub mod abletonlink;
pub mod osc;
pub mod file;
pub mod setlist;

pub trait OutputModule {
    fn bpm_changed(&mut self, _bpm: f32){}
    fn original_bpm_changed(&mut self, _bpm: f32){}
    fn playback_speed_changed(&mut self, _speed: f32){}
    fn beat_update(&mut self, _beat: f32){}
    fn time_update(&mut self, _time: f32){}

    fn track_changed(&mut self, _track: TrackInfo, _deck: usize){}
    fn master_track_changed(&mut self, _track: &TrackInfo){}

    fn slow_update(&mut self) {}
}


pub struct ModuleDefinition{
    pub config_name: String,
    pub pretty_name: String,
    pub create: fn(Config, ScopedLogger) -> Box<dyn OutputModule>
}

impl ModuleDefinition{
    pub fn new(confname: &str, prettyname: &str, create: fn(Config, ScopedLogger) -> Box<dyn OutputModule>) -> Self{
        ModuleDefinition{
            config_name: confname.to_string(), 
            pretty_name: prettyname.to_string(),
            create
        }
    }
}
