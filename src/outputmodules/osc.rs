use std::net::UdpSocket;

use rosc::{encoder::encode, OscMessage, OscPacket};

use crate::{beatkeeper::TrackInfo, config::Config, log::ScopedLogger};

use super::OutputModule;

pub struct Osc {
    socket: UdpSocket,
    info_sent: bool,
    logger: ScopedLogger
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
    pub fn create(conf: Config, logger: ScopedLogger) -> Box<dyn OutputModule> {
        let socket = UdpSocket::bind(
            conf.get_or_default("source", "127.0.0.1:8888".to_string())
        )
        .unwrap();
        socket
            .connect(
                conf.get_or_default("destination", "127.0.0.1:9999".to_string())
            )
            .unwrap();

        Box::new(Osc { socket, info_sent: false, logger })
    }
}

impl OutputModule for Osc {
    fn bpm_changed(&mut self, bpm: f32){
        self.send_float("/bpm/current", bpm);
    }

    fn original_bpm_changed(&mut self, bpm: f32){
        self.send_float("/bpm/original", bpm);
    }

    fn beat_update(&mut self, beat: f32){
        self.send_float("/beat/0", beat);
        self.send_float("/beat/1", beat % 1.);
        self.send_float("/beat/2", beat % 2.);
        self.send_float("/beat/4", beat % 4.);
    }

    fn time_update(&mut self, time: f32){
        self.send_float("/time", time);
    }
    
    fn playback_speed_changed(&mut self, speed: f32) {
        self.send_float("/playback_speed", speed);
    } 
    

    fn track_changed(&mut self, track: TrackInfo, deck: usize){
        self.send_string(&format!("/track/{deck}/title"), &track.title);
        self.send_string(&format!("/track/{deck}/artist"), &track.artist);
        self.send_string(&format!("/track/{deck}/album"), &track.album);
    }

    fn master_track_changed(&mut self, track: &TrackInfo){
        self.send_string("/track/master/title", &track.title);
        self.send_string("/track/master/artist", &track.artist);
        self.send_string("/track/master/album", &track.album);
    }

    fn slow_update(&mut self){
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
            self.logger.info(&format!("Sending {source_addr} -> {target_addr}"));
        }
    }
}
