use clap::Parser;
use log::{info, set_max_level, warn, LevelFilter};
use pc_monitoring_lib::postcard::{CobsAccumulator, FeedResult};
use pc_monitoring_lib::temperature::Thermistor;
use simple_logger::SimpleLogger;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::num::ParseIntError;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;
use systemd_journal_logger::{connected_to_journal, init_with_extra_fields};

fn parse_hex(input: &str) -> Result<u16, ParseIntError> {
    u16::from_str_radix(input, 16)
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path of serial port
    #[clap(short, long)]
    serial: String,

    /// Baud rate of serial port
    #[clap(short, long)]
    serial_baud: u32,

    /// Name of hwmon sensor
    #[clap(short, long)]
    sensor_name: String,

    /// Name of control mode file
    #[clap(short, long)]
    channel_control: String,

    /// Value to be written to control mode file
    #[clap(short, long)]
    channel_control_value: String,

    /// Name of pwm file
    #[clap(short, long)]
    channel_pwm: String,

    /// Comma-separated pairs of temperature and PWM values, i.e. "20:100, 30:150"
    #[clap(short, long)]
    control_points: String,

    /// USB PID for resetting
    #[clap(short, long, parse(try_from_str = parse_hex))]
    usb_pid: u16,

    /// USB VID for resetting
    #[clap(short, long, parse(try_from_str = parse_hex))]
    usb_vid: u16,

    /// USB PID for resetting
    #[clap(short, long)]
    usb_serial: Option<String>,
}

struct ControlPoints(Vec<(u8, u8)>);

impl ControlPoints {
    pub fn get_pwm_from_temp(&self, temp: f32) -> u8 {
        let mut pwm = None;
        self.0
            .iter()
            .zip(self.0[1..].iter())
            .for_each(|((t1, p1), (t2, p2))| {
                let t1 = *t1 as f32;
                let t2 = *t2 as f32;
                let p1 = *p1 as f32;
                let p2 = *p2 as f32;
                if temp >= t1 && temp < t2 {
                    let a = (p1 - p2) / (t1 - t2);
                    pwm = Some((a * temp + p1 - a * t1) as u8);
                }
            });

        pwm.unwrap_or(255)
    }
}

impl From<String> for ControlPoints {
    fn from(control_points: String) -> Self {
        let mut control_points: Vec<(u8, u8)> = control_points
            .split(',')
            .map(|cp| {
                let cp: Vec<&str> = cp.split(':').collect();
                (cp[0].parse::<u8>().unwrap(), cp[1].parse::<u8>().unwrap())
            })
            .collect();
        control_points.sort_by_key(|tp| tp.0);

        Self(control_points)
    }
}

fn main() {
    init_logging();

    let args: Args = Args::parse();

    info!("{:?}", args);

    let sensor_dir = fs::read_dir(Path::new("/sys/class/hwmon"))
        .unwrap()
        .find(|dir| {
            let name_file = dir.as_ref().unwrap().path().join("name");
            let mut name = String::new();
            File::open(name_file)
                .unwrap()
                .read_to_string(&mut name)
                .unwrap();

            name.contains(&args.sensor_name)
        })
        .unwrap()
        .unwrap();

    info!("Found sensor directory: {:?}", sensor_dir.path());

    let control_path = sensor_dir.path().join(args.channel_control);
    let pwm_path = sensor_dir.path().join(args.channel_pwm);

    let control_points = ControlPoints::from(args.control_points);

    loop {
        fs::write(&control_path, &args.channel_control_value).unwrap();
        fs::write(&pwm_path, "255").unwrap();

        let mut alive_counter = 0u8;

        let mut input = serialport::new(&args.serial, args.serial_baud)
            .timeout(Duration::from_millis(10))
            .open()
            .expect("Failed to open port");

        let mut raw_buf = [0u8; 64];
        let mut cobs_buf: CobsAccumulator<512> = CobsAccumulator::new();

        let mut log_divider = 0u8;

        'inner: loop {
            alive_counter += 1;
            if alive_counter >= 20 {
                warn!("Could not read data from serial port. Reconnect...");
                reset_usb(args.usb_pid, args.usb_vid, args.usb_serial.clone()).unwrap();
                sleep(Duration::from_millis(2000));
                break 'inner;
            }
            while let Ok(ct) = input.read(&mut raw_buf) {
                // Finished reading input
                if ct == 0 {
                    break;
                }

                let buf = &raw_buf[..ct];
                let mut window = buf;

                'cobs: while !window.is_empty() {
                    window = match cobs_buf.feed::<Thermistor>(window) {
                        FeedResult::Consumed => break 'cobs,
                        FeedResult::OverFull(new_wind) => new_wind,
                        FeedResult::DeserError(new_wind) => new_wind,
                        FeedResult::Success { data, remaining } => {
                            alive_counter = 0u8;

                            let temp = data.get_temperature();

                            if temp > 0.0 && temp <= 100.0 {
                                let pwm = control_points.get_pwm_from_temp(temp);
                                log_divider += 1;
                                if log_divider >= 30 {
                                    log_divider = 0;
                                    info!(
                                        "Current temperature: {:.1}°C -> Set PWM to {}",
                                        temp, pwm
                                    );
                                }
                                fs::write(&pwm_path, pwm.to_string()).unwrap();
                            }

                            remaining
                        }
                    };
                }
            }
            sleep(Duration::from_millis(500));
        }
    }
}

fn init_logging() {
    if connected_to_journal() {
        init_with_extra_fields(vec![("VERSION", env!("CARGO_PKG_VERSION"))]).unwrap();
    } else {
        SimpleLogger::new().init().unwrap();
    }
    set_max_level(LevelFilter::Info);
}

fn reset_usb(pid: u16, vid: u16, sn: Option<String>) -> rusb::Result<()> {
    let devs: Vec<_> = rusb::devices()?
        .iter()
        .filter(|dev| {
            let descriptor = dev.device_descriptor().unwrap();
            descriptor.product_id() == pid && descriptor.vendor_id() == vid
        })
        .collect();
    let dev = if devs.len() == 1 {
        devs.first().unwrap()
    } else {
        // Found multiple devices -> check serial number
        devs.iter()
            .find(|dev| {
                &dev.open()
                    .unwrap()
                    .read_serial_number_string_ascii(&dev.device_descriptor().unwrap())
                    .unwrap()
                    == sn.as_ref().unwrap()
            })
            .unwrap()
    };
    dev.open()?.reset()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_pwm_from_temp() {
        let cp = ControlPoints(vec![(0, 100), (20, 150), (50, 200), (100, 200)]);

        assert_eq!(cp.get_pwm_from_temp(0.0), 100);
        assert_eq!(cp.get_pwm_from_temp(20.0), 150);
        assert_eq!(cp.get_pwm_from_temp(50.0), 200);

        assert_eq!(cp.get_pwm_from_temp(30.0), 166);
    }
}
