use {
    anyhow::{anyhow, Result},
    bs58,
    clap::{command, Parser, Subcommand},
    solana_cli_config,
    solana_client::{rpc_client::RpcClient, rpc_request::TokenAccountsFilter},
    solana_sdk::{
        commitment_config::CommitmentConfig,
        instruction::Instruction,
        pubkey::Pubkey,
        signature::{read_keypair_file, Signer},
        signer::keypair::Keypair,
        transaction::Transaction,
    },
    std::str::FromStr,
};

const RPC_URL: &str = "";
const API_KEY: &str = "";

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Ata { mint: Pubkey },
    Close { path: String },
    Bytes { key: String },
}

fn get_url(rpc_url: &str, api_key: &str) -> String {
    format!("https://{}/?api-key={}", rpc_url, api_key)
}

fn close_account(client: &RpcClient, ix: &[Instruction], wallet: &Keypair) -> Result<()> {
    let latest_blockhash = client
        .get_latest_blockhash()
        .map_err(|e| anyhow!("Failed to get latest blockhash: {}", e))?;

    let tx = Transaction::new_signed_with_payer(
        ix,
        Some(&wallet.pubkey()),
        &[&wallet],
        latest_blockhash,
    );

    let signature = client.send_and_confirm_transaction_with_spinner(&tx)?;
    println!("Signature: {:?}", signature);

    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let url = get_url(RPC_URL, API_KEY);
    let solana_config_file = if let Some(ref config) = *solana_cli_config::CONFIG_FILE {
        solana_cli_config::Config::load(config).unwrap_or_default()
    } else {
        solana_cli_config::Config::default()
    };

    let keypair_path = &solana_config_file.keypair_path;
    let wallet = read_keypair_file(keypair_path.clone())
        .map_err(|e| anyhow!("Failed to get keypair file: {}", e))?;
    let pubkey = wallet.pubkey();

    let client = RpcClient::new_with_commitment(url, CommitmentConfig::confirmed());

    match args.cmd {
        Commands::Ata { mint } => {
            println!(
                "{}",
                spl_associated_token_account::get_associated_token_address_with_program_id(
                    &pubkey,
                    &mint,
                    &spl_token::ID
                )
            );
        }
        Commands::Close { path } => {
            let wallet = read_keypair_file(path)
                .map_err(|e| anyhow!("Failed to get keypair file: {}", e))?;
            let pubkey = wallet.pubkey();
            let token_accounts = client
                .get_token_accounts_by_owner(&pubkey, TokenAccountsFilter::ProgramId(spl_token::ID))
                .map_err(|e| anyhow!("Failed to get token accounts by owner: {}", e))?;

            let mut ix: Vec<Instruction> = Vec::with_capacity(15);

            for account in token_accounts {
                if ix.len() >= 15 {
                    close_account(&client, &ix, &wallet)?;
                    ix.clear();
                }
                let balance = client.get_token_account_balance(
                    &Pubkey::from_str(&account.pubkey)
                        .map_err(|e| anyhow!("Failed to parse pubkey: {}", e))?,
                )?;
                let ui_amount = balance.ui_amount;
                let decimals = balance.decimals;
                let amount = spl_token::ui_amount_to_amount(
                    ui_amount.ok_or(anyhow!("Could not parse amount"))?,
                    decimals,
                );

                if amount == 0 {
                    let close_ix = spl_token::instruction::close_account(
                        &spl_token::ID,
                        &Pubkey::from_str(&account.pubkey)
                            .map_err(|e| anyhow!("Failed to parse pubkey: {}", e))?,
                        &pubkey,
                        &pubkey,
                        &[],
                    )
                    .map_err(|e| anyhow!("Failed to create close account instruction: {}", e))?;
                    println!("Close account: {:?}", close_ix);
                    ix.push(close_ix);
                }
            }

            if !ix.is_empty() {
                close_account(&client, &ix, &wallet)?;
            }
        }
        Commands::Bytes { key } => {
            let bytes = bs58::decode(key).into_vec().unwrap();
            println!("{:?}", bytes);
        }
    }

    Ok(())
}
