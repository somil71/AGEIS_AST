//! `needle policy` CLI subcommand implementation for ingestion, structuring, and listing.

use needle::error::{Error, Result};
use needle::llm::LlmClient;
use needle::policy::clause::{ObligationType, PolicyDocument, Severity};
use needle::policy::parser::PolicyParser;
use needle::policy::structurer::ObligationStructurer;
use needle::storage::Storage;
use clap::Subcommand;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Subcommand, Debug, Clone)]
pub enum PolicyCommands {
    /// Ingest and parse a policy document (.pdf, .md, .txt, .policy)
    Ingest {
        /// Path to policy document file
        #[arg(required = true)]
        path: String,

        /// Custom policy name / title
        #[arg(short, long)]
        name: Option<String>,

        /// Policy semantic version
        #[arg(short = 'V', long, default_value = "1.0.0")]
        version: String,

        /// Dry-run mode: parse and extract without saving to disk
        #[arg(long)]
        dry_run: bool,

        /// Force deterministic heuristic-only structuring (bypass LLM)
        #[arg(long)]
        heuristic_only: bool,

        /// Output format: table, json, summary
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// List all ingested policy documents and obligations
    List {
        /// Output format: table, json
        #[arg(short, long, default_value = "table")]
        format: String,

        /// Show individual clauses and structured obligations
        #[arg(short, long)]
        verbose: bool,
    },
}

pub async fn run(cmd: PolicyCommands) -> Result<()> {
    match cmd {
        PolicyCommands::Ingest {
            path,
            name,
            version,
            dry_run,
            heuristic_only,
            format,
        } => run_ingest(&path, name, &version, dry_run, heuristic_only, &format).await,
        PolicyCommands::List { format, verbose } => run_list(&format, verbose).await,
    }
}

async fn run_ingest(
    file_path: &str,
    custom_name: Option<String>,
    version: &str,
    dry_run: bool,
    heuristic_only: bool,
    format: &str,
) -> Result<()> {
    let path = PathBuf::from(file_path);
    if !path.exists() {
        return Err(Error::InvalidPath(format!(
            "Policy file not found: {}",
            path.display()
        )));
    }

    let mut document = PolicyParser::parse_file(&path, None, custom_name, Some(version.to_string()))?;

    // Initialize structurer (LLM with deterministic heuristic fallback, or heuristic-only)
    let structurer = if heuristic_only {
        ObligationStructurer::heuristic_only()
    } else {
        let client = Some(LlmClient::from_env());
        ObligationStructurer::new(client)
    };

    let total_obligations = structurer.structure_document(&mut document).await?;

    if !dry_run {
        let storage = Storage::new(Storage::default_index_dir())?;
        storage.save_policy(&document)?;
    }

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&document)?);
        }
        _ => {
            print_ingest_summary(&document, total_obligations, dry_run);
        }
    }

    Ok(())
}

async fn run_list(format: &str, verbose: bool) -> Result<()> {
    let storage = Storage::new(Storage::default_index_dir())?;
    let policies = storage.list_policies()?;

    if policies.is_empty() {
        println!(
            "{}",
            "No policies ingested yet. Ingest a policy with: needle policy ingest <path>".yellow()
        );
        return Ok(());
    }

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&policies)?);
        }
        _ => {
            println!("{}", "Ingested Policy Documents\n".bold());
            for doc in &policies {
                println!(
                    "  {} {} (v{}) — {} clauses, {} obligations",
                    "•".cyan(),
                    doc.name.bold(),
                    doc.version.dimmed(),
                    doc.clauses.len().to_string().green(),
                    doc.total_obligations().to_string().green().bold()
                );
                println!("    ID:          {}", doc.id.cyan());
                println!("    Format:      {}", doc.format.as_str());
                println!("    Source:      {}", doc.source_path.dimmed());
                println!("    Ingested:    {}", doc.created_at.dimmed());

                if verbose {
                    println!();
                    for clause in &doc.clauses {
                        println!(
                            "    Clause {} — {}",
                            clause.clause_number.bold(),
                            clause.title
                        );
                        for obl in &clause.obligations {
                            let type_str = obl.obligation_type.as_str();
                            let type_colored = match obl.obligation_type {
                                ObligationType::Must => type_str.cyan(),
                                ObligationType::MustNot => type_str.red(),
                                ObligationType::Should => type_str.yellow(),
                                ObligationType::May => type_str.green(),
                                ObligationType::RequiredIf => type_str.blue(),
                                ObligationType::ProhibitedIf => type_str.magenta(),
                            };

                            let sev_str = obl.severity.as_str();
                            let sev_colored = match obl.severity {
                                Severity::Critical => sev_str.bold().red(),
                                Severity::High => sev_str.red(),
                                Severity::Medium => sev_str.yellow(),
                                Severity::Low => sev_str.blue(),
                                Severity::Informational => sev_str.dimmed(),
                            };

                            println!(
                                "      [{}] [{}] {}: {}",
                                type_colored,
                                sev_colored,
                                obl.id.dimmed(),
                                obl.title
                            );
                            if let Some(cond) = &obl.condition {
                                println!("        Condition: {}", cond.dimmed());
                            }
                            if let Some(act) = &obl.action {
                                println!("        Action:    {}", act.dimmed());
                            }
                        }
                    }
                    println!();
                }
            }
        }
    }

    Ok(())
}

fn print_ingest_summary(doc: &PolicyDocument, total_obligations: usize, dry_run: bool) {
    println!("{}", "Policy Ingestion Summary\n".bold());
    println!("  Title:       {}", doc.name.green().bold());
    println!("  Policy ID:   {}", doc.id.cyan());
    println!("  Version:     {}", doc.version);
    println!("  Format:      {}", doc.format.as_str());
    println!("  Source:      {}", doc.source_path);
    println!("  Clauses:     {}", doc.clauses.len().to_string().cyan());
    println!(
        "  Obligations: {}",
        total_obligations.to_string().green().bold()
    );

    let critical_count = doc
        .clauses
        .iter()
        .flat_map(|c| &c.obligations)
        .filter(|o| o.severity == Severity::Critical)
        .count();
    let high_count = doc
        .clauses
        .iter()
        .flat_map(|c| &c.obligations)
        .filter(|o| o.severity == Severity::High)
        .count();
    let medium_count = doc
        .clauses
        .iter()
        .flat_map(|c| &c.obligations)
        .filter(|o| o.severity == Severity::Medium)
        .count();
    let low_count = doc
        .clauses
        .iter()
        .flat_map(|c| &c.obligations)
        .filter(|o| o.severity == Severity::Low || o.severity == Severity::Informational)
        .count();

    println!(
        "  Severities:  {} critical, {} high, {} medium, {} low/info",
        critical_count.to_string().red().bold(),
        high_count.to_string().red(),
        medium_count.to_string().yellow(),
        low_count.to_string().blue()
    );

    if dry_run {
        println!(
            "  Status:      {}",
            "[DRY RUN — NOT PERSISTED]".yellow().bold()
        );
    } else {
        println!("  Status:      {}", "Successfully Ingested & Saved".green());
    }

    println!();
    println!("  {}:", "Extracted Obligations Breakdown".bold());
    for clause in &doc.clauses {
        println!(
            "  Clause {} — {}",
            clause.clause_number.bold(),
            clause.title
        );
        for obl in &clause.obligations {
            println!(
                "    • [{}] [{}] {}: {}",
                obl.obligation_type.as_str().cyan(),
                obl.severity.as_str().yellow(),
                obl.id.dimmed(),
                obl.title
            );
        }
    }
}
