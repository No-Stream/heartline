//! Guest need levels.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Needs {
    pub hunger: u16,
    pub thirst: u16,
    pub restroom: u16,
    pub nausea: u16,
}
