use std::io::Write;
use std::{cell::RefCell, rc::Rc};
use termcolor::{ColorChoice, ColorSpec, StandardStream, WriteColor};

#[derive(PartialEq)]
pub enum LogLevel {
    Debug = 0,
    Good = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
}

pub struct Logger {
    pub debug_enabled: bool,
    stdout: RefCell<StandardStream>,
    colours: [ColorSpec; 5],
}

impl Logger {
    pub fn new(debug: bool) -> Self {
        let mut colours = core::array::from_fn(|_| ColorSpec::new());
        colours[0].set_fg(Some(termcolor::Color::Cyan));
        colours[1].set_fg(Some(termcolor::Color::Green));
        colours[2].set_fg(Some(termcolor::Color::White));
        colours[3].set_fg(Some(termcolor::Color::Yellow));
        colours[4].set_fg(Some(termcolor::Color::Red));

        Logger {
            colours,
            debug_enabled: debug,
            stdout: RefCell::new(StandardStream::stdout(ColorChoice::Always)),
        }
    }

    pub fn log(&self, source: &str, message: &str, level: LogLevel) {
        if !self.debug_enabled && level == LogLevel::Debug {
            return;
        }

        self.stdout
            .borrow_mut()
            .set_color(&self.colours[level as usize])
            .unwrap();
        if writeln!(&mut self.stdout.borrow_mut(), "[{source}]  {message}").is_err() {
            println!("Log failed: [{source}]  {message}");
        }
    }

    pub fn debug(&self, source: &str, message: &str) {
        self.log(source, message, LogLevel::Debug);
    }

    pub fn good(&self, source: &str, message: &str) {
        self.log(source, message, LogLevel::Good);
    }

    pub fn info(&self, source: &str, message: &str) {
        self.log(source, message, LogLevel::Info);
    }

    pub fn warning(&self, source: &str, message: &str) {
        self.log(source, message, LogLevel::Warning);
    }

    pub fn error(&self, source: &str, message: &str) {
        self.log(source, message, LogLevel::Error);
    }
}

#[derive(Clone)]
pub struct ScopedLogger {
    pub logger: Rc<Logger>,
    source: String,
}

impl ScopedLogger {
    pub fn new(logger: &Rc<Logger>, source: &str) -> Self {
        ScopedLogger {
            logger: logger.clone(),
            source: source.to_string(),
        }
    }

    pub fn debug(&self, message: &str) {
        self.logger.debug(&self.source, message);
    }

    pub fn good(&self, message: &str) {
        self.logger.good(&self.source, message);
    }

    pub fn info(&self, message: &str) {
        self.logger.info(&self.source, message);
    }

    pub fn warn(&self, message: &str) {
        self.logger.warning(&self.source, message);
    }

    pub fn err(&self, message: &str) {
        self.logger.error(&self.source, message);
    }
}
