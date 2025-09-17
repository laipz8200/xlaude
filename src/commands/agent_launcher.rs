use anyhow::{Context, Result};
use std::process::Command;

use crate::commands::agent_prompt::{AgentSelection, option_info, prompt_agent_selection};
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

    let option = option_info(selection);

    if let Some(command) = option.command {
        let command_to_run = command.to_string();
        spawn_agent(worktree, AgentCommand::Override(&command_to_run))?;
    }

    Ok(selection)
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

    if config.eq_ignore_ascii_case("gemini") {
        return AgentSelection::Gemini;
    }

    let normalized = config.to_ascii_lowercase();
    if normalized.starts_with("codex") {
        AgentSelection::Codex
    } else if normalized.starts_with("gemini") {
        AgentSelection::Gemini
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

    #[test]
    fn gemini_config_sets_gemini_default() {
        assert_eq!(
            default_agent_selection_from_config(Some("gemini")),
            AgentSelection::Gemini
        );
    }

    #[test]
    fn gemini_with_flags_defaults_to_gemini() {
        assert_eq!(
            default_agent_selection_from_config(Some("gemini -y")),
            AgentSelection::Gemini
        );
    }
}
