mod hash;
mod nft_create;
mod nft_fighting;
mod nft_transfer;

use ckb_crypto::secp::Privkey;
use ckb_fixed_hash::H256;
use ckb_testtool::context::Context;
use ckb_tool::ckb_types::{
    bytes::Bytes,
    core::TransactionView,
    packed::{self, WitnessArgs},
    prelude::*,
};

use lazy_static::lazy_static;
use openssl::base64;
use rand::{thread_rng, Rng};
use sha2::{Digest as SHA2Digest, Sha256};
use std::{env, fs, path::PathBuf, str::FromStr};

pub const SIGNATURE_SIZE: usize = 65;
pub const CHAIN_ID_ETH: u8 = 1;
pub const CHAIN_ID_EOS: u8 = 2;
pub const CHAIN_ID_TRON: u8 = 3;
pub const CHAIN_ID_BTC: u8 = 4;
pub const CHAIN_ID_DOGE: u8 = 5;

use sha3::{Digest, Keccak256};

use self::hash::blake2b_160;

const MAX_CYCLES: u64 = 100_000_000;

lazy_static! {
    pub static ref SOURLY_CAT_BIN: Bytes = Loader::default().load_binary("sourly_cat");
    pub static ref SECP256K1_DATA_BIN: Bytes =
        Bytes::from(&include_bytes!("../../../pw-lock/specs/cells/secp256k1_data")[..]);
    pub static ref KECCAK256_ALL_ACPL_BIN: Bytes =
        Bytes::from(&include_bytes!("../../../pw-lock/specs/cells/pw_anyone_can_pay")[..]);
}

const TEST_ENV_VAR: &str = "CAPSULE_TEST_ENV";

pub enum TestEnv {
    Debug,
    Release,
}

impl FromStr for TestEnv {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debug" => Ok(TestEnv::Debug),
            "release" => Ok(TestEnv::Release),
            _ => Err("no match"),
        }
    }
}

pub struct Loader(PathBuf);

impl Default for Loader {
    fn default() -> Self {
        let test_env = match env::var(TEST_ENV_VAR) {
            Ok(val) => val.parse().expect("test env"),
            Err(_) => TestEnv::Debug,
        };
        Self::with_test_env(test_env)
    }
}

impl Loader {
    fn with_test_env(env: TestEnv) -> Self {
        let load_prefix = match env {
            TestEnv::Debug => "debug",
            TestEnv::Release => "release",
        };
        let dir = env::current_dir().unwrap();
        let mut base_path = PathBuf::new();
        base_path.push(dir);
        base_path.push("..");
        base_path.push("build");
        base_path.push(load_prefix);
        Loader(base_path)
    }

    pub fn load_binary(&self, name: &str) -> Bytes {
        let mut path = self.0.clone();
        path.push(name);
        fs::read(path).expect("binary").into()
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct NFTData {
    name: [u8; 16],
    hash: [u8; 20],
    fishes: i32,
}

impl NFTData {
    fn new() -> Self {
        return NFTData {
            name: [0u8; 16],
            hash: [0u8; 20],
            fishes: 0,
        };
    }

    fn serialize(&self) -> [u8; 40] {
        let mut buf = [0u8; 40];
        buf[0..16].copy_from_slice(&self.name);
        buf[16..36].copy_from_slice(&self.hash);
        buf[36..40].copy_from_slice(&self.fishes.to_be_bytes());
        return buf;
    }

    fn gen_random_nft(lock_hash: &[u8]) -> NFTData {
        let mut nft = NFTData::new();

        //name
        let mut rand_name_bytes = [0u8; 4];
        let mut rng = thread_rng();
        rng.fill(&mut rand_name_bytes);
        let rand_name = base64::encode_block(&rand_name_bytes);
        nft.name[0..8].copy_from_slice(rand_name.as_bytes());

        //hash
        let mut conc: Vec<u8> = Vec::with_capacity(28);
        conc.extend(rand_name.as_bytes().iter());
        conc.extend(lock_hash.iter());
        nft.hash = blake2b_160(&conc);

        //fishes
        let stat: Statistics = nft.hash.into();
        //prevent add with overflow
        nft.fishes =
            100 - (stat.hp as i32 + stat.atk as i32 + stat.def as i32 + stat.lck as i32) / 5;
        nft
    }
}

impl From<&[u8]> for NFTData {
    fn from(data: &[u8]) -> Self {
        let mut name = [0u8; 16];
        name.copy_from_slice(&data[0..16]);
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&data[16..36]);
        let mut fishes_byte = [0u8; 4];
        fishes_byte.copy_from_slice(&data[36..40]);
        //采用大端序
        let fishes = i32::from_be_bytes(fishes_byte);
        NFTData {
            name: name,
            hash: hash,
            fishes: fishes,
        }
    }
}

#[derive(Debug)]
pub struct Statistics {
    hp: u8,
    atk: u8,
    def: u8,
    lck: u8,
}

impl From<[u8; 20]> for Statistics {
    fn from(hash: [u8; 20]) -> Self {
        //只看每五个字节的最后一个字节
        let hp = hash[4] % 100 + 1;
        let atk = hash[9] % 100 + 1;
        let def = hash[14] % 100 + 1;
        let lck = hash[19] % 100 + 1;
        return Statistics { hp, atk, def, lck };
    }
}

pub fn random_20bytes() -> Bytes {
    let mut rng = thread_rng();
    let mut buf = vec![0u8; 20];
    rng.fill(&mut buf[..]);
    Bytes::from(buf)
}

pub fn get_current_chain_id() -> u8 {
    if let Ok(v) = env::var("CHAIN_ID") {
        let chain_id = u8::from_str_radix(&v, 16).unwrap();
        chain_id
    } else {
        1
    }
}

pub fn is_compressed() -> bool {
    if let Ok(v) = env::var("COMPRESSED") {
        let id = u8::from_str_radix(&v, 16).unwrap();
        if id > 0 {
            true
        } else {
            false
        }
    } else {
        true
    }
}

pub fn sign_tx_keccak256(
    dummy: &mut Context,
    tx: TransactionView,
    key: &Privkey,
) -> TransactionView {
    sign_tx_keccak256_with_flag(dummy, tx, key, true)
}

pub fn sign_tx_keccak256_with_flag(
    dummy: &mut Context,
    tx: TransactionView,
    key: &Privkey,
    set_chain_flag: bool,
) -> TransactionView {
    let witnesses_len = tx.witnesses().len();
    sign_tx_by_input_group_keccak256_flag(dummy, tx, key, 0, witnesses_len, set_chain_flag)
}

pub fn sign_tx_by_input_group_keccak256(
    dummy: &mut Context,
    tx: TransactionView,
    key: &Privkey,
    begin_index: usize,
    len: usize,
) -> TransactionView {
    sign_tx_by_input_group_keccak256_flag(dummy, tx, key, begin_index, len, true)
}

pub fn sign_tx_by_input_group_keccak256_flag(
    _: &mut Context,
    tx: TransactionView,
    key: &Privkey,
    begin_index: usize,
    len: usize,
    set_chain_flag: bool,
) -> TransactionView {
    let tx_hash = tx.hash();
    let mut signed_witnesses: Vec<packed::Bytes> = tx
        .inputs()
        .into_iter()
        .enumerate()
        .map(|(i, _)| -> packed::Bytes {
            if i == begin_index {
                let mut blake2b = ckb_hash::new_blake2b();

                let mut message = [0u8; 32];

                let lock_size = match set_chain_flag {
                    true => SIGNATURE_SIZE + 1,
                    false => SIGNATURE_SIZE,
                };

                let start_index = match set_chain_flag {
                    true => 1,
                    false => 0,
                };

                let end_index = start_index + SIGNATURE_SIZE;

                blake2b.update(&tx_hash.raw_data());
                // digest the first witness
                let witness = WitnessArgs::new_unchecked(tx.witnesses().get(i).unwrap().unpack());
                let zero_lock: Bytes = {
                    let mut buf = Vec::new();
                    buf.resize(lock_size, 0);
                    buf.into()
                };
                let mut lock = [0u8; SIGNATURE_SIZE + 1];
                lock[0] = get_current_chain_id();

                let witness_for_digest = witness
                    .clone()
                    .as_builder()
                    .lock(Some(zero_lock).pack())
                    .build();
                let witness_len = witness_for_digest.as_bytes().len() as u64;
                println!("witness_len = {}", witness_len);
                blake2b.update(&witness_len.to_le_bytes());
                blake2b.update(&witness_for_digest.as_bytes());
                ((i + 1)..(i + len)).for_each(|n| {
                    let witness = tx.witnesses().get(n).unwrap();
                    let witness_len = witness.raw_data().len() as u64;
                    blake2b.update(&witness_len.to_le_bytes());
                    blake2b.update(&witness.raw_data());
                });
                blake2b.finalize(&mut message);

                // blake2b.finalize(&mut message);

                print!("chain-{}\n", get_current_chain_id());
                if get_current_chain_id() == CHAIN_ID_ETH {
                    // Ethereum personal sign prefix \x19Ethereum Signed Message:\n32
                    let prefix: [u8; 28] = [
                        0x19, 0x45, 0x74, 0x68, 0x65, 0x72, 0x65, 0x75, 0x6d, 0x20, 0x53, 0x69,
                        0x67, 0x6e, 0x65, 0x64, 0x20, 0x4d, 0x65, 0x73, 0x73, 0x61, 0x67, 0x65,
                        0x3a, 0x0a, 0x33, 0x32,
                    ];
                    let mut keccak_hasher = Keccak256::default();
                    keccak_hasher.input(&prefix);
                    keccak_hasher.input(&message);
                    message.copy_from_slice(&keccak_hasher.result()[0..32]);

                    let message1 = H256::from(message);

                    let sig = key.sign_recoverable(&message1).expect("sign");
                    lock[start_index..end_index].copy_from_slice(&sig.serialize().to_vec());
                } else if get_current_chain_id() == CHAIN_ID_TRON {
                    // Tron sign prefix \x19TRON Signed Message:\n32
                    let prefix: [u8; 24] = [
                        0x19, 0x54, 0x52, 0x4f, 0x4e, 0x20, 0x53, 0x69, 0x67, 0x6e, 0x65, 0x64,
                        0x20, 0x4d, 0x65, 0x73, 0x73, 0x61, 0x67, 0x65, 0x3a, 0x0a, 0x33, 0x32,
                    ];
                    let mut keccak_hasher = Keccak256::default();
                    keccak_hasher.input(&prefix);
                    keccak_hasher.input(&message);
                    message.copy_from_slice(&keccak_hasher.result()[0..32]);

                    let message1 = H256::from(message);

                    let sig = key.sign_recoverable(&message1).expect("sign");
                    lock[start_index..end_index].copy_from_slice(&sig.serialize().to_vec());
                } else if get_current_chain_id() == CHAIN_ID_EOS {
                    // EOS scatter.getArbitrarySignature() requires each word of message
                    // to be less than 12 characters. so insert blank char every 12 char for
                    // transaction message digest.
                    let mut message_hex = faster_hex::hex_string(&message).unwrap();
                    println!("message_hex {}", message_hex);
                    message_hex.insert_str(12, " ");
                    message_hex.insert_str(25, " ");
                    message_hex.insert_str(38, " ");
                    message_hex.insert_str(51, " ");
                    message_hex.insert_str(64, " ");
                    println!("message_hex {}", message_hex);

                    let mut sha256hasher = Sha256::default();
                    sha256hasher.update(&message_hex.as_bytes());

                    message.copy_from_slice(&sha256hasher.finalize().to_vec());
                    let message1 = H256::from(message);
                    let sig = key.sign_recoverable(&message1).expect("sign");
                    lock[start_index..end_index].copy_from_slice(&sig.serialize().to_vec());
                } else if get_current_chain_id() == CHAIN_ID_BTC {
                    let message_hex = faster_hex::hex_string(&message).unwrap();
                    println!("message_hex {}, len {}", message_hex, message_hex.len());

                    let mut sha256hasher = Sha256::default();
                    sha256hasher.update(b"\x18Bitcoin Signed Message:\n\x40");
                    sha256hasher.update(&message_hex);
                    message.copy_from_slice(&sha256hasher.finalize().to_vec());

                    let temp = Sha256::digest(&message).to_vec();
                    message.copy_from_slice(&temp);

                    let message1 = H256::from(message);
                    let sig = key.sign_recoverable(&message1).expect("sign");
                    let sig_vec = sig.serialize().to_vec();

                    let mut data = [0u8; SIGNATURE_SIZE];
                    if is_compressed() {
                        data[0] = sig_vec[64] + 27 + 4;
                    } else {
                        data[0] = sig_vec[64] + 27;
                    }
                    data[1..].copy_from_slice(&sig_vec[..64]);

                    lock[start_index..end_index].copy_from_slice(&data);
                } else if get_current_chain_id() == CHAIN_ID_DOGE {
                    let message_hex = faster_hex::hex_string(&message).unwrap();
                    println!("message_hex {}, len {}", message_hex, message_hex.len());
                    let mut sha256hasher = Sha256::default();
                    sha256hasher.update(b"\x19Dogecoin Signed Message:\n\x40");
                    sha256hasher.update(&message_hex);
                    message.copy_from_slice(&sha256hasher.finalize().to_vec());

                    let temp = Sha256::digest(&message).to_vec();
                    message.copy_from_slice(&temp);

                    let message1 = H256::from(message);
                    let sig = key.sign_recoverable(&message1).expect("sign");
                    let sig_vec = sig.serialize().to_vec();

                    let mut data = [0u8; SIGNATURE_SIZE];
                    if is_compressed() {
                        data[0] = sig_vec[64] + 27 + 4;
                    } else {
                        data[0] = sig_vec[64] + 27;
                    }
                    data[1..].copy_from_slice(&sig_vec[..64]);

                    lock[start_index..end_index].copy_from_slice(&data);
                }

                println!("lock is {}", faster_hex::hex_string(&lock).unwrap());

                let lock_vec = match set_chain_flag {
                    true => lock.to_vec(),
                    false => lock[..SIGNATURE_SIZE].to_vec(),
                };

                witness
                    .as_builder()
                    .lock(Some(Bytes::from(lock_vec)).pack())
                    .build()
                    .as_bytes()
                    .pack()
            } else {
                tx.witnesses().get(i).unwrap_or_default()
            }
        })
        .collect();
    for i in signed_witnesses.len()..tx.witnesses().len() {
        signed_witnesses.push(tx.witnesses().get(i).unwrap());
    }

    tx.as_advanced_builder()
        .set_witnesses(signed_witnesses)
        .build()
}
