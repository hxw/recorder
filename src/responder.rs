// responder.rs

use base64::STANDARD;
use base64_serde::base64_serde_type;
use serde_derive::{Deserialize, Serialize};
use simple_error::bail;

base64_serde_type!(Base64Standard, STANDARD);

type MyResult<T> = Result<T, Box<dyn std::error::Error>>;

use super::block;

#[derive(Debug, Serialize, Deserialize)]
pub struct Job {
    #[serde(rename = "job", alias = "Job")]
    job: String,

    #[serde(rename = "header", alias = "Header")]
    header: block::Header,

    #[serde(rename = "txZero", alias = "TxZero", with = "Base64Standard")]
    tx_zero: Vec<u8>,

    //#[serde(rename = "txIds", with = "hex_serde")]
    #[serde(rename = "txIds", alias = "TxIds")]
    tx_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    #[serde(rename = "request")]
    pub request: String,

    #[serde(rename = "job")]
    pub job: String,

    #[serde(rename = "packed", with = "Base64Standard")]
    pub packed: Vec<u8>,
}

pub fn send_job(
    set: i64,
    s: &str,
    txs: &mut Vec<spmc::Sender<(bytes::Bytes, u64, std::string::String)>>,
) -> MyResult<()> {
    let p: Job = serde_json::from_str(s)?;

    // debugging
    log::debug!("C{}: job:    {}", set, p.job);
    log::trace!("C{}: tx_0:   {:02x?}", set, p.tx_zero);
    log::debug!("C{}: header: {:?}", set, p.header);

    let h = p.header;

    let mut nnn = u64::from_le_bytes(h.nonce);

    let buf = bytes::Bytes::from(h);
    if buf.len() != 100 - 8 {
        bail!("block header wrong");
    }

    let mut w = 1;
    for tx in txs.iter_mut() {
        log::info!("C{}: send: {}  nonce: {:016x}", set, w, nnn);
        let blk = buf.clone();
        tx.send((blk, nnn, p.job.clone()))?;
        nnn += 0x100000000;
        w += 1;
    }

    Ok(())
}
