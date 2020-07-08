
use regex::Regex;
use std::time::Duration;

pub fn parse_duration(input: &str) -> Result<Duration, &str> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"(?x)
            -?(?P<hours>[0-9][0-9])?:?
            (?P<minutes>[0-5][0-9]):
            (?P<seconds>[0-5]?[0-9])(?:.[0-9]+)?
            |
            -?(?P<value>[0-9]+)(?:.[0-9]+)?
            (?P<unit>ms|us)?
        ").unwrap();
    }
    
    let captures = RE.captures(input).unwrap();

    let hours = match captures.name("hours") {
        Some(hours) => hours.as_str().parse::<u64>().unwrap(),
        None => 0
    };
    let minutes = match captures.name("minutes") {
        Some(minutes) => minutes.as_str().parse::<u64>().unwrap(),
        None => 0
    };
    let mut seconds = match captures.name("seconds") {
        Some(seconds) => seconds.as_str().parse::<u64>().unwrap(),
        None => 0
    };
    let value = match captures.name("value") {
        Some(value) => value.as_str().parse::<u64>().unwrap(),
        None => 0
    };
    let unit = match captures.name("unit") {
        Some(unit) => unit.as_str(),
        None => {
            seconds = value;
            ""
        }
    };

    let duration;
    
    if unit == "us" {
        duration = Duration::from_micros(value);
    } else if unit == "ms" {
        duration = Duration::from_millis(value);
    } else {
        duration = Duration::from_secs(hours * 3600 + minutes * 60 + seconds);
    }
    
    return Ok(duration);
}