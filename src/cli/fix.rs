use needle::Result;
use colored::Colorize;
use std::fs;
use needle::llm::LlmClient;

pub async fn run(file_path: &str, issue: &str) -> Result<()> {
    println!("{}", format!("Analyzing {} for issue: {}", file_path, issue).bold());

    let content = fs::read_to_string(file_path)
        .map_err(|e| needle::Error::Io(e))?;

    let system_prompt = "You are a senior security engineer. You are given a file and a description of a security vulnerability. Your job is to output ONLY the completely rewritten, secured file content. Do not include markdown code block backticks (e.g. ```rust), do not include any explanations, do not include any intro or outro text. Just the raw source code so it can be directly piped into the file.";
    
    let user_prompt = format!("File:\n{}\n\nIssue to fix:\n{}\n\nOutput only the raw fixed file contents:", content, issue);

    println!("Contacting LLM to generate fix...");
    
    let mut provider = LlmClient::from_env();
    
    let fixed_content = provider.complete(system_prompt, &user_prompt).await
        .map_err(|e| needle::Error::QueryError(e.to_string()))?;

    // Trim any accidental markdown code blocks if the LLM didn't listen
    let mut clean_content = fixed_content.trim();
    if clean_content.starts_with("```") {
        if let Some(idx) = clean_content.find('\n') {
            clean_content = &clean_content[idx + 1..];
        }
        if clean_content.ends_with("```") {
            clean_content = &clean_content[..clean_content.len() - 3];
        }
    }
    let clean_content = clean_content.trim().to_string();

    let backup_path = format!("{}.bak", file_path);
    if let Err(e) = fs::copy(file_path, &backup_path) {
        eprintln!("{}", format!("⚠ Failed to create backup at {}: {}", backup_path, e).yellow());
    } else {
        println!("{}", format!("ℹ Created safety backup at {}", backup_path).cyan());
    }

    fs::write(file_path, &clean_content)
        .map_err(|e| needle::Error::Io(e))?;

    println!("{}", format!("Successfully applied fix to {}!", file_path).green().bold());
    
    Ok(())
}
