use rekordcrate::anlz::Phrase;

pub struct PhraseParser {
    phrase_names: Vec<Vec<String>>,
    hi_phrase_names: Vec<Vec<String>>,
}

impl PhraseParser {
    pub fn new() -> Self {
        Self {
            phrase_names: vec![
                ["Intro", "Verse 1", "Verse 1", "Verse 1", "Verse 2", "Verse 2", "Verse 2", "Bridge", "Chorus", "Outro"].iter().map(|s| s.to_string()).collect(),
                ["Intro", "Verse 1", "Verse 2", "Verse 3", "Verse 4", "Verse 5", "Verse 6", "Bridge", "Chorus", "Outro"].iter().map(|s| s.to_string()).collect(),
                // ["Intro", "Up", "Down", "-", "Chorus", "Outro", "-", "-", "-"].iter().map(|s| s.to_string()).collect(),
                ["Intro 1", "Intro 2", "Up 1", "Up 2", "Up 3", "Down", "Chorus 1", "Chorus 2", "Outro 1", "Outro 2"].iter().map(|s| s.to_string()).collect(),
            ],
            hi_phrase_names: [
                vec!["Intro 2", "Intro 1"],
                vec!["Up 1", "Up 2", "Up 3"],
                vec!["Down"],
                vec!["Chorus 2", "Chorus 1"],
                vec!["Outro 2", "Outro 1"],
            ].iter().map(|x| x.iter().map(|s| s.to_string()).collect()).collect(),
        }
    }

    pub fn get_phrase_name(&self, mood: &rekordcrate::anlz::Mood, phrase: &Phrase) -> String {
        if mood == &rekordcrate::anlz::Mood::High {
            let phrase_kind = match phrase.kind {
                1 => 0,
                2 => 1,
                3 => 2,
                5 => 3,
                6 => 4,
                _ => 99
            };
            return self.hi_phrase_names[phrase_kind][(phrase.k1 + 2 * phrase.k2 + phrase.k3) as usize].clone();
        }
        self.phrase_names[self.mood_to_int(mood)][phrase.kind as usize - 1].clone()
    }

    fn mood_to_int(&self, mood: &rekordcrate::anlz::Mood) -> usize {
        match mood {
            rekordcrate::anlz::Mood::Low => 0,
            rekordcrate::anlz::Mood::Mid => 1,
            rekordcrate::anlz::Mood::High => 2,
        }
    }

    pub fn phrase_name_to_index (phrase_name: &str) -> i32{
        match phrase_name {
            "Intro" | "Intro 1" | "Intro 2" => 1,
            "Verse 1" | "Verse 2" | "Verse 3" | "Verse 4" | "Verse 5" | "Verse 6" | "Up 1" | "Up 2" | "Up 3" => 2,
            "Chorus" | "Chorus 1" | "Chorus 2" => 3,
            "Bridge" | "Down" => 4,
            "Outro" | "Outro 1" | "Outro 2" => 5,
            _ => 0,
        }
    }
}
