mod constants;
mod primitives;
pub mod utils;

use core::panic;
use reqwest::{header::CONTENT_TYPE, Body};
use std::{time::Duration, vec};
use tracing_subscriber::{filter, prelude::*};

use constants::VALID_OIDC_PROVIDER_KEY;
use goose::prelude::*;
use mpc_recovery::{
    msg::{
        ClaimOidcRequest, MpcPkRequest, NewAccountRequest, SignRequest, UserCredentialsRequest,
        UserCredentialsResponse,
    },
    sign_node::oidc::OidcToken,
    transaction::CreateAccountOptions,
    utils::{
        claim_oidc_request_digest, sign_digest, sign_request_digest,
        user_credentials_request_digest,
    },
};
use near_crypto::SecretKey;
use near_primitives::{
    account::{AccessKey, AccessKeyPermission},
    borsh::BorshSerialize,
    delegate_action::DelegateAction,
    transaction::{Action, AddKeyAction},
    types::AccountId,
};
use primitives::UserSession;
use rand::{distributions::Alphanumeric, Rng};
use utils::build_send_and_check_request;

#[tokio::main]
async fn main() -> Result<(), GooseError> {
    let stdout_log = tracing_subscriber::fmt::layer().pretty();

    tracing_subscriber::registry()
        .with(stdout_log.with_filter(filter::LevelFilter::INFO))
        .init();

    GooseAttack::initialize()?
        .register_scenario(
            scenario!("registration")
                .register_transaction(transaction!(prepare_user_credentials).set_sequence(1))
                .register_transaction(transaction!(claim_oidc).set_sequence(2))
                .register_transaction(transaction!(new_account).set_sequence(3)),
        )
        .register_scenario(
            scenario!("registrationAndSign")
                .register_transaction(transaction!(prepare_user_credentials).set_sequence(1))
                .register_transaction(transaction!(claim_oidc).set_sequence(2))
                .register_transaction(transaction!(new_account).set_sequence(3))
                .register_transaction(transaction!(user_credentials).set_sequence(4))
                .register_transaction(
                    transaction!(sign)
                        .set_sequence(5)
                        .set_weight(1000) // In this scenario we are mostly testing /sign functionality
                        .expect("Failed to set weight"),
                ),
        )
        .register_scenario(
            scenario!("simpleClaimOidc")
                .register_transaction(transaction!(prepare_user_credentials).set_sequence(1))
                .register_transaction(
                    transaction!(claim_oidc)
                        .set_sequence(2)
                        .set_weight(100)
                        .expect("Failed to set weight"),
                ),
        )
        .register_scenario(
            scenario!("simpleUserCredentials")
                .register_transaction(transaction!(prepare_user_credentials).set_sequence(1))
                .register_transaction(transaction!(claim_oidc).set_sequence(2))
                .register_transaction(
                    transaction!(user_credentials)
                        .set_sequence(3)
                        .set_weight(100)
                        .expect("Failed to set weight"),
                ),
        )
        .register_scenario(
            scenario!("simpleMpcPublicKey").register_transaction(transaction!(mpc_public_key)),
        )
        .execute()
        .await?;

    Ok(())
}

async fn prepare_user_credentials(user: &mut GooseUser) -> TransactionResult {
    tracing::info!("prepare_user_credentials");
    // Generate 2 key pairs
    let fa_sk = SecretKey::from_random(near_crypto::KeyType::ED25519);
    let la_sk = SecretKey::from_random(near_crypto::KeyType::ED25519);

    // Create JWT with random sub (usually done by OIDC Provider)
    let oidc_token = OidcToken::new(&utils::create_jwt_token(
        VALID_OIDC_PROVIDER_KEY,
        constants::VALID_OIDC_AUD,
        constants::VALID_OIDC_ISS,
        None,
    ));

    // Generate random near account id
    let account_id_rand: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();

    let near_account_id: AccountId = format!("acc-{}.near", account_id_rand.to_lowercase())
        .try_into()
        .expect("Failed to generate random account Id");

    let session = UserSession {
        jwt_token: oidc_token,
        near_account_id,
        fa_sk,
        la_sk,
        recovery_pk: None,
    };

    user.set_session_data(session);

    Ok(())
}

async fn user_credentials(user: &mut GooseUser) -> TransactionResult {
    tracing::info!("user_credentials");
    let sesion = user
        .get_session_data::<UserSession>()
        .expect("Session Data must be set");

    let oidc_token = sesion.jwt_token.clone();
    let fa_sk = sesion.fa_sk.clone();
    let fa_pk = fa_sk.public_key();
    let la_sk = sesion.la_sk.clone();
    let near_account_id = sesion.near_account_id.clone();

    let user_credentials_request_digest =
        user_credentials_request_digest(&oidc_token, &fa_pk).expect("Failed to create digest");

    let user_credentials_frp_signature =
        sign_digest(&user_credentials_request_digest, &fa_sk).expect("Failed to sign digest");

    let user_credentials_request = UserCredentialsRequest {
        oidc_token: oidc_token.clone(),
        frp_public_key: fa_pk,
        frp_signature: user_credentials_frp_signature,
    };

    let body_json =
        serde_json::to_string(&user_credentials_request).expect("json serialization failed");

    let body = Body::from(body_json.to_owned());
    let request_builder = user
        .get_request_builder(&GooseMethod::Post, "user_credentials")?
        .body(body)
        .header(CONTENT_TYPE, "application/json")
        .timeout(Duration::from_secs(10));

    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();

    let goose_responce = user.request(goose_request).await?;

    let response = goose_responce.response.expect("Expected response ... .");

    let user_credentials_response = response
        .json::<UserCredentialsResponse>()
        .await
        .expect("Failed to parse user credentials response");

    if let UserCredentialsResponse::Ok { recovery_pk } = user_credentials_response {
        tracing::info!("UserCredentialsResponce has Ok, setting session data");
        let session = UserSession {
            jwt_token: oidc_token,
            near_account_id,
            fa_sk,
            la_sk,
            recovery_pk: Some(recovery_pk),
        };
        user.set_session_data(session);
    } else {
        panic!(
            "UserCredentialsResponce has Error: {:?}",
            user_credentials_response
        );
    }

    Ok(())
}

async fn mpc_public_key(user: &mut GooseUser) -> TransactionResult {
    tracing::info!("mpc_public_key");
    let body_json = serde_json::to_string(&MpcPkRequest {}).expect("json serialization failed");
    build_send_and_check_request(user, "mpc_public_key", &body_json).await
}

async fn claim_oidc(user: &mut GooseUser) -> TransactionResult {
    tracing::info!("claim_oidc");
    let sesion = user
        .get_session_data::<UserSession>()
        .expect("Session Data must be set");
    let oidc_token_hash = sesion.jwt_token.digest_hash();
    let frp_secret_key = sesion.fa_sk.clone();
    let frp_public_key = frp_secret_key.public_key();

    let request_digest = claim_oidc_request_digest(&oidc_token_hash, &frp_public_key)
        .expect("Failed to create digest");
    let frp_signature =
        sign_digest(&request_digest, &frp_secret_key).expect("Failed to sign digest");

    let claim_oidc_request = ClaimOidcRequest {
        oidc_token_hash: oidc_token_hash.to_owned(),
        frp_public_key,
        frp_signature,
    };

    let body_json = serde_json::to_string(&claim_oidc_request).expect("json serialization failed");

    build_send_and_check_request(user, "claim_oidc", &body_json).await
}

async fn new_account(user: &mut GooseUser) -> TransactionResult {
    let sesion = user
        .get_session_data::<UserSession>()
        .expect("Session Data must be set");
    let oidc_token = sesion.jwt_token.clone();
    let fa_secret_key = sesion.fa_sk.clone();
    let fa_public_key = fa_secret_key.public_key();
    let user_account_id = sesion.near_account_id.clone();

    let create_account_options = CreateAccountOptions {
        full_access_keys: Some(vec![fa_public_key.clone()]),
        limited_access_keys: None,
        contract_bytes: None,
    };

    let user_credentials_request_digest =
        user_credentials_request_digest(&oidc_token, &fa_public_key)
            .expect("Failed to create digest");

    let user_credentials_frp_signature =
        sign_digest(&user_credentials_request_digest, &fa_secret_key)
            .expect("Failed to sign digest");

    let new_account_request = NewAccountRequest {
        near_account_id: user_account_id,
        create_account_options,
        oidc_token: sesion.jwt_token.clone(),
        user_credentials_frp_signature,
        frp_public_key: fa_public_key,
    };

    let body_json = serde_json::to_string(&new_account_request).expect("json serialization failed");
    build_send_and_check_request(user, "new_account", &body_json).await
}

async fn sign(user: &mut GooseUser) -> TransactionResult {
    tracing::info!("sign");
    let session = user
        .get_session_data::<UserSession>()
        .expect("Session Data must be set");
    let oidc_token = session.jwt_token.clone();
    let fa_secret_key = session.fa_sk.clone();
    let fa_public_key = fa_secret_key.public_key();
    let account_id = session.near_account_id.clone();
    let recovery_pk = session
        .recovery_pk
        .clone()
        .expect("Recovery PK must be set before calling /sign");

    let new_secret_key = SecretKey::from_random(near_crypto::KeyType::ED25519);
    let new_public_key = new_secret_key.public_key();

    let nonce = 0; // Set real nonce in case transaction is entend to be executed
    let block_height = 0; // Set real block height in case transaction is entend to be executed

    let add_key_delegate_action = DelegateAction {
        sender_id: account_id.clone(),
        receiver_id: account_id.clone(),
        actions: vec![Action::AddKey(AddKeyAction {
            public_key: new_public_key.clone(),
            access_key: AccessKey {
                nonce: 0,
                permission: AccessKeyPermission::FullAccess,
            },
        })
        .try_into()
        .unwrap()],
        nonce,
        max_block_height: block_height + 100,
        public_key: recovery_pk,
    };

    let sign_request_digest =
        sign_request_digest(&add_key_delegate_action, &oidc_token, &fa_public_key)
            .expect("Failed to create digest");
    let sign_request_frp_signature =
        sign_digest(&sign_request_digest, &fa_secret_key).expect("Failed to sign digest");

    let user_credentials_request_digest =
        user_credentials_request_digest(&oidc_token, &fa_public_key)
            .expect("Failed to create digest");
    let user_credentials_frp_signature =
        sign_digest(&user_credentials_request_digest, &fa_secret_key)
            .expect("Failed to sign digest");

    let sign_request = SignRequest {
        delegate_action: add_key_delegate_action
            .try_to_vec()
            .expect("Failed to serialize delegate action"),
        oidc_token,
        frp_signature: sign_request_frp_signature,
        user_credentials_frp_signature,
        frp_public_key: fa_public_key,
    };

    let body_json = serde_json::to_string(&sign_request).expect("json serialization failed");
    build_send_and_check_request(user, "sign", &body_json).await
}
