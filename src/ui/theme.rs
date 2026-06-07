use std::{collections::BTreeMap, env, fs, path::PathBuf, str::FromStr};

use ratatui::style::Color;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    TokyoNight,
    Catppuccin,
}

impl ThemeName {
    pub const ALL: [Self; 2] = [Self::TokyoNight, Self::Catppuccin];

    pub const fn id(self) -> &'static str {
        match self {
            Self::TokyoNight => "tokyo_night",
            Self::Catppuccin => "catppuccin",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::TokyoNight => "Tokyo Night",
            Self::Catppuccin => "Catppuccin Mocha",
        }
    }
}

impl Default for ThemeName {
    fn default() -> Self {
        Self::TokyoNight
    }
}

impl FromStr for ThemeName {
    type Err = ThemeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "tokyo_night" | "tokyonight" | "tira_dark" => Ok(Self::TokyoNight),
            "catppuccin" | "catppuccin_mocha" | "catpuccin" => Ok(Self::Catppuccin),
            other => Err(ThemeError(format!("Unknown theme `{other}`"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeChoice {
    pub name: ThemeName,
}

impl ThemeChoice {
    pub fn label(self) -> String {
        self.name.label().to_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    name: ThemeName,
    status_bar_bg: Color,
    status_text: Color,
    status_mode_fg: Color,
    status_normal_bg: Color,
    status_input_bg: Color,
    status_project_bg: Color,
    status_project_fg: Color,
    status_time_bg: Color,
    status_time_fg: Color,
    selected_fg: Color,
    selected_bg: Color,
    selected_alt_fg: Color,
    muted_fg: Color,
    subtle_fg: Color,
    accent_fg: Color,
    success_fg: Color,
    error_fg: Color,
    border_fg: Color,
    highlight_fg: Color,
    highlight_bg: Color,
    key_fg: Color,
    epic_fg: Color,
    task_fg: Color,
    subtask_fg: Color,
    overrides: BTreeMap<String, String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self::named(ThemeName::default())
    }
}

impl Theme {
    pub fn named(name: ThemeName) -> Self {
        match name {
            ThemeName::TokyoNight => Self::tokyo_night(),
            ThemeName::Catppuccin => Self::catppuccin(),
        }
    }

    pub fn from_toml_str(text: &str) -> Result<Self, ThemeError> {
        let file = toml::from_str::<ThemeFile>(text)
            .map_err(|error| ThemeError(format!("Theme config could not be read: {error}")))?;
        let name = file
            .theme
            .as_deref()
            .map(ThemeName::from_str)
            .transpose()?
            .unwrap_or_default();
        let mut theme = Self::named(name);
        if let Some(overrides) = file.colors {
            for (role, value) in &overrides {
                theme.set_role(role.as_str(), value.as_str())?;
            }
            theme.overrides = overrides;
        }
        Ok(theme)
    }

    pub fn choices(&self) -> Vec<ThemeChoice> {
        ThemeName::ALL
            .into_iter()
            .map(|name| ThemeChoice { name })
            .collect()
    }

    pub fn with_name(&self, name: ThemeName) -> Self {
        let mut theme = Self::named(name);
        for (role, value) in &self.overrides {
            let _ = theme.set_role(role, value);
        }
        theme.overrides = self.overrides.clone();
        theme
    }

    pub fn name(&self) -> ThemeName {
        self.name
    }
    pub fn name_label(&self) -> &'static str {
        self.name.label()
    }
    pub fn status_bar_bg(&self) -> Color {
        self.status_bar_bg
    }
    pub fn status_text(&self) -> Color {
        self.status_text
    }
    pub fn status_mode_fg(&self) -> Color {
        self.status_mode_fg
    }
    pub fn status_normal_bg(&self) -> Color {
        self.status_normal_bg
    }
    pub fn status_input_bg(&self) -> Color {
        self.status_input_bg
    }
    pub fn status_project_bg(&self) -> Color {
        self.status_project_bg
    }
    pub fn status_project_fg(&self) -> Color {
        self.status_project_fg
    }
    pub fn status_time_bg(&self) -> Color {
        self.status_time_bg
    }
    pub fn status_time_fg(&self) -> Color {
        self.status_time_fg
    }
    pub fn selected_fg(&self) -> Color {
        self.selected_fg
    }
    pub fn selected_bg(&self) -> Color {
        self.selected_bg
    }
    pub fn selected_alt_fg(&self) -> Color {
        self.selected_alt_fg
    }
    pub fn muted_fg(&self) -> Color {
        self.muted_fg
    }
    pub fn subtle_fg(&self) -> Color {
        self.subtle_fg
    }
    pub fn accent_fg(&self) -> Color {
        self.accent_fg
    }
    pub fn success_fg(&self) -> Color {
        self.success_fg
    }
    pub fn error_fg(&self) -> Color {
        self.error_fg
    }
    pub fn border_fg(&self) -> Color {
        self.border_fg
    }
    pub fn highlight_fg(&self) -> Color {
        self.highlight_fg
    }
    pub fn highlight_bg(&self) -> Color {
        self.highlight_bg
    }
    pub fn key_fg(&self) -> Color {
        self.key_fg
    }
    pub fn issue_type_fg(&self, kind: &str) -> Color {
        match kind {
            "Epic" => self.epic_fg,
            "Sub-task" => self.subtask_fg,
            _ => self.task_fg,
        }
    }

    fn tokyo_night() -> Self {
        Self {
            name: ThemeName::TokyoNight,
            status_bar_bg: Color::Rgb(26, 27, 38),
            status_text: Color::Rgb(192, 202, 245),
            status_mode_fg: Color::Rgb(26, 27, 38),
            status_normal_bg: Color::Rgb(224, 175, 104),
            status_input_bg: Color::Rgb(158, 206, 106),
            status_project_bg: Color::Rgb(187, 154, 247),
            status_project_fg: Color::Rgb(26, 27, 38),
            status_time_bg: Color::Rgb(125, 207, 255),
            status_time_fg: Color::Rgb(26, 27, 38),
            selected_fg: Color::Rgb(158, 206, 106),
            selected_bg: Color::Rgb(41, 46, 66),
            selected_alt_fg: Color::Rgb(192, 202, 245),
            muted_fg: Color::Rgb(86, 95, 137),
            subtle_fg: Color::Rgb(169, 177, 214),
            accent_fg: Color::Rgb(125, 207, 255),
            success_fg: Color::Rgb(158, 206, 106),
            error_fg: Color::Rgb(247, 118, 142),
            border_fg: Color::Rgb(65, 72, 104),
            highlight_fg: Color::Rgb(26, 27, 38),
            highlight_bg: Color::Rgb(224, 175, 104),
            key_fg: Color::Rgb(122, 162, 247),
            epic_fg: Color::Rgb(187, 154, 247),
            task_fg: Color::Rgb(125, 207, 255),
            subtask_fg: Color::Rgb(122, 162, 247),
            overrides: BTreeMap::new(),
        }
    }

    fn catppuccin() -> Self {
        Self {
            name: ThemeName::Catppuccin,
            status_bar_bg: Color::Rgb(30, 30, 46),
            status_text: Color::Rgb(205, 214, 244),
            status_mode_fg: Color::Rgb(30, 30, 46),
            status_normal_bg: Color::Rgb(249, 226, 175),
            status_input_bg: Color::Rgb(166, 227, 161),
            status_project_bg: Color::Rgb(203, 166, 247),
            status_project_fg: Color::Rgb(30, 30, 46),
            status_time_bg: Color::Rgb(137, 220, 235),
            status_time_fg: Color::Rgb(30, 30, 46),
            selected_fg: Color::Rgb(166, 227, 161),
            selected_bg: Color::Rgb(49, 50, 68),
            selected_alt_fg: Color::Rgb(205, 214, 244),
            muted_fg: Color::Rgb(108, 112, 134),
            subtle_fg: Color::Rgb(166, 173, 200),
            accent_fg: Color::Rgb(137, 220, 235),
            success_fg: Color::Rgb(166, 227, 161),
            error_fg: Color::Rgb(243, 139, 168),
            border_fg: Color::Rgb(69, 71, 90),
            highlight_fg: Color::Rgb(30, 30, 46),
            highlight_bg: Color::Rgb(249, 226, 175),
            key_fg: Color::Rgb(137, 180, 250),
            epic_fg: Color::Rgb(203, 166, 247),
            task_fg: Color::Rgb(137, 220, 235),
            subtask_fg: Color::Rgb(137, 180, 250),
            overrides: BTreeMap::new(),
        }
    }

    fn set_role(&mut self, role: &str, value: &str) -> Result<(), ThemeError> {
        let color = parse_hex_color(value)?;
        match role {
            "status_bar_bg" => self.status_bar_bg = color,
            "status_text" => self.status_text = color,
            "status_mode_fg" => self.status_mode_fg = color,
            "status_normal_bg" => self.status_normal_bg = color,
            "status_input_bg" => self.status_input_bg = color,
            "status_project_bg" => self.status_project_bg = color,
            "status_project_fg" => self.status_project_fg = color,
            "status_time_bg" => self.status_time_bg = color,
            "status_time_fg" => self.status_time_fg = color,
            "selected_fg" => self.selected_fg = color,
            "selected_bg" => self.selected_bg = color,
            "selected_alt_fg" => self.selected_alt_fg = color,
            "muted_fg" => self.muted_fg = color,
            "subtle_fg" => self.subtle_fg = color,
            "accent_fg" => self.accent_fg = color,
            "success_fg" => self.success_fg = color,
            "error_fg" => self.error_fg = color,
            "border_fg" => self.border_fg = color,
            "highlight_fg" => self.highlight_fg = color,
            "highlight_bg" => self.highlight_bg = color,
            "key_fg" => self.key_fg = color,
            "epic_fg" => self.epic_fg = color,
            "task_fg" => self.task_fg = color,
            "subtask_fg" => self.subtask_fg = color,
            _ => return Err(ThemeError(format!("Unknown theme role `{role}`"))),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError(pub String);

#[derive(Debug, Deserialize)]
struct ThemeFile {
    theme: Option<String>,
    colors: Option<BTreeMap<String, String>>,
}

pub fn load_theme() -> Result<Theme, ThemeError> {
    let Some(path) = theme_path() else {
        return Ok(Theme::default());
    };
    match fs::read_to_string(path) {
        Ok(text) => Theme::from_toml_str(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Theme::default()),
        Err(error) => Err(ThemeError(format!(
            "Theme config could not be opened: {error}"
        ))),
    }
}

fn theme_path() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".tira/tui.toml"))
}

fn parse_hex_color(value: &str) -> Result<Color, ThemeError> {
    let value = value.trim();
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| ThemeError(format!("Theme color `{value}` must start with #")))?;
    if hex.len() != 6 {
        return Err(ThemeError(format!("Theme color `{value}` must be #RRGGBB")));
    }
    let red = parse_hex_pair(&hex[0..2], value)?;
    let green = parse_hex_pair(&hex[2..4], value)?;
    let blue = parse_hex_pair(&hex[4..6], value)?;
    Ok(Color::Rgb(red, green, blue))
}

fn parse_hex_pair(pair: &str, original: &str) -> Result<u8, ThemeError> {
    u8::from_str_radix(pair, 16)
        .map_err(|_| ThemeError(format!("Theme color `{original}` contains invalid hex")))
}

pub fn save_theme_name(name: ThemeName) -> Result<(), ThemeError> {
    let Some(path) = theme_path() else {
        return Err(ThemeError(String::from("HOME is not set")));
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ThemeError(format!(
                "Theme config directory could not be created: {error}"
            ))
        })?;
    }
    let mut table = match fs::read_to_string(&path) {
        Ok(text) => text
            .parse::<toml::Table>()
            .map_err(|error| ThemeError(format!("Theme config could not be read: {error}")))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => toml::Table::new(),
        Err(error) => {
            return Err(ThemeError(format!(
                "Theme config could not be opened: {error}"
            )));
        }
    };
    table.insert(
        "theme".to_owned(),
        toml::Value::String(name.id().to_owned()),
    );
    let text = toml::to_string_pretty(&table)
        .map_err(|error| ThemeError(format!("Theme config could not be serialized: {error}")))?;
    fs::write(&path, text)
        .map_err(|error| ThemeError(format!("Theme config could not be saved: {error}")))
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::{Theme, ThemeName};

    #[test]
    fn theme_overrides_status_roles_by_name() {
        let theme = Theme::from_toml_str(
            r##"
            theme = "catppuccin"

            [colors]
            status_bar_bg = "#010203"
            status_text = "#AABBCC"
            "##,
        )
        .expect("theme");

        assert_eq!(theme.name(), ThemeName::Catppuccin);
        assert_eq!(theme.status_bar_bg(), Color::Rgb(1, 2, 3));
        assert_eq!(theme.status_text(), Color::Rgb(170, 187, 204));
    }

    #[test]
    fn legacy_tira_dark_theme_name_loads_as_tokyo_night() {
        let theme = Theme::from_toml_str(
            r##"
            theme = "tira-dark"
            show_footer = true

            [themes.tira-dark]
            background = "#0f1117"
            foreground = "#d7dae0"
            "##,
        )
        .expect("legacy theme");

        assert_eq!(theme.name(), ThemeName::TokyoNight);
    }

    #[test]
    fn unknown_theme_roles_are_rejected() {
        let error = Theme::from_toml_str(
            r##"
            [colors]
            typo = "#010203"
            "##,
        )
        .expect_err("invalid theme");

        assert!(error.0.contains("Unknown theme role"));
    }
}
