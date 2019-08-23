// main.rs

use base64;
use base64_serde::base64_serde_type;
use clap::{load_yaml, App};
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
use zmq;

mod block;
mod config;
mod responder;
mod worker;

base64_serde_type!(Base64Standard, base64::STANDARD);

type MyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> MyResult<()> {
    // The YAML file is found relative to the current file, similar to how modules are found
    let yaml = load_yaml!("cli.yaml");
    let matches = App::from_yaml(yaml).get_matches();

    let debug = matches.is_present("debug");

    let c = matches.value_of("config").unwrap();
    let cfg = config::read(c, debug)?;

    if debug {
        println!("Value for config: {}", c);
        println!("Value for cfg: {:?}", cfg);
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

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("logfile")
                .build(LevelFilter::Debug),
        )
        .unwrap();

    // start logging
    let _handle = log4rs::init_config(config).unwrap();
    log::info!("start logging..");

    // open connections
    let connection = cfg.connection;

    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB)?;
    let requester = context.socket(zmq::REQ)?;

    let client_pair = zmq::CurveKeyPair::new()?;

    let server_public_key = hex::decode(connection.public_key)?;
    let subscriber_address =
        "tcp://".to_owned() + &connection.host + ":" + &connection.subscribe_port.to_string();
    let requester_address =
        "tcp://".to_owned() + &connection.host + ":" + &connection.request_port.to_string();

    log::debug!("subscribe to: {}", subscriber_address);
    log::debug!("resquests to: {}", requester_address);

    // setup encrypted subscriber connection
    subscriber.set_curve_server(false)?;
    subscriber
        .set_curve_serverkey(&server_public_key)
        .expect("server public");
    subscriber.set_curve_publickey(&client_pair.public_key)?;
    subscriber.set_curve_secretkey(&client_pair.secret_key)?;

    let s = b""; // empty string ⇒ subscribe to everything
    subscriber.set_subscribe(s)?;

    // setup encrypted request connection
    requester.set_curve_server(false)?;
    requester
        .set_curve_serverkey(&server_public_key)
        .expect("server public");
    requester.set_curve_publickey(&client_pair.public_key)?;
    requester.set_curve_secretkey(&client_pair.secret_key)?;

    // connect
    log::info!("connecting…");
    subscriber
        .connect(&subscriber_address)
        .expect("could not connect to publisher");
    requester
        .connect(&requester_address)
        .expect("could not connect to requester");

    let (response_tx, response_rx) = std::sync::mpsc::channel::<responder::Response>();

    let workers = connection.workers;
    let handles = worker::create_workers(workers, response_tx);

    // zmq sender
    let sender = std::thread::spawn(move || {
        loop {
            log::debug!("waiting..");
            let request = response_rx.recv().unwrap();
            log::info!("send: {}  packed: {:02x?}", request.job, request.packed);

            let s = serde_json::to_string(&request).unwrap();
            log::debug!("request JSON: {}", s);
            requester.send(zmq::Message::from(&s), 0).unwrap();

            let data = requester.recv_msg(0).unwrap();
            let reply = std::str::from_utf8(&data).unwrap();
            log::debug!("reply JSON: {}", reply);
        }
        //drop(requester);
    });

    // poller
    let mut txs = handles.channel_txs;

    let poller = std::thread::spawn(move || {
        let items = &mut [subscriber.as_poll_item(zmq::POLLIN)];
        loop {
            log::info!("polling…");
            let n = match zmq::poll(items, -1) {
                Ok(n) => n,
                Err(_) => 0,
            };
            if n != 0 {
                log::info!("receive");
                let data = subscriber.recv_msg(0).unwrap();
                let s = std::str::from_utf8(&data).unwrap();
                log::info!("JSON: {}", s);
                log::info!("{}", std::str::from_utf8(&data).unwrap());

                responder::send_job(s, &mut txs).unwrap();
            }
        }
        //drop(subscriber);
    });

    sender.join().unwrap();
    poller.join().unwrap();
    for handle in handles.join_handles {
        handle.join().unwrap();
    }

    Ok(())
}
