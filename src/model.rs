use crate::config::parse_argument_list;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl DockEdge {
    pub(crate) fn korean_name(self) -> &'static str {
        match self {
            DockEdge::Left => "왼쪽",
            DockEdge::Right => "오른쪽",
            DockEdge::Top => "위쪽",
            DockEdge::Bottom => "아래쪽",
        }
    }

    pub(crate) fn ini_value(self) -> &'static str {
        match self {
            DockEdge::Left => "left",
            DockEdge::Right => "right",
            DockEdge::Top => "top",
            DockEdge::Bottom => "bottom",
        }
    }

    pub(crate) fn from_ini_value(value: &str) -> Option<DockEdge> {
        match value.trim().to_ascii_lowercase().as_str() {
            "left" => Some(DockEdge::Left),
            "right" => Some(DockEdge::Right),
            "top" => Some(DockEdge::Top),
            "bottom" => Some(DockEdge::Bottom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ActionItem {
    CopyText {
        name: String,
        text: String,
    },
    RunApplication {
        name: String,
        command: String,
        arguments: Vec<String>,
    },
    OpenPath {
        name: String,
        path: String,
    },
}

impl ActionItem {
    pub(crate) fn name(&self) -> &str {
        match self {
            ActionItem::CopyText { name, .. } => name,
            ActionItem::RunApplication { name, .. } => name,
            ActionItem::OpenPath { name, .. } => name,
        }
    }

    pub(crate) fn kind(&self) -> ActionKind {
        match self {
            ActionItem::CopyText { .. } => ActionKind::CopyText,
            ActionItem::RunApplication { .. } => ActionKind::RunApplication,
            ActionItem::OpenPath { .. } => ActionKind::OpenPath,
        }
    }

    pub(crate) fn search_payload(&self) -> String {
        match self {
            ActionItem::CopyText { text, .. } => text.clone(),
            ActionItem::RunApplication {
                command, arguments, ..
            } => format!("{command} {}", arguments.join(" ")),
            ActionItem::OpenPath { path, .. } => path.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionKind {
    CopyText,
    RunApplication,
    OpenPath,
}

impl ActionKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            ActionKind::CopyText => "복사",
            ActionKind::RunApplication => "실행",
            ActionKind::OpenPath => "열기",
        }
    }

    pub(crate) fn ini_value(self) -> &'static str {
        match self {
            ActionKind::CopyText => "copy_text",
            ActionKind::RunApplication => "run_app",
            ActionKind::OpenPath => "open_path",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EditableActionItem {
    pub(crate) kind: ActionKind,
    pub(crate) name: String,
    pub(crate) text: String,
    pub(crate) command: String,
    pub(crate) arguments: String,
    pub(crate) path: String,
}

impl EditableActionItem {
    pub(crate) fn blank(kind: ActionKind) -> Self {
        Self {
            kind,
            name: "새 항목".to_owned(),
            text: String::new(),
            command: String::new(),
            arguments: String::new(),
            path: String::new(),
        }
    }

    pub(crate) fn from_action_item(item: &ActionItem) -> Self {
        match item {
            ActionItem::CopyText { name, text } => Self {
                kind: ActionKind::CopyText,
                name: name.clone(),
                text: text.clone(),
                command: String::new(),
                arguments: String::new(),
                path: String::new(),
            },
            ActionItem::RunApplication {
                name,
                command,
                arguments,
            } => Self {
                kind: ActionKind::RunApplication,
                name: name.clone(),
                text: String::new(),
                command: command.clone(),
                arguments: arguments.join("|"),
                path: String::new(),
            },
            ActionItem::OpenPath { name, path } => Self {
                kind: ActionKind::OpenPath,
                name: name.clone(),
                text: String::new(),
                command: String::new(),
                arguments: String::new(),
                path: path.clone(),
            },
        }
    }

    pub(crate) fn to_action_item(&self) -> ActionItem {
        match self.kind {
            ActionKind::CopyText => ActionItem::CopyText {
                name: self.name.clone(),
                text: self.text.clone(),
            },
            ActionKind::RunApplication => ActionItem::RunApplication {
                name: self.name.clone(),
                command: self.command.clone(),
                arguments: parse_argument_list(&self.arguments),
            },
            ActionKind::OpenPath => ActionItem::OpenPath {
                name: self.name.clone(),
                path: self.path.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidationSeverity {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub(crate) struct ValidationMessage {
    pub(crate) severity: ValidationSeverity,
    pub(crate) text: String,
}

impl ValidationMessage {
    pub(crate) fn ok(text: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Ok,
            text: text.into(),
        }
    }

    pub(crate) fn warning(text: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Warning,
            text: text.into(),
        }
    }

    pub(crate) fn error(text: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum EditorActionRequest {
    Test(ActionItem),
    Status { message: String, is_error: bool },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ButtonActionRequest {
    Execute(usize),
    RunAsAdmin(usize),
    OpenItemLocation(usize),
    CopyItemPath(usize),
    EditItem(usize),
    DuplicateItem(usize),
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
}

#[derive(Debug, Clone)]
pub(crate) struct QuickDockTab {
    pub(crate) name: String,
    pub(crate) items: Vec<ActionItem>,
    pub(crate) editable_items: Vec<EditableActionItem>,
}

impl QuickDockTab {
    pub(crate) fn new(name: String, items: Vec<ActionItem>) -> Self {
        let editable_items = items
            .iter()
            .map(EditableActionItem::from_action_item)
            .collect();

        Self {
            name,
            items,
            editable_items,
        }
    }
}
