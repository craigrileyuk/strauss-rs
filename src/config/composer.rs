//! The shapes shared with Composer: `autoload` objects, and the parts of `composer.json` used to
//! infer settings.

use std::fmt;
use std::marker::PhantomData;

use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::Value;

/// An `autoload` object, as in `composer.json`.
#[derive(Debug, Default, Deserialize)]
pub struct Autoload {
    #[serde(rename = "psr-4", default)]
    pub psr4: Ordered<StringOrList>,
    #[serde(rename = "psr-0", default)]
    pub psr0: Ordered<StringOrList>,
    #[serde(default)]
    pub classmap: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
}

impl Autoload {
    /// Every path the autoload section refers to.
    pub(super) fn paths(&self) -> impl Iterator<Item = &str> {
        let mapped = self
            .psr4
            .0
            .iter()
            .chain(self.psr0.0.iter())
            .flat_map(|(_, paths)| paths.iter());

        mapped
            .chain(self.classmap.iter().map(String::as_str))
            .chain(self.files.iter().map(String::as_str))
    }
}

/// A JSON string or an array of strings, as Composer allows for autoload
/// paths.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StringOrList {
    One(String),
    Many(Vec<String>),
}

impl StringOrList {
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        let (one, many) = match self {
            StringOrList::One(path) => (Some(path.as_str()), &[][..]),
            StringOrList::Many(paths) => (None, paths.as_slice()),
        };

        one.into_iter().chain(many.iter().map(String::as_str))
    }
}

/// A JSON object whose key order is kept. Order matters for
/// `namespace_replacement_patterns` (priority) and for Composer's `psr-4`
/// (Strauss infers the prefix from the first key).
#[derive(Debug)]
pub struct Ordered<V>(pub Vec<(String, V)>);

impl<V> Default for Ordered<V> {
    fn default() -> Self {
        Ordered(Vec::new())
    }
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for Ordered<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor<V>(PhantomData<V>);

        impl<'de, V: Deserialize<'de>> Visitor<'de> for OrderedVisitor<V> {
            type Value = Ordered<V>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();

                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }

                Ok(Ordered(entries))
            }
        }

        deserializer.deserialize_map(OrderedVisitor(PhantomData))
    }
}

/// The parts of `composer.json` used to infer settings.
#[derive(Debug, Default, Deserialize)]
pub(super) struct ComposerJson {
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) autoload: Autoload,
    #[serde(default)]
    pub(super) require: Ordered<Value>,
    #[serde(default)]
    pub(super) config: ComposerConfig,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ComposerConfig {
    #[serde(rename = "vendor-dir")]
    pub(super) vendor_dir: Option<String>,
}
