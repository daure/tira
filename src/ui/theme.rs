use std::{collections::BTreeMap, env, fs, path::PathBuf, str::FromStr};

use ratatui::style::Color;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Amoled,
    Aura,
    Ayu,
    Carbonfox,
    Catppuccin,
    CatppuccinFrappe,
    CatppuccinMacchiato,
    Cobalt2,
    Cursor,
    Dracula,
    Everforest,
    Flexoki,
    Github,
    Gruvbox,
    Kanagawa,
    LucentOrng,
    Material,
    Matrix,
    Mercury,
    Monokai,
    NightOwl,
    Nord,
    Oc2,
    OneDark,
    Onedarkpro,
    Opencode,
    Orng,
    OsakaJade,
    Palenight,
    RosePine,
    ShadesOfPurple,
    Solarized,
    Synthwave84,
    TokyoNight,
    Vercel,
    Vesper,
    Zenburn,
}

impl ThemeName {
    pub const ALL: [Self; 37] = [
        Self::Amoled,
        Self::Aura,
        Self::Ayu,
        Self::Carbonfox,
        Self::Catppuccin,
        Self::CatppuccinFrappe,
        Self::CatppuccinMacchiato,
        Self::Cobalt2,
        Self::Cursor,
        Self::Dracula,
        Self::Everforest,
        Self::Flexoki,
        Self::Github,
        Self::Gruvbox,
        Self::Kanagawa,
        Self::LucentOrng,
        Self::Material,
        Self::Matrix,
        Self::Mercury,
        Self::Monokai,
        Self::NightOwl,
        Self::Nord,
        Self::Oc2,
        Self::OneDark,
        Self::Onedarkpro,
        Self::Opencode,
        Self::Orng,
        Self::OsakaJade,
        Self::Palenight,
        Self::RosePine,
        Self::ShadesOfPurple,
        Self::Solarized,
        Self::Synthwave84,
        Self::TokyoNight,
        Self::Vercel,
        Self::Vesper,
        Self::Zenburn,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Amoled => "amoled",
            Self::Aura => "aura",
            Self::Ayu => "ayu",
            Self::Carbonfox => "carbonfox",
            Self::Catppuccin => "catppuccin",
            Self::CatppuccinFrappe => "catppuccin_frappe",
            Self::CatppuccinMacchiato => "catppuccin_macchiato",
            Self::Cobalt2 => "cobalt2",
            Self::Cursor => "cursor",
            Self::Dracula => "dracula",
            Self::Everforest => "everforest",
            Self::Flexoki => "flexoki",
            Self::Github => "github",
            Self::Gruvbox => "gruvbox",
            Self::Kanagawa => "kanagawa",
            Self::LucentOrng => "lucent_orng",
            Self::Material => "material",
            Self::Matrix => "matrix",
            Self::Mercury => "mercury",
            Self::Monokai => "monokai",
            Self::NightOwl => "nightowl",
            Self::Nord => "nord",
            Self::Oc2 => "oc_2",
            Self::OneDark => "one_dark",
            Self::Onedarkpro => "onedarkpro",
            Self::Opencode => "opencode",
            Self::Orng => "orng",
            Self::OsakaJade => "osaka_jade",
            Self::Palenight => "palenight",
            Self::RosePine => "rosepine",
            Self::ShadesOfPurple => "shadesofpurple",
            Self::Solarized => "solarized",
            Self::Synthwave84 => "synthwave84",
            Self::TokyoNight => "tokyonight",
            Self::Vercel => "vercel",
            Self::Vesper => "vesper",
            Self::Zenburn => "zenburn",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Amoled => "Amoled",
            Self::Aura => "Aura",
            Self::Ayu => "Ayu",
            Self::Carbonfox => "Carbonfox",
            Self::Catppuccin => "Catppuccin",
            Self::CatppuccinFrappe => "Catppuccin Frappé",
            Self::CatppuccinMacchiato => "Catppuccin Macchiato",
            Self::Cobalt2 => "Cobalt2",
            Self::Cursor => "Cursor",
            Self::Dracula => "Dracula",
            Self::Everforest => "Everforest",
            Self::Flexoki => "Flexoki",
            Self::Github => "GitHub",
            Self::Gruvbox => "Gruvbox",
            Self::Kanagawa => "Kanagawa",
            Self::LucentOrng => "Lucent Orng",
            Self::Material => "Material",
            Self::Matrix => "Matrix",
            Self::Mercury => "Mercury",
            Self::Monokai => "Monokai",
            Self::NightOwl => "Night Owl",
            Self::Nord => "Nord",
            Self::Oc2 => "OC-2",
            Self::OneDark => "One Dark",
            Self::Onedarkpro => "OneDark Pro",
            Self::Opencode => "Opencode",
            Self::Orng => "Orng",
            Self::OsakaJade => "Osaka Jade",
            Self::Palenight => "Palenight",
            Self::RosePine => "Rosé Pine",
            Self::ShadesOfPurple => "Shades of Purple",
            Self::Solarized => "Solarized",
            Self::Synthwave84 => "Synthwave '84",
            Self::TokyoNight => "Tokyo Night",
            Self::Vercel => "Vercel",
            Self::Vesper => "Vesper",
            Self::Zenburn => "Zenburn",
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
            "amoled" => Ok(Self::Amoled),
            "aura" => Ok(Self::Aura),
            "ayu" | "ayu_dark" => Ok(Self::Ayu),
            "carbonfox" => Ok(Self::Carbonfox),
            "catppuccin" | "catppuccin_mocha" | "catpuccin" => Ok(Self::Catppuccin),
            "catppuccin_frappe" => Ok(Self::CatppuccinFrappe),
            "catppuccin_macchiato" => Ok(Self::CatppuccinMacchiato),
            "cobalt2" => Ok(Self::Cobalt2),
            "cursor" => Ok(Self::Cursor),
            "dracula" => Ok(Self::Dracula),
            "everforest" | "everforest_dark" => Ok(Self::Everforest),
            "flexoki" => Ok(Self::Flexoki),
            "github" => Ok(Self::Github),
            "gruvbox" | "gruvbox_dark" => Ok(Self::Gruvbox),
            "kanagawa" => Ok(Self::Kanagawa),
            "lucent_orng" => Ok(Self::LucentOrng),
            "material" => Ok(Self::Material),
            "matrix" => Ok(Self::Matrix),
            "mercury" => Ok(Self::Mercury),
            "monokai" => Ok(Self::Monokai),
            "nightowl" | "night_owl" => Ok(Self::NightOwl),
            "nord" => Ok(Self::Nord),
            "oc_2" | "oc2" => Ok(Self::Oc2),
            "one_dark" | "onedark" => Ok(Self::OneDark),
            "onedarkpro" | "one_dark_pro" => Ok(Self::Onedarkpro),
            "opencode" => Ok(Self::Opencode),
            "orng" => Ok(Self::Orng),
            "osaka_jade" => Ok(Self::OsakaJade),
            "palenight" | "pale_night" => Ok(Self::Palenight),
            "rose_pine" | "rosepine" => Ok(Self::RosePine),
            "shadesofpurple" | "shades_of_purple" => Ok(Self::ShadesOfPurple),
            "solarized" | "solarized_dark" => Ok(Self::Solarized),
            "synthwave84" | "synthwave_84" => Ok(Self::Synthwave84),
            "tokyo_night" | "tokyonight" | "tira_dark" => Ok(Self::TokyoNight),
            "vercel" => Ok(Self::Vercel),
            "vesper" => Ok(Self::Vesper),
            "zenburn" => Ok(Self::Zenburn),
            other => Err(ThemeError(format!("Unknown theme `{other}`"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeChoice {
    pub name: ThemeName,
}

impl crate::ui::selector::HasShortcut for ThemeChoice {
    fn shortcut(&self, _keybindings: &crate::KeyBindings) -> Option<String> {
        None
    }
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
    warning_fg: Color,
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
            ThemeName::Amoled => Self::amoled(),
            ThemeName::Aura => Self::aura(),
            ThemeName::Ayu => Self::ayu_dark(),
            ThemeName::Carbonfox => Self::carbonfox(),
            ThemeName::Catppuccin => Self::catppuccin(),
            ThemeName::CatppuccinFrappe => Self::catppuccin_frappe(),
            ThemeName::CatppuccinMacchiato => Self::catppuccin_macchiato(),
            ThemeName::Cobalt2 => Self::cobalt2(),
            ThemeName::Cursor => Self::cursor(),
            ThemeName::Dracula => Self::dracula(),
            ThemeName::Everforest => Self::everforest_dark(),
            ThemeName::Flexoki => Self::flexoki(),
            ThemeName::Github => Self::github(),
            ThemeName::Gruvbox => Self::gruvbox_dark(),
            ThemeName::Kanagawa => Self::kanagawa(),
            ThemeName::LucentOrng => Self::lucent_orng(),
            ThemeName::Material => Self::material(),
            ThemeName::Matrix => Self::matrix(),
            ThemeName::Mercury => Self::mercury(),
            ThemeName::Monokai => Self::monokai(),
            ThemeName::NightOwl => Self::night_owl(),
            ThemeName::Nord => Self::nord(),
            ThemeName::Oc2 => Self::oc_2(),
            ThemeName::OneDark => Self::one_dark(),
            ThemeName::Onedarkpro => Self::onedarkpro(),
            ThemeName::Opencode => Self::opencode(),
            ThemeName::Orng => Self::orng(),
            ThemeName::OsakaJade => Self::osaka_jade(),
            ThemeName::Palenight => Self::palenight(),
            ThemeName::RosePine => Self::rose_pine(),
            ThemeName::ShadesOfPurple => Self::shades_of_purple(),
            ThemeName::Solarized => Self::solarized_dark(),
            ThemeName::Synthwave84 => Self::synthwave84(),
            ThemeName::TokyoNight => Self::tokyo_night(),
            ThemeName::Vercel => Self::vercel(),
            ThemeName::Vesper => Self::vesper(),
            ThemeName::Zenburn => Self::zenburn(),
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
    pub fn warning_fg(&self) -> Color {
        self.warning_fg
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
            warning_fg: palette.yellow,
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
            ThemeName::Gruvbox,
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
            ThemeName::Solarized,
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
            ThemeName::Ayu,
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

    fn amoled() -> Self {
        Self::from_palette(
            ThemeName::Amoled,
            Palette::new(
                [0, 0, 0],
                [12, 12, 12],
                [32, 32, 32],
                [242, 242, 242],
                [196, 196, 196],
                [128, 128, 128],
                [86, 156, 214],
                [198, 120, 221],
                [78, 201, 176],
                [152, 195, 121],
                [229, 192, 123],
                [224, 108, 117],
            ),
        )
    }

    fn aura() -> Self {
        Self::from_palette(
            ThemeName::Aura,
            Palette::new(
                [21, 18, 27],
                [36, 31, 49],
                [69, 58, 94],
                [237, 233, 254],
                [178, 165, 209],
                [122, 109, 156],
                [130, 170, 255],
                [199, 146, 234],
                [132, 235, 209],
                [167, 233, 175],
                [255, 203, 107],
                [255, 103, 149],
            ),
        )
    }

    fn carbonfox() -> Self {
        Self::from_palette(
            ThemeName::Carbonfox,
            Palette::new(
                [22, 25, 30],
                [42, 45, 53],
                [82, 88, 100],
                [242, 244, 248],
                [196, 203, 211],
                [109, 114, 124],
                [120, 169, 255],
                [190, 149, 255],
                [63, 203, 212],
                [66, 200, 142],
                [190, 156, 63],
                [255, 125, 125],
            ),
        )
    }

    fn catppuccin_frappe() -> Self {
        Self::from_palette(
            ThemeName::CatppuccinFrappe,
            Palette::new(
                [48, 52, 70],
                [65, 69, 89],
                [115, 121, 148],
                [198, 208, 245],
                [181, 191, 226],
                [140, 145, 172],
                [140, 170, 238],
                [244, 184, 228],
                [153, 209, 219],
                [166, 209, 137],
                [229, 200, 144],
                [231, 130, 132],
            ),
        )
    }

    fn catppuccin_macchiato() -> Self {
        Self::from_palette(
            ThemeName::CatppuccinMacchiato,
            Palette::new(
                [36, 39, 58],
                [54, 58, 79],
                [110, 115, 141],
                [202, 211, 245],
                [184, 192, 224],
                [128, 135, 162],
                [138, 173, 244],
                [245, 189, 230],
                [145, 215, 227],
                [166, 218, 149],
                [238, 212, 159],
                [237, 135, 150],
            ),
        )
    }

    fn cobalt2() -> Self {
        Self::from_palette(
            ThemeName::Cobalt2,
            Palette::new(
                [25, 36, 76],
                [31, 44, 92],
                [63, 83, 161],
                [255, 255, 255],
                [187, 205, 255],
                [124, 150, 222],
                [0, 194, 255],
                [255, 159, 218],
                [96, 218, 251],
                [61, 214, 140],
                [255, 214, 10],
                [255, 98, 140],
            ),
        )
    }

    fn cursor() -> Self {
        Self::from_palette(
            ThemeName::Cursor,
            Palette::new(
                [27, 31, 39],
                [41, 47, 58],
                [73, 83, 100],
                [230, 236, 241],
                [182, 191, 202],
                [122, 132, 145],
                [87, 164, 255],
                [189, 147, 249],
                [106, 227, 255],
                [110, 203, 132],
                [244, 191, 117],
                [242, 112, 122],
            ),
        )
    }

    fn flexoki() -> Self {
        Self::from_palette(
            ThemeName::Flexoki,
            Palette::new(
                [16, 15, 15],
                [28, 27, 26],
                [64, 62, 60],
                [206, 205, 195],
                [185, 173, 146],
                [135, 124, 99],
                [67, 133, 190],
                [177, 98, 134],
                [58, 169, 159],
                [102, 128, 11],
                [173, 131, 1],
                [209, 77, 65],
            ),
        )
    }

    fn github() -> Self {
        Self::from_palette(
            ThemeName::Github,
            Palette::new(
                [13, 17, 23],
                [22, 27, 34],
                [48, 54, 61],
                [230, 237, 243],
                [139, 148, 158],
                [110, 118, 129],
                [121, 192, 255],
                [214, 130, 250],
                [57, 211, 83],
                [63, 185, 80],
                [210, 153, 34],
                [248, 81, 73],
            ),
        )
    }

    fn lucent_orng() -> Self {
        Self::from_palette(
            ThemeName::LucentOrng,
            Palette::new(
                [24, 21, 18],
                [38, 32, 27],
                [83, 68, 55],
                [247, 240, 231],
                [219, 197, 173],
                [157, 132, 108],
                [94, 163, 255],
                [236, 160, 88],
                [88, 205, 176],
                [140, 201, 118],
                [255, 176, 84],
                [255, 110, 85],
            ),
        )
    }

    fn material() -> Self {
        Self::from_palette(
            ThemeName::Material,
            Palette::new(
                [38, 50, 56],
                [55, 71, 79],
                [84, 110, 122],
                [238, 255, 255],
                [176, 190, 197],
                [120, 144, 156],
                [130, 170, 255],
                [199, 146, 234],
                [137, 221, 255],
                [195, 232, 141],
                [255, 203, 107],
                [240, 113, 120],
            ),
        )
    }

    fn matrix() -> Self {
        Self::from_palette(
            ThemeName::Matrix,
            Palette::new(
                [4, 12, 4],
                [9, 25, 9],
                [23, 58, 23],
                [166, 255, 166],
                [108, 201, 108],
                [63, 140, 63],
                [61, 214, 140],
                [126, 255, 126],
                [61, 214, 140],
                [89, 255, 89],
                [178, 255, 89],
                [255, 89, 89],
            ),
        )
    }

    fn mercury() -> Self {
        Self::from_palette(
            ThemeName::Mercury,
            Palette::new(
                [26, 29, 33],
                [39, 43, 48],
                [79, 86, 94],
                [233, 238, 242],
                [195, 203, 211],
                [132, 142, 151],
                [110, 163, 255],
                [210, 160, 255],
                [98, 212, 208],
                [120, 208, 146],
                [255, 199, 99],
                [255, 119, 127],
            ),
        )
    }

    fn night_owl() -> Self {
        Self::from_palette(
            ThemeName::NightOwl,
            Palette::new(
                [1, 22, 39],
                [10, 34, 57],
                [18, 54, 86],
                [214, 222, 235],
                [127, 219, 202],
                [99, 119, 119],
                [130, 170, 255],
                [199, 146, 234],
                [127, 219, 202],
                [173, 219, 103],
                [250, 208, 0],
                [239, 83, 80],
            ),
        )
    }

    fn oc_2() -> Self {
        Self::from_palette(
            ThemeName::Oc2,
            Palette::new(
                [20, 22, 26],
                [32, 35, 42],
                [70, 76, 89],
                [235, 236, 240],
                [187, 192, 199],
                [124, 131, 143],
                [97, 175, 239],
                [198, 120, 221],
                [86, 182, 194],
                [152, 195, 121],
                [229, 192, 123],
                [224, 108, 117],
            ),
        )
    }

    fn onedarkpro() -> Self {
        Self::from_palette(
            ThemeName::Onedarkpro,
            Palette::new(
                [34, 37, 44],
                [43, 47, 58],
                [79, 86, 103],
                [213, 218, 227],
                [171, 178, 191],
                [101, 109, 126],
                [97, 175, 239],
                [198, 120, 221],
                [86, 182, 194],
                [152, 195, 121],
                [229, 192, 123],
                [224, 108, 117],
            ),
        )
    }

    fn opencode() -> Self {
        Self::from_palette(
            ThemeName::Opencode,
            Palette::new(
                [17, 20, 26],
                [28, 33, 42],
                [61, 70, 87],
                [230, 236, 245],
                [182, 190, 202],
                [120, 130, 147],
                [102, 163, 255],
                [191, 149, 255],
                [79, 209, 197],
                [126, 211, 140],
                [255, 195, 102],
                [255, 107, 107],
            ),
        )
    }

    fn orng() -> Self {
        Self::from_palette(
            ThemeName::Orng,
            Palette::new(
                [25, 22, 19],
                [39, 33, 29],
                [82, 67, 59],
                [244, 236, 229],
                [221, 188, 161],
                [160, 129, 108],
                [92, 159, 255],
                [255, 147, 91],
                [99, 205, 177],
                [153, 205, 102],
                [255, 183, 77],
                [255, 101, 84],
            ),
        )
    }

    fn osaka_jade() -> Self {
        Self::from_palette(
            ThemeName::OsakaJade,
            Palette::new(
                [22, 29, 27],
                [33, 43, 40],
                [63, 82, 77],
                [223, 235, 231],
                [177, 204, 195],
                [116, 151, 141],
                [108, 164, 255],
                [162, 135, 230],
                [93, 202, 182],
                [133, 201, 129],
                [226, 190, 109],
                [226, 124, 111],
            ),
        )
    }

    fn palenight() -> Self {
        Self::from_palette(
            ThemeName::Palenight,
            Palette::new(
                [41, 45, 62],
                [54, 58, 79],
                [103, 114, 229],
                [166, 172, 205],
                [149, 157, 203],
                [103, 114, 149],
                [130, 170, 255],
                [199, 146, 234],
                [137, 221, 255],
                [195, 232, 141],
                [255, 203, 107],
                [240, 113, 120],
            ),
        )
    }

    fn shades_of_purple() -> Self {
        Self::from_palette(
            ThemeName::ShadesOfPurple,
            Palette::new(
                [43, 18, 68],
                [62, 32, 93],
                [104, 74, 137],
                [255, 255, 255],
                [199, 187, 255],
                [149, 131, 214],
                [130, 170, 255],
                [255, 121, 198],
                [94, 236, 255],
                [173, 255, 47],
                [255, 183, 77],
                [255, 99, 132],
            ),
        )
    }

    fn synthwave84() -> Self {
        Self::from_palette(
            ThemeName::Synthwave84,
            Palette::new(
                [38, 24, 67],
                [53, 33, 92],
                [107, 74, 145],
                [255, 255, 255],
                [241, 223, 255],
                [170, 123, 255],
                [54, 247, 255],
                [255, 124, 237],
                [54, 247, 255],
                [114, 255, 184],
                [255, 209, 102],
                [255, 111, 145],
            ),
        )
    }

    fn vercel() -> Self {
        Self::from_palette(
            ThemeName::Vercel,
            Palette::new(
                [0, 0, 0],
                [20, 20, 20],
                [44, 44, 44],
                [255, 255, 255],
                [170, 170, 170],
                [112, 112, 112],
                [0, 112, 243],
                [121, 40, 202],
                [0, 166, 255],
                [0, 204, 136],
                [247, 181, 0],
                [255, 0, 80],
            ),
        )
    }

    fn vesper() -> Self {
        Self::from_palette(
            ThemeName::Vesper,
            Palette::new(
                [16, 18, 24],
                [27, 30, 38],
                [54, 60, 74],
                [245, 245, 245],
                [185, 188, 193],
                [116, 122, 136],
                [91, 157, 255],
                [197, 149, 255],
                [95, 205, 228],
                [110, 204, 136],
                [255, 190, 98],
                [255, 110, 114],
            ),
        )
    }

    fn zenburn() -> Self {
        Self::from_palette(
            ThemeName::Zenburn,
            Palette::new(
                [63, 63, 63],
                [76, 76, 76],
                [98, 98, 98],
                [220, 220, 204],
                [181, 189, 104],
                [127, 159, 127],
                [140, 208, 211],
                [220, 140, 195],
                [147, 224, 227],
                [95, 126, 93],
                [240, 223, 175],
                [204, 147, 147],
            ),
        )
    }
    fn everforest_dark() -> Self {
        Self::from_palette(
            ThemeName::Everforest,
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
            "warning_fg" => self.warning_fg = color,
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

        assert_eq!(choices.len(), 37);
        assert!(
            choices
                .iter()
                .any(|choice| choice.name == ThemeName::Dracula)
        );
        assert!(choices.iter().any(|choice| choice.name == ThemeName::Nord));
        assert!(
            choices
                .iter()
                .any(|choice| choice.name == ThemeName::Gruvbox)
        );
        assert!(
            choices
                .iter()
                .any(|choice| choice.name == ThemeName::Everforest)
        );
        assert!(
            choices
                .iter()
                .any(|choice| choice.name == ThemeName::CatppuccinMacchiato)
        );
        assert!(
            choices
                .iter()
                .any(|choice| choice.name == ThemeName::Synthwave84)
        );
        assert!(
            choices
                .iter()
                .any(|choice| choice.name == ThemeName::Vesper)
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
