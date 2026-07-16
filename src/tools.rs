//! Launching external tools (diff viewer, file viewer, editor).
//!
//! Depends only on `config` — the app layer decides *which* action to run
//! and owns the terminal handoff around it; this module only spawns the
//! process and reports a non-zero exit as a user-facing message.

use crate::config::{ToolCommand, ToolsConfig};
use anyhow::Result;
use std::path::PathBuf;

/// An external tool invocation, queued by the app and executed after the
/// current frame (the terminal must be released first).
pub enum ExternalAction {
    Diff { left: PathBuf, right: PathBuf },
    ViewLeft(PathBuf),
    EditLeft(PathBuf),
    ViewRight(PathBuf),
    EditRight(PathBuf),
}

/// Runs `action` with the matching configured tool. Returns a status-bar
/// message when the tool exits non-zero; `None` on success or when the
/// corresponding tool is not configured.
pub fn run_action(tools: &ToolsConfig, action: &ExternalAction) -> Result<Option<String>> {
    let msg = match action {
        ExternalAction::Diff { left, right } => {
            if let Some(cmd) = &tools.diff_tool {
                launch_cmd(cmd, &[left, right])?
            } else {
                None
            }
        }
        ExternalAction::ViewLeft(p) | ExternalAction::ViewRight(p) => {
            if let Some(cmd) = &tools.viewer {
                launch_cmd(cmd, &[p])?
            } else {
                None
            }
        }
        ExternalAction::EditLeft(p) | ExternalAction::EditRight(p) => {
            if let Some(cmd) = &tools.editor {
                launch_cmd(cmd, &[p])?
            } else {
                None
            }
        }
    };
    Ok(msg)
}

/// Whether the tool required by `action` is configured. The app layer checks
/// this before queuing an action so the terminal is never suspended (a visible
/// flicker) for an action `run_action` would treat as a no-op.
pub fn action_tool_configured(tools: &ToolsConfig, action: &ExternalAction) -> bool {
    match action {
        ExternalAction::Diff { .. } => tools.diff_tool.is_some(),
        ExternalAction::ViewLeft(_) | ExternalAction::ViewRight(_) => tools.viewer.is_some(),
        ExternalAction::EditLeft(_) | ExternalAction::EditRight(_) => tools.editor.is_some(),
    }
}

fn launch_cmd(tool: &ToolCommand, args: &[&PathBuf]) -> Result<Option<String>> {
    let (program, pre_args) = tool.program_and_args();
    if program.is_empty() {
        return Err(anyhow::anyhow!("empty command"));
    }
    let mut cmd = std::process::Command::new(program);
    for arg in pre_args {
        cmd.arg(arg);
    }
    for path in args {
        cmd.arg(path);
    }
    let status = cmd.status()?;
    if status.success() {
        Ok(None)
    } else {
        Ok(Some(format!("Tool exited with {status}")))
    }
}
