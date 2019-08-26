// config.rs

use rlua::{Lua, Result, Table};
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

#[derive(Debug, PartialEq)]
pub struct Configuration {
    pub data_directory: String,
    pub connection: Connection,
    pub logging: Logging,
}

#[derive(Debug, PartialEq)]
pub struct Connection {
    pub workers: u32,
    pub use_ipv4: bool,
    pub host: String,
    pub public_key: String,
    pub subscribe_port: u16,
    pub request_port: u16,
}

#[derive(Debug, PartialEq)]
pub struct Logging {
    pub directory: String,
    pub file: String,
    pub size: u64,
    pub count: u32,
    pub console: bool,
    pub level: String,
}

const DEFAULT_DATA_DIRECTORY: &str = ".";

const DEFAULT_PUBLISH: u16 = 2138;
const DEFAULT_REQUEST: u16 = 2139;
const DEFAULT_WORKERS: u32 = 1;

const DEFAULT_LOG_DIRECTORY: &str = "log";
const DEFAULT_LOG_FILE: &str = "recorder.log";
const DEFAULT_LOG_SIZE: u64 = 10000;
const DEFAULT_LOG_COUNT: u32 = 1;

// allow use of '?' to quick return error
//type MyResult<T> = Result<T, Box<Error>>;

pub fn read(filename: &str, debug: bool) -> Result<Configuration> {
    if debug {
        println!("configuration file: {}", filename);
    }

    let file = File::open(filename).unwrap();
    let mut buf_reader = BufReader::new(file);
    let mut contents = String::new();
    buf_reader.read_to_string(&mut contents).unwrap();

    if debug {
        println!("configuration text: {}", contents);
    }

    let lua = Lua::new();
    let result = lua.context(|lua| {
        let arg = lua.create_table()?;
        arg.set(0, filename)?;

        let globals = lua.globals();
        globals.set("arg", arg)?;

        let config = lua.load(&contents).set_name("config")?.eval::<Table>()?;

        let mut data_directory: String = config
            .get::<_, String>("data_directory")
            .unwrap_or(DEFAULT_DATA_DIRECTORY.to_string())
            .trim()
            .to_string();
        if data_directory.is_empty() {
            data_directory = DEFAULT_DATA_DIRECTORY.to_string()
        }

        let connection: Table = config.get("connection")?;
        let logging: Table = config.get("logging")?;

        let cn = Connection {
            host: connection.get("host")?,
            public_key: connection.get("public_key")?,
            subscribe_port: connection
                .get::<_, String>("subscribe_port")?
                .parse::<u16>()
                .unwrap_or(DEFAULT_PUBLISH),
            request_port: match connection.get::<_, String>("request_port")?.parse::<u16>() {
                Ok(n) => n,
                Err(_) => DEFAULT_REQUEST,
            },
            workers: match connection.get::<_, String>("workers")?.parse::<u32>() {
                Ok(n) => n,
                Err(_) => DEFAULT_WORKERS,
            },
            use_ipv4: match logging.get::<_, bool>("use_ipv4") {
                Ok(n) => n,
                Err(_) => false,
            },
        };

        let lg = Logging {
            directory: {
                let mut d = logging
                    .get::<_, String>("data_directory")
                    .unwrap_or(DEFAULT_LOG_DIRECTORY.to_string())
                    .trim()
                    .to_string();
                if d.is_empty() {
                    d = DEFAULT_LOG_DIRECTORY.to_string()
                }
                if d.starts_with("/") {
                    d
                } else {
                    data_directory.clone() + "/" + d.as_str()
                }
            },
            file: match logging.get::<_, String>("file") {
                Ok(s) => s.trim().to_string(),
                Err(_) => DEFAULT_LOG_FILE.to_string(),
            },
            size: match logging.get::<_, String>("size")?.parse::<u64>() {
                Ok(n) => n,
                Err(_) => DEFAULT_LOG_SIZE,
            },
            count: match logging.get::<_, String>("count")?.parse::<u32>() {
                Ok(n) => n,
                Err(_) => DEFAULT_LOG_COUNT,
            },
            console: match logging.get::<_, bool>("console") {
                Ok(n) => n,
                Err(_) => false,
            },
            level: logging.get("level")?,
        };

        let result = Configuration {
            data_directory: data_directory,
            connection: cn,
            logging: lg,
        };
        Ok(result)
    })?;

    Ok(result)
}
