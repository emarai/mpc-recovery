use ed25519_dalek::SecretKey;
use rand::rngs::OsRng;
use threshold_crypto::{PublicKeySet, SecretKeySet, SecretKeyShare};

pub(crate) mod client;
mod key_recovery;
mod leader_node;
pub mod msg;
mod oauth;
mod primitives;
mod sign_node;
mod transaction;

type NodeId = u64;

pub use leader_node::run as run_leader_node;
pub use sign_node::run as run_sign_node;

#[tracing::instrument(level = "debug", skip_all, fields(n = n, threshold = t))]
pub fn generate(
    n: usize,
    t: usize,
) -> anyhow::Result<(PublicKeySet, Vec<SecretKeyShare>, SecretKey)> {
    let sk_set = SecretKeySet::random(t - 1, &mut rand::thread_rng());
    let pk_set = sk_set.public_keys();
    tracing::debug!(public_key = ?pk_set.public_key());

    let mut sk_shares = Vec::new();
    for i in 1..=n {
        let sk_share = sk_set.secret_key_share(i);
        sk_shares.push(sk_share);
    }

    let mut csprng = OsRng {};
    let root_secret_key = SecretKey::generate(&mut csprng);

    Ok((pk_set, sk_shares, root_secret_key))
}
