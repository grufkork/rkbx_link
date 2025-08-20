use std::net::UdpSocket;

use rosc::{encoder::encode, OscMessage, OscPacket};

use crate::{beatkeeper::TrackInfo, config::Config, log::ScopedLogger};

use super::{ModuleCreateOutput, OutputModule};

struct MessageToggles{
    beat: bool,
    beat_div_1: bool,
    beat_div_2: bool,
    beat_div_4: bool,
    beat_master: bool,
    beat_master_div_1: bool,
    beat_master_div_2: bool,
    beat_master_div_4: bool,
    time: bool,
    time_master: bool,
    phrase: bool,
    phrase_master: bool
}


impl MessageToggles{
    fn new(conf: &Config) -> Self{
        MessageToggles { 
            beat: conf.get_or_default("msg.beat", false), 
            beat_div_1: conf.get_or_default("msg.beat.div_1", false), 
            beat_div_2: conf.get_or_default("msg.beat.div_2", false), 
            beat_div_4: conf.get_or_default("msg.beat.div_4", false), 
            beat_master: conf.get_or_default("msg.beat_master", true), 
            beat_master_div_1: conf.get_or_default("msg.beat_master.div_1", false), 
            beat_master_div_2: conf.get_or_default("msg.beat_master.div_2", false), 
            beat_master_div_4: conf.get_or_default("msg.beat_master.div_4", false), 
            time: conf.get_or_default("msg.time", false), 
            time_master: conf.get_or_default("msg.time_master", true), 
            phrase: conf.get_or_default("msg.phrase", false), 
            phrase_master:  conf.get_or_default("msg.phrase_master", true)}
    } 
}

pub struct Osc {
    socket: UdpSocket,
    info_sent: bool,
    logger: ScopedLogger,
    message_toggles: MessageToggles,
    send_period: i32,
    send_period_counter: i32
}





impl Osc {
    fn send_float(&mut self, addr: &str, value: f32) {
        let msg = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![rosc::OscType::Float(value)],
        });
        let packet = encode(&msg).unwrap();
        self.socket.send(&packet).unwrap();
    }

    fn send_string(&mut self, addr: &str, value: &str) {
        let msg = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![rosc::OscType::String(value.to_string())],
        });
        let packet = encode(&msg).unwrap();
        self.socket.send(&packet).unwrap();
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
            logger,
            message_toggles: MessageToggles::new(&conf),
            send_period: conf.get_or_default("send_every_nth", 2),
            send_period_counter: 0,
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
        if self.message_toggles.beat_master{
            self.send_float("/beat/master", beat);
        }
        if self.message_toggles.beat_master_div_1{
            self.send_float("/beat/master/div1", beat % 1.);
        }
        if self.message_toggles.beat_master_div_2{
            self.send_float("/beat/master/div2", (beat % 2.) / 2.);
        }
        if self.message_toggles.beat_master_div_4{
            self.send_float("/beat/master/div4", (beat % 4.) / 4.);
        }
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
        if self.message_toggles.beat{
            self.send_float(&format!("/beat/{deck}"), beat);
        }
        if self.message_toggles.beat_div_1{
            self.send_float(&format!("/beat/{deck}/div1"), beat % 1.);
        }
        if self.message_toggles.beat_div_2{
            self.send_float(&format!("/beat/{deck}/div2"), beat % 2.);
        }
        if self.message_toggles.beat_div_4{
            self.send_float(&format!("/beat/{deck}/div4"), beat % 4.);
        }
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
            self.send_string("/phrase/master/current", phrase);
        }
    }

    fn next_phrase_changed_master(&mut self, phrase: &str) {
        if self.message_toggles.phrase_master{
            self.send_string("/phrase/master/next", phrase);
        }
    }

    fn next_phrase_in_master(&mut self, beats: i32) {
        if self.message_toggles.phrase_master{
            self.send_float("/phrase/master/countin", beats as f32);
        }
    }

    fn phrase_changed(&mut self, phrase: &str, deck: usize) {
        if self.message_toggles.phrase{
            self.send_string(&format!("/phrase/{deck}/current"), phrase);
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
