use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Rectangle,
    Arrow,
    Text,
    Save,
    Copy,
    Quit,
}

impl Tool {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Select => "🖱",
            Self::Rectangle => "□",
            Self::Arrow => "→",
            Self::Text => "T",
            Self::Save => "💾",
            Self::Copy => "📋",
            Self::Quit => "✕",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::Select,
            Self::Rectangle,
            Self::Arrow,
            Self::Text,
            Self::Save,
            Self::Copy,
            Self::Quit,
        ]
    }
}

pub struct Toolbar {
    pub current: Tool,
}

impl Default for Toolbar {
    fn default() -> Self {
        Self {
            current: Tool::Select,
        }
    }
}

impl Toolbar {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for tool in Tool::all() {
                if ui
                    .selectable_label(self.current == tool, tool.icon())
                    .clicked()
                {
                    self.current = tool;
                }
            }
        });
    }
}