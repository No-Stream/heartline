//! Guest attributes and generation distributions.
//!
//! | Trait | Distribution and correlation |
//! |---|---|
//! | Intensity taste | Integer 0–100, uniform for adults (mean 50); children are shifted down by 8 and clipped (mean about 42.3). |
//! | Nausea tolerance | Integer 0–100, uniform for adults (mean 50); children are shifted up by 10 and clipped (mean about 59.5). |
//! | Favourite ride types | One or two distinct types from keys 0–5; children choose gentle types 0–1 70% of the time. |
//! | Theming care | Integer 0–100, uniform. |
//! | Preferred theme | Uniform among the available theme keys. |
//! | Spending | Base adult rates are budget 35%, regular 45%, premium 20%; couple rolls shift up by 5 and family rolls down by 5, while children are budget. |
//! | Height | Children 90–139 cm, adults 145–204 cm. |
//! | Age | Child, adult, or senior; family groups contain one or two adults and one to three children. |
//! | Group kind | Single 20%, couple 25%, family 35%, and friends 20% of generated groups. |
//! | Walking speed | Base integer 70–130; children are shifted down by 6 and seniors by 12, with endpoints clamped. |
//! | Appearance | Uniform skin keys 0–7, hair keys 0–11, clothing keys 0–15, and body keys 0–4. |
//!
//! Values are derived from the seed and stable guest ID, so population creation is
//! reproducible and independent of executor partitioning or thread count.
//!
//! Each eight-entry memory ring occupies 136 bytes. The 2 MB budget for 10,000
//! guests allows 200 bytes per guest, leaving 64 bytes per ring for growth or padding.

pub const RIDE_TYPE_COUNT: u8 = 6;
pub const THEME_COUNT: u8 = 2;
pub const SKIN_TONE_COUNT: u8 = 8;
pub const HAIR_STYLE_COUNT: u8 = 12;
pub const CLOTHING_STYLE_COUNT: u8 = 16;
pub const BODY_SHAPE_COUNT: u8 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AgeGroup {
    Child,
    Adult,
    Senior,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SpendingLevel {
    Budget,
    Regular,
    Premium,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GroupKind {
    Single,
    Couple,
    Family,
    Friends,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GuestId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Appearance {
    pub skin_tone: u8,
    pub hair_style: u8,
    pub clothing_style: u8,
    pub body_shape: u8,
}
