use blake2b_rs::Blake2bBuilder;
use ckb_crypto::secp::Pubkey;
use ckb_fixed_hash::{H160, H256, H512};
use ckb_tool::ckb_types::bytes::Bytes;
use ripemd160::Digest;
use secp256k1::key;

use sha2::{Digest as SHA2Digest, Sha256};
use sha3::Keccak256;

pub const CKB_HASH_PERSONALIZATION: &[u8] = b"ckb-default-hash";
pub const BLANK_HASH: [u8; 32] = [
    68, 244, 198, 151, 68, 213, 248, 197, 93, 100, 32, 98, 148, 157, 202, 228, 155, 196, 231, 239,
    67, 211, 136, 197, 161, 47, 66, 181, 99, 61, 22, 62,
];

pub fn new_blake2b() -> blake2b_rs::Blake2b {
    Blake2bBuilder::new(32)
        .personal(CKB_HASH_PERSONALIZATION)
        .build()
}

fn inner_blake2b_256<T: AsRef<[u8]>>(s: T) -> [u8; 32] {
    let mut result = [0u8; 32];
    let mut blake2b = new_blake2b();
    blake2b.update(s.as_ref());
    blake2b.finalize(&mut result);
    result
}

pub fn blake2b_256<T: AsRef<[u8]>>(s: T) -> [u8; 32] {
    if s.as_ref().is_empty() {
        return BLANK_HASH;
    }
    inner_blake2b_256(s)
}

pub fn blake2b_160<T: AsRef<[u8]>>(s: T) -> [u8; 20] {
    let mut result = [0u8; 20];
    let hash = blake2b_256(s);
    result.copy_from_slice(&hash[0..20]);
    result
}

fn ripemd160(data: &[u8]) -> H160 {
    use ripemd160::Ripemd160;
    let digest: [u8; 20] = Ripemd160::digest(data).into();
    H160::from(digest)
}

fn sha256(data: &[u8]) -> H256 {
    let digest: [u8; 32] = Sha256::digest(data).into();
    H256::from(digest)
}

pub fn pubkey_uncompressed(pubkey: &Pubkey) -> Vec<u8> {
    let mut serialized = vec![4u8; 65];
    serialized[1..65].copy_from_slice(pubkey.as_ref());
    serialized
}

pub fn pubkey_compressed(pubkey: &Pubkey) -> Vec<u8> {
    pubkey.serialize()
}

pub fn ripemd_sha(serialized_pubkey: &[u8]) -> Bytes {
    Bytes::from(
        ripemd160(sha256(serialized_pubkey).as_bytes())
            .as_ref()
            .to_owned(),
    )
}

// pub fn eth160(message: &[u8]) -> Bytes {
pub fn eth160(pubkey1: Pubkey) -> Bytes {
    let prefix_key: [u8; 65] = {
        let mut temp = [4u8; 65];
        let h512: H512 = pubkey1.into();
        temp[1..65].copy_from_slice(h512.as_bytes());
        temp
    };
    let pubkey = key::PublicKey::from_slice(&prefix_key).unwrap();
    let message = Vec::from(&pubkey.serialize_uncompressed()[1..]);
    // println!("{}", faster_hex::hex_string(&message).unwrap());
    // println!("{}", faster_hex::hex_string(&message1).unwrap());

    let mut hasher = Keccak256::default();
    hasher.input(&message);

    Bytes::from(hasher.result().to_vec()).slice(12..32)
}
