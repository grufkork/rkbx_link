use std::collections::VecDeque;
use std::thread;
use crate::config::Config;
use crate::log::ScopedLogger;
use crate::outputmodules::ModuleDefinition;
use crate::outputmodules::OutputModule;
use std::{marker::PhantomData, time::Duration};
use crate::offsets::Pointer;
use toy_arms::external::error::TAExternalError;
use toy_arms::external::{read, Process};
use crate::RekordboxOffsets;
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
            address = match read::<usize>(h, address + offset){
                Ok(val) => val,
                Err(e) => return Err(ReadError{pointer: Some(offsets.clone()), address: address+offset, error: e}),
            }
        }
        address += offsets.final_offset;

        Ok(Value::<T> {
            address,
            handle: h,
            _marker: PhantomData::<T>,
        })
    }
    fn pointers_to_vals(h: *mut c_void, base: usize, pointers: &[Pointer]) -> Result<Vec<Value<T>>, ReadError> {
        pointers
            .iter()
            .map(|x| {Value::new(h, base, x)})
            .collect()
    }

    fn read(&self) -> Result<T, ReadError> {
        match read::<T>(self.handle, self.address){
            Ok(val) => Ok(val),
            Err(e) => Err(ReadError{pointer: None, address:self.address, error: e}),
        }
    }
}

struct PointerChainValue<T> {
    handle: *mut c_void,
    base: usize,
    pointer: Pointer,
    _marker: PhantomData<T>,
}

impl<T> PointerChainValue<T>{
    fn new(h: *mut c_void, base: usize, pointer: Pointer) -> PointerChainValue<T>{
        Self{
            handle: h,
            base,
            pointer,
            _marker: PhantomData::<T>,
        }
    }

    fn pointers_to_vals(h: *mut c_void, base: usize, pointers: &[Pointer]) -> Vec<PointerChainValue<T>> {
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
    playback_speeds: Vec<Value<f32>>,
    beat_displays: Vec<Value<i32>>,
    bar_displays: Vec<Value<i32>>,
    sample_positions: Vec<Value<i64>>,
    track_infos: Vec<PointerChainValue<[u8; 200]>>,
    anlz_files: Vec<PointerChainValue<[u8; 200]>>,
    deckcount: usize,
}



impl Rekordbox {
    fn new(offsets: RekordboxOffsets, decks: usize) -> Result<Self, ReadError> {
        let rb = match Process::from_process_name("rekordbox.exe"){
            Ok(p) => p,
            Err(e) => return Err(ReadError{pointer: None, address: 0, error: e}),
        };
        let h = rb.process_handle;


        let base = match rb.get_module_base("rekordbox.exe"){
            Ok(b) => b,
            Err(e) => return Err(ReadError{pointer: None, address: 0, error: e}),
        };


        let current_bpms = Value::pointers_to_vals(h, base, &offsets.current_bpm[0..decks])?;
        let playback_speeds = Value::pointers_to_vals(h, base, &offsets.playback_speed[0..decks])?;
        let beat_displays = Value::pointers_to_vals(h, base, &offsets.beat_display[0..decks])?;
        let bar_displays = Value::pointers_to_vals(h, base, &offsets.bar_display[0..decks])?;
        let sample_positions = Value::pointers_to_vals(h, base, &offsets.sample_position[0..decks])?;
        let track_infos = PointerChainValue::pointers_to_vals(h, base, &offsets.track_info[0..decks]);
        let anlz_files = PointerChainValue::pointers_to_vals(h, base, &offsets.anlz_file[0..decks]);

        let deckcount = current_bpms.len();

        let masterdeck_index_val: Value<u8> = Value::new(h, base, &offsets.masterdeck_index)?;

        Ok(Self {
            current_bpms,
            playback_speeds,
            beat_displays,
            bar_displays,
            sample_positions,
            masterdeck_index: masterdeck_index_val,
            deckcount,
            track_infos,
            anlz_files,
        })
    }

    fn read_timing_data(&self, deck: usize) -> Result<TimingDataRaw, ReadError> {
        let sample_position = self.sample_positions[deck].read()?;
        let beat = self.beat_displays[deck].read()?;
        let bar = self.bar_displays[deck].read()?;
        let current_bpm = self.current_bpms[deck].read()?;
        let playback_speed = self.playback_speeds[deck].read()?;

        Ok(TimingDataRaw{
            current_bpm,
            sample_position,
            playback_speed,
            beat,
            bar
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
                let text = String::from_utf8(raw).unwrap_or("ERR".to_string());
                let mut lines = text
                    .lines()
                    .map(|x| x.split_once(": ").unwrap_or(("", "")).1)
                    .map(|x| x.to_string());
                Ok(
                    TrackInfo {
                        title: lines.next().unwrap_or("".to_string()),
                        artist: lines.next().unwrap_or("".to_string()),
                        album: lines.next().unwrap_or("".to_string()),
                    }
                )
            })
        .collect()
    }

fn get_anlz_files(&self) -> Result<Vec<AnlzFile>, ReadError> {
    (0..self.deckcount)
        .map(|i| {
            let raw = self.anlz_files[i]
                .read()?
                .into_iter()
                .take_while(|x| *x != 0x00)
                .collect::<Vec<u8>>();

            let text = String::from_utf8(raw)
                .unwrap_or_else(|_| "ERR".to_string())
                .trim_end_matches(|c: char| c.is_whitespace() || c.is_ascii_control()).to_string();

            Ok(
                AnlzFile {
                    path: text,
                }
            )
        })
        .collect()
}

}

#[derive(Debug)]
struct TimingDataRaw{
    current_bpm: f32,
    sample_position: i64,
    beat: i32,
    bar: i32,
    playback_speed: f32,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
}
// Fügen Sie diese Zeile hinzu
#[derive(PartialEq, Default, Clone)]
pub struct AnlzFile {
    pub path: String,
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
    bpm: ChangeTrackedValue<f32>,
    original_bpm: ChangeTrackedValue<f32>,
    playback_speed: ChangeTrackedValue<f32>,
    running_modules: Vec<Box<dyn OutputModule>>,

    track_infos: Vec<ChangeTrackedValue<TrackInfo>>,
    anlz_files: Vec<ChangeTrackedValue<AnlzFile>>,
    track_trackers: Vec<TrackTracker>,

    logger: ScopedLogger,
    last_error: Option<ReadError>,
    bar_jitter_tolerance: i32, // Updates
    keep_warm: bool,
    decks: usize,

    last_beat: ChangeTrackedValue<f32>,
    last_pos: ChangeTrackedValue<i64>
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
            running_modules.push((module.create)(conf, ScopedLogger::new(&logger.logger, &module.pretty_name)));
        }

        let mut keeper = BeatKeeper {
            masterdeck_index: ChangeTrackedValue::new(0),
            offset_samples: (keeper_config.get_or_default("delay_compensation", 0.) * 44100. / 1000.) as i64,
            bpm: ChangeTrackedValue::new(120.),
            original_bpm: ChangeTrackedValue::new(120.),
            playback_speed: ChangeTrackedValue::new(1.),
            track_infos: vec![ChangeTrackedValue::new(Default::default()); 4],
            anlz_files: vec![ChangeTrackedValue::new(Default::default()); 4],
            running_modules,
            logger: logger.clone(),
            last_error: None,
            track_trackers: (0..4).map(|_| TrackTracker::new()).collect(),
            // last_beat: ChangeTrackedValue::new(1),
            // last_pos: 0,
            // grid_shift: 0,
            // new_bar_measurements: VecDeque::new(),
            // last_playback_speed: ChangeTrackedValue::new(1.),
            // measurements_since_bar_jump: 0,
            // last_calculated_beat: 0.0,
            bar_jitter_tolerance: keeper_config.get_or_default("bar_jitter_tolerance", 10), // seconds
            keep_warm: keeper_config.get_or_default("keep_warm", true),
            decks: keeper_config.get_or_default("decks", 4),
            last_beat: ChangeTrackedValue::new(0.0),
            last_pos: ChangeTrackedValue::new(0),


        };

        let mut rekordbox = None;

        let period = Duration::from_micros(1000000 / update_rate); // 50Hz
        let mut n = 0;

        logger.info("Looking for Rekordbox...");
        println!();

        let mut last_time = std::time::Instant::now();

        loop {
            if let Some(rb) = &rekordbox {
                let update_start_time = std::time::Instant::now();
                if let Err(e) = keeper.update(rb, n == 0, last_time.elapsed()) {
                    keeper.report_error(e);
                    
                    rekordbox = None;
                    logger.err("Connection to Rekordbox lost");
                    // thread::sleep(Duration::from_secs(3));
                    logger.info("Reconnecting...");

                }else{
                    n = (n + 1) % slow_update_denominator;
                    last_time = update_start_time;
                    if period > update_start_time.elapsed(){
                        thread::sleep(period - update_start_time.elapsed());
                    }
                }
            }else {
                match Rekordbox::new(offsets.clone(), config.get_or_default("keeper.decks", 2)){
                    Ok(rb) => {
                        rekordbox = Some(rb);
                        println!();
                        logger.good("Connected to Rekordbox!");
                        keeper.last_error = None;
                    },
                    Err(e) => {
                        keeper.report_error(e);
                        logger.info("...");
                        thread::sleep(Duration::from_secs(3));
                    }
                }
            }


        }
    }

    fn report_error(&mut self, e: ReadError){
        if let Some(last) = &self.last_error{
            if e == *last{
                return;
            }
        }
        match &e.error {
            TAExternalError::ProcessNotFound | TAExternalError::ModuleNotFound => {
                self.logger.err("Rekordbox process not found!");
            },
            TAExternalError::SnapshotFailed(e) => {
                self.logger.err(&format!("Snapshot failed: {e}"));
                self.logger.info("    Ensure Rekordbox is running!");
            },
            TAExternalError::ReadMemoryFailed(e) => {
                self.logger.err(&format!("Read memory failed: {e}"));
                self.logger.info("    Try the following:");
                self.logger.info("    - Wait for Rekordbox to start and load a track");
                self.logger.info("    - Ensure you have selected the correct Rekordbox version in the config");
                self.logger.info("    - Check the number of decks in the config");
                self.logger.info("    - Update the offsets and program");
                self.logger.info("    If nothing works, wait for an update, or enable Debug in config and submit this entire error message on an Issue on GitHub.");
            },
            TAExternalError::WriteMemoryFailed(e) => {
                self.logger.err(&format!("Write memory failed: {e}"));
            },
        };
        if let Some(p) = &e.pointer{
            self.logger.debug(&format!("Pointer: {p}"));
        }
        if e.address != 0{
            self.logger.debug(&format!("Address: {:X}", e.address));
        }
        self.last_error = Some(e);
    }

    fn update(&mut self, rb: &Rekordbox, slow_update: bool, delta: Duration) -> Result<(), ReadError> {
        // let masterdeck_index_changed = self.masterdeck_index.set(td.masterdeck_index as usize);
        let masterdeck_index_changed = self.masterdeck_index.set(rb.read_masterdeck_index()?);
        if self.masterdeck_index.value >= rb.deckcount {
            return Ok(()); // No master deck selected - rekordbox is not initialised
        }

        let mut tracker_data = None;

        for (i, tracker) in self.track_trackers[0..self.decks].iter_mut().enumerate() {
            // for (i, tracker) in trackers.iter_mut().enumerate() {
            if i == self.masterdeck_index.value || self.keep_warm{
                let res = tracker.update(rb, self.bar_jitter_tolerance, self.offset_samples, i, delta);

                if i == self.masterdeck_index.value{
                    match res {
                        Ok(res) => {
                            tracker_data = Some(res);
                        }
                        Err(e) => {
                            return Err(e);
                        },
                    }
                }
            }
        }

        if let Some(tracker_data) = tracker_data {
            // for _ in 0..((tracker_data.beat * 10. % (16. * 10.)) as usize){
            //     print!("#");
            // }
            // println!();
            // println!("{}", tracker_data.beat);

            let bpm_changed = self.bpm.set(tracker_data.timing_data_raw.current_bpm);
            let original_bpm_changed = self.original_bpm.set(tracker_data.original_bpm);
            let playback_speed_changed = self.playback_speed.set(tracker_data.timing_data_raw.playback_speed);
            let beat_changed = self.last_beat.set(tracker_data.beat);
            let pos_changed = self.last_pos.set(tracker_data.timing_data_raw.sample_position);

            for module in &mut self.running_modules {
                if beat_changed{
                    module.beat_update(tracker_data.beat);
                }
                if pos_changed{
                    module.time_update(tracker_data.timing_data_raw.sample_position as f32 / 44100.);
                }
                if bpm_changed {
                    module.bpm_changed(self.bpm.value);
                }
                if original_bpm_changed {
                    module.original_bpm_changed(self.original_bpm.value);
                }

                if playback_speed_changed {
                    module.playback_speed_changed(self.playback_speed.value);
                }
            }
        }else{
            println!("ERRRRR");
        }




        let mut masterdeck_track_changed = false;

        if slow_update{
            for (i, track) in rb.get_track_infos()?.iter().enumerate(){
                if self.track_infos[i].set(track.clone()){
                    for module in &mut self.running_modules {
                        module.track_changed(track.clone(), i);
                    }
                    self.track_trackers[i].track_changed = true;
                    masterdeck_track_changed |= self.masterdeck_index.value == i;
                }
            }
            for module in &mut self.running_modules{
                module.slow_update();
            }
        }

        // Neue Schleife für die .DAT-Dateipfade
        for (i, anlz_file) in rb.get_anlz_files()?.iter().enumerate(){
            // `set()` aktualisiert den Wert nur, wenn er sich geändert hat
            if self.anlz_files[i].set(anlz_file.clone()){
                // Hier können Sie Logik hinzufügen, wenn sich der Pfad ändert
                // z.B. eine Debug-Meldung ausgeben
                self.logger.debug(&format!("New .DAT file path for deck {}: {}", i + 1, anlz_file.path));
            }
        }

        if masterdeck_index_changed || masterdeck_track_changed {
            let track = &self.track_infos[self.masterdeck_index.value].value;
            self.logger.debug(&format!("Master track changed: {track:?}"));
            for module in &mut self.running_modules {
                module.master_track_changed(track);
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

    struct TrackTracker{
        last_original_bpm: f32,
        time_since_bpm_change: Duration,
        last_beat: ChangeTrackedValue<i32>, // Last beat read from GUI
        last_pos: i64,
        grid_shift: i64,
        new_bar_measurements: VecDeque<i64>,
        measurements_since_bar_jump: i32, // Loops since a bar-sized jump in beat was detected
        last_calculated_beat: f32, // Previous total calculated beat
        track_changed: bool, // External flag to indicate that the track has changed
    }

    impl TrackTracker {
        fn new() -> Self {
            Self {
                last_original_bpm: 120.,
                time_since_bpm_change: Duration::from_secs(0),
                last_beat: ChangeTrackedValue::new(1),
                last_pos: 0,
                grid_shift: 0,
                new_bar_measurements: VecDeque::new(),
                measurements_since_bar_jump: 0,
                last_calculated_beat: 0.0,
                track_changed: false,
            }
        }

        fn update(&mut self, rb: &Rekordbox, bar_jitter_tolerance: i32, offset_samples: i64, deck: usize, delta: Duration) -> Result<TrackTrackerResult, ReadError>{
            let mut td = rb.read_timing_data(deck)?;
            if td.current_bpm == 0.0{
                td.current_bpm = 120.0;
            }



            let original_bpm = td.current_bpm / td.playback_speed;
            let original_bpm_diff = original_bpm - self.last_original_bpm;


            // --- Update original BPM
            let mut original_bpm_changed = false;

            if original_bpm_diff.abs() > 0.001{
                // There's a delay between the value of the playback speed changing and the displayed BPM
                // changing, usually <0.1s. 
                if self.time_since_bpm_change.as_secs_f32() > 0.2 {
                    self.last_original_bpm = original_bpm;
                    original_bpm_changed = true;
                }
                self.time_since_bpm_change += delta;
            }else{
                self.time_since_bpm_change = Duration::from_secs(0);
            }

            // This flag is required, because if the tempo changes the grid shift must be recalculated
            // in the new BPM. Otherwise the grid shift assumes the previous tempo, while
            // seconds_since_last_measure is calculated in the new tempo causing a jump until it is
            // actually recalculated.
            let mut calculate_grid_shift = false;

            // --- Find grid offset
            // Clear the queue if the beat grid has changed, such as if:
            // - The master track has been changed
            // - The original BPM has been changed due to dynamic beat analysis or manual adjustment
            if original_bpm_changed {
                // Keep the latest measurement since it is still valid
                while self.new_bar_measurements.len() > 1{
                    self.new_bar_measurements.pop_front();
                }
                calculate_grid_shift = true;
            }
            if self.track_changed {
                self.new_bar_measurements.clear();
                self.track_changed = false;
            }

            let bps = self.last_original_bpm / 60.;
            let spb = 1. / bps;
            let samples_per_measure = (44100. * spb) as i64 * 4; // TODO: This can be zero, leading to division by zero errors when moduloing

            // How much playback position should have advanced since previous loop
            let expected_posdiff = (delta.as_micros() as f32 / 1_000_000. * 44100. * td.playback_speed) as i64;
            let posdiff = td.sample_position - self.last_pos;
            self.last_pos = td.sample_position;
            let expectation_error = (expected_posdiff - posdiff) as f32/expected_posdiff as f32;

            // If there's a new beat, playback has advanced forward and playback position advancement is not greater than +/- 50% of expected value
            if self.last_beat.set(td.beat) && posdiff > 0 && expectation_error.abs() < 0.5{
                // Subtract half of the time advancment, as that's the expected value.
                let shift = td.sample_position - posdiff/2 - ((td.beat - 1)as f32 * 44100. * spb) as i64;
                self.new_bar_measurements.push_back(shift);
                if self.new_bar_measurements.len() > 8{ // Number of new beats measurements to average
                    self.new_bar_measurements.pop_front();
                }

                calculate_grid_shift = true;
            }

            if calculate_grid_shift && !self.new_bar_measurements.is_empty(){
                // To avoid the seam problem when moduloing the values, center all measurements with
                // the assumption that the first value is good enough (should be +/- 1/update rate wrong)
                // This means that the queue must be cleared at any discontinuity in original BPM and
                // that any erroneous measurements must be filtered by looking at the change in playback
                // position
                let phase_shift_guess = samples_per_measure / 2 - self.new_bar_measurements.front().unwrap() % samples_per_measure;
                self.grid_shift = self.new_bar_measurements.iter().map(|x| (x + phase_shift_guess) % samples_per_measure).sum::<i64>() / self.new_bar_measurements.len() as i64 - phase_shift_guess;
            }



            // Sample position seems to always be counted as if the track is 44100Hz
            // - even when track or audio interface is 48kHz
            let seconds_since_new_measure = (td.sample_position - self.grid_shift + offset_samples) as f32 / 44100.;
            let subdivision = 4.;

            // println!("{}", td.bar);
            // println!("{}", (seconds_since_new_measure % (subdivision * spb)) * bps);

            let mut beat = ((seconds_since_new_measure) % (subdivision * spb)) * bps + (td.bar - (td.bar > 0) as i32)  as f32 * subdivision; 

            // The GUI does not update as frequently as the playback position. This means that reading
            // the bar offset from the GUI will not be accurate - it might change both before or after
            // the bar actually changes. If not accounted for, this means that for a split second
            // around the bar change the beat might jump 4 beats in either direction.
            // This might however trigger false positives for 1-bar loops/jumps, so we only ignore the
            // jump for a little while.
            let beat_diff = beat - self.last_calculated_beat;
            if (beat_diff.abs() - 4.0).abs() < 0.1{
                self.measurements_since_bar_jump += 1;
                if self.measurements_since_bar_jump < bar_jitter_tolerance{
                    beat -= beat_diff.signum() * 4.0;
                }
            }else{
                self.measurements_since_bar_jump = 0;
            }
            self.last_calculated_beat = beat;



            // Unadjusted tracks have shift = 0. Adjusted tracks that begin on the first beat, have shift = 1
            // Or maybe not, rather it looks like:
            // Unadjusted tracks have bar 1 = 0, adjusted tracks have bar 1 = 1
            // So unadjusted tracks have a lowest possible beat shift of 0, adjusted have 1

            if beat.is_nan(){
                beat = 0.0;
            }

            Ok(TrackTrackerResult {
                beat,
                original_bpm,
                timing_data_raw: td,
            })
        }
    }
