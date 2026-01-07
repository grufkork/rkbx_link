use std::net::UdpSocket;

use rosc::{encoder::encode, OscMessage, OscPacket};

use crate::{beatkeeper::TrackInfo, config::Config, log::ScopedLogger, utils::PhraseParser};

use super::{ModuleCreateOutput, OutputModule};

enum OutputFormat{
    String,
    Int,
    Float
}

impl OutputFormat {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "string" => Some(OutputFormat::String),
            "int" => Some(OutputFormat::Int),
            "float" => Some(OutputFormat::Float),
            _ => None,
        }
    }
}

struct BeatDivTrackData {
    subdivision: f32,
    address: String

}

struct MessageToggles{
    beat_subdivs: Vec<BeatDivTrackData>,
    beat_master_subdivs: Vec<BeatDivTrackData>,
    beat_triggers: Vec<BeatDivTrackData>,
    beat_master_triggers: Vec<BeatDivTrackData>,

    beat_trigger_autorelease: bool,
    time: bool,
    time_master: bool,
    phrase: bool,
    phrase_master: bool,
    phrase_output_format: OutputFormat,
}


impl MessageToggles{
    fn new(conf: &Config, logger: ScopedLogger) -> Self{
        let mut divs = 
            ["msg.beat.subdiv", "msg.beat.trigger", "msg.beat_master.subdiv", "msg.beat_master.trigger"].iter().zip(
            ["/beat/subdiv/", "/beat/trigger/", "/beat/master/subdiv/", "/beat/master/trigger/"].iter()).map(|(conf_key, addr)|{
            conf.get_or_default(conf_key, String::new()).split(",").filter_map(|x|{
                if x.is_empty(){
                    return None;
                }
                if let Ok(val) = x.trim().parse::<f32>(){
                    Some(
                        BeatDivTrackData{
                            subdivision: val,
                            address: format!("{addr}{val}"),
                        }
                    )
                }else{
                    logger.err(&format!("Error parsing value '{x}' in key {conf_key}"));
                    None
                }
            }).collect::<Vec<BeatDivTrackData>>()
        });

        MessageToggles { 
            beat_subdivs: divs.next().unwrap(),
            beat_triggers: divs.next().unwrap(),
            beat_master_subdivs: divs.next().unwrap(),
            beat_master_triggers: divs.next().unwrap(),

            beat_trigger_autorelease: conf.get_or_default("msg.trigger_autorelease", false),
            time: conf.get_or_default("msg.time", false), 
            time_master: conf.get_or_default("msg.time_master", true), 
            phrase: conf.get_or_default("msg.phrase", false), 
            phrase_master:  conf.get_or_default("msg.phrase_master", true),
            phrase_output_format: {
                let fmt = conf.get_or_default("phrase_output_format", "string".to_string());
                match OutputFormat::from_str(&fmt) {
                    Some(format) => format,
                    None => {
                        logger.err(&format!("Unknown phrase output format: {fmt}"));
                        OutputFormat::String
                    }
                }
            }
        }
    } 
}

pub struct Osc {
    socket: UdpSocket,
    info_sent: bool,
    logger: ScopedLogger,
    message_toggles: MessageToggles,
    send_period: i32,
    send_period_counter: i32,
    last_beat_master: f32,
    last_beats: Vec<f32>
}




impl Osc {
    fn send_float(&self, addr: &str, value: f32) {
        let msg = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![rosc::OscType::Float(value)],
        });
        self.send(msg);
    }

    fn send_string(&self, addr: &str, value: &str) {
        let msg = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![rosc::OscType::String(value.to_string())],
        });
        self.send(msg);
    }

    fn send_int(&self, addr: &str, value: i32) {
        let msg = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![rosc::OscType::Int(value)],
        });
        self.send(msg);
    }

    fn send(&self, msg: OscPacket) {
        let packet = match encode(&msg){
            Ok(packet) => packet,
            Err(e) => {
                self.logger.err(&format!("Failed to encode OSC message: {e}"));
                return;
            }
        };
        if let Err(e) = self.socket.send(&packet) {
            self.logger.err(&format!("Failed to send OSC message: {e}"));
        };
    }
}

impl Osc {
    pub fn create(conf: Config, logger: ScopedLogger) -> ModuleCreateOutput {
        let socket =
            match UdpSocket::bind(conf.get_or_default("source", "127.0.0.1:8888".to_string())) {
                Ok(socket) => socket,
                Err(e) => {
                    logger.err(&format!("Failed to open source socket: {e}"));
                    return Err(());
                }
            };

        if let Err(e) =
            socket.connect(conf.get_or_default("destination", "127.0.0.1:9999".to_string()))
        {
            logger.err(&format!("Failed to open connection to receiver: {e}"));
            return Err(());
        }

        Ok(Box::new(Osc {
            socket,
            info_sent: false,
            logger: logger.clone(),
            message_toggles: MessageToggles::new(&conf, logger),
            send_period: conf.get_or_default("send_every_nth", 2),
            send_period_counter: 0,
            last_beat_master: 0.0,
            last_beats: vec![0.0; 4],
        }))
    }
}

impl OutputModule for Osc {
    fn pre_update(&mut self) {
        self.send_period_counter = (self.send_period_counter + 1) % self.send_period;
    }

    fn bpm_changed_master(&mut self, bpm: f32) {
        self.send_float("/bpm/master/current", bpm);
    }

    fn original_bpm_changed_master(&mut self, bpm: f32) {
        self.send_float("/bpm/master/original", bpm);
    }

    fn beat_update_master(&mut self, beat: f32) {
        if self.send_period_counter != 0 {
            return;
        }

        for d in &self.message_toggles.beat_master_subdivs{
            let value = (beat % d.subdivision) / d.subdivision;
            self.send_float(&d.address, value);
        }

        for d in &self.message_toggles.beat_master_triggers{
            if beat % d.subdivision < self.last_beat_master % d.subdivision {
                self.send_float(&d.address, 1.);
            }else if self.message_toggles.beat_trigger_autorelease && (beat + d.subdivision * 0.2) % d.subdivision < (self.last_beat_master + d.subdivision * 0.2) % d.subdivision{
                self.send_float(&d.address, 0.);
            }
        }
        
        self.last_beat_master = beat;
    }


    fn time_update_master(&mut self, time: f32) {
        if self.send_period_counter != 0 {
            return;
        }
        if self.message_toggles.time_master{
            self.send_float("/time/master", time);
        }
    }

    fn beat_update(&mut self, beat: f32, deck: usize) {
        if self.send_period_counter != 0 {
            return;
        }

        for d in &self.message_toggles.beat_subdivs{
            let value = (beat % d.subdivision) / d.subdivision;
            self.send_float(&d.address, value);
        }


        for d in &self.message_toggles.beat_triggers{
            if beat % d.subdivision < self.last_beats[deck] % d.subdivision {
                self.send_float(&d.address, 1.);
            }else if self.message_toggles.beat_trigger_autorelease && (beat + d.subdivision * 0.2) % d.subdivision < (self.last_beats[deck] + d.subdivision * 0.2) % d.subdivision{
                self.send_float(&d.address, 0.);
            }
        }
        self.last_beats[deck] = beat;
    }

    fn time_update(&mut self, time: f32, deck: usize) {
        if self.send_period_counter != 0 {
            return;
        }
        if self.message_toggles.time{
            self.send_float(&format!("/time/{deck}"), time);
        }
    }

    fn track_changed(&mut self, track: &TrackInfo, deck: usize) {
        self.send_string(&format!("/track/{deck}/title"), &track.title);
        self.send_string(&format!("/track/{deck}/artist"), &track.artist);
        self.send_string(&format!("/track/{deck}/album"), &track.album);
    }

    fn track_changed_master(&mut self, track: &TrackInfo) {
        self.send_string("/track/master/title", &track.title);
        self.send_string("/track/master/artist", &track.artist);
        self.send_string("/track/master/album", &track.album);
    }

    fn slow_update(&mut self) {
        if !self.info_sent {
            self.info_sent = true;

            let target_addr = if let Ok(addr) = self.socket.peer_addr() {
                addr.to_string()
            } else {
                "No target!!".to_string()
            };

            let source_addr = if let Ok(addr) = self.socket.local_addr() {
                addr.to_string()
            } else {
                "No source!!".to_string()
            };
            self.logger
                .info(&format!("Sending {source_addr} -> {target_addr}"));
            }
    }

    fn phrase_changed_master(&mut self, phrase: &str) {
        if self.message_toggles.phrase_master{
            self.output_phrase("/phrase/master/current", phrase);
        }
    }

    fn next_phrase_changed_master(&mut self, phrase: &str) {
        if self.message_toggles.phrase_master{
            self.output_phrase("/phrase/master/next", phrase);
        }
    }

    fn next_phrase_in_master(&mut self, beats: i32) {
        if self.message_toggles.phrase_master{
            self.send_float("/phrase/master/countin", beats as f32);
        }
    }

    fn phrase_changed(&mut self, phrase: &str, deck: usize) {
        if self.message_toggles.phrase{
            self.output_phrase(&format!("/phrase/{deck}/current"), phrase);
        }
    }

    fn next_phrase_changed(&mut self, phrase: &str, deck: usize) {
        if self.message_toggles.phrase{
            self.send_string(&format!("/phrase/{deck}/next"), phrase);
        }
    }

    fn next_phrase_in(&mut self, beats: i32, deck: usize) {
        if self.message_toggles.phrase{
            self.send_float(&format!("/phrase/{deck}/countin"), beats as f32);
        }
    }
}

impl Osc{
    fn output_phrase(&mut self, addr: &str, phrase: &str){
        match self.message_toggles.phrase_output_format {
            OutputFormat::String => self.send_string(addr, phrase),
            OutputFormat::Int => self.send_int(addr, PhraseParser::phrase_name_to_index(phrase)),
            OutputFormat::Float => self.send_float(addr, PhraseParser::phrase_name_to_index(phrase) as f32),
        }
    }
}
