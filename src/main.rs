// main.rs

use base64;
use base64_serde::base64_serde_type;
use clap::Parser;
use hex;
use log;
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use simple_error::bail;
use std::path::Path;
use zmq;

mod block;
mod config;
mod responder;
mod worker;

base64_serde_type!(Base64Standard, base64::engine::general_purpose::STANDARD);

type MyResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// display more details
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// print debug information
    #[arg(short = 'D', long, default_value_t = false)]
    debug: bool,

    /// configuration file
    #[arg(short, long)]
    config: String,
}


fn main() -> MyResult<()> {
    let args = Args::parse();

    let debug = args.debug;
    let cfg = config::read(&args.config, debug)?;

    if debug {
        println!("Value for args: {:?}", args);
        println!("Value for cfg: {:?}", cfg);
    }

    if !Path::new(&cfg.logging.directory).exists() {
        bail!(
            "logging directory: {} does not exist",
            cfg.logging.directory
        );
    }

    let pattern = "{d(%Y-%m-%d %H:%M:%S)(utc)} [{l}] {M}: {m}{n}";
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(pattern)))
        .build();

    let roller = FixedWindowRoller::builder()
        .base(0)
        .build(
            &format!("{}/{}.{{}}", cfg.logging.directory, cfg.logging.file),
            cfg.logging.count,
        )
        .unwrap();

    let logfile = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(pattern)))
        .build(
            &format!("{}/{}", cfg.logging.directory, cfg.logging.file),
            Box::new(CompoundPolicy::new(
                Box::new(SizeTrigger::new(cfg.logging.size)),
                Box::new(roller),
            )),
        )
        .unwrap();

    let filter = match cfg.logging.level.as_ref() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Error,
    };

    let config = if cfg.logging.console {
        Config::builder().appender(Appender::builder().build("stdout", Box::new(stdout)))
    } else {
        Config::builder()
    }
    .appender(Appender::builder().build("logfile", Box::new(logfile)))
    .build(
        if cfg.logging.console {
            Root::builder().appender("stdout")
        } else {
            Root::builder()
        }
        .appender("logfile")
        .build(filter),
    )
    .unwrap();

    // start logging
    let _handle = log4rs::init_config(config).unwrap();
    log::warn!("=== start ===");

    // open connections
    let mut handles = Vec::new();
    for connection in cfg.connections {
        if connection.enable && connection.public_key != "" {
            log::debug!("connection: {}", connection.number);
            handles.push(create_connection(connection)?);
        } else {
            log::debug!("connection: {} is disabled", connection.number);
        }
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Ok(())
}

fn create_connection(connection: config::Connection) -> MyResult<std::thread::JoinHandle<()>> {
    let set = connection.number;

    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB)?;
    let requester = context.socket(zmq::REQ)?;

    let client_pair = zmq::CurveKeyPair::new()?;

    let server_public_key = hex::decode(connection.public_key)?;
    let subscriber_address =
        "tcp://".to_owned() + &connection.host + ":" + &connection.subscribe_port.to_string();
    let requester_address =
        "tcp://".to_owned() + &connection.host + ":" + &connection.request_port.to_string();

    log::info!("C{}: subscribe to: {}", set, subscriber_address);
    log::info!("C{}: resquests to: {}", set, requester_address);

    // setup encrypted subscriber connection
    subscriber.set_ipv6(!connection.use_ipv4)?;
    subscriber.set_curve_server(false)?;
    subscriber
        .set_curve_serverkey(&server_public_key)
        .expect("server public");
    subscriber.set_curve_publickey(&client_pair.public_key)?;
    subscriber.set_curve_secretkey(&client_pair.secret_key)?;

    let s = b""; // empty string ⇒ subscribe to everything
    subscriber.set_subscribe(s)?;

    // setup encrypted request connection
    requester.set_ipv6(!connection.use_ipv4)?;
    requester.set_curve_server(false)?;
    requester
        .set_curve_serverkey(&server_public_key)
        .expect("server public");
    requester.set_curve_publickey(&client_pair.public_key)?;
    requester.set_curve_secretkey(&client_pair.secret_key)?;

    // connect
    log::debug!("C{}: connecting…", set);
    subscriber
        .connect(&subscriber_address)
        .expect("could not connect to publisher");
    requester
        .connect(&requester_address)
        .expect("could not connect to requester");

    let (response_tx, response_rx) = std::sync::mpsc::channel::<responder::Response>();

    let workers = connection.workers;
    let handles = worker::create_workers(set, workers, response_tx);

    // zmq sender
    let _sender = std::thread::spawn(move || {
        loop {
            log::debug!("C{}: waiting..", set);
            let request = response_rx.recv().unwrap();
            log::debug!(
                "C{}: send: {}  packed: {:02x?}",
                set,
                request.job,
                request.packed
            );

            let s = serde_json::to_string(&request).unwrap();
            log::info!("C{}: request JSON: {}", set, s);
            requester.send(zmq::Message::from(&s), 0).unwrap();

            let data = requester.recv_msg(0).unwrap();
            let reply = std::str::from_utf8(&data).unwrap();
            log::info!("C{}: reply JSON: {}", set, reply);
        }
        //drop(requester);
    });

    // poller
    let mut txs = handles.channel_txs;

    let poller = std::thread::spawn(move || {
        let items = &mut [subscriber.as_poll_item(zmq::POLLIN)];
        loop {
            log::debug!("C{}: polling…", set);
            let n = match zmq::poll(items, -1) {
                Ok(n) => n,
                Err(_) => 0,
            };
            if n != 0 {
                log::debug!("C{}: receive", set);
                let data = subscriber.recv_msg(0).unwrap();
                let s = std::str::from_utf8(&data).unwrap();
                log::trace!("C{}: JSON: {}", set, s);
                log::debug!("C{}: decoded: {}", set, std::str::from_utf8(&data).unwrap());

                match responder::send_job(set, s, &mut txs) {
                    Ok(_) => log::debug!("send_job success"),
                    Err(e) => log::error!("send_job error: {}", e),
                };
            }
        }
        //drop(subscriber);

        //sender.join().unwrap();
        //for handle in handles.join_handles {
        //    handle.join().unwrap();
        //}
    });

    Ok(poller)
}
