//! Versioned snapshots of the live simulation.

pub(crate) type CatalogText = &'static str;

pub(crate) mod catalog_text {
    use serde::{Deserialize, Deserializer, Serializer, de::Error, ser::Error as _};

    fn known(value: &str) -> Option<&'static str> {
        crate::track::catalog()
            .iter()
            .map(|piece| piece.name)
            .find(|name| *name == value)
            .or_else(|| {
                crate::track::coaster_models()
                    .into_iter()
                    .flat_map(|model| [model.manufacturer, model.name, model.description])
                    .find(|name| *name == value)
            })
    }

    pub fn serialize<S: Serializer>(
        value: &&'static str,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if known(value).is_none() {
            return Err(S::Error::custom("unknown catalog text"));
        }
        serializer.serialize_str(value)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<&'static str, D::Error> {
        let value = String::deserialize(deserializer)?;
        known(&value).ok_or_else(|| D::Error::custom("unknown catalog text"))
    }
}
