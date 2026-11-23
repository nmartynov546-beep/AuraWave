#[derive(Clone, Copy)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn colorref(self) -> u32 {
        (self.r as u32) | ((self.g as u32) << 8) | ((self.b as u32) << 16)
    }
}

#[derive(Clone, Copy)]
pub struct ThemeDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub dark_title_bar: bool,
    pub toolbar_bg: Rgb,
    pub body_bg: Rgb,
    pub border_color: Rgb,
    pub text_primary: Rgb,
    pub text_muted: Rgb,
    pub accent_color: Rgb,
    pub button_hover: Rgb,
    pub button_down: Rgb,
}

pub const THEMES: &[ThemeDefinition] = &[
    ThemeDefinition {
        id: "aurawave",
        name: "AuraWave Midnight",
        dark_title_bar: true,
        toolbar_bg: Rgb::new(0x13, 0x13, 0x1C),
        body_bg: Rgb::new(0x0E, 0x0E, 0x14),
        border_color: Rgb::new(0x28, 0x28, 0x3A),
        text_primary: Rgb::new(0xF0, 0xF0, 0xF7),
        text_muted: Rgb::new(0x98, 0x98, 0xB0),
        accent_color: Rgb::new(168, 85, 247),
        button_hover: Rgb::new(0x1F, 0x1F, 0x2E),
        button_down: Rgb::new(0x2D, 0x2D, 0x42),
    },
    ThemeDefinition {
        id: "original",
        name: "Original Web (Dark Shell)",
        dark_title_bar: true,
        toolbar_bg: Rgb::new(24, 24, 24),
        body_bg: Rgb::new(18, 18, 18),
        border_color: Rgb::new(40, 40, 40),
        text_primary: Rgb::new(235, 235, 235),
        text_muted: Rgb::new(165, 165, 165),
        accent_color: Rgb::new(255, 85, 0),
        button_hover: Rgb::new(44, 44, 44),
        button_down: Rgb::new(62, 62, 62),
    },
    ThemeDefinition {
        id: "dark",
        name: "Dark Obsidian",
        dark_title_bar: true,
        toolbar_bg: Rgb::new(24, 24, 27),
        body_bg: Rgb::new(17, 17, 19),
        border_color: Rgb::new(45, 45, 50),
        text_primary: Rgb::new(236, 236, 239),
        text_muted: Rgb::new(154, 154, 162),
        accent_color: Rgb::new(255, 85, 0),
        button_hover: Rgb::new(44, 44, 52),
        button_down: Rgb::new(60, 60, 70),
    },
    ThemeDefinition {
        id: "amoled",
        name: "Midnight OLED",
        dark_title_bar: true,
        toolbar_bg: Rgb::new(10, 10, 10),
        body_bg: Rgb::new(0, 0, 0),
        border_color: Rgb::new(30, 30, 30),
        text_primary: Rgb::new(242, 242, 242),
        text_muted: Rgb::new(138, 138, 138),
        accent_color: Rgb::new(255, 85, 0),
        button_hover: Rgb::new(26, 26, 26),
        button_down: Rgb::new(42, 42, 42),
    },
    ThemeDefinition {
        id: "light",
        name: "Light Classic",
        dark_title_bar: false,
        toolbar_bg: Rgb::new(244, 244, 246),
        body_bg: Rgb::new(255, 255, 255),
        border_color: Rgb::new(220, 220, 224),
        text_primary: Rgb::new(26, 26, 29),
        text_muted: Rgb::new(108, 108, 116),
        accent_color: Rgb::new(255, 85, 0),
        button_hover: Rgb::new(228, 228, 234),
        button_down: Rgb::new(210, 210, 218),
    },
    ThemeDefinition {
        id: "cyber",
        name: "Synthwave Purple",
        dark_title_bar: true,
        toolbar_bg: Rgb::new(21, 16, 34),
        body_bg: Rgb::new(14, 10, 24),
        border_color: Rgb::new(45, 34, 70),
        text_primary: Rgb::new(240, 233, 255),
        text_muted: Rgb::new(157, 142, 192),
        accent_color: Rgb::new(192, 92, 255),
        button_hover: Rgb::new(38, 28, 61),
        button_down: Rgb::new(56, 40, 88),
    },
    ThemeDefinition {
        id: "ocean",
        name: "Deep Ocean",
        dark_title_bar: true,
        toolbar_bg: Rgb::new(14, 23, 34),
        body_bg: Rgb::new(9, 16, 24),
        border_color: Rgb::new(30, 48, 68),
        text_primary: Rgb::new(230, 241, 250),
        text_muted: Rgb::new(133, 161, 186),
        accent_color: Rgb::new(34, 184, 207),
        button_hover: Rgb::new(24, 39, 57),
        button_down: Rgb::new(36, 58, 84),
    },
];

pub fn get_theme(id: &str) -> &'static ThemeDefinition {
    let trimmed = id.trim();
    for theme in THEMES {
        if theme.id.eq_ignore_ascii_case(trimmed) {
            return theme;
        }
    }
    &THEMES[0]
}
