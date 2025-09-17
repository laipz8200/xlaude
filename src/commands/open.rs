use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;

use crate::commands::agent_launcher::launch_with_menu;
use crate::git::{get_current_branch, get_repo_name, is_base_branch, is_in_worktree};
use crate::input::{get_command_arg, is_piped_input, smart_confirm, smart_select};
use crate::state::{WorktreeInfo, XlaudeState};
use crate::utils::sanitize_branch_name;

pub fn handle_open(name: Option<String>) -> Result<()> {
    let mut state = XlaudeState::load()?;

    // Check if current path is a worktree when no name is provided
    // Note: base branches (main/master/develop) are not considered worktrees
    // Skip this check if we have piped input waiting to be read
    if name.is_none() && is_in_worktree()? && !is_base_branch()? {
        // If there's piped input waiting, don't use current worktree detection
        // This allows piped input to override current directory detection
        if is_piped_input() && std::env::var("XLAUDE_TEST_MODE").is_err() {
            // There's piped input, so skip current worktree detection
        } else {
            // Get current repository info
            let repo_name = get_repo_name().context("Not in a git repository")?;
            let current_branch = get_current_branch()?;
            let current_dir = std::env::current_dir()?;

            // Sanitize branch name for key lookup
            let worktree_name = sanitize_branch_name(&current_branch);

            // Check if this worktree is already managed
            let key = XlaudeState::make_key(&repo_name, &worktree_name);

            let worktree_info = if let Some(info) = state.worktrees.get(&key).cloned() {
                info
            } else {
                // Not managed, ask if user wants to add it
                println!(
                    "{} Current directory is a worktree but not managed by xlaude",
                    "ℹ️".blue()
                );
                println!(
                    "  {} {}/{}",
                    "Worktree:".bright_black(),
                    repo_name,
                    current_branch
                );
                println!("  {} {}", "Path:".bright_black(), current_dir.display());

                // Use smart confirm for pipe support
                let should_add = smart_confirm(
                    "Would you like to add this worktree to xlaude and open it?",
                    true,
                )?;

                if !should_add {
                    return Ok(());
                }

                // Add to state
                println!(
                    "{} Adding worktree '{}' to xlaude management...",
                    "➕".green(),
                    worktree_name.cyan()
                );

                state.worktrees.insert(
                    key.clone(),
                    WorktreeInfo {
                        name: worktree_name.clone(),
                        branch: current_branch.clone(),
                        path: current_dir.clone(),
                        repo_name: repo_name.clone(),
                        created_at: Utc::now(),
                    },
                );
                state.save()?;

                println!("{} Worktree added successfully", "✅".green());
                state.worktrees.get(&key).cloned().unwrap_or(WorktreeInfo {
                    name: worktree_name,
                    branch: current_branch,
                    path: current_dir,
                    repo_name,
                    created_at: Utc::now(),
                })
            };

            let _ = launch_with_menu(
                &worktree_info,
                "Select an agent to open the current worktree with:",
            )
            .context("Failed to launch agent")?;

            return Ok(());
        }
    }

    if state.worktrees.is_empty() {
        anyhow::bail!("No worktrees found. Create one first with 'xlaude create'");
    }

    // Get the name from CLI args or pipe
    let target_name = get_command_arg(name)?;

    // Determine which worktree to open
    let (_key, worktree_info) = if let Some(n) = target_name {
        // Find worktree by name across all projects
        state
            .worktrees
            .iter()
            .find(|(_, w)| w.name == n)
            .map(|(k, w)| (k.clone(), w.clone()))
            .context(format!("Worktree '{n}' not found"))?
    } else {
        // Interactive selection - show repo/name format
        let worktree_list: Vec<(String, WorktreeInfo)> = state
            .worktrees
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let selection = smart_select("Select a worktree to open", &worktree_list, |(_, info)| {
            format!("{}/{}", info.repo_name, info.name)
        })?;

        match selection {
            Some(idx) => worktree_list[idx].clone(),
            None => anyhow::bail!(
                "Interactive selection not available in non-interactive mode. Please specify a worktree name."
            ),
        }
    };

    let _ = launch_with_menu(&worktree_info, "Select an agent to open the worktree with:")
        .context("Failed to launch agent")?;

    Ok(())
}
