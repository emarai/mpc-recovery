pub mod primitives;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, AccountId, PanicOnDefault, Promise, PromiseOrValue, PublicKey};
use primitives::{CandidateInfo, Candidates, Participants, PkVotes, Votes};
use std::collections::{BTreeMap, HashSet};

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug)]
pub struct InitializingContractState {
    pub candidates: Candidates,
    pub threshold: usize,
    pub pk_votes: PkVotes,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug)]
pub struct RunningContractState {
    pub epoch: u64,
    pub participants: Participants,
    pub threshold: usize,
    pub public_key: PublicKey,
    pub candidates: Candidates,
    pub join_votes: Votes,
    pub leave_votes: Votes,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug)]
pub struct ResharingContractState {
    pub old_epoch: u64,
    pub old_participants: Participants,
    // TODO: only store diff to save on storage
    pub new_participants: Participants,
    pub threshold: usize,
    pub public_key: PublicKey,
    pub finished_votes: HashSet<AccountId>,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug)]
pub enum ProtocolContractState {
    NotInitialized,
    Initializing(InitializingContractState),
    Running(RunningContractState),
    Resharing(ResharingContractState),
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct MpcContract {
    protocol_state: ProtocolContractState,
    pending_requests: LookupMap<[u8; 32], Option<(String, String)>>,
}

#[near_bindgen]
impl MpcContract {
    #[init(ignore_state)]
    pub fn init(threshold: usize, candidates: BTreeMap<AccountId, CandidateInfo>) -> Self {
        MpcContract {
            protocol_state: ProtocolContractState::Initializing(InitializingContractState {
                candidates: Candidates { candidates },
                threshold,
                pk_votes: PkVotes::new(),
            }),
            pending_requests: LookupMap::new(b"m"),
        }
    }

    pub fn state(self) -> ProtocolContractState {
        self.protocol_state
    }

    pub fn join(
        &mut self,
        url: String,
        cipher_pk: primitives::hpke::PublicKey,
        sign_pk: PublicKey,
    ) {
        match &mut self.protocol_state {
            ProtocolContractState::Running(RunningContractState {
                participants,
                candidates,
                ..
            }) => {
                let signer_account_id = env::signer_account_id();
                if participants.contains_key(&signer_account_id) {
                    env::panic_str("this participant is already in the participant set");
                }
                candidates.insert(
                    signer_account_id.clone(),
                    CandidateInfo {
                        account_id: signer_account_id,
                        url,
                        cipher_pk,
                        sign_pk,
                    },
                );
            }
            _ => env::panic_str("protocol state can't accept new participants right now"),
        }
    }

    pub fn vote_join(&mut self, candidate_account_id: AccountId) -> bool {
        match &mut self.protocol_state {
            ProtocolContractState::Running(RunningContractState {
                epoch,
                participants,
                threshold,
                public_key,
                candidates,
                join_votes,
                ..
            }) => {
                let signer_account_id = env::signer_account_id();
                if !participants.contains_key(&signer_account_id) {
                    env::panic_str("calling account is not in the participant set");
                }
                let candidate_info = candidates
                    .get(&candidate_account_id)
                    .unwrap_or_else(|| env::panic_str("candidate is not registered"));
                let voted = join_votes.entry(candidate_account_id.clone());
                voted.insert(signer_account_id);
                if voted.len() >= *threshold {
                    let mut new_participants = participants.clone();
                    new_participants
                        .insert(candidate_account_id.clone(), candidate_info.clone().into());
                    self.protocol_state =
                        ProtocolContractState::Resharing(ResharingContractState {
                            old_epoch: *epoch,
                            old_participants: participants.clone(),
                            new_participants,
                            threshold: *threshold,
                            public_key: public_key.clone(),
                            finished_votes: HashSet::new(),
                        });
                    true
                } else {
                    false
                }
            }
            _ => env::panic_str("protocol state can't accept new participants right now"),
        }
    }

    pub fn vote_leave(&mut self, acc_id_to_leave: AccountId) -> bool {
        match &mut self.protocol_state {
            ProtocolContractState::Running(RunningContractState {
                epoch,
                participants,
                threshold,
                public_key,
                leave_votes,
                ..
            }) => {
                let signer_account_id = env::signer_account_id();
                if !participants.contains_key(&signer_account_id) {
                    env::panic_str("calling account is not in the participant set");
                }
                if !participants.contains_key(&acc_id_to_leave) {
                    env::panic_str("account to leave is not in the participant set");
                }
                let voted = leave_votes.entry(acc_id_to_leave.clone());
                voted.insert(signer_account_id);
                if voted.len() >= *threshold {
                    let mut new_participants = participants.clone();
                    new_participants.remove(&acc_id_to_leave);
                    self.protocol_state =
                        ProtocolContractState::Resharing(ResharingContractState {
                            old_epoch: *epoch,
                            old_participants: participants.clone(),
                            new_participants,
                            threshold: *threshold,
                            public_key: public_key.clone(),
                            finished_votes: HashSet::new(),
                        });
                    true
                } else {
                    false
                }
            }
            _ => env::panic_str("protocol state can't kick participants right now"),
        }
    }

    pub fn vote_pk(&mut self, public_key: PublicKey) -> bool {
        match &mut self.protocol_state {
            ProtocolContractState::Initializing(InitializingContractState {
                candidates,
                threshold,
                pk_votes,
            }) => {
                let signer_account_id = env::signer_account_id();
                if !candidates.contains_key(&signer_account_id) {
                    env::panic_str("calling account is not in the participant set");
                }
                let voted = pk_votes.entry(public_key.clone());
                voted.insert(signer_account_id);
                if voted.len() >= *threshold {
                    self.protocol_state = ProtocolContractState::Running(RunningContractState {
                        epoch: 0,
                        participants: candidates.clone().into(),
                        threshold: *threshold,
                        public_key,
                        candidates: Candidates::new(),
                        join_votes: Votes::new(),
                        leave_votes: Votes::new(),
                    });
                    true
                } else {
                    false
                }
            }
            ProtocolContractState::Running(state) if state.public_key == public_key => true,
            ProtocolContractState::Resharing(state) if state.public_key == public_key => true,
            _ => env::panic_str("can't change public key anymore"),
        }
    }

    pub fn vote_reshared(&mut self, epoch: u64) -> bool {
        match &mut self.protocol_state {
            ProtocolContractState::Resharing(ResharingContractState {
                old_epoch,
                old_participants,
                new_participants,
                threshold,
                public_key,
                finished_votes,
            }) => {
                if *old_epoch + 1 != epoch {
                    env::panic_str("mismatched epochs");
                }
                let signer_account_id = env::signer_account_id();
                if !old_participants.contains_key(&signer_account_id) {
                    env::panic_str("calling account is not in the old participant set");
                }
                finished_votes.insert(signer_account_id);
                if finished_votes.len() >= *threshold {
                    self.protocol_state = ProtocolContractState::Running(RunningContractState {
                        epoch: *old_epoch + 1,
                        participants: new_participants.clone(),
                        threshold: *threshold,
                        public_key: public_key.clone(),
                        candidates: Candidates::new(),
                        join_votes: Votes::new(),
                        leave_votes: Votes::new(),
                    });
                    true
                } else {
                    false
                }
            }
            ProtocolContractState::Running(state) => {
                if state.epoch == epoch {
                    true
                } else {
                    env::panic_str("protocol is not resharing right now")
                }
            }
            _ => env::panic_str("protocol is not resharing right now"),
        }
    }

    #[allow(unused_variables)]
    pub fn sign(&mut self, payload: [u8; 32], path: String) -> Promise {
        match self.pending_requests.get(&payload) {
            None => {
                self.pending_requests.insert(&payload, &None);
                env::log_str(&serde_json::to_string(&near_sdk::env::random_seed_array()).unwrap());
                Self::ext(env::current_account_id()).sign_helper(payload, 0)
            }
            Some(_) => env::panic_str("Signature for this payload already requested"),
        }
    }

    #[private]
    pub fn sign_helper(
        &mut self,
        payload: [u8; 32],
        depth: usize,
    ) -> PromiseOrValue<(String, String)> {
        if let Some(signature) = self.pending_requests.get(&payload) {
            match signature {
                Some(signature) => {
                    self.pending_requests.remove(&payload);
                    PromiseOrValue::Value(signature)
                }
                None => {
                    env::log_str(&format!("not ready yet (depth={})", depth));
                    let account_id = env::current_account_id();
                    PromiseOrValue::Promise(Self::ext(account_id).sign_helper(payload, depth + 1))
                }
            }
        } else {
            env::panic_str("unexpected request");
        }
    }

    pub fn respond(&mut self, payload: [u8; 32], big_r: String, s: String) {
        self.pending_requests.insert(&payload, &Some((big_r, s)));
    }

    #[private]
    #[init(ignore_state)]
    pub fn clean(keys: Vec<near_sdk::json_types::Base64VecU8>) -> Self {
        for key in keys.iter() {
            env::storage_remove(&key.0);
        }
        Self {
            protocol_state: ProtocolContractState::NotInitialized,
            pending_requests: LookupMap::new(b"m"),
        }
    }

    /// This is the root public key combined from all the public keys of the participants.
    pub fn public_key(&self) -> PublicKey {
        match &self.protocol_state {
            ProtocolContractState::Running(state) => state.public_key.clone(),
            ProtocolContractState::Resharing(state) => state.public_key.clone(),
            _ => env::panic_str("public key not available (protocol is not running or resharing)"),
        }
    }
}
