use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Harness {
    Claude,
    Codex,
}

impl Harness {
    pub fn as_str(self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::Codex => "codex",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Common {
    pub harness: Harness,
    pub session_id: String,
    pub cwd: PathBuf,
    pub event_name: String,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tool {
    Bash,
    Edit,
    Write,
    MultiEdit,
    NotebookEdit,
    ApplyPatch,
    Other(String),
}

impl Tool {
    pub fn as_str(&self) -> String {
        match self {
            Tool::Bash => "Bash".into(),
            Tool::Edit => "Edit".into(),
            Tool::Write => "Write".into(),
            Tool::MultiEdit => "MultiEdit".into(),
            Tool::NotebookEdit => "NotebookEdit".into(),
            Tool::ApplyPatch => "apply_patch".into(),
            Tool::Other(s) => s.clone(),
        }
    }
    pub fn is_edit(&self) -> bool {
        matches!(
            self,
            Tool::Edit | Tool::Write | Tool::MultiEdit | Tool::NotebookEdit | Tool::ApplyPatch
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolInput {
    pub command: Option<String>,
    pub file_path: Option<PathBuf>,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolResponse {
    pub stdout: String,
    pub stderr: String,
    pub is_error: bool,
    pub interrupted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    PreToolUse {
        common: Common,
        tool: Tool,
        tool_use_id: Option<String>,
        input: ToolInput,
    },
    PostToolUse {
        common: Common,
        tool: Tool,
        tool_use_id: Option<String>,
        input: ToolInput,
        response: ToolResponse,
    },
    /// Stop and SubagentStop; SubagentStop carries `common.agent_id`.
    Stop {
        common: Common,
        last_message: String,
        stop_hook_active: bool,
    },
    PreCompact {
        common: Common,
        trigger: String,
    },
    Other {
        common: Common,
    },
}

impl Event {
    pub fn common(&self) -> &Common {
        match self {
            Event::PreToolUse { common, .. }
            | Event::PostToolUse { common, .. }
            | Event::Stop { common, .. }
            | Event::PreCompact { common, .. }
            | Event::Other { common } => common,
        }
    }
}
