use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use rand::{RngCore, SeedableRng};
use std::path::Path;

pub fn generate_random_name() -> Result<String> {
    // Allow setting seed for testing
    let mut rng = if let Ok(seed_str) = std::env::var("XLAUDE_TEST_SEED") {
        let seed: u64 = seed_str.parse().unwrap_or(42);
        Box::new(rand::rngs::StdRng::seed_from_u64(seed)) as Box<dyn RngCore>
    } else {
        Box::new(rand::thread_rng()) as Box<dyn RngCore>
    };

    // Generate 128 bits of entropy for a 12-word mnemonic
    let mut entropy = [0u8; 16];
    rng.fill_bytes(&mut entropy);

    let mnemonic = bip39::Mnemonic::from_entropy(&entropy)?;
    let words: Vec<&str> = mnemonic.words().collect();

    // Use the same RNG for choosing the word
    let mut chooser_rng = if let Ok(seed_str) = std::env::var("XLAUDE_TEST_SEED") {
        let seed: u64 = seed_str.parse().unwrap_or(42);
        rand::rngs::StdRng::seed_from_u64(seed)
    } else {
        rand::rngs::StdRng::from_entropy()
    };

    words
        .choose(&mut chooser_rng)
        .map(|&word| word.to_string())
        .context("Failed to generate random name")
}

/// Sanitize a branch name for use in directory names
/// Replaces forward slashes with hyphens to avoid creating subdirectories
pub fn sanitize_branch_name(branch: &str) -> String {
    branch.replace('/', "-")
}

pub fn execute_in_dir<P, F, R>(path: P, f: F) -> Result<R>
where
    P: AsRef<Path>,
    F: FnOnce() -> Result<R>,
{
    let original_dir = std::env::current_dir().context("Failed to get current directory")?;
    std::env::set_current_dir(&path)
        .with_context(|| format!("Failed to change to directory: {}", path.as_ref().display()))?;

    let result = f();

    std::env::set_current_dir(&original_dir).context("Failed to restore original directory")?;

    result
}

/// Resolve agent command from state or default, and split into program + args.
pub fn resolve_agent_command() -> Result<(String, Vec<String>)> {
    let state = crate::state::XlaudeState::load()?;
    let raw = state
        .agent
        .clone()
        .unwrap_or_else(crate::state::get_default_agent);
    let cmdline = normalize_agent_command(&raw);
    split_command_line(&cmdline)
}

/// Split a command line string into executable and arguments using shell-style parsing.
pub fn split_command_line(cmdline: &str) -> Result<(String, Vec<String>)> {
    let parts = shell_words::split(cmdline)
        .map_err(|e| anyhow::anyhow!("Invalid agent command: {} ({e})", cmdline))?;

    if parts.is_empty() {
        anyhow::bail!("Agent command is empty");
    }

    let program = parts[0].clone();
    let args = parts[1..].to_vec();
    Ok((program, args))
}

/// Normalize common agent aliases to full commands.
/// - "claude"  -> "claude --dangerously-skip-permissions"
/// - "gemini"  -> "gemini -y"
/// - others are returned unchanged.
pub fn normalize_agent_command(cmd: &str) -> String {
    let trimmed = cmd.trim();
    if trimmed.eq_ignore_ascii_case("claude") {
        return crate::state::get_default_agent();
    }
    if trimmed.eq_ignore_ascii_case("gemini") {
        return "gemini -y".to_string();
    }
    trimmed.to_string()
}
