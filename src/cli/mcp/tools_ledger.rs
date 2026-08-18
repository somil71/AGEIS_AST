//! MCP tools for cryptographic audit ledger management.
//!
//! Exposes tools: get_ledger_status, verify_ledger, sign_ledger

use needle::ledger::{
    append_to_ledger, default_key_priv_path, default_key_pub_path, default_ledger_path,
    verify_ledger_file, EntryType, LedgerKeypair,
};
use serde_json::Value;
use std::path::PathBuf;
use std::str::FromStr;

type ToolResult = Result<String, String>;

/// Returns the current status of the cryptographic ledger (number of blocks, latest hash).
pub fn get_ledger_status(_args: &Value) -> ToolResult {
    let ledger_path = default_ledger_path();

    if !ledger_path.exists() {
        return Ok("Ledger is empty (0 blocks).".to_string());
    }

    match verify_ledger_file(&ledger_path) {
        Ok(summary) => {
            let mut out = format!("## Ledger Status\n\n**Total Blocks:** {}\n", summary.total_blocks);
            if let Some(hash) = summary.latest_block_hash {
                out.push_str(&format!("**Latest Block Hash:** `{}`\n", hash));
            }
            out.push_str("**Integrity:** Intact and verified.\n");
            Ok(out)
        }
        Err(e) => Err(format!("Ledger verification failed: {e}")),
    }
}

/// Cryptographically verifies the entire ledger chain.
pub fn verify_ledger(_args: &Value) -> ToolResult {
    let ledger_path = default_ledger_path();

    if !ledger_path.exists() {
        return Ok("Ledger is empty (0 blocks). Chain is trivially intact.".to_string());
    }

    match verify_ledger_file(&ledger_path) {
        Ok(summary) => {
            Ok(format!(
                "✅ Ledger verified successfully.\n\n- **Valid Blocks:** {}\n- **Latest Hash:** `{}`\n\nAll Ed25519 signatures and SHA-256 hash chains are intact.",
                summary.total_blocks,
                summary.latest_block_hash.unwrap_or_default()
            ))
        }
        Err(e) => Err(format!("❌ Ledger verification failed! Chain is broken or tampered: {e}")),
    }
}

/// Appends a new JSON payload to the ledger and signs it.
pub fn sign_ledger(args: &Value) -> ToolResult {
    let report_content = args["report_json"]
        .as_str()
        .ok_or("report_json string is required")?;
        
    let entry_type_str = args["entry_type"]
        .as_str()
        .unwrap_or("compliance_audit");

    let payload: Value = serde_json::from_str(report_content)
        .map_err(|e| format!("Failed to parse report_json: {e}"))?;

    let entry_type = EntryType::from_str(entry_type_str)
        .map_err(|e| format!("Invalid entry_type: {e}"))?;

    let priv_key_path = default_key_priv_path();
    let pub_key_path = default_key_pub_path();

    let keypair = if priv_key_path.exists() {
        LedgerKeypair::load_from_file(&priv_key_path)
            .map_err(|e| format!("Failed to load keypair: {e}"))?
    } else {
        if let Some(parent) = priv_key_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("IO error: {e}"))?;
        }
        let kp = LedgerKeypair::generate();
        kp.save_to_files(&priv_key_path, &pub_key_path)
            .map_err(|e| format!("Failed to save new keypair: {e}"))?;
        kp
    };

    let ledger_path = default_ledger_path();
    let block = append_to_ledger(&ledger_path, &keypair, entry_type, payload)
        .map_err(|e| format!("Failed to append to ledger: {e}"))?;

    let out = format!(
        "## Block Appended to Ledger\n\n- **Sequence (Block #):** {}\n- **Block Hash:** `{}`\n- **Payload Hash:** `{}`\n- **Entry Type:** `{:?}`\n- **Signer (Ed25519 Pubkey):** `{}`\n- **Timestamp:** `{}`",
        block.sequence,
        block.block_hash,
        block.payload_hash,
        block.entry_type,
        block.signer_public_key,
        block.timestamp
    );

    Ok(out)
}

/// Compacts the ledger into a single genesis block and archives the history.
pub fn snapshot_ledger(_args: &Value) -> ToolResult {
    use needle::ledger::compact_ledger;

    let ledger_path = default_ledger_path();
    let priv_key_path = default_key_priv_path();

    if !priv_key_path.exists() {
        return Err(format!(
            "Private key not found at '{}'. Please generate a keypair first.",
            priv_key_path.display()
        ));
    }

    let keypair = LedgerKeypair::load_from_file(&priv_key_path)
        .map_err(|e| format!("Failed to load keypair: {e}"))?;

    let block = compact_ledger(&ledger_path, &keypair)
        .map_err(|e| format!("Failed to compact ledger: {e}"))?;

    let out = format!(
        "🚀 **Ledger Compacted & Archived Successfully**\n\n- **New Genesis Block Hash:** `{}`\n- **Signer:** `{}`\n- **Timestamp:** `{}`\n\nThe previous chain history has been archived. The main ledger file now contains only 1 snapshot block.",
        block.block_hash,
        block.signer_public_key,
        block.timestamp
    );

    Ok(out)
}
