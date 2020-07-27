use regex::Regex;
use influent::create_client;
use influent::client::{Client, Credentials};
use influent::measurement::{Measurement, Value};
use futures::Future;
use maxminddb::geoip2;

#[macro_use] extern crate lazy_static;

mod log_watcher;
use log_watcher::LogWatcher;

fn is_log_from_sshd(line: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"sshd\[[0-9]+\]:").unwrap();
    }
    return RE.is_match(line);
}

fn get_logline(line: &str) -> &str {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"sshd\[[0-9]+\]: +(.+)$").unwrap();
    }
    let caps = RE.captures(line).unwrap();
    return caps.get(1).unwrap().as_str();
}

fn is_log_accepted(line: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^Accepted.+$").unwrap();
    }
    return RE.is_match(line);
}

fn is_log_good_disconnect(line: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^Disconnected from user.+$").unwrap();
    }
    return RE.is_match(line);
}

fn is_log_bad_disconnect(line: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^Disconnected from.+$").unwrap();
    }
    return RE.is_match(line);
}

fn is_log_invalid_user(line: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^Connection closed|reset by invalid user.+$").unwrap();
    }
    return RE.is_match(line);
}

fn get_ipaddr(line: &str) -> &str {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"([0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3})").unwrap();
    }
    let caps = RE.captures(line).unwrap();
    return caps.get(1).unwrap().as_str();
}

pub struct InfluxInjector<'a> {
    client: influent::client::http::HttpClient<'a>,
    reader: maxminddb::Reader<Vec<u8>>,
}

impl<'a> InfluxInjector<'a> {
    pub fn new() -> InfluxInjector<'a> {
        let credentials = Credentials {
            username: "root",
            password: "root",
            database: "db0"
        };
        let hosts = vec!["http://influxdb:8086"];

        InfluxInjector {
            client: create_client(credentials, hosts),
            reader: maxminddb::Reader::open_readfile("/etc/authmap/GeoLite2-City.mmdb").unwrap(),
        }
    }

    pub fn callback(&self, line: String) {
        let line_str: &str = line.as_str();
        if !is_log_from_sshd(line_str) {
            return;
        }
        let logline = get_logline(line_str);
        let tag;

        if is_log_accepted(logline) {
            tag = "accepted";
        } else if is_log_good_disconnect(logline) {
            tag = "good_disconnect";
        } else if is_log_bad_disconnect(logline) {
            tag = "bad_disconnect";
        } else if is_log_invalid_user(logline) {
            tag = "invalid_user";
        } else {
            return;
        }
        let ip_addr = get_ipaddr(logline);
        println!("Got a {} hit from {}", tag, ip_addr);

        let mut measurement = Measurement::new("ssh_hits");
        measurement.add_tag("type", tag);
        measurement.add_tag("ip", ip_addr);
        measurement.add_field("ip_addr", Value::String(ip_addr));

        match self.reader.lookup(ip_addr.parse::<std::net::IpAddr>().unwrap()) {
            Ok(x) => {
                let city: geoip2::City = x;
                let location = city.location.unwrap();
                let country = city.country.unwrap().names.unwrap()["en"];
                measurement.add_field("latitude", Value::Float(location.latitude.unwrap()));
                measurement.add_field("longitude", Value::Float(location.longitude.unwrap()));
                measurement.add_field("country", Value::String(country));
            }
            Err(err) => {
                println!("Failed to get a position for {} - {}", ip_addr, err);
            }
        }
        

        // As far as I can tell, this is the easiest way to force a blocking async run
        // in a non async app
        ::tokio::run(self.client.write_one(measurement, None).map_err(|e| panic!(e)));
    }
}

fn main() {
    let filename = "/var/log/auth.log".to_string();

    let influx_injector = InfluxInjector::new();

    // Closures are apparently the easy way to do callbacks into stateful objects
    let callback = |line: String| influx_injector.callback(line);

    let mut log_watcher = LogWatcher::register(filename).unwrap();

    log_watcher.watch(&callback);
}
