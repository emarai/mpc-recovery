use std::str::FromStr;

use digest::Digest;
use hex_literal::hex;
use k256::elliptic_curve::group::GroupEncoding;
use k256::elliptic_curve::sec1::FromEncodedPoint;
use k256::{self, AffinePoint, EncodedPoint, Scalar, Secp256k1};
use mpc_recovery_node::util::AffinePointExt;
use mpc_recovery_node::{kdf, types::PublicKey};
use near_primitives::types::AccountId;
use ripemd160::Ripemd160;
use sha2::Sha256;
use rust_base58::{ToBase58, FromBase58};


pub fn sha256(key: Vec<u8>) -> Vec<u8> {
    let mut hasher = Sha256::new(); // create a Sha256 object
    hasher.input(key); // write input message
    hasher.result().to_vec() // read hash digest and consume hasher
}

pub fn ripemd160(key: Vec<u8>) -> Vec<u8> {
    let mut hasher = Ripemd160::new(); // create a RIPEMD-160 hasher instance
    hasher.input(key); // process input message
    hasher.result().to_vec() // acquire hash digest in the form of GenericArray, which in this case is equivalent to [u8; 20]
}

fn main() {
    let account_id = AccountId::from_str("irfi.near").unwrap();
    // https://github.com/near/fast-auth-signer/blob/f990caa574a413aa0dec4476d98230ea8c84224b/packages/near-fast-auth-signer/src/utils/config.ts#L16
    let bytes: &[u8] = &hex!(
        "0479BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798
            483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8"
    );

    let bytes: &[u8] = &vec![
        0x03, 187, 24, 21, 82, 105, 165, 254, 26, 167, 89, 195, 236, 44, 83, 69, 87, 30, 151, 139,
        229, 233, 182, 65, 230, 7, 234, 204, 91, 38, 70, 254, 254,
    ]
    .to_vec();

    println!("bytes {:?}", bytes);

    let point = EncodedPoint::from_bytes(bytes).unwrap();
    println!("point {:?}", point);

    let public_key = AffinePoint::from_encoded_point(&point).unwrap();
    let epsilon = kdf::derive_epsilon(&account_id, ",bitcoin-2");
    let pk = kdf::derive_key(public_key, epsilon).to_bytes();
    println!("pk {:?}", pk);

    let new_point = EncodedPoint::from_bytes(pk).unwrap();
    let new_public_key = AffinePoint::from_encoded_point(&new_point).unwrap();

    // https://github.com/Andrii32/rust-bitcoin-explore/blob/master/src/main.rs
    println!("1 - Take the corresponding public key generated with it");
    println!("new point {:?}", new_point);
    println!("new public key {:?}", pk);
    println!();

    println!("2 - Perform SHA-256 hashing on the public key");
    let pbk_c_sha256 = sha256(pk.to_vec());
    println!();

    println!("3 - Perform RIPEMD-160 hashing on the result of SHA-256");
    let pbk_c_sha256_ripemd160 = ripemd160(pbk_c_sha256);
    println!("COMPRESSED:   {:?}", hex::encode(&pbk_c_sha256_ripemd160));

    println!("4 - Add version byte in front of RIPEMD-160 hash (0x00 for Main Network)");
    let mut pbk_c_sha256_ripemd160_mn = pbk_c_sha256_ripemd160.to_vec();
    pbk_c_sha256_ripemd160_mn.insert(0, 0x00);
    println!(
        "COMPRESSED:   {:?}",
        hex::encode(&pbk_c_sha256_ripemd160_mn)
    );

    println!("\n(note that below steps are the Base58Check encoding, which has multiple library options available implementing it)\n");

    println!("5 - Perform SHA-256 hash on the extended RIPEMD-160 result");
    let pbk_c_sha256_ripemd160_mn_sha256 = sha256(pbk_c_sha256_ripemd160_mn.to_vec());
    println!(
        "COMPRESSED:   {:?}",
        hex::encode(&pbk_c_sha256_ripemd160_mn_sha256)
    );

    println!("6 - Perform SHA-256 hash on the result of the previous SHA-256 hash");
    let pbk_c_sha256_ripemd160_mn_sha256_sha256 = sha256(pbk_c_sha256_ripemd160_mn_sha256);
    println!(
        "COMPRESSED:   {:?}",
        hex::encode(&pbk_c_sha256_ripemd160_mn_sha256_sha256)
    );

    println!("7 - Take the first 4 bytes of the second SHA-256 hash. This is the address checksum");
    let pbk_c_sha256_ripemd160_mn_sha256_sha256_checksum =
        &pbk_c_sha256_ripemd160_mn_sha256_sha256[0..4];
    println!(
        "COMPRESSED:   {:?}",
        hex::encode(&pbk_c_sha256_ripemd160_mn_sha256_sha256_checksum)
    );

    println!("8 - Add the 4 checksum bytes from stage 7 at the end of extended RIPEMD-160 hash from stage 4. This is the 25-byte binary Bitcoin Address.");
    let mut pbk_c_sha256_ripemd160_mn_extended = pbk_c_sha256_ripemd160_mn.to_vec();
    pbk_c_sha256_ripemd160_mn_extended.extend(pbk_c_sha256_ripemd160_mn_sha256_sha256_checksum);
    println!(
        "COMPRESSED:   {:?}",
        hex::encode(&pbk_c_sha256_ripemd160_mn_extended)
    );

    println!("9 - Convert the result from a byte string into a base58 string using Base58Check encoding. This is the most commonly used Bitcoin Address format");
    let pbk_c_sha256_ripemd160_mn_extended_base58 = pbk_c_sha256_ripemd160_mn_extended.to_base58();
    println!(
        "COMPRESSED:   {:?}",
        pbk_c_sha256_ripemd160_mn_extended_base58
    );
}
