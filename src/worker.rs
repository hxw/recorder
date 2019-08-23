// worker.rs

use bytes::BufMut;
use log;
use std::time::{Duration, Instant};

use super::block;
use super::responder;

// to limit the hashing if no more jobs (2 minutes)
const MAXIMUM_HASH_SECONDS: u64 = 120;

pub struct Result {
    pub join_handles: Vec<std::thread::JoinHandle<()>>,
    pub channel_txs: Vec<spmc::Sender<(bytes::Bytes, u64, String)>>,
}

pub fn create_workers(workers: u32, tx: std::sync::mpsc::Sender<responder::Response>) -> Result {
    let hash_count = std::sync::atomic::AtomicU64::new(0);
    let arc_hash_count = std::sync::Arc::new(hash_count);

    let mut result = Result {
        join_handles: Vec::new(),
        channel_txs: Vec::new(),
    };

    for w in 1..=workers {
        let (subscribe_tx, subscribe_rx) = spmc::channel::<(bytes::Bytes, u64, String)>();

        result.channel_txs.push(subscribe_tx);
        let tx = tx.clone();
        let hash_count = arc_hash_count.clone();
        result.join_handles.push(std::thread::spawn(move || {
            'waiting: loop {
                log::debug!("{}: waiting..", w);
                let (mut blk, mut nonce, mut job) = subscribe_rx.recv().unwrap();

                // let mut hex_blk = String::new();
                // let _res = blk.write_hex(&mut hex_blk);
                // log::info!(
                //     "{}:  blk: {}  nonce: 0x{:x}  job: {}",
                //     w, hex_blk, nonce, job
                // );

                'receiving: loop {
                    log::debug!("{}: start hashing", w);
                    let mut i = 0;
                    let start = Instant::now();
                    let end = start + Duration::new(MAXIMUM_HASH_SECONDS, 0);
                    let mut wait = false;

                    'hashing: loop {
                        let mut buf = bytes::BytesMut::with_capacity(100);
                        buf.put_slice(&blk);
                        buf.put_u64_le(nonce);
                        assert_eq!(buf.len(), 100);

                        let hg = block::block_digest(&buf);
                        i += 1;

                        // check little_endian MSB
                        if hg[31] == 0 {
                            log::trace!("{}:  hg: {:02x?}  nonce: {:016x}", w, hg, nonce);

                            let response = responder::Response {
                                request: "block.nonce".to_string(),
                                job: job.clone(),
                                packed: nonce.to_le_bytes().to_vec(),
                            };
                            tx.send(response).unwrap();
                        }
                        if Instant::now() > end {
                            wait = true;
                            break 'hashing;
                        }
                        nonce += 1;

                        match subscribe_rx.try_recv() {
                            Ok((b, n, j)) => {
                                blk = b;
                                nonce = n;
                                job = j;
                                break 'hashing;
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => continue 'hashing,
                            Err(_) => break 'waiting,
                        };
                    }
                    hash_count.fetch_add(i, std::sync::atomic::Ordering::SeqCst);
                    let duration = start.elapsed();
                    let elapsed = duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9;
                    let average = i as f64 / elapsed;
                    log::info!(
                        "{}:  hashes: {}  in: {:6.2}  average: {:7.3}",
                        w,
                        i,
                        elapsed,
                        average
                    );
                    if wait {
                        break 'receiving;
                    }
                }
            }
            log::debug!("worker: {}  failed", w);
        }));
    }

    result
}
