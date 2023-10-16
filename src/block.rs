// block.rs

//use argon2::{Config, ThreadMode, Variant, Version};
use base64;
use base64_serde::base64_serde_type;
use bytes::BufMut;
use hex_serde;
use serde_aux::prelude::deserialize_number_from_string;
use serde_derive::{Deserialize, Serialize};

use argonautica::config::{Backend, Variant, Version};
use argonautica::Hasher;
//use futures_cpupool::CpuPool;

base64_serde_type!(Base64Standard, base64::engine::general_purpose::STANDARD);
#[derive(Debug, Serialize, Deserialize)]
pub struct Header {
    #[serde(rename = "version")]
    pub version: u16,

    #[serde(rename = "transactionCount")]
    pub transaction_count: u16,

    #[serde(rename = "number", deserialize_with = "deserialize_number_from_string")]
    pub number: u64,

    #[serde(rename = "previousBlock", with = "hex_serde")]
    pub previous_block: [u8; 32],

    #[serde(rename = "merkleRoot", with = "hex_serde")]
    pub merkle_root: [u8; 32],

    #[serde(
        rename = "timestamp",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub timestamp: u64,

    #[serde(rename = "difficulty", with = "hex_serde")]
    pub difficulty: [u8; 8],

    #[serde(rename = "nonce", with = "hex_serde")]
    pub nonce: [u8; 8],
}

impl From<Header> for bytes::Bytes {
    fn from(h: Header) -> Self {
        let mut buf = bytes::BytesMut::with_capacity(100);
        buf.put_u16_le(h.version);
        buf.put_u16_le(h.transaction_count);
        buf.put_u64_le(h.number);
        buf.put_slice(&h.previous_block);
        buf.put_slice(&h.merkle_root);
        buf.put_u64_le(h.timestamp);
        buf.put_slice(&h.difficulty);
        buf.freeze()
    }
}

// pub fn block_digest(data: &[u8]) -> std::vec::Vec<u8> {
//     // const (
//     // 	digestMode        = argon2.ModeArgon2d
//     // 	digestMemory      = 1 << 17 // 128 MiB
//     // 	digestParallelism = 1
//     // 	digestIterations  = 4
//     // 	digestVersion     = argon2.Version13
//     // )
//     let config = Config {
//         variant: Variant::Argon2d,
//         version: Version::Version13,
//         mem_cost: 1 << 17,
//         time_cost: 4,
//         lanes: 1,
//         thread_mode: ThreadMode::from_threads(1),
//         secret: &[],
//         ad: &[],
//         hash_length: 32,
//     };

//     argon2::hash_raw(data, data, &config).unwrap()
// }

pub fn block_digest(data: &[u8]) -> std::vec::Vec<u8> {
    let mut hasher = Hasher::default();
    hasher
        .configure_backend(Backend::C)
        //.configure_cpu_pool(CpuPool::new(2))
        .configure_hash_len(32)
        .configure_iterations(4)
        .configure_lanes(1)
        .configure_memory_size(1 << 17)
        .configure_password_clearing(false)
        .configure_secret_key_clearing(false)
        .configure_threads(1)
        .configure_variant(Variant::Argon2d)
        .configure_version(Version::_0x13) // Default is `Version::_0x13`
        .opt_out_of_secret_key(true);

    let hash = hasher
        .with_password(data)
        .with_salt(data)
        .hash_raw()
        .unwrap();
    hash.raw_hash_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_pack() {
        let live_genesis_block: [u8; 231] = [
            0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x63, 0x8c, 0x15, 0x9c, 0x1f, 0x11, 0x3f, 0x70, 0xa9, 0x86, 0x6d, 0x9a,
            0x9e, 0x52, 0xe9, 0xef, 0xe9, 0xb9, 0x92, 0x08, 0x48, 0xad, 0x1d, 0xf3, 0x48, 0x51,
            0xbe, 0x8a, 0x56, 0x2a, 0x99, 0x8d, 0xb7, 0x9a, 0x80, 0x56, 0x00, 0x00, 0x00, 0x00,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x11, 0x5a, 0x38, 0xbf, 0x3a, 0x90,
            0x9f, 0xe1, 0x01, 0x00, 0x14, 0x44, 0x4f, 0x57, 0x4e, 0x20, 0x74, 0x68, 0x65, 0x20,
            0x52, 0x41, 0x42, 0x42, 0x49, 0x54, 0x20, 0x68, 0x6f, 0x6c, 0x65, 0x21, 0x11, 0x4a,
            0x65, 0xf1, 0xd2, 0x06, 0x50, 0x08, 0x12, 0x76, 0xf0, 0x1d, 0xf4, 0x3e, 0x70, 0x55,
            0x4e, 0x95, 0x49, 0x8f, 0x37, 0x78, 0xe5, 0x6d, 0xaa, 0x2c, 0x49, 0x82, 0x03, 0xae,
            0x9c, 0x70, 0xe6, 0xf4, 0xca, 0xb9, 0xd2, 0xd2, 0xcc, 0xdd, 0xb4, 0x4c, 0x40, 0xc2,
            0xa3, 0x84, 0xeb, 0xc9, 0x01, 0xa1, 0x8a, 0x13, 0xa2, 0x70, 0xaa, 0x9f, 0x5e, 0x08,
            0x06, 0x77, 0xd7, 0xab, 0x2f, 0xd8, 0x88, 0xa5, 0xf6, 0x57, 0xd2, 0xc6, 0xd4, 0x69,
            0x2e, 0x6f, 0xcd, 0xe7, 0x1c, 0x04, 0xb9, 0x1b, 0xe1, 0x40, 0x0e, 0x7c, 0x1e, 0x8d,
            0x5e, 0x2b, 0x34, 0x83, 0xc4, 0x77, 0xfe, 0xa1, 0x7b, 0xc1, 0xde, 0xe0, 0x05, 0xcc,
            0x8d, 0x4d, 0xf8, 0x62, 0x77, 0x0d, 0x0c,
        ];
        let live_genesis_digest: [u8; 32] = [
            0x5c, 0x93, 0xf7, 0x39, 0xeb, 0x01, 0xcd, 0xde, 0x30, 0x55, 0x79, 0xf0, 0x3c, 0xcf,
            0xb3, 0x7a, 0x74, 0x29, 0x71, 0x31, 0x3f, 0xf9, 0x8d, 0x35, 0xb4, 0xc0, 0x7c, 0x43,
            0x8f, 0xaf, 0x12, 0x00,
        ];

        let h = Header {
            version: 1,
            transaction_count: 1,
            number: 1,
            previous_block: [0; 32],
            merkle_root: [
                0x63, 0x8c, 0x15, 0x9c, 0x1f, 0x11, 0x3f, 0x70, 0xa9, 0x86, 0x6d, 0x9a, 0x9e, 0x52,
                0xe9, 0xef, 0xe9, 0xb9, 0x92, 0x08, 0x48, 0xad, 0x1d, 0xf3, 0x48, 0x51, 0xbe, 0x8a,
                0x56, 0x2a, 0x99, 0x8d,
            ],
            timestamp: 0x56809ab7,

            difficulty: [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00],
            nonce: [0x11, 0x5a, 0x38, 0xbf, 0x3a, 0x90, 0x9f, 0xe1],
        };

        let buf = bytes::Bytes::from(h);
        assert_eq!(buf.len(), 100 - 8);
        assert_eq!(live_genesis_block[0..92], buf[..]);

        let nonce: u64 = 0xe19f903abf385a11;

        use bytes::BufMut;
        let mut buf2 = bytes::BytesMut::with_capacity(100);
        buf2.put_slice(&buf);
        buf2.put_u64_le(nonce);
        assert_eq!(buf2.len(), 100);
        assert_eq!(live_genesis_block[0..100], buf2[..]);

        let digest = block_digest(&buf2);
        assert_eq!(digest, live_genesis_digest);
    }

    // #[test]
    // fn test_two() {
    //     // stuff here
    //     assert_eq!(bad_add(1, 2), 3);
    // }
}
