//! Themis client
use bn::{Fr, Group, G1};
use elgamal_bn::{/*ciphertext::Ciphertext,*/ private::SecretKey, public::PublicKey};
use futures::future::join_all;
use solana_banks_client::{BanksClient, BanksClientExt};
use solana_sdk::{
    commitment_config::CommitmentLevel,
    message::Message,
    native_token::sol_to_lamports,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_themis::{
    instruction,
    state::generate_keys, // recover_scalar, User},
};
use std::{io, time::Instant};
//use tarpc::context;

/// For a single user, create interactions, calculate the aggregate, submit a proof, and verify it.
async fn run_user_workflow(
    mut client: BanksClient,
    sender_keypair: Keypair,
    (_sk, pk): (SecretKey, PublicKey),
    interactions: Vec<(G1, G1)>,
    policies_pubkey: Pubkey,
    _expected_scalar_aggregate: Fr,
) -> io::Result<u64> {
    let sender_pubkey = sender_keypair.pubkey();
    let mut num_transactions = 0;

    // Create the users account
    let user_keypair = Keypair::new();
    let user_pubkey = user_keypair.pubkey();
    let ixs =
        instruction::create_user_account(&sender_pubkey, &user_pubkey, sol_to_lamports(0.001));
    let msg = Message::new(&ixs, Some(&sender_keypair.pubkey()));
    let recent_blockhash = client.get_recent_blockhash().await?;
    let tx = Transaction::new(&[&sender_keypair, &user_keypair], msg, recent_blockhash);
    let tx_size = bincode::serialize(&tx).unwrap().len();
    assert!(
        tx_size <= 1200,
        "transaction over 1200 bytes: {} bytes",
        tx_size
    );
    client
        .process_transaction_with_commitment(tx, CommitmentLevel::Recent)
        .await
        .unwrap();
    num_transactions += 1;

    let ix = instruction::calculate_aggregate(&user_pubkey, &policies_pubkey, interactions, pk);
    let msg = Message::new(&[ix], Some(&sender_keypair.pubkey()));
    let recent_blockhash = client.get_recent_blockhash().await?;
    let tx = Transaction::new(&[&sender_keypair, &user_keypair], msg, recent_blockhash);
    let tx_size = bincode::serialize(&tx).unwrap().len();
    assert!(
        tx_size <= 1200,
        "transaction over 1200 bytes: {} bytes",
        tx_size
    );
    client
        .process_transaction_with_commitment(tx, CommitmentLevel::Recent)
        .await
        .unwrap();
    num_transactions += 1;

    //let user_account = client
    //    .get_account_with_commitment_and_context(
    //        context::current(),
    //        user_pubkey,
    //        CommitmentLevel::Recent,
    //    )
    //    .await
    //    .unwrap()
    //    .unwrap();
    //let user = User::deserialize(&user_account.data).unwrap();
    //let ciphertext = Ciphertext {
    //    points: user.fetch_encrypted_aggregate(),
    //    pk,
    //};

    //let decrypted_aggregate = sk.decrypt(&ciphertext);
    let decrypted_aggregate = G1::one();
    //let scalar_aggregate = recover_scalar(decrypted_aggregate, 16);
    //assert_eq!(scalar_aggregate, expected_scalar_aggregate);

    //let ((announcement_g, announcement_ctx), response) =
    //    sk.prove_correct_decryption_no_Merlin(&ciphertext, &decrypted_aggregate).unwrap();
    let ((announcement_g, announcement_ctx), response) =
        ((G1::one(), G1::one()), Fr::new(0.into()).unwrap());
    //sk.prove_correct_decryption_no_Merlin(&ciphertext, &decrypted_aggregate).unwrap();

    let ix = instruction::submit_proof_decryption(
        &user_pubkey,
        decrypted_aggregate,
        announcement_g,
        announcement_ctx,
        response,
    );
    let msg = Message::new(&[ix], Some(&sender_keypair.pubkey()));
    let recent_blockhash = client.get_recent_blockhash().await?;
    let tx = Transaction::new(&[&sender_keypair, &user_keypair], msg, recent_blockhash);
    let tx_size = bincode::serialize(&tx).unwrap().len();
    assert!(
        tx_size <= 1200,
        "transaction over 1200 bytes: {} bytes",
        tx_size
    );
    client
        .process_transaction_with_commitment(tx, CommitmentLevel::Recent)
        .await
        .unwrap();
    num_transactions += 1;

    //let user_account = client.get_account_with_commitment_and_context(context::current(), user_pubkey, CommitmentLevel::Recent).await.unwrap().unwrap();
    //let user = User::deserialize(&user_account.data).unwrap();
    //assert!(user.fetch_proof_verification());

    Ok(num_transactions)
}

pub async fn test_e2e(
    client: &mut BanksClient,
    sender_keypair: Keypair,
    policies: Vec<Fr>,
    num_users: u64,
    expected_scalar_aggregate: Fr,
) -> io::Result<()> {
    let sender_pubkey = sender_keypair.pubkey();
    let policies_keypair = Keypair::new();
    let policies_pubkey = policies_keypair.pubkey();
    let policies_len = policies.len();

    // Create the policies account
    let ixs = instruction::create_policies_account(
        &sender_pubkey,
        &policies_pubkey,
        sol_to_lamports(0.01),
        policies,
    );
    let msg = Message::new(&ixs, Some(&sender_keypair.pubkey()));
    let recent_blockhash = client.get_recent_blockhash().await?;
    let tx = Transaction::new(&[&sender_keypair, &policies_keypair], msg, recent_blockhash);
    let tx_size = bincode::serialize(&tx).unwrap().len();
    assert!(
        tx_size <= 1200,
        "transaction over 1200 bytes: {} bytes",
        tx_size
    );
    client
        .process_transaction_with_commitment(tx, CommitmentLevel::Recent)
        .await
        .unwrap();

    // Send feepayer_keypairs some SOL
    let feepayers: Vec<_> = (0..num_users).map(|_| Keypair::new()).collect();
    for feepayers in feepayers.chunks(20) {
        println!("Seeding feepayer accounts...");
        let payments: Vec<_> = feepayers
            .iter()
            .map(|keypair| (keypair.pubkey(), sol_to_lamports(0.0011)))
            .collect();
        let ixs = system_instruction::transfer_many(&sender_pubkey, &payments);
        let msg = Message::new(&ixs, Some(&sender_keypair.pubkey()));
        let recent_blockhash = client.get_recent_blockhash().await.unwrap();
        let tx = Transaction::new(&[&sender_keypair], msg, recent_blockhash);
        let tx_size = bincode::serialize(&tx).unwrap().len();
        assert!(
            tx_size <= 1200,
            "transaction over 1200 bytes: {} bytes",
            tx_size
        );
        client
            .process_transaction_with_commitment(tx, CommitmentLevel::Recent)
            .await
            .unwrap();
    }

    println!("Starting benchmark...");
    let now = Instant::now();

    let (sk, pk) = generate_keys();
    let interactions: Vec<_> = (0..policies_len)
        .map(|_| pk.encrypt(&G1::one()).points)
        .collect();

    let futures: Vec<_> = feepayers
        .into_iter()
        .map(move |feepayer_keypair| {
            run_user_workflow(
                client.clone(),
                feepayer_keypair,
                (sk.clone(), pk),
                interactions.clone(),
                policies_pubkey,
                expected_scalar_aggregate,
            )
        })
        .collect();
    let results = join_all(futures).await;
    let elapsed = now.elapsed();
    println!("Benchmark complete.");

    let num_transactions = results
        .into_iter()
        .map(|result| result.unwrap())
        .sum::<u64>();
    println!(
        "{} transactions in {:?} ({} TPS)",
        num_transactions,
        elapsed,
        num_transactions as f64 / elapsed.as_secs_f64()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_banks_client::start_client;
    use solana_banks_server::banks_server::start_local_server;
    use solana_runtime::{bank::Bank, bank_forks::BankForks};
    use solana_sdk::{
        account::{Account, KeyedAccount},
        account_info::AccountInfo,
        genesis_config::create_genesis_config,
        instruction::InstructionError,
        program_error::ProgramError,
    };
    use spl_themis::processor::process_instruction;
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock},
        {cell::RefCell, rc::Rc},
    };
    use tokio::runtime::Runtime;

    fn to_instruction_error(error: ProgramError) -> InstructionError {
        match error {
            ProgramError::Custom(err) => InstructionError::Custom(err),
            ProgramError::InvalidArgument => InstructionError::InvalidArgument,
            ProgramError::InvalidInstructionData => InstructionError::InvalidInstructionData,
            ProgramError::InvalidAccountData => InstructionError::InvalidAccountData,
            ProgramError::AccountDataTooSmall => InstructionError::AccountDataTooSmall,
            ProgramError::InsufficientFunds => InstructionError::InsufficientFunds,
            ProgramError::IncorrectProgramId => InstructionError::IncorrectProgramId,
            ProgramError::MissingRequiredSignature => InstructionError::MissingRequiredSignature,
            ProgramError::AccountAlreadyInitialized => InstructionError::AccountAlreadyInitialized,
            ProgramError::UninitializedAccount => InstructionError::UninitializedAccount,
            ProgramError::NotEnoughAccountKeys => InstructionError::NotEnoughAccountKeys,
            ProgramError::AccountBorrowFailed => InstructionError::AccountBorrowFailed,
            ProgramError::MaxSeedLengthExceeded => InstructionError::MaxSeedLengthExceeded,
            ProgramError::InvalidSeeds => InstructionError::InvalidSeeds,
        }
    }

    // Same as process_instruction, but but can be used as a builtin program. Handy for unit-testing.
    pub fn process_instruction_native(
        program_id: &Pubkey,
        keyed_accounts: &[KeyedAccount],
        input: &[u8],
    ) -> Result<(), InstructionError> {
        // Copy all the accounts into a HashMap to ensure there are no duplicates
        let mut accounts: HashMap<Pubkey, Account> = keyed_accounts
            .iter()
            .map(|ka| (*ka.unsigned_key(), ka.account.borrow().clone()))
            .collect();

        // Create shared references to each account's lamports/data/owner
        let account_refs: HashMap<_, _> = accounts
            .iter_mut()
            .map(|(key, account)| {
                (
                    *key,
                    (
                        Rc::new(RefCell::new(&mut account.lamports)),
                        Rc::new(RefCell::new(&mut account.data[..])),
                        &account.owner,
                    ),
                )
            })
            .collect();

        // Create AccountInfos
        let account_infos: Vec<AccountInfo> = keyed_accounts
            .iter()
            .map(|keyed_account| {
                let key = keyed_account.unsigned_key();
                let (lamports, data, owner) = &account_refs[key];
                AccountInfo {
                    key,
                    is_signer: keyed_account.signer_key().is_some(),
                    is_writable: keyed_account.is_writable(),
                    lamports: lamports.clone(),
                    data: data.clone(),
                    owner,
                    executable: keyed_account.executable().unwrap(),
                    rent_epoch: keyed_account.rent_epoch().unwrap(),
                }
            })
            .collect();

        // Execute the BPF entrypoint
        process_instruction(program_id, &account_infos, input).map_err(to_instruction_error)?;

        // Commit changes to the KeyedAccounts
        for keyed_account in keyed_accounts {
            let mut account = keyed_account.account.borrow_mut();
            let key = keyed_account.unsigned_key();
            let (lamports, data, _owner) = &account_refs[key];
            account.lamports = **lamports.borrow();
            account.data = data.borrow().to_vec();
        }

        Ok(())
    }

    #[test]
    fn test_local_e2e_2ads() {
        let (genesis_config, sender_keypair) = create_genesis_config(sol_to_lamports(9_000_000.0));
        let mut bank = Bank::new(&genesis_config);
        bank.add_builtin_program("Themis", spl_themis::id(), process_instruction_native);
        let bank_forks = Arc::new(RwLock::new(BankForks::new(bank)));
        Runtime::new().unwrap().block_on(async {
            let transport = start_local_server(&bank_forks).await;
            let mut banks_client = start_client(transport).await.unwrap();
            let policies = vec![Fr::new(1u64.into()).unwrap(), Fr::new(2u64.into()).unwrap()];
            test_e2e(
                &mut banks_client,
                sender_keypair,
                policies,
                10,
                Fr::new(3u64.into()).unwrap(),
            )
            .await
            .unwrap();
        });
    }
}
