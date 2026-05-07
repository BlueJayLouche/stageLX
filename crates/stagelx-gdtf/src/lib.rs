pub mod error;
pub mod gdtf;
pub mod mvr;

pub use gdtf::{parse_gdtf, GdtfFixtureType};
pub use mvr::{parse_mvr, MvrScene};

use std::collections::HashMap;

/// Runtime library of loaded GDTF fixture types, keyed by FixtureTypeID.
#[derive(Default)]
pub struct FixtureLibrary {
    fixtures: HashMap<String, GdtfFixtureType>,
}

impl FixtureLibrary {
    /// Parse and register a GDTF file from raw ZIP bytes. Returns the FixtureTypeID.
    pub fn load(&mut self, data: &[u8]) -> Result<String, error::GdtfError> {
        let fixture = parse_gdtf(data)?;
        let id = fixture.fixture_type_id.clone();
        self.fixtures.insert(id.clone(), fixture);
        Ok(id)
    }

    pub fn get(&self, id: &str) -> Option<&GdtfFixtureType> {
        self.fixtures.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &GdtfFixtureType> {
        self.fixtures.values()
    }

    pub fn len(&self) -> usize {
        self.fixtures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fixtures.is_empty()
    }
}
