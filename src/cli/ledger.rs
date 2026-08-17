//! CLI subcommand for cryptographic audit ledger management: `sentinel ledger [append|verify|keygen]`.

use clap::Subcommand;
use colored::Colorize;
use needle::ledger::{
    append_to_ledger, default_key_priv_path, default_key_pub_path, default_ledger_dir,
    default_ledger_path, verify_ledger_file, EntryType, LedgerKeypair,
};
use needle::{Error, Result};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Subcommand, Debug, Clone)]
pub enum LedgerCommands {
    /// Append an audit report or payload to the cryptographic ledger
    Append {
        /// Path to JSON report file
        #[arg(short, long, required = true)]
        report: String,

        /// Entry type: compliance_audit, security_scan, policy_ingest, codebase_snapshot, system_event
        #[arg(short, long, default_value = "compliance_audit")]
        r#type: String,

        /// Path to Ed25519 private key (defaults to .needle/ledger/key.priv)
        #[arg(short, long)]
        key: Option<String>,

        /// Automatically generate Ed25519 keypair if missing
        #[arg(long)]
        gen_key_if_missing: bool,
    },

    /// Verify the cryptographic integrity of the ledger chain
    Verify {
        /// Custom path to ledger file (default: .needle/ledger/audit_chain.jsonl)
        #[arg(short, long)]
        ledger: Option<String>,

        /// Show verification status of each individual block
        #[arg(short, long)]
        verbose: bool,
    },

    /// Compacts the ledger into a single genesis block and archives the history
    Snapshot {
        /// Custom path to ledger file (default: .needle/ledger/audit_chain.jsonl)
        #[arg(short, long)]
        ledger: Option<String>,

        /// Path to Ed25519 private key for signing the snapshot
        #[arg(short, long)]
        key: Option<String>,
    },

    /// Generate a new Ed25519 keypair for ledger signing
    Keygen {
        /// Output directory for keypair (default: .needle/ledger/)
        #[arg(short, long)]
        output_dir: Option<String>,

        /// Overwrite existing keypair files
        #[arg(long)]
        force: bool,
    },
}

pub async fn run(action: LedgerCommands) -> Result<()> {
    match action {
        LedgerCommands::Keygen { output_dir, force } => run_keygen(output_dir, force),
        LedgerCommands::Append {
            report,
            r#type,
            key,
            gen_key_if_missing,
        } => run_append(report, r#type, key, gen_key_if_missing),
        LedgerCommands::Verify { ledger, verbose } => run_verify(ledger, verbose),
        LedgerCommands::Snapshot { ledger, key } => run_snapshot(ledger, key),
    }
}

fn run_keygen(output_dir: Option<String>, force: bool) -> Result<()> {
    let out_dir = match output_dir {
        Some(d) => PathBuf::from(d),
        None => default_ledger_dir(),
    };

    if !out_dir.exists() {
        std::fs::create_dir_all(&out_dir).map_err(Error::Io)?;
    }

    let priv_path = out_dir.join("key.priv");
    let pub_path = out_dir.join("key.pub");

    if (priv_path.exists() || pub_path.exists()) && !force {
        eprintln!(
            "{}: Key files already exist in {}.\n  Use --force to overwrite.",
            "Error".red().bold(),
            out_dir.display()
        );
        return Err(Error::LedgerError(format!(
            "Key files already exist at '{}'. Use --force to overwrite.",
            out_dir.display()
        )));
    }

    let keypair = LedgerKeypair::generate();
    keypair.save_to_files(&priv_path, &pub_path)?;

    println!("{}", "✓ Generated new Ed25519 keypair".green().bold());
    println!(
        "  {}: {}",
        "Private key".bold(),
        priv_path.display().to_string().cyan()
    );
    println!(
        "  {}: {} (pubkey: {})",
        "Public key".bold(),
        pub_path.display().to_string().cyan(),
        keypair.public_key_hex().dimmed()
    );

    Ok(())
}

fn run_append(
    report: String,
    r#type: String,
    key: Option<String>,
    gen_key_if_missing: bool,
) -> Result<()> {
    let report_path = PathBuf::from(&report);
    if !report_path.exists() {
        eprintln!(
            "{}: Report file not found at '{}'.",
            "Error".red().bold(),
            report_path.display()
        );
        return Err(Error::InvalidPath(format!(
            "Report file not found: {}",
            report_path.display()
        )));
    }

    let content = std::fs::read_to_string(&report_path).map_err(Error::Io)?;
    let payload: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        Error::SerializationError(format!(
            "Failed to parse report JSON at '{}': {e}",
            report_path.display()
        ))
    })?;

    let entry_type = EntryType::from_str(&r#type)?;

    let priv_key_path = match key {
        Some(k) => PathBuf::from(k),
        None => default_key_priv_path(),
    };
    let pub_key_path = default_key_pub_path();

    let keypair = if priv_key_path.exists() {
        LedgerKeypair::load_from_file(&priv_key_path)?
    } else if gen_key_if_missing {
        if let Some(parent) = priv_key_path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        let kp = LedgerKeypair::generate();
        kp.save_to_files(&priv_key_path, &pub_key_path)?;
        println!(
            "  {}",
            format!(
                "(Generated missing keypair at {})",
                priv_key_path.display()
            )
            .dimmed()
        );
        kp
    } else {
        eprintln!(
            "{}: Private key not found at '{}'.\n  Run `sentinel ledger keygen` or pass `--gen-key-if-missing`.",
            "Error".red().bold(),
            priv_key_path.display()
        );
        return Err(Error::LedgerError(format!(
            "Private key not found at '{}'. Run `sentinel ledger keygen` or pass `--gen-key-if-missing`.",
            priv_key_path.display()
        )));
    };

    let ledger_path = default_ledger_path();
    let block = append_to_ledger(&ledger_path, &keypair, entry_type, payload)?;

    println!(
        "{}",
        format!("✓ Block #{} appended to ledger", block.sequence)
            .green()
            .bold()
    );
    println!("  {}:   {}", "Block Hash".bold(), block.block_hash.cyan());
    println!(
        "  {}: {}",
        "Payload Hash".bold(),
        block.payload_hash.dimmed()
    );
    println!("  {}:  {:?}", "Entry Type".bold(), block.entry_type);
    println!(
        "  {}:      {}",
        "Signer".bold(),
        block.signer_public_key.dimmed()
    );
    println!("  {}:   {}", "Timestamp".bold(), block.timestamp.dimmed());
    println!(
        "  {}:      {}",
        "Ledger File".bold(),
        ledger_path.display().to_string().dimmed()
    );

    Ok(())
}

fn run_verify(ledger: Option<String>, verbose: bool) -> Result<()> {
    let ledger_path = match ledger {
        Some(l) => PathBuf::from(l),
        None => default_ledger_path(),
    };

    if !ledger_path.exists() {
        println!(
            "{}",
            "✓ Ledger verified: 0 blocks (empty chain).".green().bold()
        );
        println!(
            "  {}: {}",
            "Ledger path".bold(),
            ledger_path.display().to_string().dimmed()
        );
        return Ok(());
    }

    if verbose {
        println!("{}", "Verifying ledger blocks sequentially...".bold());
    }

    let summary = match verify_ledger_file(&ledger_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {e}", "Error".red().bold());
            return Err(e);
        }
    };

    if summary.total_blocks == 0 {
        println!(
            "{}",
            "✓ Ledger verified: 0 blocks (empty chain).".green().bold()
        );
    } else {
        println!(
            "{}",
            format!(
                "✓ Ledger verified: {} blocks valid. Chain integrity intact.",
                summary.total_blocks
            )
            .green()
            .bold()
        );
        if let Some(hash) = &summary.latest_block_hash {
            println!("  {}: {}", "Latest Block Hash".bold(), hash.cyan());
        }
    }
    println!(
        "  {}: {}",
        "Ledger path".bold(),
        ledger_path.display().to_string().dimmed()
    );

    Ok(())
}

fn run_snapshot(ledger: Option<String>, key: Option<String>) -> Result<()> {
    use needle::ledger::compact_ledger;

    let ledger_path = match ledger {
        Some(l) => PathBuf::from(l),
        None => default_ledger_path(),
    };

    let priv_key_path = match key {
        Some(k) => PathBuf::from(k),
        None => default_key_priv_path(),
    };

    if !priv_key_path.exists() {
        eprintln!(
            "{}: Private key not found at '{}'.\n  Run `sentinel ledger keygen` to generate one.",
            "Error".red().bold(),
            priv_key_path.display()
        );
        return Err(Error::LedgerError(format!(
            "Private key not found at '{}'.",
            priv_key_path.display()
        )));
    }

    let keypair = LedgerKeypair::load_from_file(&priv_key_path)?;

    println!("{}", "Compacting ledger...".bold());
    
    let block = compact_ledger(&ledger_path, &keypair).map_err(|e| {
        eprintln!("{}: {}", "Error".red().bold(), e);
        e
    })?;

    println!(
        "{}",
        "🚀 Ledger successfully compacted and archived.".green().bold()
    );
    println!("  {}:   {}", "New Genesis Block Hash".bold(), block.block_hash.cyan());
    println!("  {}: {}", "Compacted Payload Hash".bold(), block.payload_hash.dimmed());
    println!("  {}:      {}", "Signer".bold(), block.signer_public_key.dimmed());
    println!("  {}:   {}", "Timestamp".bold(), block.timestamp.dimmed());
    println!(
        "  {}:      {}",
        "New Ledger File".bold(),
        ledger_path.display().to_string().dimmed()
    );

    Ok(())
}
