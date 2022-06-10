extern crate chrono;
extern crate dirs;

use std::fmt;
use std::fs::{File};
use std::io::{self, prelude::*};
use std::path::{PathBuf};

use chrono::{Local, NaiveDate, NaiveDateTime};

/**
 * Single timelog entry
 */

const TIME_FMT: &'static str = "%Y-%m-%d %H:%M";

#[derive(Debug)]
#[derive(PartialEq)]
pub struct Entry {
    pub stop: NaiveDateTime,
    pub task: String,
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.stop.format(TIME_FMT), self.task)
    }
}

/**
 * Collection of all entries
 */

pub struct Timelog {
    entries: Vec<Entry>,
    filename: Option<PathBuf>,
}

impl Timelog {
    pub fn new_from_default_file() -> Timelog {
        Timelog::new_from_file(&Timelog::get_default_file())
    }

    pub fn new_from_file(path: &PathBuf) -> Timelog {
        Timelog { entries: Timelog::parse(&Timelog::read(path)), filename: Some(path.clone()) }
    }

    #[cfg(test)]
    pub fn new_from_string(contents: &str) -> Timelog {
        Timelog { entries: Timelog::parse(contents), filename: None }
    }

    pub fn get_default_file() -> PathBuf {
        let mut log_path = dirs::home_dir().expect("Cannot determine home directory");
        log_path.push(".gtimelog");
        log_path.push("timelog.txt");
        log_path
    }

    fn read(path: &PathBuf) -> String {
        match File::open(&path) {
            Ok(mut f) => {
                let mut contents = String::new();
                f.read_to_string(&mut contents)
                    .expect(&format!("Failed to read {}", path.display()));
                contents
            },

            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    println!("No existing {}, starting new log", path.display());
                    String::new()
                } else {
                    panic!("Could not open {}: {:?}", path.display(), e);
                }
            }
        }
    }

    fn parse(raw: &str) -> Vec<Entry> {
        let mut entries = Vec::new();
        let mut prev: Option<NaiveDateTime> = None;

        for line in raw.lines() {
            if let Some(e) = Timelog::parse_line(line) {
                // require a monotonously increasing file
                if prev.is_some() {
                    if e.stop < prev.unwrap() {
                        panic!("line {} goes back in time", line);
                    }
                }
                prev = Some(e.stop);
                entries.push(e);
            }
        }
        entries
    }

    fn parse_line(line: &str) -> Option<Entry> {
        let line = line.trim();
        if line.len() == 0 {
            return None
        }

        if let Some((time, task)) = line.split_once(": ") {
            if let Ok(dt) = NaiveDateTime::parse_from_str(time, TIME_FMT) {
                Some(Entry { stop: dt, task: task.to_string() })
            } else {
                eprintln!("WARNING: ignoring line with invalid date in timelog: {}", line);
                None
            }
        } else {
            eprintln!("WARNING: ignoring invalid line in timelog: {}", line);
            None
        }
    }

    fn format_store(&self) -> String {
        let mut output = String::new();
        let mut prev: Option<NaiveDate> = None;

        for entry in &self.entries {
            // leave an empty line between days
            if prev.is_some() && prev.unwrap() != entry.stop.date() {
                output.push_str("\n");
            }
            prev = Some(entry.stop.date());
            output.push_str(&format!("{}\n", entry));
        }

        output
    }

    pub fn save(&self) {
        assert!(self.filename.is_some());
        let filename = self.filename.as_ref().unwrap();
        let mut f = File::create(filename)
            .expect(&format!("Failed to open {:?} for writing", filename));
        write!(f, "{}", self.format_store())
            .expect(&format!("Failed to write {:?}", filename));
    }

    #[cfg(test)]
    pub fn get_all(&self) -> impl Iterator<Item = &Entry> {
        return self.entries.iter()
    }

    pub fn get_day(&self, day: &NaiveDate) -> impl Iterator<Item = &Entry> {
        let begin = day.and_hms(0, 0, 0);
        let end = day.and_hms(23, 59, 59);
        self.entries.iter().filter(move |e| e.stop >= begin && e.stop < end)
    }

    pub fn get_today(&self) -> impl Iterator<Item = &Entry> {
        self.get_day(&Local::today().naive_local())
    }

    pub fn add(&mut self, task: String) {
        self.entries.push(Entry { task, stop: Local::now().naive_local() });
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    const TWO_DAYS: &'static str = "
2022-06-09 06:02: arrived
2022-06-09 06:27: email
2022-06-09 06:32: **tea
2022-06-09 12:00: work

2022-06-10 07:00: arrived
2022-06-10 12:05: rtimelog: code
2022-06-10 12:30: **lunch
2022-06-10 14:00: rtimelog: code
2022-06-10 15:00: bug triage
2022-06-10 16:00: customer joe: support
";

    #[test]
    fn test_parse_line_valid() {
        let e1 = Timelog::parse_line("2022-05-31 13:59: email").unwrap();
        assert_eq!(e1.task, "email");
        assert_eq!(e1.stop.format(TIME_FMT).to_string(), "2022-05-31 13:59");

        let e2 = Timelog::parse_line("2022-05-31 14:07: read docs").unwrap();
        assert_eq!(e2.task, "read docs");
        assert_eq!(e2.stop.format(TIME_FMT).to_string(), "2022-05-31 14:07");

        assert_eq!(e2.stop.signed_duration_since(e1.stop), Duration::minutes(8));
    }

    #[test]
    fn test_parse_line_invalid() {
        assert_eq!(Timelog::parse_line(""), None);
        assert_eq!(Timelog::parse_line("  "), None);
        assert_eq!(Timelog::parse_line("a"), None);
        // no ': -'
        assert_eq!(Timelog::parse_line("2022-05-31 13:59 email"), None);
        // invalid time
        assert_eq!(Timelog::parse_line("2022-05-31 25:61: email"), None);
        // invalid date
        assert_eq!(Timelog::parse_line("2022-13-32 13:59: email"), None);
    }

    #[test]
    fn test_parse_valid() {
        let entries = Timelog::parse("");
        assert_eq!(entries.len(), 0);

        let entries = Timelog::parse(TWO_DAYS);
        assert_eq!(entries.len(), 10);
        assert_eq!(&format!("{}", entries[0]), "2022-06-09 06:02: arrived");
        assert_eq!(&format!("{}", entries[9]), "2022-06-10 16:00: customer joe: support");
    }

    #[test]
    #[should_panic]
    fn test_parse_out_of_order() {
        Timelog::parse("
2022-06-09 06:02: arrived
2022-06-09 06:10: ** tea
2022-06-08 07:32: huh, previous day
");
    }

    #[test]
    fn test_constructor() {
        let tl = Timelog::new_from_string("");
        assert_eq!(tl.get_all().next(), None);

        let tl = Timelog::new_from_file(&PathBuf::from("/nonexisting"));
        assert_eq!(tl.get_all().next(), None);

        let tl = Timelog::new_from_string(TWO_DAYS);
        let mut entries = tl.get_all();
        assert_eq!(&format!("{}", entries.next().unwrap()), "2022-06-09 06:02: arrived");
        let mut entries = entries.skip(8);
        assert_eq!(&format!("{}", entries.next().unwrap()), "2022-06-10 16:00: customer joe: support");
        assert_eq!(entries.next(), None);
    }

    #[test]
    fn test_get_day() {
        let tl = Timelog::new_from_string("");
        assert_eq!(tl.get_day(&NaiveDate::from_ymd(2022, 6, 8)).next(), None);

        let tl = Timelog::new_from_string(TWO_DAYS);
        assert_eq!(tl.get_day(&NaiveDate::from_ymd(2022, 6, 8)).next(), None);

        let entries = tl.get_day(&NaiveDate::from_ymd(2022, 6, 9)).collect::<Vec<&Entry>>();
        assert_eq!(entries.len(), 4);
        assert_eq!(&format!("{}", entries[0]), "2022-06-09 06:02: arrived");
        assert_eq!(&format!("{}", entries[3]), "2022-06-09 12:00: work");

        let entries = tl.get_day(&NaiveDate::from_ymd(2022, 6, 10)).collect::<Vec<&Entry>>();
        assert_eq!(entries.len(), 6);
        assert_eq!(&format!("{}", entries[0]), "2022-06-10 07:00: arrived");
        assert_eq!(&format!("{}", entries[5]), "2022-06-10 16:00: customer joe: support");
    }

    #[test]
    fn test_format_store() {
        let tl = Timelog::new_from_string(TWO_DAYS);
        // simple roundtrip; but our constant starts with an empty line
        assert_eq!(tl.format_store(), TWO_DAYS.trim_start());
    }

    #[test]
    fn test_add() {
        let mut tl = Timelog::new_from_string("");
        tl.add("think hard".to_string());
        assert_eq!(tl.entries.len(), 1);
        assert_eq!(tl.entries[0].task, "think hard");
    }
}
