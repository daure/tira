use std::{collections::BTreeMap, env, fs, path::PathBuf, str::FromStr};

use ratatui::style::Color;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    TokyoNight,
    Catppuccin,
    Dracula,
    Nord,
    GruvboxDark,
    SolarizedDark,
    OneDark,
    Monokai,
    AyuDark,
    Kanagawa,
    RosePine,
    EverforestDark,
}

impl ThemeName {
    pub const ALL: [Self; 12] = [
        Self::TokyoNight,
        Self::Catppuccin,
        Self::Dracula,
        Self::Nord,
        Self::GruvboxDark,
        Self::SolarizedDark,
        Self::OneDark,
        Self::Monokai,
        Self::AyuDark,
        Self::Kanagawa,
        Self::RosePine,
        Self::EverforestDark,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::TokyoNight => "tokyo_night",
            Self::Catppuccin => "catppuccin",
            Self::Dracula => "dracula",
            Self::Nord => "nord",
            Self::GruvboxDark => "gruvbox_dark",
            Self::SolarizedDark => "solarized_dark",
            Self::OneDark => "one_dark",
            Self::Monokai => "monokai",
            Self::AyuDark => "ayu_dark",
            Self::Kanagawa => "kanagawa",
            Self::RosePine => "rose_pine",
            Self::EverforestDark => "everforest_dark",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::TokyoNight => "Tokyo Night",
            Self::Catppuccin => "Catppuccin Mocha",
            Self::Dracula => "Dracula",
            Self::Nord => "Nord",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::SolarizedDark => "Solarized Dark",
            Self::OneDark => "One Dark",
            Self::Monokai => "Monokai",
            Self::AyuDark => "Ayu Dark",
            Self::Kanagawa => "Kanagawa",
            Self::RosePine => "Rosé Pine",
            Self::EverforestDark => "Everforest Dark",
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
            "dracula" => Ok(Self::Dracula),
            "nord" => Ok(Self::Nord),
            "gruvbox" | "gruvbox_dark" => Ok(Self::GruvboxDark),
            "solarized" | "solarized_dark" => Ok(Self::SolarizedDark),
            "one_dark" | "onedark" => Ok(Self::OneDark),
            "monokai" => Ok(Self::Monokai),
            "ayu" | "ayu_dark" => Ok(Self::AyuDark),
            "kanagawa" => Ok(Self::Kanagawa),
            "rose_pine" | "rosepine" => Ok(Self::RosePine),
            "everforest" | "everforest_dark" => Ok(Self::EverforestDark),
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
            ThemeName::Dracula => Self::dracula(),
            ThemeName::Nord => Self::nord(),
            ThemeName::GruvboxDark => Self::gruvbox_dark(),
            ThemeName::SolarizedDark => Self::solarized_dark(),
            ThemeName::OneDark => Self::one_dark(),
            ThemeName::Monokai => Self::monokai(),
            ThemeName::AyuDark => Self::ayu_dark(),
            ThemeName::Kanagawa => Self::kanagawa(),
            ThemeName::RosePine => Self::rose_pine(),
            ThemeName::EverforestDark => Self::everforest_dark(),
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
            "Story" => self.success_fg,
            "Task" | "Subtask" | "Sub-task" => self.task_fg,
            "Bug" => self.error_fg,
            _ => self.status_normal_bg,
        }
    }

    fn from_palette(name: ThemeName, palette: Palette) -> Self {
        Self {
            name,
            status_bar_bg: palette.base,
            status_text: palette.text,
            status_mode_fg: palette.base,
            status_normal_bg: palette.yellow,
            status_input_bg: palette.green,
            status_project_bg: palette.magenta,
            status_project_fg: palette.base,
            status_time_bg: palette.cyan,
            status_time_fg: palette.base,
            selected_fg: palette.green,
            selected_bg: palette.surface,
            selected_alt_fg: palette.text,
            muted_fg: palette.muted,
            subtle_fg: palette.subtle,
            accent_fg: palette.cyan,
            success_fg: palette.green,
            error_fg: palette.red,
            border_fg: palette.border,
            highlight_fg: palette.base,
            highlight_bg: palette.yellow,
            key_fg: palette.blue,
            epic_fg: palette.magenta,
            task_fg: palette.cyan,
            subtask_fg: palette.blue,
            overrides: BTreeMap::new(),
        }
    }

    fn tokyo_night() -> Self {
        Self::from_palette(
            ThemeName::TokyoNight,
            Palette::new(
                [26, 27, 38],
                [41, 46, 66],
                [65, 72, 104],
                [192, 202, 245],
                [169, 177, 214],
                [86, 95, 137],
                [122, 162, 247],
                [187, 154, 247],
                [125, 207, 255],
                [158, 206, 106],
                [224, 175, 104],
                [247, 118, 142],
            ),
        )
    }

    fn catppuccin() -> Self {
        Self::from_palette(
            ThemeName::Catppuccin,
            Palette::new(
                [30, 30, 46],
                [49, 50, 68],
                [69, 71, 90],
                [205, 214, 244],
                [166, 173, 200],
                [108, 112, 134],
                [137, 180, 250],
                [203, 166, 247],
                [137, 220, 235],
                [166, 227, 161],
                [249, 226, 175],
                [243, 139, 168],
            ),
        )
    }

    fn dracula() -> Self {
        Self::from_palette(
            ThemeName::Dracula,
            Palette::new(
                [40, 42, 54],
                [68, 71, 90],
                [98, 114, 164],
                [248, 248, 242],
                [189, 147, 249],
                [98, 114, 164],
                [139, 233, 253],
                [255, 121, 198],
                [139, 233, 253],
                [80, 250, 123],
                [241, 250, 140],
                [255, 85, 85],
            ),
        )
    }

    fn nord() -> Self {
        Self::from_palette(
            ThemeName::Nord,
            Palette::new(
                [46, 52, 64],
                [59, 66, 82],
                [76, 86, 106],
                [216, 222, 233],
                [229, 233, 240],
                [129, 161, 193],
                [94, 129, 172],
                [180, 142, 173],
                [136, 192, 208],
                [163, 190, 140],
                [235, 203, 139],
                [191, 97, 106],
            ),
        )
    }

    fn gruvbox_dark() -> Self {
        Self::from_palette(
            ThemeName::GruvboxDark,
            Palette::new(
                [40, 40, 40],
                [60, 56, 54],
                [80, 73, 69],
                [235, 219, 178],
                [213, 196, 161],
                [146, 131, 116],
                [131, 165, 152],
                [211, 134, 155],
                [142, 192, 124],
                [184, 187, 38],
                [250, 189, 47],
                [251, 73, 52],
            ),
        )
    }

    fn solarized_dark() -> Self {
        Self::from_palette(
            ThemeName::SolarizedDark,
            Palette::new(
                [0, 43, 54],
                [7, 54, 66],
                [88, 110, 117],
                [131, 148, 150],
                [147, 161, 161],
                [101, 123, 131],
                [38, 139, 210],
                [211, 54, 130],
                [42, 161, 152],
                [133, 153, 0],
                [181, 137, 0],
                [220, 50, 47],
            ),
        )
    }

    fn one_dark() -> Self {
        Self::from_palette(
            ThemeName::OneDark,
            Palette::new(
                [40, 44, 52],
                [49, 54, 63],
                [92, 99, 112],
                [171, 178, 191],
                [190, 195, 202],
                [92, 99, 112],
                [97, 175, 239],
                [198, 120, 221],
                [86, 182, 194],
                [152, 195, 121],
                [229, 192, 123],
                [224, 108, 117],
            ),
        )
    }

    fn monokai() -> Self {
        Self::from_palette(
            ThemeName::Monokai,
            Palette::new(
                [39, 40, 34],
                [49, 51, 45],
                [73, 72, 62],
                [248, 248, 242],
                [230, 219, 116],
                [117, 113, 94],
                [102, 217, 239],
                [174, 129, 255],
                [102, 217, 239],
                [166, 226, 46],
                [230, 219, 116],
                [249, 38, 114],
            ),
        )
    }

    fn ayu_dark() -> Self {
        Self::from_palette(
            ThemeName::AyuDark,
            Palette::new(
                [11, 18, 24],
                [15, 29, 39],
                [36, 49, 62],
                [191, 199, 213],
                [171, 180, 194],
                [94, 104, 117],
                [57, 186, 230],
                [216, 127, 212],
                [95, 210, 229],
                [195, 232, 141],
                [255, 204, 102],
                [255, 51, 102],
            ),
        )
    }

    fn kanagawa() -> Self {
        Self::from_palette(
            ThemeName::Kanagawa,
            Palette::new(
                [31, 31, 40],
                [42, 42, 55],
                [84, 84, 109],
                [220, 215, 186],
                [200, 192, 147],
                [114, 113, 105],
                [126, 156, 216],
                [149, 127, 184],
                [112, 192, 183],
                [152, 187, 108],
                [230, 195, 132],
                [224, 105, 99],
            ),
        )
    }

    fn rose_pine() -> Self {
        Self::from_palette(
            ThemeName::RosePine,
            Palette::new(
                [25, 23, 36],
                [31, 29, 46],
                [64, 61, 82],
                [224, 222, 244],
                [144, 140, 170],
                [110, 106, 134],
                [49, 116, 143],
                [196, 167, 231],
                [156, 207, 216],
                [156, 207, 216],
                [246, 193, 119],
                [235, 111, 146],
            ),
        )
    }

    fn everforest_dark() -> Self {
        Self::from_palette(
            ThemeName::EverforestDark,
            Palette::new(
                [45, 53, 59],
                [52, 63, 68],
                [75, 86, 91],
                [211, 198, 170],
                [168, 176, 162],
                [127, 137, 125],
                [127, 187, 179],
                [211, 134, 155],
                [131, 192, 146],
                [167, 192, 128],
                [219, 188, 127],
                [230, 126, 128],
            ),
        )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Palette {
    base: Color,
    surface: Color,
    border: Color,
    text: Color,
    subtle: Color,
    muted: Color,
    blue: Color,
    magenta: Color,
    cyan: Color,
    green: Color,
    yellow: Color,
    red: Color,
}

impl Palette {
    fn new(
        base: [u8; 3],
        surface: [u8; 3],
        border: [u8; 3],
        text: [u8; 3],
        subtle: [u8; 3],
        muted: [u8; 3],
        blue: [u8; 3],
        magenta: [u8; 3],
        cyan: [u8; 3],
        green: [u8; 3],
        yellow: [u8; 3],
        red: [u8; 3],
    ) -> Self {
        Self {
            base: rgb(base),
            surface: rgb(surface),
            border: rgb(border),
            text: rgb(text),
            subtle: rgb(subtle),
            muted: rgb(muted),
            blue: rgb(blue),
            magenta: rgb(magenta),
            cyan: rgb(cyan),
            green: rgb(green),
            yellow: rgb(yellow),
            red: rgb(red),
        }
    }
}

const fn rgb(value: [u8; 3]) -> Color {
    Color::Rgb(value[0], value[1], value[2])
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

pub fn prefers_plain_icons() -> bool {
    env::var_os("TIRA_PLAIN_ICONS").is_some()
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
    fn built_in_theme_choices_include_popular_dark_themes() {
        let choices = Theme::default().choices();

        assert_eq!(choices.len(), 12);
        assert!(
            choices
                .iter()
                .any(|choice| choice.name == ThemeName::Dracula)
        );
        assert!(choices.iter().any(|choice| choice.name == ThemeName::Nord));
        assert!(
            choices
                .iter()
                .any(|choice| choice.name == ThemeName::GruvboxDark)
        );
        assert!(
            choices
                .iter()
                .any(|choice| choice.name == ThemeName::EverforestDark)
        );
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
