use structopt::StructOpt;

use crate::dezoomer::Dezoomer;

use super::{auto, stdin_line, Vec2d, ZoomError};
use std::time::Duration;
use std::path::PathBuf;
use regex::Regex;

#[derive(StructOpt, Debug)]
#[structopt(author, about)]
pub struct Arguments {
    /// Input URL or local file name
    pub input_uri: Option<String>,

    /// File to which the resulting image should be saved
    #[structopt(parse(from_os_str))]
    pub outfile: Option<PathBuf>,

    /// Name of the dezoomer to use
    #[structopt(short, long, default_value = "auto")]
    dezoomer: String,

    /// If several zoom levels are available, then select the largest one
    #[structopt(short, long)]
    pub largest: bool,

    /// If several zoom levels are available, then select the one with the largest width that
    /// is inferior to max-width.
    #[structopt(short = "w", long = "max-width")]
    max_width: Option<u32>,

    /// If several zoom levels are available, then select the one with the largest height that
    /// is inferior to max-height.
    #[structopt(short = "h", long = "max-height")]
    max_height: Option<u32>,

    /// Degree of parallelism to use. At most this number of
    /// tiles will be downloaded at the same time.
    #[structopt(short = "n", long = "parallelism", default_value = "16")]
    pub parallelism: usize,

    /// Number of new attempts to make when a tile load fails
    /// before giving up. Setting this to 0 is useful to speed up the
    /// generic dezoomer, which relies on failed tile loads to detect the
    /// dimensions of the image. On the contrary, if a server is not reliable,
    /// set this value to a higher number.
    #[structopt(short = "r", long = "retries", default_value = "1")]
    pub retries: usize,

    /// Amount of time to wait before retrying a request that failed
    #[structopt(long, default_value = "2s", parse(try_from_str = parse_duration))]
    pub retry_delay: Duration,

    /// Sets an HTTP header to use on requests.
    /// This option can be repeated in order to set multiple headers.
    /// You can use `-H "Referer: URL"` where URL is the URL of the website's
    /// viewer page in order to let the site think you come from the legitimate viewer.
    #[structopt(
    short = "H",
    long = "header",
    parse(try_from_str = parse_header),
    number_of_values = 1
    )]
    pub headers: Vec<(String, String)>,

    /// Maximum number of idle connections per host allowed at the same time
    #[structopt(long, default_value = "32")]
    pub max_idle_per_host: usize,

    /// Whether to accept connecting to insecure HTTPS servers
    #[structopt(long)]
    pub accept_invalid_certs: bool,

    /// Maximum time between the beginning of a request and the end of a response before
    ///the request should be interrupted and considered failed
    #[structopt(long, default_value = "30s", parse(try_from_str = parse_duration))]
    pub timeout: Duration,

    /// Time after which we should give up when trying to connect to a server
    #[structopt(long = "connect-timeout", default_value = "6s", parse(try_from_str = parse_duration))]
    pub connect_timeout: Duration,

    /// Level of logging verbosity. Set it to "debug" to get all logging messages.
    #[structopt(long, default_value="warn")]
    pub logging: String,
}

impl Default for Arguments {
    fn default() -> Self {
        Arguments {
            input_uri: None,
            outfile: None,
            dezoomer: "auto".to_string(),
            largest: false,
            max_width: None,
            max_height: None,
            parallelism: 16,
            retries: 1,
            retry_delay: Duration::from_secs(2),
            headers: vec![],
            max_idle_per_host: 32,
            accept_invalid_certs: false,
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(6),
            logging: "warn".to_string(),
        }
    }
}

impl Arguments {
    pub fn choose_input_uri(&self) -> String {
        match &self.input_uri {
            Some(uri) => uri.clone(),
            None => {
                println!("Enter an URL or a path to a tiles.yaml file: ");
                stdin_line()
            }
        }
    }
    pub fn find_dezoomer(&self) -> Result<Box<dyn Dezoomer>, ZoomError> {
        auto::all_dezoomers(true)
            .into_iter()
            .find(|d| d.name() == self.dezoomer)
            .ok_or_else(|| ZoomError::NoSuchDezoomer {
                name: self.dezoomer.clone(),
            })
    }
    pub fn best_size<I: Iterator<Item = Vec2d>>(&self, sizes: I) -> Option<Vec2d> {
        if self.largest {
            sizes.max_by_key(|s| s.area())
        } else if self.max_width.is_some() || self.max_height.is_some() {
            sizes
                .filter(|s| {
                    self.max_width.map(|w| s.x <= w).unwrap_or(true)
                        && self.max_height.map(|h| s.y <= h).unwrap_or(true)
                })
                .max_by_key(|s| s.area())
        } else {
            None
        }
    }

    pub fn headers(&self) -> impl Iterator<Item = (&String, &String)> {
        self.headers.iter().map(|(k, v)| (k, v))
    }
}

fn parse_header(s: &str) -> Result<(String, String), &'static str> {
    let vals: Vec<&str> = s.splitn(2, ':').map(str::trim).collect();
    if let [key, value] = vals[..] {
        Ok((key.into(), value.into()))
    } else {
        Err("Invalid header format. Expected 'Name: Value'")
    }
}

fn parse_duration(s: &str) -> Result<Duration, &'static str> {
    let err_msg = "Invalid duration. \
                        A duration is a number followed by a unit, such as '10ms' or '5s'";
    let re = Regex::new(r"^(\d+)\s*(min|s|ms|ns)$").unwrap();
    let caps = re.captures(s).ok_or(err_msg)?;
    let val: u64 = caps[1].parse().map_err(|_| err_msg)?;
    match &caps[2] {
        "min" => Ok(Duration::from_secs(60 * val)),
        "s" => Ok(Duration::from_secs(val)),
        "ms" => Ok(Duration::from_millis(val)),
        "ns" => Ok(Duration::from_nanos(val)),
        _ => Err(err_msg)
    }
}


#[test]
fn test_headers_and_input() -> Result<(), structopt::clap::Error> {
    let args: Arguments = StructOpt::from_iter_safe(
        [
            "dezoomify-rs",
            "--header",
            "Referer: http://test.com",
            "--header",
            "User-Agent: custom",
            "--header",
            "A:B",
            "input-url",
        ]
        .iter(),
    )?;
    assert_eq!(args.input_uri, Some("input-url".into()));
    assert_eq!(
        args.headers,
        vec![
            ("Referer".into(), "http://test.com".into()),
            ("User-Agent".into(), "custom".into()),
            ("A".into(), "B".into()),
        ]
    );
    Ok(())
}

#[test]
fn test_parse_duration() {
    assert_eq!(parse_duration("2s"), Ok(Duration::from_secs(2)));
    assert_eq!(parse_duration("29 s"), Ok(Duration::from_secs(29)));
    assert_eq!(parse_duration("2min"), Ok(Duration::from_secs(120)));
    assert_eq!(parse_duration("1000 ms"), Ok(Duration::from_secs(1)));
    assert!(parse_duration("1 2 ms").is_err());
    assert!(parse_duration("1 s s").is_err());
    assert!(parse_duration("ms").is_err());
    assert!(parse_duration("1j").is_err());
    assert!(parse_duration("").is_err());
}