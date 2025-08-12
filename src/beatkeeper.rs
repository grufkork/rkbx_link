use crate::config::Config;
use crate::log::ScopedLogger;
use crate::offsets::Pointer;
use crate::outputmodules::ModuleDefinition;
use crate::outputmodules::OutputModule;
use crate::RekordboxOffsets;
use binrw::BinRead;
use rekordcrate::anlz::{self, BeatGrid};
use std::io::Cursor;
use std::thread;
use std::{marker::PhantomData, time::Duration};
use toy_arms::external::error::TAExternalError;
use toy_arms::external::{read, Process};
use winapi::ctypes::c_void;

#[derive(PartialEq, Clone)]
struct ReadError {
    pointer: Option<Pointer>,
    address: usize,
    error: TAExternalError,
}
struct Value<T> {
    address: usize,
    handle: *mut c_void,
    _marker: PhantomData<T>,
}

impl<T> Value<T> {
    fn new(h: *mut c_void, base: usize, offsets: &Pointer) -> Result<Value<T>, ReadError> {
        let mut address = base;

        for offset in &offsets.offsets {
            address = match read::<usize>(h, address + offset) {
                Ok(val) => val,
                Err(e) => {
                    return Err(ReadError {
                        pointer: Some(offsets.clone()),
                        address: address + offset,
                        error: e,
                    })
                }
            }
        }
        address += offsets.final_offset;

        Ok(Value::<T> {
            address,
            handle: h,
            _marker: PhantomData::<T>,
        })
    }
    fn pointers_to_vals(
        h: *mut c_void,
        base: usize,
        pointers: &[Pointer],
    ) -> Result<Vec<Value<T>>, ReadError> {
        pointers.iter().map(|x| Value::new(h, base, x)).collect()
    }

    fn read(&self) -> Result<T, ReadError> {
        match read::<T>(self.handle, self.address) {
            Ok(val) => Ok(val),
            Err(e) => Err(ReadError {
                pointer: None,
                address: self.address,
                error: e,
            }),
        }
    }
}

struct PointerChainValue<T> {
    handle: *mut c_void,
    base: usize,
    pointer: Pointer,
    _marker: PhantomData<T>,
}

impl<T> PointerChainValue<T> {
    fn new(h: *mut c_void, base: usize, pointer: Pointer) -> PointerChainValue<T> {
        Self {
            handle: h,
            base,
            pointer,
            _marker: PhantomData::<T>,
        }
    }

    fn pointers_to_vals(
        h: *mut c_void,
        base: usize,
        pointers: &[Pointer],
    ) -> Vec<PointerChainValue<T>> {
        pointers
            .iter()
            .map(|x| PointerChainValue::new(h, base, x.clone()))
            .collect()
    }

    fn read(&self) -> Result<T, ReadError> {
        Value::<T>::new(self.handle, self.base, &self.pointer)?.read()
    }
}

pub struct Rekordbox {
    masterdeck_index: Value<u8>,
    current_bpms: Vec<Value<f32>>,
    sample_positions: Vec<Value<i64>>,
    track_infos: Vec<PointerChainValue<[u8; 200]>>,
    anlz_paths: Vec<PointerChainValue<[u8; 500]>>,
    deckcount: usize,
}

impl Rekordbox {
    fn new(offsets: RekordboxOffsets, decks: usize) -> Result<Self, ReadError> {
        let rb = match Process::from_process_name("rekordbox.exe") {
            Ok(p) => p,
            Err(e) => {
                return Err(ReadError {
                    pointer: None,
                    address: 0,
                    error: e,
                })
            }
        };
        let h = rb.process_handle;

        let base = match rb.get_module_base("rekordbox.exe") {
            Ok(b) => b,
            Err(e) => {
                return Err(ReadError {
                    pointer: None,
                    address: 0,
                    error: e,
                })
            }
        };

        let current_bpms = Value::pointers_to_vals(h, base, &offsets.current_bpm[0..decks])?;
        let sample_positions =
            Value::pointers_to_vals(h, base, &offsets.sample_position[0..decks])?;
        let track_infos =
            PointerChainValue::pointers_to_vals(h, base, &offsets.track_info[0..decks]);
        let anlz_paths = PointerChainValue::pointers_to_vals(h, base, &offsets.anlz_path[0..decks]);

        let deckcount = current_bpms.len();

        let masterdeck_index_val: Value<u8> = Value::new(h, base, &offsets.masterdeck_index)?;

        Ok(Self {
            current_bpms,
            sample_positions,
            masterdeck_index: masterdeck_index_val,
            deckcount,
            track_infos,
            anlz_paths,
        })
    }

    fn read_timing_data(&self, deck: usize) -> Result<TimingDataRaw, ReadError> {
        let sample_position = self.sample_positions[deck].read()?;
        let current_bpm = self.current_bpms[deck].read()?;

        Ok(TimingDataRaw {
            current_bpm,
            sample_position,
        })
    }

    fn read_masterdeck_index(&self) -> Result<usize, ReadError> {
        Ok(self.masterdeck_index.read()? as usize)
    }

    fn get_track_infos(&self) -> Result<Vec<TrackInfo>, ReadError> {
        (0..self.deckcount)
            .map(|i| {
                let raw = self.track_infos[i]
                    .read()?
                    .into_iter()
                    .take_while(|x| *x != 0x00)
                    .collect::<Vec<u8>>();
                let text = String::from_utf8(raw).unwrap_or_else(|_| "ERR".to_string());
                let mut lines = text
                    .lines()
                    .map(|x| x.split_once(": ").unwrap_or(("", "")).1)
                    .map(|x| x.to_string());

                Ok(TrackInfo {
                    title: lines.next().unwrap_or("".to_string()),
                    artist: lines.next().unwrap_or("".to_string()),
                    album: lines.next().unwrap_or("".to_string()),
                })
            })
            .collect()
    }

    fn get_anlz_paths(&self) -> Result<Vec<String>, ReadError> {
        (0..self.deckcount)
            .map(|i| {
                let raw = self.anlz_paths[i]
                    .read()?
                    .into_iter()
                    .take_while(|x| *x != 0x00)
                    .collect::<Vec<u8>>();
                Ok(String::from_utf8(raw).unwrap_or_else(|_| "ERR".to_string()))
            })
            .collect()
    }
}

#[derive(Debug)]
struct TimingDataRaw {
    current_bpm: f32,
    sample_position: i64,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
}
impl Default for TrackInfo {
    fn default() -> Self {
        Self {
            title: "".to_string(),
            artist: "".to_string(),
            album: "".to_string(),
        }
    }
}

#[derive(Clone)]
struct ChangeTrackedValue<T> {
    value: T,
}
impl<T: std::cmp::PartialEq> ChangeTrackedValue<T> {
    fn new(value: T) -> Self {
        Self { value }
    }
    fn set(&mut self, value: T) -> bool {
        if self.value != value {
            self.value = value;
            true
        } else {
            false
        }
    }
}

pub struct BeatKeeper {
    masterdeck_index: ChangeTrackedValue<usize>,
    offset_samples: i64,
    running_modules: Vec<Box<dyn OutputModule>>,

    track_infos: Vec<ChangeTrackedValue<TrackInfo>>,
    track_trackers: Vec<TrackTracker>,

    anlz_paths: Vec<ChangeTrackedValue<String>>,

    logger: ScopedLogger,
    last_error: Option<ReadError>,
    keep_warm: bool,
    decks: usize,


    td_trackers: Vec<TrackingDataTracker>,
    master_td_tracker: TrackingDataTracker,
}

struct TrackingDataTracker {
    bpm_changed: ChangeTrackedValue<f32>,
    original_bpm_changed: ChangeTrackedValue<f32>,
    beat_changed: ChangeTrackedValue<f32>,
    pos_changed: ChangeTrackedValue<i64>,
}

impl TrackingDataTracker {
    fn new() -> Self {
        Self {
            bpm_changed: ChangeTrackedValue::new(0.),
            original_bpm_changed: ChangeTrackedValue::new(0.),
            beat_changed: ChangeTrackedValue::new(0.),
            pos_changed: ChangeTrackedValue::new(0),
        }
    }
}

impl BeatKeeper {
    pub fn start(
        offsets: RekordboxOffsets,
        modules: Vec<ModuleDefinition>,
        config: Config,
        logger: ScopedLogger,
    ) {
        let keeper_config = config.reduce_to_namespace("keeper");
        let update_rate = keeper_config.get_or_default("update_rate", 50);
        let slow_update_denominator = keeper_config.get_or_default("slow_update_every_nth", 50);

        let mut running_modules = vec![];

        logger.info("Active modules:");
        for module in modules {
            if !config.get_or_default(&format!("{}.enabled", module.config_name), false) {
                continue;
            }
            logger.info(&format!(" - {}", module.pretty_name));

            let conf = config.reduce_to_namespace(&module.config_name);
            match (module.create)(conf, ScopedLogger::new(&logger.logger, &module.pretty_name)) {
                Ok(module) => {
                    running_modules.push(module);
                }
                Err(()) => {
                    logger.err(&format!("Failed to start module {}", module.pretty_name));
                }
            }
        }

        let mut keeper = BeatKeeper {
            masterdeck_index: ChangeTrackedValue::new(0),
            offset_samples: (keeper_config.get_or_default("delay_compensation", 0.) * 44100. / 1000.) as i64,
            track_infos: vec![ChangeTrackedValue::new(Default::default()); 4],
            running_modules,
            logger: logger.clone(),
            last_error: None,
            track_trackers: (0..4).map(|_| TrackTracker::new()).collect(),
            keep_warm: keeper_config.get_or_default("keep_warm", true),
            decks: keeper_config.get_or_default("decks", 4),
            td_trackers: (0..4).map(|_| TrackingDataTracker::new()).collect(),
            master_td_tracker: TrackingDataTracker::new(),
            anlz_paths: vec![ChangeTrackedValue::new("".to_string()); 4],
        };

        let mut rekordbox = None;

        let period = Duration::from_micros(1000000 / update_rate); // 50Hz
        let mut n = 0;

        logger.info("Looking for Rekordbox...");
        println!();

        loop {
            if let Some(rb) = &rekordbox {
                let update_start_time = std::time::Instant::now();
                if let Err(e) = keeper.update(rb, n == 0) {
                    keeper.report_error(e);

                    rekordbox = None;
                    logger.err("Connection to Rekordbox lost");
                    logger.info("Reconnecting in 3s...");
                    thread::sleep(Duration::from_secs(3));
                } else {
                    n = (n + 1) % slow_update_denominator;
                    if period > update_start_time.elapsed() {
                        thread::sleep(period - update_start_time.elapsed());
                    }
                }
            } else {
                match Rekordbox::new(offsets.clone(), config.get_or_default("keeper.decks", 2)) {
                    Ok(rb) => {
                        rekordbox = Some(rb);
                        println!();
                        logger.good("Connected to Rekordbox!");
                        keeper.last_error = None;
                    }
                    Err(e) => {
                        keeper.report_error(e);
                        logger.info("...");
                        thread::sleep(Duration::from_secs(3));
                    }
                }
            }
        }
    }

    fn report_error(&mut self, e: ReadError) {
        if let Some(last) = &self.last_error {
            if e == *last {
                return;
            }
        }
        match &e.error {
            TAExternalError::ProcessNotFound | TAExternalError::ModuleNotFound => {
                self.logger.err("Rekordbox process not found!");
            }
            TAExternalError::SnapshotFailed(e) => {
                self.logger.err(&format!("Snapshot failed: {e}"));
                self.logger.info("    Ensure Rekordbox is running!");
            }
            TAExternalError::ReadMemoryFailed(e) => {
                self.logger.err(&format!("Read memory failed: {e}"));
                self.logger.info("    Try the following:");
                self.logger
                    .info("    - Wait for Rekordbox to start and load a track");
                self.logger.info(
                    "    - Ensure you have selected the correct Rekordbox version in the config",
                );
                self.logger
                    .info("    - Check the number of decks in the config");
                self.logger.info("    - Update the offsets and program");
                self.logger.info("    If nothing works, wait for an update, or enable Debug in config and submit this entire error message on an Issue on GitHub.");
            }
            TAExternalError::WriteMemoryFailed(e) => {
                self.logger.err(&format!("Write memory failed: {e}"));
            }
        };
        if let Some(p) = &e.pointer {
            self.logger.debug(&format!("Pointer: {p}"));
        }
        if e.address != 0 {
            self.logger.debug(&format!("Address: {:X}", e.address));
        }
        self.last_error = Some(e);
    }

    fn update(
        &mut self,
        rb: &Rekordbox,
        slow_update: bool,
    ) -> Result<(), ReadError> {
        // let masterdeck_index_changed = self.masterdeck_index.set(td.masterdeck_index as usize);
        let masterdeck_index_changed = self.masterdeck_index.set(rb.read_masterdeck_index()?);
        if self.masterdeck_index.value >= rb.deckcount {
            return Ok(()); // No master deck selected - rekordbox is not initialised
        }

        // let mut tracker_data = None;

        for (i, (tracker, td_tracker)) in (self.track_trackers[0..self.decks])
            .iter_mut()
            .zip(self.td_trackers[0..self.decks].iter_mut())
            .enumerate()
        {
            let is_master = i == self.masterdeck_index.value;
            if is_master | self.keep_warm {
                let res =
                    tracker.update(rb, self.offset_samples, i);
                let Ok(res) = res else {
                    continue;
                };

                let bpm_changed = td_tracker.bpm_changed.set(res.timing_data_raw.current_bpm);
                let original_bpm_changed = td_tracker.original_bpm_changed.set(res.original_bpm);
                let beat_changed = td_tracker.beat_changed.set(res.beat);
                let pos_changed = td_tracker.pos_changed.set(res.timing_data_raw.sample_position);

                for module in &mut self.running_modules {
                    if beat_changed {
                        module.beat_update(res.beat, i);
                    }
                    if pos_changed {
                        module.time_update(res.timing_data_raw.sample_position as f32 / 44100., i);
                    }
                    if bpm_changed {
                        module.bpm_changed(res.timing_data_raw.current_bpm, i);
                    }
                    if original_bpm_changed {
                        module.original_bpm_changed(res.original_bpm, i);
                    }
                }

                if is_master {
                    let bpm_changed = self
                        .master_td_tracker
                        .bpm_changed
                        .set(res.timing_data_raw.current_bpm);
                    let original_bpm_changed = self
                        .master_td_tracker
                        .original_bpm_changed
                        .set(res.original_bpm);
                    let beat_changed = self.master_td_tracker.beat_changed.set(res.beat);
                    let pos_changed = self
                        .master_td_tracker
                        .pos_changed
                        .set(res.timing_data_raw.sample_position);

                    for module in &mut self.running_modules {
                        if beat_changed {
                            module.beat_update_master(res.beat);
                        }
                        if pos_changed {
                            module.time_update_master(
                                res.timing_data_raw.sample_position as f32 / 44100.,
                            );
                        }
                        if bpm_changed {
                            module.bpm_changed_master(res.timing_data_raw.current_bpm);
                        }
                        if original_bpm_changed {
                            module.original_bpm_changed_master(res.original_bpm);
                        }
                    }
                }

                // if i == self.masterdeck_index.value{
                //     match res {
                //         Ok(res) => {
                //             tracker_data = Some(res);
                //         }
                //         Err(e) => {
                //             return Err(e);
                //         },
                //     }
                // }
            }
        }

        // if let Some(tracker_data) = tracker_data {
        //     // for _ in 0..((tracker_data.beat * 10. % (16. * 10.)) as usize){
        //     //     print!("#");
        //     // }
        //     // println!();
        //     // println!("{}", tracker_data.beat);
        //
        //     let bpm_changed = self.bpm.set(tracker_data.timing_data_raw.current_bpm);
        //     let original_bpm_changed = self.original_bpm.set(tracker_data.original_bpm);
        //     let playback_speed_changed = self.playback_speed.set(tracker_data.timing_data_raw.playback_speed);
        //     let beat_changed = self.last_beat.set(tracker_data.beat);
        //     let pos_changed = self.last_pos.set(tracker_data.timing_data_raw.sample_position);
        //
        //     for module in &mut self.running_modules {
        //         if beat_changed{
        //             module.beat_update(tracker_data.beat);
        //         }
        //         if pos_changed{
        //             module.time_update(tracker_data.timing_data_raw.sample_position as f32 / 44100.);
        //         }
        //         if bpm_changed {
        //             module.bpm_changed(self.bpm.value);
        //         }
        //         if original_bpm_changed {
        //             module.original_bpm_changed(self.original_bpm.value);
        //         }
        //
        //         if playback_speed_changed {
        //             module.playback_speed_changed(self.playback_speed.value);
        //         }
        //     }
        // }else{
        //     println!("ERRRRR");
        // }

        let mut masterdeck_track_changed = false;

        if slow_update {
            for (i, track) in rb.get_track_infos()?.into_iter().enumerate() {
                if self.track_infos[i].set(track) {
                    for module in &mut self.running_modules {
                        module.track_changed(&self.track_infos[i].value, i);
                    }
                    self.track_trackers[i].track_changed = true;
                    masterdeck_track_changed |= self.masterdeck_index.value == i;
                }
            }
            for (i, path) in rb.get_anlz_paths()?.into_iter().enumerate() {
                if self.anlz_paths[i].set(path) {
                    println!("Anlz path changed: {}", &self.anlz_paths[i].value);
                    let Ok(bytes) = std::fs::read(&self.anlz_paths[i].value) else {
                        println!("Failed to read anlz file: {}", &self.anlz_paths[i].value);
                        continue;
                    };
                    let mut reader = Cursor::new(bytes);
                    let anlz = rekordcrate::anlz::ANLZ::read(&mut reader).unwrap();
                    for section in anlz.sections {
                        match section.content {
                            anlz::Content::BeatGrid(grid) => {
                                self.track_trackers[i].beatgrid = Some(grid);
                            }
                            anlz::Content::SongStructure(phrases) => {}
                            _ => (),
                        }
                    }
                }
            }
            for module in &mut self.running_modules {
                module.slow_update();
            }
        }

        if masterdeck_index_changed || masterdeck_track_changed {
            let track = &self.track_infos[self.masterdeck_index.value].value;
            self.logger
                .debug(&format!("Master track changed: {track:?}"));
            for module in &mut self.running_modules {
                module.track_changed_master(track);
            }
        }

        Ok(())
    }
}

struct TrackTrackerResult {
    beat: f32,
    original_bpm: f32,
    timing_data_raw: TimingDataRaw,
}

struct TrackTracker {
    track_changed: bool,              // External flag to indicate that the track has changed
    beatgrid: Option<BeatGrid>,
}

impl TrackTracker {
    fn new() -> Self {
        Self {
            track_changed: false,
            beatgrid: None,
        }
    }

    fn update(
        &mut self,
        rb: &Rekordbox,
        offset_samples: i64,
        deck: usize,
    ) -> Result<TrackTrackerResult, ReadError> {
        let mut td = rb.read_timing_data(deck)?;
        if td.current_bpm == 0.0 {
            td.current_bpm = 120.0;
        }

        let mut beat = 0.0;
        let mut original_bpm = 120.0;

        let time_now = (td.sample_position + offset_samples) as f32 / 44100.;
        if let Some(grid) = &self.beatgrid {
            let mut idx: usize = 0;
            for gridbeat in grid.beats.iter() {
                if gridbeat.time as f32 / 1000. >= time_now {
                    break;
                }
                idx += 1;
            }
            idx = idx.saturating_sub(1);
            let gridbeat = &grid.beats[idx];
            // println!("{} - {}", time, time_now);
            let remainder = time_now - gridbeat.time as f32 / 1000.;
            original_bpm = gridbeat.tempo as f32 / 100.0;
            let spb = 1. / (gridbeat.tempo as f32 / 100. / 60.0);

            let b = (gridbeat.beat_number + 3) % 4;
            // println!("{b} {idx}");
            beat = b as f32 + remainder / spb;
        }

        Ok(TrackTrackerResult {
            beat,
            original_bpm,
            timing_data_raw: td,
        })
    }
}
