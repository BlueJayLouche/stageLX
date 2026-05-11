use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use crate::fixture::FixtureInstance;
use crate::types::FixtureId;

/// Maps fixture IDs to their patched instances in insertion order.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Patch {
    fixtures: IndexMap<FixtureId, FixtureInstance>,
    next_id: u32,
}

impl Patch {
    pub fn add(&mut self, mut fixture: FixtureInstance) -> FixtureId {
        let id = FixtureId(self.next_id);
        self.next_id += 1;
        fixture.id = id;
        self.fixtures.insert(id, fixture);
        id
    }

    pub fn remove(&mut self, id: FixtureId) -> Option<FixtureInstance> {
        self.fixtures.shift_remove(&id)
    }

    pub fn get(&self, id: FixtureId) -> Option<&FixtureInstance> {
        self.fixtures.get(&id)
    }

    pub fn get_mut(&mut self, id: FixtureId) -> Option<&mut FixtureInstance> {
        self.fixtures.get_mut(&id)
    }

    pub fn fixtures(&self) -> impl Iterator<Item = &FixtureInstance> {
        self.fixtures.values()
    }

    pub fn len(&self) -> usize {
        self.fixtures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fixtures.is_empty()
    }
}
