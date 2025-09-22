use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use sacn::packet::ACN_SDT_MULTICAST_PORT;
use sacn::source::SacnSource;

use crate::{config::Config, log::ScopedLogger};
use super::ModuleCreateOutput;
use super::OutputModule;

/// sACN (E1.31) output module
///
/// Config keys (with defaults):
/// - `source` (String): local bind address, e.g. "0.0.0.0:5569". Default: bind to 0.0.0.0 on ACN port+1 (5569).
/// - `mode` (String): "multicast" (default) or "unicast".
/// - `universe` (u16): sACN universe (1..=63999), default 1.
/// - `start_channel` (u16): DMX start/offset (1..=511), default 1. (We need 2 slots: beat count and BPM.)
/// - `targets` (String): comma-separated IPv4 list for unicast. Example: "192.168.0.50,192.168.0.51".
/// - `priority` (u8): sACN priority 1..200, default 100.
/// - `source_name` (String): up to 63 ASCII chars shown by receivers. Default: "rkbx_link".
///
/// Slot mapping (starting at `start_channel`):
/// - +0 : BPM (u8). Capped to 250. Values > 250 are sent as 250.
/// - +1 : Beat absolute counter (u8). Wraps 0..=255.
///
pub struct SACN {
    src: SacnSource,
    mode: Mode,
    targets: Vec<SocketAddr>,
    universe: u16,
    start_slot: usize, // 1..=511 (we need 2 slots)
    priority: u8,
    local_addr: SocketAddr,
    dmx: [u8; 513], // index 0 is start code = 0, then 512 DMX slots
    logger: ScopedLogger,
    last_beat_floor: i32,
    beat_counter: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode { Multicast, Unicast }

impl SACN 
{
    pub fn create(conf: Config, logger: ScopedLogger) -> ModuleCreateOutput {
        // Local bind address
        let source_name = conf.get_or_default("source_name", String::from("rkbx_link"));
        let bind_str: Option<String> = conf.get("source");
        let local_addr = match bind_str {
            Some(s) => {
                if s.contains(':') {
                    match s.parse::<SocketAddr>() {
                        Ok(addr) => addr,
                        Err(e) => {
                            logger.err(&format!("Invalid sACN bind addr '{}': {}", s, e));
                            return Err(());
                        }
                    }
                } else {
                    match s.parse::<IpAddr>() {
                        Ok(ip) => SocketAddr::new(ip, 0),
                        Err(e) => {
                            logger.err(&format!("Invalid sACN bind IP '{}': {}", s, e));
                            return Err(());
                        }
                    }
                }
            }
            None => {
                logger.warn("source not specified, defaulting to 0.0.0.0:0");
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
            },
        };

        let mut src = match SacnSource::with_ip(&source_name, local_addr) {
            Ok(src) => src,
            Err(e) => {
                logger.err(&format!("Failed to create SacnSource: {}", e));
                return Err(());
            }
        };

        // Mode
        let mode_str = conf.get_or_default("mode", String::from("multicast"));
        let mode = match mode_str.to_ascii_lowercase().as_str() {
            "unicast" => Mode::Unicast,
            "multicast" => Mode::Multicast,
            _ => {
                logger.warn("unknown mode set, using Multicast");
                Mode::Multicast
            }
        };

        // Universe
        let mut universe: u16 = conf.get_or_default("universe", 1u16);
        if universe == 0 {
            logger.warn("Universe 0 is invalid, using 1");
            universe = 1;
        }
        if let Err(e) = src.register_universe(universe) {
            logger.err(&format!("register_universe failed: {}", e));
            return Err(());
        }

        // Start slot (1-511 so we have 2 slots available)
        let mut start_slot: usize = conf.get_or_default("start_channel", 1u16) as usize;
        if start_slot < 1 {
            logger.warn("start_channel < 1 invalid, using 1");
            start_slot = 1;
        }
        if start_slot > 511 {
            logger.warn("start_channel > 511 invalid, using 511");
            start_slot = 511;
        }

        // Priority
        let mut priority: u8 = conf.get_or_default("priority", 100u8);
        if priority < 1 {
            logger.warn("priority < 1 invalid, using 1");
            priority = 1;
        }
        if priority > 200 {
            logger.warn("priority > 200 invalid, using 200");
            priority = 200;
        }

        // Targets
        let mut targets: Vec<SocketAddr> = Vec::new();
        if matches!(mode, Mode::Unicast) {
            let list = conf.get_or_default("targets", String::new());
            for ip in list.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                // Default to the standard ACN port if no port was given
                let sa = if ip.contains(':') { ip.to_string() } else { format!("{}:{}", ip, ACN_SDT_MULTICAST_PORT) };
                if let Ok(sa) = sa.parse::<SocketAddr>() { 
                    targets.push(sa); 
                } else {
                    logger.err(&format!("Invalid sACN target address '{}'", ip));
                }
            }
        }
        logger.info(&format!(
            "sACN config: priority={}, start_slot={}, universe={}, mode={}, local_addr={}, targets={:?}",
            priority,
            start_slot,
            universe,
            mode_str,
            local_addr,
            targets
        ));

        // DMX buffer (start code + 512 slots)
        let mut dmx = [0u8; 513];
        dmx[0] = 0x00; // start code

        Ok(Box::new(SACN {
            src,
            mode,
            targets,
            universe,
            start_slot,
            priority,
            local_addr,
            dmx,
            logger,
            last_beat_floor: i32::MIN,
            beat_counter: 0,
        }))
    }

    fn send(&mut self) {
        //only send up to the bytes we actually use (using a low start_slot prevents sending the whole universe on update)
        let last_slot = (self.start_slot + 1).min(512);
        let len = 1 + last_slot; // +1 for start code
        let data: &[u8] = &self.dmx[..len];

        match self.mode {
            Mode::Multicast => {
                let _ = self
                    .src
                    .send(&[self.universe], data, Some(self.priority), None, None);
                }
            Mode::Unicast => {
                for &dst in &self.targets {
                    let _ = self
                        .src
                        .send(&[self.universe], data, Some(self.priority), Some(dst), None);
                    }
            }
        }

        match self.mode {
            Mode::Multicast => {
                self.logger.debug(&format!(
                    "sending multicast @{} -> universe {} ({} bytes)",
                    self.local_addr, self.universe, len
                ));
            }
            Mode::Unicast => {
                self.logger.debug(&format!(
                    "sending unicast @{} -> {} targets, universe {} ({} bytes)",
                    self.local_addr, self.targets.len(), self.universe, len
                ));
            }
        }
    }



    #[inline]
    fn write_u8_slot(&mut self, slot_1based: usize, value: u8) {
        // DMX slots live at dmx[1..=512]. slot_1based in 1..=512
        if (1..=512).contains(&slot_1based) {
            self.dmx[slot_1based] = value; // +0 because index 0 is start code
        }
    }
}

impl OutputModule for SACN {
    fn bpm_changed_master(&mut self, bpm: f32){
        let mut v = bpm.round() as i32;
        v = v.clamp(0, 250);
        self.write_u8_slot(self.start_slot, v as u8); //only send/flush on beat change and slow update to avoid congestion.
        self.logger.debug(&format!("sACN: BPM changed to {}", v));
    }

    fn beat_update_master(&mut self, beat: f32){
        let floor_now = beat.floor() as i32;
       
        if self.last_beat_floor != floor_now {
            self.last_beat_floor = floor_now;
            self.beat_counter = self.beat_counter.wrapping_add(1);
            self.write_u8_slot(self.start_slot + 1, self.beat_counter);
            self.send();
            self.logger.debug(&format!("sACN: Beat updated to {}, counter={}", beat, self.beat_counter));
        }
    }

    fn slow_update(&mut self) {
        //this is done as a keepalive.
        //eventually add some info here like play/pause state, etc.
        self.send();
    }
}
