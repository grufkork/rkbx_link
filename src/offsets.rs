use core::fmt;
use std::{collections::HashMap, fs::File, io::Read};

impl RekordboxOffsets {
    pub fn from_lines(lines: &[String]) -> RekordboxOffsets {
        let mut rows = lines.iter();
        RekordboxOffsets {
            rbversion: rows.next().unwrap().to_string(),
            deck1bar: Pointer::from_string(rows.next().unwrap()),
            deck1beat: Pointer::from_string(rows.next().unwrap()),
            deck2bar: Pointer::from_string(rows.next().unwrap()),
            deck2beat: Pointer::from_string(rows.next().unwrap()),
            deck3bar: Pointer::from_string(rows.next().unwrap()),
            deck3beat: Pointer::from_string(rows.next().unwrap()),
            deck4bar: Pointer::from_string(rows.next().unwrap()),
            deck4beat: Pointer::from_string(rows.next().unwrap()),
            master_bpm: Pointer::from_string(rows.next().unwrap()),
            masterdeck_index: Pointer::from_string(rows.next().unwrap()),
        }
    }

    pub fn from_file(name: &str) -> HashMap<String, RekordboxOffsets> {
        let mut file = File::open(name).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        drop(file);

        let mut map = HashMap::new();

        let mut lines = vec![];
        for line in contents.lines() {
            if line.is_empty() {
                if !lines.is_empty() {
                    let o = RekordboxOffsets::from_lines(&lines);
                    map.insert(o.rbversion.clone(), o);
                    lines.clear();
                }
            } else if !line.starts_with('#') {
                lines.push(line.to_string());
            }
        }

        map
    }
}

#[derive(Clone)]
pub struct RekordboxOffsets {
    pub rbversion: String,
    pub deck1bar: Pointer,
    pub deck1beat: Pointer,
    pub deck2bar: Pointer,
    pub deck2beat: Pointer,
    pub deck3bar: Pointer,
    pub deck3beat: Pointer,
    pub deck4bar: Pointer,
    pub deck4beat: Pointer,
    pub master_bpm: Pointer,
    pub masterdeck_index: Pointer,
}

#[derive(Clone)]
pub struct Pointer {
    pub offsets: Vec<usize>,
    pub final_offset: usize,
}

impl Pointer {
    pub fn new(offests: Vec<usize>, final_offset: usize) -> Pointer {
        Pointer {
            offsets: offests,
            final_offset,
        }
    }

    pub fn from_string(input: &str) -> Self {
        let split = input.split(' ').map(hexparse).collect::<Vec<usize>>();
        Self::new(split[0..split.len() - 1].to_vec(), *split.last().unwrap())
    }
}

impl fmt::Display for Pointer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut res = "[".to_string();
        for offset in &self.offsets{
            res += &format!("{offset:X}, ");
        }
        res += &format!("{:X}]", self.final_offset);

        write!(f, "{res}")
    }
}

fn hexparse(input: &str) -> usize {
    usize::from_str_radix(input, 16).unwrap()
}
