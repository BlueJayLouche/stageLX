pub mod error;
pub mod gdtf;
pub mod mvr;
pub mod mvr_export;

pub use gdtf::{parse_gdtf, GdtfFixtureType};
pub use mvr::{parse_mvr, MvrScene, MvrSceneObject, MvrTruss, MvrGeometry3D};
pub use mvr_export::export_mvr;

use std::collections::HashMap;

/// Runtime library of loaded GDTF fixture types, keyed by FixtureTypeID.
#[derive(Default)]
pub struct FixtureLibrary {
    fixtures: HashMap<String, GdtfFixtureType>,
    /// Original GDTF ZIP bytes for each fixture type, keyed by FixtureTypeID.
    /// Kept so that MVR export can re-embed the fixture definitions.
    gdtf_raw_bytes: HashMap<String, Vec<u8>>,
}

impl FixtureLibrary {
    /// Parse and register a GDTF file from raw ZIP bytes. Returns the FixtureTypeID.
    pub fn load(&mut self, data: &[u8]) -> Result<String, error::GdtfError> {
        let fixture = parse_gdtf(data)?;
        let id = fixture.fixture_type_id.clone();
        self.fixtures.insert(id.clone(), fixture);
        self.gdtf_raw_bytes.insert(id.clone(), data.to_vec());
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

    /// Retrieve the original GDTF ZIP bytes for a given fixture type ID.
    pub fn raw_bytes(&self, id: &str) -> Option<&[u8]> {
        self.gdtf_raw_bytes.get(id).map(|v| v.as_slice())
    }
}
