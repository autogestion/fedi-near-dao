
use super::*;
use near_sdk::test_utils::{accounts, VMContextBuilder};
use near_sdk::testing_env;

fn vote(dao: &mut SputnikDAO, proposal_id: u64, votes: Vec<(usize, Vote)>) {
    for (id, vote) in votes {
        testing_env!(VMContextBuilder::new()
            .predecessor_account_id(accounts(id))
            .build());
        dao.vote(proposal_id, vote);
    }
}

// vec![accounts(0).as_ref(), accounts(1).as_ref()],
#[test]
fn test_basics() {
    let mut dao = SputnikDAO::new(
        "test".to_string(),
        vec![accounts(0), accounts(1)],
        10.into(),
        1_000.into(),
        10.into(),
    );

    // assert_eq!(dao.get_bond(), 10.into());
    assert_eq!(dao.get_vote_period(), 1_000.into());
    // assert_eq!(dao.get_purpose(), "test");

    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(2),
        description: "add new member".to_string(),
        kind: ProposalKind::NewCouncil,
    });
    assert_eq!(dao.get_num_proposals(), 1);
    assert_eq!(dao.get_proposals(0, 1).len(), 1);
    vote(&mut dao, id, vec![(0, Vote::Yes)]);
    assert_eq!(dao.get_proposal(id).vote_yes, 1);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Vote);
    let account_0: AccountId = accounts(0);
    let account_1: AccountId = accounts(1);
    let account_2: AccountId = accounts(2);
    assert_eq!(
        dao.get_council(),
        vec![account_0.clone(), account_1.clone()]
    );
    vote(&mut dao, id, vec![(1, Vote::Yes)]);
    assert_eq!(dao.get_council(), vec![account_0, account_1, account_2]);

    // Pay out money for proposal. 2 votes yes vs 1 vote no.
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(2),
        description: "give me money".to_string(),
        kind: ProposalKind::Payout { amount: 10.into() },
    });
    vote(
        &mut dao,
        id,
        vec![(0, Vote::No), (1, Vote::Yes), (2, Vote::Yes)],
    );
    assert_eq!(dao.get_proposal(id).vote_yes, 2);
    assert_eq!(dao.get_proposal(id).vote_no, 1);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Success);

    // No vote for proposal.
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(2),
        description: "give me more money".to_string(),
        kind: ProposalKind::Payout { amount: 10.into() },
    });
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(3))
        .block_timestamp(1_001)
        .build());
    dao.finalize(id);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Fail);

    // Change policy.
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(2),
        description: "policy".to_string(),
        kind: ProposalKind::ChangePolicy {
            policy: vec![
                PolicyItem {
                    max_amount: 100.into(),
                    votes: NumOrRatio::Number(2),
                },
                PolicyItem {
                    max_amount: 1_000_000.into(),
                    votes: NumOrRatio::Ratio(1, 1),
                },
            ],
        },
    });
    vote(&mut dao, id, vec![(0, Vote::Yes), (1, Vote::Yes)]);

    // Try new policy with small amount.
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(2),
        description: "give me more money".to_string(),
        kind: ProposalKind::Payout { amount: 10.into() },
    });
    assert_eq!(dao.get_proposal(id).vote_period_end, 1_000);
    vote(&mut dao, id, vec![(0, Vote::Yes)]);
    assert_eq!(dao.get_proposal(id).vote_period_end, 1_000);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Vote);
    vote(&mut dao, id, vec![(1, Vote::Yes)]);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Delay);
    assert_eq!(dao.get_proposal(id).vote_period_end, 10);
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(3))
        .block_timestamp(11)
        .build());
    dao.finalize(id);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Success);

    // New policy for bigger amounts requires 100% votes.
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(2),
        description: "give me more money".to_string(),
        kind: ProposalKind::Payout {
            amount: 10_000.into(),
        },
    });
    vote(&mut dao, id, vec![(0, Vote::Yes)]);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Vote);
    vote(&mut dao, id, vec![(1, Vote::Yes)]);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Vote);
    vote(&mut dao, id, vec![(2, Vote::Yes)]);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Success);
}

#[test]
fn test_expiration() {
    let mut dao = SputnikDAO::new(
        "test".to_string(),
        vec![accounts(0), accounts(1), accounts(2)],
        10.into(),
        1_000.into(),
        10.into(),
    );

    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(5),
        description: "add new member".to_string(),
        kind: ProposalKind::NewCouncil,
    });
    let vote_period_end = dao.get_proposal(id).vote_period_end;
    vote(&mut dao, id, vec![(0, Vote::Yes)]);
    assert_eq!(dao.get_proposal(id).vote_period_end, vote_period_end);
    vote(&mut dao, id, vec![(1, Vote::Yes)]);
    assert_eq!(dao.get_proposal(id).vote_period_end, vote_period_end);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Success);
}

#[test]
fn test_single_council() {
    let mut dao = SputnikDAO::new(
        "".to_string(),
        vec![accounts(0)],
        10.into(),
        1_000.into(),
        10.into(),
    );

    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(1),
        description: "add new member".to_string(),
        kind: ProposalKind::NewCouncil,
    });
    vote(&mut dao, id, vec![(0, Vote::Yes)]);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Success);
    let account_0: AccountId = accounts(0);
    let account_1: AccountId = accounts(1);
    assert_eq!(dao.get_council(), vec![account_0, account_1]);
}

#[test]
#[should_panic]
fn test_double_vote() {
    let mut dao = SputnikDAO::new(
        "".to_string(),
        vec![accounts(0), accounts(1)],
        10.into(),
        1000.into(),
        10.into(),
    );
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(2),
        description: "add new member".to_string(),
        kind: ProposalKind::NewCouncil,
    });
    assert_eq!(dao.get_proposals(0, 1).len(), 1);
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(0))
        .build());
    dao.vote(id, Vote::Yes);
    dao.vote(id, Vote::Yes);
}

#[test]
fn test_two_council() {
    let mut dao = SputnikDAO::new(
        "".to_string(),
        vec![accounts(0), accounts(1)],
        10.into(),
        1_000.into(),
        10.into(),
    );

    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(1),
        description: "add new member".to_string(),
        kind: ProposalKind::Payout { amount: 100.into() },
    });
    vote(&mut dao, id, vec![(0, Vote::Yes), (1, Vote::No)]);
    assert_eq!(dao.get_proposal(id).status, ProposalStatus::Fail);
}

#[test]
#[should_panic]
fn test_run_out_of_money() {
    let mut dao = SputnikDAO::new(
        "".to_string(),
        vec![accounts(0)],
        10.into(),
        1000.into(),
        10.into(),
    );
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    let id = dao.add_proposal(ProposalInput {
        target: accounts(2),
        description: "add new member".to_string(),
        kind: ProposalKind::Payout {
            amount: 1000.into(),
        },
    });
    assert_eq!(dao.get_proposals(0, 1).len(), 1);
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(0))
        .account_balance(10)
        .build());
    dao.vote(id, Vote::Yes);
}

#[test]
#[should_panic]
fn test_incorrect_policy() {
    let mut dao = SputnikDAO::new(
        "".to_string(),
        vec![accounts(0), accounts(1)],
        10.into(),
        1000.into(),
        10.into(),
    );
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(accounts(2))
        .attached_deposit(10)
        .build());
    dao.add_proposal(ProposalInput {
        target: accounts(2),
        description: "policy".to_string(),
        kind: ProposalKind::ChangePolicy {
            policy: vec![
                PolicyItem {
                    max_amount: 100.into(),
                    votes: NumOrRatio::Number(5),
                },
                PolicyItem {
                    max_amount: 5.into(),
                    votes: NumOrRatio::Number(3),
                },
            ],
        },
    });
}

