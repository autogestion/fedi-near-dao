// use std::str::FromStr;

// use secp256k1::bitcoin_hashes::sha256;
// use secp256k1::{Message, Secp256k1, PublicKey, Signature};

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{UnorderedSet, Vector, UnorderedMap};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, AccountId, Balance, Duration, Promise};
use std::collections::HashMap;

const MAX_DESCRIPTION_LENGTH: usize = 280;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum Vote {
    Yes,
    No,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
pub enum NumOrRatio {
    Number(u64),
    Ratio(u64, u64),
}

impl NumOrRatio {
    pub fn as_ratio(&self) -> Option<(u64, u64)> {
        match self {
            NumOrRatio::Number(_) => None,
            NumOrRatio::Ratio(a, b) => Some((*a, *b)),
        }
    }
}

/// Policy item, defining how many votes required to approve up to this much amount.
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct PolicyItem {
    pub max_amount: U64,
    pub votes: NumOrRatio,
}

impl PolicyItem {
    pub fn num_votes(&self, num_council: u64) -> u64 {
        match self.votes {
            NumOrRatio::Number(num_votes) => num_votes,
            NumOrRatio::Ratio(l, r) => std::cmp::min(num_council * l / r + 1, num_council),
        }
    }
}

fn vote_requirement(policy: &[PolicyItem], num_council: u64, amount: Option<Balance>) -> u64 {
    if let Some(amount) = amount {
        // TODO: replace with binary search.
        for item in policy {
            if u128::from(item.max_amount.0) > amount {
                return item.num_votes(num_council);
            }
        }
    }
    policy[policy.len() - 1].num_votes(num_council)
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Eq))]
#[serde(crate = "near_sdk::serde")]
pub enum ProposalStatus {
    /// Proposal is in active voting stage.
    Vote,
    /// Proposal has successfully passed.
    Success,
    /// Proposal was rejected by the vote.
    Reject,
    /// Vote for proposal has failed due (not enough votes).
    Fail,
    /// Given voting policy, the uncontested minimum of votes was acquired.
    /// Delaying the finalization of the proposal to check that there is no contenders (who would vote against).
    Delay,
}

impl ProposalStatus {
    pub fn is_finalized(&self) -> bool {
        self != &ProposalStatus::Vote && self != &ProposalStatus::Delay
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(tag = "type")]
pub enum ProposalKind {
    // NewCouncil,
    RemoveCouncil,
    Payout { amount: U128 },
    ChangeVotePeriod { vote_period: U64 },
    // ChangeBond { bond: U128 },
    ChangePolicy { policy: Vec<PolicyItem> },
    // ChangePurpose { purpose: String },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Proposal {
    status: ProposalStatus,
    proposer: AccountId,
    target: AccountId,
    description: String,
    kind: ProposalKind,
    vote_period_end: Duration,
    vote_yes: u64,
    vote_no: u64,
    votes: HashMap<AccountId, Vote>,
}

impl Proposal {
    pub fn get_amount(&self) -> Option<Balance> {
        match self.kind {
            ProposalKind::Payout { amount } => Some(amount.0.into()),
            _ => None,
        }
    }

    /// Compute new vote status given council size and current timestamp.
    pub fn vote_status(&self, policy: &[PolicyItem], num_council: u64) -> ProposalStatus {
        let votes_required = vote_requirement(policy, num_council, self.get_amount());
        let max_votes = policy[policy.len() - 1].num_votes(num_council);
        if self.vote_yes >= max_votes {
            ProposalStatus::Success
        } else if self.vote_yes >= votes_required && self.vote_no == 0 {
            if env::block_timestamp() > self.vote_period_end {
                ProposalStatus::Success
            } else {
                ProposalStatus::Delay
            }
        } else if self.vote_no >= max_votes {
            ProposalStatus::Reject
        } else if env::block_timestamp() > self.vote_period_end
            || self.vote_yes + self.vote_no == num_council
        {
            ProposalStatus::Fail
        } else {
            ProposalStatus::Vote
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ProposalInput {
    target: AccountId,
    description: String,
    kind: ProposalKind,
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct FediDAO {
    // purpose: String,
    // bond: Balance,
    vote_period: Duration,
    grace_period: Duration,
    policy: Vec<PolicyItem>,
    council: UnorderedMap<AccountId, String>,
    proposals: Vector<Proposal>,
    public_key: String,
    domain: String,
}

impl Default for FediDAO {
    fn default() -> Self {
        env::panic_str("FediDAO should be initialized before usage")
    }
}

#[near_bindgen]
impl FediDAO {
    #[init]
    pub fn new(
        // purpose: String,
        // council: Vec<AccountId>,
        // bond: U128,
        vote_period: U64,
        grace_period: U64,
        public_key: String,
        domain: String,
    ) -> Self {
        assert!(!env::state_exists(), "The contract is already initialized");

        Self {
            // purpose,
            // bond: bond.into(),
            domain,
            public_key,
            vote_period: vote_period.into(),
            grace_period: grace_period.into(),
            policy: vec![PolicyItem {
                max_amount: 0.into(),
                votes: NumOrRatio::Ratio(1, 2),
            }],
            council: UnorderedMap::new(b"c".to_vec()),
            proposals: Vector::new(b"p".to_vec()),
        }
        // ;
        // for account_id in council {
        //     dao.council.insert(&account_id);
        // }
        // dao
    }

    pub fn join_dao(&mut self, dao_ticket: String, username: String) -> u64 {

        // Verification disabled as far as Secp256k1::new() consumes all the gas
        let _skip = dao_ticket;
        // let secp = Secp256k1::new();
        // let sig = Signature::from_str(&dao_ticket).unwrap();
        // let public_key = PublicKey::from_str(&self.public_key).unwrap();
        // let message = Message::from_hashed_data::<sha256::Hash>(&username.as_bytes());
        // assert!(secp.verify(&message, &sig, &public_key).is_ok());

        self.council.insert(&env::predecessor_account_id(), &username);
        1
    }

    #[payable]
    pub fn add_proposal(&mut self, proposal: ProposalInput) -> u64 {
        // TODO: add also extra storage cost for the proposal itself.
        // assert!(env::attached_deposit() >= self.bond, "Not enough deposit");
        assert!(
            proposal.description.len() < MAX_DESCRIPTION_LENGTH,
            "Description length is too long"
        );
        // Input verification.
        match proposal.kind {
            ProposalKind::ChangePolicy { ref policy } => {
                assert_ne!(policy.len(), 0, "Policy shouldn't be empty");
                for i in 1..policy.len() {
                    assert!(
                        policy[i].max_amount.0 > policy[i - 1].max_amount.0,
                        "Policy must be sorted, item {} is wrong",
                        i
                    );
                }
                let last_ratio = policy[policy.len() - 1]
                    .votes
                    .as_ratio()
                    .expect("Last item in policy must be a ratio");
                assert!(
                    last_ratio.0 * 2 / last_ratio.1 >= 1,
                    "Last item in policy must be equal or above 1/2 ratio"
                );
            }
            _ => {}
        }
        let p = Proposal {
            status: ProposalStatus::Vote,
            proposer: env::predecessor_account_id(),
            target: proposal.target,
            description: proposal.description,
            kind: proposal.kind,
            vote_period_end: env::block_timestamp() + self.vote_period,
            vote_yes: 0,
            vote_no: 0,
            votes: HashMap::default(),
        };
        self.proposals.push(&p);
        self.proposals.len() - 1
    }

    pub fn get_vote_period(&self) -> U64 {
        self.vote_period.into()
    }

    // pub fn get_bond(&self) -> U128 {
    //     self.bond.into()
    // }

    pub fn get_council(self) -> Vec<String> {
        self.council.values_as_vector().to_vec()
    }

    pub fn get_num_proposals(&self) -> u64 {
        self.proposals.len()
    }

    // pub fn get_dao_balance() -> Balance {
    //     env::account_balance()
    // }

    pub fn get_proposals(&self, from_index: u64, limit: u64) -> Vec<Proposal> {
        (from_index..std::cmp::min(from_index + limit, self.proposals.len()))
            .map(|index| self.proposals.get(index).unwrap())
            .collect()
    }

    pub fn get_proposals_by_status(
        &self,
        status: ProposalStatus,
        from_index: u64,
        limit: u64,
    ) -> HashMap<u64, Proposal> {
        let filtered_proposal_ids: Vec<u64> = (0..self.proposals.len())
            .filter(|index| self.proposals.get(index.clone()).unwrap().status == status)
            .collect();

        (from_index..std::cmp::min(from_index + limit, filtered_proposal_ids.len() as u64))
            .map(|index| {
                let proposal_id: u64 = filtered_proposal_ids[index as usize];
                (proposal_id, self.proposals.get(proposal_id).unwrap())
            })
            .collect()
    }

    pub fn get_proposals_by_statuses(
        &self,
        statuses: Vec<ProposalStatus>,
        from_index: u64,
        limit: u64,
    ) -> HashMap<u64, Proposal> {
        let filtered_proposal_ids: Vec<u64> = (0..self.proposals.len())
            .filter(|index| statuses.contains(&self.proposals.get(index.clone()).unwrap().status))
            .collect();

        (from_index..std::cmp::min(from_index + limit, filtered_proposal_ids.len() as u64))
            .map(|index| {
                let proposal_id: u64 = filtered_proposal_ids[index as usize];
                (proposal_id, self.proposals.get(proposal_id).unwrap())
            })
            .collect()
    }

    pub fn get_proposal(&self, id: u64) -> Proposal {
        self.proposals.get(id).expect("Proposal not found")
    }

    // pub fn get_purpose(&self) -> String {
    //     self.purpose.clone()
    // }

    pub fn vote(&mut self, id: u64, vote: Vote) {
        // assert!(
        //     self.council.contains(&env::predecessor_account_id()),
        //     "Only council can vote"
        // );
        let mut proposal = self.proposals.get(id).expect("No proposal with such id");
        assert_eq!(
            proposal.status,
            ProposalStatus::Vote,
            "Proposal already finalized"
        );
        if proposal.vote_period_end < env::block_timestamp() {
            env::log_str("Voting period expired, finalizing the proposal");
            self.finalize(id);
            return;
        }
        assert!(
            !proposal.votes.contains_key(&env::predecessor_account_id()),
            "Already voted"
        );
        match vote {
            Vote::Yes => proposal.vote_yes += 1,
            Vote::No => proposal.vote_no += 1,
        }
        proposal.votes.insert(env::predecessor_account_id(), vote);
        let post_status = proposal.vote_status(&self.policy, self.council.len());
        // If just changed from vote to Delay, adjust the expiration date to grace period.
        if !post_status.is_finalized() && post_status != proposal.status {
            proposal.vote_period_end = env::block_timestamp() + self.grace_period;
            proposal.status = post_status.clone();
        }
        self.proposals.replace(id, &proposal);
        // Finalize if this vote is done.
        if post_status.is_finalized() {
            self.finalize(id);
        }
    }

    pub fn finalize(&mut self, id: u64) {
        let mut proposal = self.proposals.get(id).expect("No proposal with such id");
        assert!(
            !proposal.status.is_finalized(),
            "Proposal already finalized"
        );
        proposal.status = proposal.vote_status(&self.policy, self.council.len());
        match proposal.status {
            ProposalStatus::Success => {
                env::log_str("Vote succeeded");
                let target = proposal.target.clone();
                // Promise::new(proposal.proposer.clone()).transfer(self.bond);
                match proposal.kind {
                    // ProposalKind::NewCouncil => {
                    //     self.council.insert(&target);
                    // }
                    ProposalKind::RemoveCouncil => {
                        self.council.remove(&target);
                    }
                    ProposalKind::Payout { amount } => {
                        Promise::new(target).transfer(amount.0.into());
                    }
                    ProposalKind::ChangeVotePeriod { vote_period } => {
                        self.vote_period = vote_period.into();
                    }
                    // ProposalKind::ChangeBond { bond } => {
                    //     self.bond = bond.into();
                    // }
                    ProposalKind::ChangePolicy { ref policy } => {
                        self.policy = policy.clone();
                    }
                    // ProposalKind::ChangePurpose { ref purpose } => {
                    //     self.purpose = purpose.clone();
                    // }
                };
            }
            ProposalStatus::Reject => {
                env::log_str("Proposal rejected");
            }
            ProposalStatus::Fail => {
                // If no majority vote, let's return the bond.
                env::log_str("Proposal vote failed");
                // Promise::new(proposal.proposer.clone()).transfer(self.bond);
            }
            ProposalStatus::Vote | ProposalStatus::Delay => {
                env::panic_str("voting period has not expired and no majority vote yet")
            }
        }
        self.proposals.replace(id, &proposal);
    }
}
