use anyhow::{Context, Result};
use colored::Colorize;
use std::process::{Command, Stdio};

use crate::commands::agent_prompt::{AgentSelection, prompt_agent_selection};
use crate::input::{drain_stdin, is_piped_input};
use crate::state::WorktreeInfo;
use crate::utils::split_command_line;

pub fn launch_with_menu(worktree: &WorktreeInfo, prompt: &str) -> Result<AgentSelection> {
    let state = crate::state::XlaudeState::load()?;
    let configured_agent = state
        .agent
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let default_choice = default_agent_selection_from_config(configured_agent.as_deref());
    let selection = prompt_agent_selection(prompt, default_choice)?;

    match selection {
        AgentSelection::Codex => {
            print_opening_message(worktree, "codex");
            if std::env::var("XLAUDE_TEST_MODE").is_ok() {
                return Ok(AgentSelection::Codex);
            }
            spawn_agent(worktree, AgentCommand::Override("codex"))?;
            Ok(AgentSelection::Codex)
        }
        AgentSelection::Claude => {
            let command_to_run = configured_agent
                .clone()
                .unwrap_or_else(crate::state::get_default_agent);
            print_opening_message(worktree, &command_to_run);
            if std::env::var("XLAUDE_TEST_MODE").is_ok() {
                return Ok(AgentSelection::Claude);
            }
            spawn_agent(worktree, AgentCommand::Override(&command_to_run))?;
            Ok(AgentSelection::Claude)
        }
        AgentSelection::Skip => {
            println!(
                "{} Skipping launch for '{}/{}'.",
                "⏭️".yellow(),
                worktree.repo_name,
                worktree.name.cyan()
            );
            Ok(AgentSelection::Skip)
        }
    }
}

fn print_opening_message(worktree: &WorktreeInfo, agent: &str) {
    println!(
        "{} Opening worktree '{}/{}' with `{}`...",
        "🚀".green(),
        worktree.repo_name,
        worktree.name.cyan(),
        agent.cyan()
    );
}

enum AgentCommand<'a> {
    Override(&'a str),
}

fn spawn_agent(worktree: &WorktreeInfo, command: AgentCommand<'_>) -> Result<()> {
    std::env::set_current_dir(&worktree.path).context("Failed to change directory")?;

    let (program, args) = match command {
        AgentCommand::Override(cmdline) => split_command_line(cmdline)?,
    };

    let mut cmd = Command::new(&program);
    cmd.args(&args);

    cmd.envs(std::env::vars());

    if is_piped_input() {
        drain_stdin()?;
        cmd.stdin(Stdio::null());
    }

    let status = cmd.status().context("Failed to launch agent")?;

    if !status.success() {
        anyhow::bail!("Agent exited with error");
    }

    Ok(())
}

fn default_agent_selection_from_config(agent_config: Option<&str>) -> AgentSelection {
    let Some(config) = agent_config
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return AgentSelection::Claude;
    };

    if config.eq_ignore_ascii_case("codex") {
        return AgentSelection::Codex;
    }

    if config.eq_ignore_ascii_case("claude") {
        return AgentSelection::Claude;
    }

    let normalized = config.to_ascii_lowercase();
    if normalized.starts_with("codex") {
        AgentSelection::Codex
    } else {
        AgentSelection::Claude
    }
}

#[cfg(test)]
mod tests {
    use super::default_agent_selection_from_config;
    use crate::commands::agent_prompt::AgentSelection;

    #[test]
    fn codex_config_sets_codex_default() {
        assert_eq!(
            default_agent_selection_from_config(Some("codex")),
            AgentSelection::Codex
        );
    }

    #[test]
    fn claude_config_sets_claude_default() {
        assert_eq!(
            default_agent_selection_from_config(Some("claude")),
            AgentSelection::Claude
        );
    }

    #[test]
    fn null_config_defaults_to_claude() {
        assert_eq!(
            default_agent_selection_from_config(None),
            AgentSelection::Claude
        );
    }

    #[test]
    fn claude_with_extra_flags_defaults_to_claude() {
        assert_eq!(
            default_agent_selection_from_config(Some("claude --dangerously-skip-permissions")),
            AgentSelection::Claude
        );
    }

    #[test]
    fn unknown_config_defaults_to_claude() {
        assert_eq!(
            default_agent_selection_from_config(Some("true")),
            AgentSelection::Claude
        );
    }
}
