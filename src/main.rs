use rosc::{encoder::encode, OscMessage, OscPacket, OscType};
use rusty_link::{AblLink, SessionState};
use std::process::Command;
use std::{
    env,
    io::{stdout, Write},
    marker::PhantomData,
    net::UdpSocket,
    path::Path,
    sync::mpsc::channel,
    thread::{sleep, spawn},
    time::{Duration, Instant},
};
use toy_arms::external::{read, Process};
use winapi::um::winnt::HANDLE;

mod offsets;
use offsets::{Pointer, RekordboxOffsets};

extern "C" {
    fn _getch() -> core::ffi::c_char;
}

fn getch() -> i8 {
    unsafe { _getch() }
}

struct Value<T> {
    address: usize,
    handle: HANDLE,
    _marker: PhantomData<T>,
}

impl<T> Value<T> {
    fn new(h: HANDLE, base: usize, offsets: Pointer) -> Value<T> {
        let mut address = base;

        for offset in &offsets.offsets {
            address = read::<usize>(h, address + offset)
                .expect(&format!("\nMemory read failed, check your Rekordbox version! Try updating with -u.\nIf nothing works, wait for an update or send this entire error message to @grufkork. \n\nBase: {base:X}, Offsets: {offsets}"));
        }
        address += offsets.final_offset;

        Value::<T> {
            address,
            handle: h,
            _marker: PhantomData::<T>,
        }
    }

    fn read(&self) -> T {
        read::<T>(self.handle, self.address).unwrap()
    }
}

pub struct Rekordbox {
    master_bpm_val: Value<f32>,
    bar1_val: Value<i32>,
    beat1_val: Value<i32>,
    bar2_val: Value<i32>,
    beat2_val: Value<i32>,
    masterdeck_index_val: Value<u8>,

    pub bars1: i32,
    pub bars2: i32,
    pub beats1: i32,
    pub beats2: i32,
    pub master_bars: i32,
    pub master_beats: i32,
    pub master_bpm: f32,
    pub masterdeck_index: u8,
}

impl Rekordbox {
    fn new(offsets: RekordboxOffsets) -> Self {
        let rb = Process::from_process_name("rekordbox.exe")
            .expect("Could not find Rekordbox process! ");
        let h = rb.process_handle;

        let base = rb.get_module_base("rekordbox.exe").unwrap();

        let master_bpm_val: Value<f32> = Value::new(h, base, offsets.master_bpm);

        let bar1_val: Value<i32> = Value::new(h, base, offsets.deck1bar);
        let beat1_val: Value<i32> = Value::new(h, base, offsets.deck1beat);
        let bar2_val: Value<i32> = Value::new(h, base, offsets.deck2bar);
        let beat2_val: Value<i32> = Value::new(h, base, offsets.deck2beat);

        let masterdeck_index_val: Value<u8> = Value::new(h, base, offsets.masterdeck_index);

        Self {
            master_bpm_val,
            bar1_val,
            beat1_val,
            bar2_val,
            beat2_val,

            masterdeck_index_val,

            bars1: -1,
            bars2: -1,
            beats1: -1,
            beats2: -1,
            master_bpm: 120.0,
            masterdeck_index: 0,
            master_bars: 0,
            master_beats: 0,
        }
    }

    fn update(&mut self) {
        self.master_bpm = self.master_bpm_val.read();
        self.bars1 = self.bar1_val.read();
        self.bars2 = self.bar2_val.read();
        self.beats1 = self.bar1_val.read() * 4 + self.beat1_val.read();
        self.beats2 = self.bar2_val.read() * 4 + self.beat2_val.read();
        self.masterdeck_index = self.masterdeck_index_val.read();
        
        self.master_bars = if self.masterdeck_index == 0 {
            self.bars1
        } else {
            self.beats2
        };

        self.master_beats = if self.masterdeck_index == 0 {
            self.beats1
        } else {
            self.beats2
        };
        
    }
}

pub struct BeatKeeper {
    rb: Option<Rekordbox>,
    last_bar: i32,
    last_beat: i32,
    pub beat_fraction: f32,
    pub last_masterdeck_index: u8,
    pub offset_micros: f32,
    pub last_bpm: f32,
    pub new_bar: bool,
    pub new_beat: bool,
}

impl BeatKeeper {
    pub fn new(offsets: RekordboxOffsets) -> Self {
        BeatKeeper {
            rb: Some(Rekordbox::new(offsets)),
            last_bar: 0,
            last_beat: 0,
            beat_fraction: 1.,
            last_masterdeck_index: 0,
            offset_micros: 0.,
            last_bpm: 0.,
            new_bar: false,
            new_beat: false,
        }
    }

    pub fn dummy() -> Self {
        BeatKeeper {
            rb: None,
            last_bar: 0,
            last_beat: 0,
            beat_fraction: 1.,
            last_masterdeck_index: 0,
            offset_micros: 0.,
            last_bpm: 0.,
            new_bar: false,
            new_beat: false,
        }
    }

    pub fn update(&mut self, delta: Duration) {
        if let Some(rb) = &mut self.rb {
            let beats_per_micro = rb.master_bpm / 60. / 1000000.;

            rb.update(); // Fetch values from rkbx memory

            if rb.masterdeck_index != self.last_masterdeck_index {
                self.last_masterdeck_index = rb.masterdeck_index;
                self.last_beat = rb.master_beats;
            }

            if (rb.master_bars - self.last_bar).abs() > 0 {
                self.last_bar = rb.master_bars;
                self.new_bar = true;
            }

            if (rb.master_beats - self.last_beat).abs() > 0 {
                self.last_beat = rb.master_beats;
                self.beat_fraction = 0.;
                self.new_beat = true;
            }
            self.beat_fraction =
                (self.beat_fraction + delta.as_micros() as f32 * beats_per_micro) % 1.;
        } else {
            self.beat_fraction = (self.beat_fraction + delta.as_secs_f32() * 130. / 60.) % 1.;
        }
    }
    pub fn get_beat_faction(&mut self) -> f32 {
        (self.beat_fraction
            + if let Some(rb) = &self.rb {
                let beats_per_micro = rb.master_bpm / 60. / 1000000.;
                self.offset_micros * beats_per_micro
            } else {
                0.
            }
            + 1.)
            % 1.
    }

    pub fn get_bpm_changed(&mut self) -> Option<f32> {
        if let Some(rb) = &self.rb {
            if rb.master_bpm != self.last_bpm {
                self.last_bpm = rb.master_bpm;
                return Some(rb.master_bpm);
            }
        }
        None
    }

    pub fn get_last_bar(&mut self) -> i32 {self.last_bar}

    pub fn get_new_beat(&mut self) -> bool {
        if self.new_beat {
            self.new_beat = false;
            return true;
        }
        false
    }

    pub fn change_beat_offset(&mut self, offset: f32) {
        self.offset_micros += offset;
    }
}

const CHARS: [&str; 4] = ["|", "/", "-", "\\"];

fn main() {
    if !Path::new("./offsets").exists() {
        println!("Offsets not found, downloading from repo...");
        download_offsets();
    }

    let (tx, rx) = channel::<i8>();
    spawn(move || loop {
        tx.send(getch()).unwrap();
    });

    let args: Vec<String> = env::args().collect();

    let mut source_address = "0.0.0.0:0".to_string();
    let mut target_address = "127.0.0.1:6669".to_string();

    let mut osc_enabled = false;

    let version_offsets = RekordboxOffsets::from_file("offsets");
    let mut versions: Vec<String> = version_offsets.keys().map(|x| x.to_string()).collect();
    versions.sort();
    versions.reverse();
    let mut target_version = versions[0].clone();

    let mut args_iter = args.iter();
    args_iter.next();
    while let Some(arg) = args_iter.next() {
        let mut chars = arg.chars();
        if let Some(char) = chars.next() {
            if char == '-' {
                if let Some(flag) = chars.next() {
                    match flag.to_string().as_str() {
                        "u" => {
                            println!("Updating offsets...");
                            download_offsets();
                            return;
                        }
                        "o" => {
                            osc_enabled = true;
                        }
                        "s" => {
                            source_address = args_iter.next().unwrap().to_string();
                        }
                        "t" => {
                            target_address = args_iter.next().unwrap().to_string();
                        }
                        "v" => {
                            target_version = args_iter.next().unwrap().to_string();
                        }
                        "h" => {
                            println!(
                                " - Rekordbox OSC v{} -
A tool for sending Rekordbox timing data to visualizers using OSC

Flags:

 -h  Print this help
 -u  Fetch latest offset list from GitHub and exit
 -v  Rekordbox version to target, eg. 6.7.3

-- OSC --
 -o  Enable OSC
 -s  Source address, eg. 127.0.0.1:1337
 -t  Target address, eg. 192.168.1.56:6667

Use i/k to change the beat offset by +/- 1ms

Current default version: {}
Available versions:",
                                env!("CARGO_PKG_VERSION"),
                                versions[0]
                            );
                            println!("{}", versions.join(", "));

                            /*for v in  {
                                print!("{v}, ");
                            }*/
                            println!();
                            return;
                        }

                        c => {
                            println!("Unknown flag -{c}");
                        }
                    }
                }
            }
        }
    }

    let offsets = if let Some(offsets) = version_offsets.get(target_version.as_str()) {
        offsets
    } else {
        println!("Unsupported version! {target_version}");
        return;
    };
    println!("Targeting Rekordbox version {target_version}");

    let socket = if osc_enabled {
        println!("Connecting from: {}", source_address);
        println!("Connecting to:   {}", target_address);
        let socket = match UdpSocket::bind(&source_address) {
            Ok(socket) => socket,
            Err(e) => {
                println!("Failed to bind to address {source_address}. Error:\n{}", e);
                return;
            }
        };
        match socket.connect(&target_address) {
            Ok(_) => (),
            Err(e) => {
                println!(
                    "Failed to open socket to address {target_address}. Error:\n{}",
                    e
                );
                return;
            }
        };
        Some(socket)
    } else {
        None
    };

    println!();
    println!(
        "Press i/k to change offset in milliseconds. c to quit. -h flag for help and version info."
    );
    println!();

    let mut keeper = BeatKeeper::new(offsets.clone());
    let link = AblLink::new(120.);
    link.enable(false);

    let mut state = SessionState::new();
    link.capture_app_session_state(&mut state);
    link.enable(true);

    // Due to Windows timers having a default resolution 0f 15.6ms, we need to use a "too high"
    // value to acheive ~60Hz
    let period = Duration::from_micros(1000000 / 120);

    let mut last_instant = Instant::now();

    let mut count = 0;
    let mut step = 0;

    let mut stdout = stdout();

    println!("Entering loop");
    loop {
        let delta = Instant::now() - last_instant; // Is this timer accurate enough?
        last_instant = Instant::now();

        keeper.update(delta); // Get values, advance time

        let bfrac = keeper.get_beat_faction();
        let lbar: i32 = keeper.get_last_bar();

        if let Some(socket) = &socket {
            let msg = OscPacket::Message(OscMessage {
                addr: "/beat".to_string(),
                args: vec![OscType::Float(bfrac)],
            });
            let packet = encode(&msg).unwrap();
            socket.send(&packet[..]).unwrap();
        }

        if let Some(socket) = &socket {
            let msg = OscPacket::Message(OscMessage {
                addr: "/bar".to_string(),
                args: vec![OscType::Int(lbar)],
            });
            let packet = encode(&msg).unwrap();
            socket.send(&packet[..]).unwrap();
        }

        if let Some(bpm) = keeper.get_bpm_changed() {
            state.set_tempo(bpm.into(), link.clock_micros());
            link.commit_app_session_state(&state);

            if let Some(socket) = &socket {
                let msg = OscPacket::Message(OscMessage {
                    addr: "/bpm".to_string(),
                    args: vec![OscType::Float(bpm)],
                });
                let packet = encode(&msg).unwrap();
                socket.send(&packet[..]).unwrap();
            }
        }

        if keeper.get_new_beat() {
            let current_link_beat_approx = state.beat_at_time(link.clock_micros(), 4.).round();
            let target_beat = ((keeper.last_beat as f64) % 4. - current_link_beat_approx % 4. + 4.)
                % 4.
                + current_link_beat_approx
                - 1.; // Ensure the 1 is on the 1

            state.request_beat_at_time(target_beat, link.clock_micros(), 4.);
            link.commit_app_session_state(&state);
        }

        while let Ok(key) = rx.try_recv() {
            match key {
                99 => {
                    //"c"
                    return;
                }
                105 => {
                    keeper.change_beat_offset(1000.);
                }
                107 => {
                    keeper.change_beat_offset(-1000.);
                }
                _ => (),
            }
        }

        if count % 20 == 0 {
            step = (step + 1) % 4;

            let frac = (keeper.last_beat - 1) % 4;

            print!(
                "\rRunning {} [{}] Deck {}     OSC Offset: {}ms     Frq: {: >3}Hz    Peers:{}   BPM:{}    ",
                CHARS[step],
                (0..4)
                .map(|i| {
                    if i == frac {
                        "."
                    } else {
                        " "
                    }
                })
                .collect::<String>(),
                keeper.last_masterdeck_index,
                keeper.offset_micros / 1000.,
                1000000 / (delta.as_micros().max(1)),
                link.num_peers(),
                keeper.last_bpm
                );

            stdout.flush().unwrap();
        }
        count = (count + 1) % 120;

        sleep(period);
    }
}

fn download_offsets() {
    match Command::new("curl")
        .args([
            "-o",
            "offsets",
            "https://raw.githubusercontent.com/grufkork/rkbx_osc/master/offsets",
        ])
        .output()
    {
        Ok(output) => {
            println!("{}", String::from_utf8(output.stdout).unwrap());
            println!("{}", String::from_utf8(output.stderr).unwrap());
        }
        Err(error) => println!("{}", error),
    }
    println!("Done!");
}
