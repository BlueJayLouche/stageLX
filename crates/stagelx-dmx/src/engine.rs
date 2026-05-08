use bevy::prelude::*;
use std::collections::HashSet;
use stagelx_core::universe::{DmxBuffer, UniverseSet};
use crate::merge::MergeStrategy;

/// Bevy Resource wrapper around the DMX engine.
#[derive(Resource)]
pub struct DmxEngineRes(pub DmxEngine);

impl Default for DmxEngineRes {
    fn default() -> Self {
        Self(DmxEngine::default())
    }
}

/// Priority-ordered DMX source.
#[derive(Debug)]
pub struct DmxSource {
    pub name: String,
    pub priority: u8,
    pub strategy: MergeStrategy,
    pub universes: UniverseSet,
}

/// Combines multiple DMX sources into a single output universe set.
///
/// Sources are processed in ascending priority order; higher priority wins per strategy.
#[derive(Debug, Default)]
pub struct DmxEngine {
    sources: Vec<DmxSource>,
    output: UniverseSet,
    /// Cached universe IDs — rebuilt only when sources change.
    cached_universe_ids: Vec<u16>,
    /// Scratch set for deduplication — reused to avoid per-tick allocation.
    id_scratch: HashSet<u16>,
    /// Set to true when a source is added or a universe is modified.
    dirty: bool,
}

impl DmxEngine {
    pub fn add_source(&mut self, source: DmxSource) {
        self.sources.push(source);
        self.sources.sort_by_key(|s| s.priority);
        self.dirty = true;
    }

    /// Return an existing source by name, or create it with the given priority/strategy.
    pub fn get_or_add_source(
        &mut self,
        name: &str,
        priority: u8,
        strategy: MergeStrategy,
    ) -> &mut DmxSource {
        if let Some(pos) = self.sources.iter().position(|s| s.name == name) {
            return &mut self.sources[pos];
        }
        self.add_source(DmxSource {
            name: name.to_string(),
            priority,
            strategy,
            universes: UniverseSet::default(),
        });
        let pos = self.sources.iter().position(|s| s.name == name).unwrap();
        &mut self.sources[pos]
    }

    /// Recompute the output universes from all sources.
    pub fn tick(&mut self) {
        // Rebuild cached universe IDs only when structurally dirty.
        if self.dirty {
            self.cached_universe_ids.clear();
            self.id_scratch.clear();
            for source in &self.sources {
                for (id, _) in source.universes.universes() {
                    if self.id_scratch.insert(id) {
                        self.cached_universe_ids.push(id);
                    }
                }
            }
            self.dirty = false;
        }

        for &uid in &self.cached_universe_ids {
            let out = self.output.get_or_insert(uid);
            out.clear();
            for source in &self.sources {
                if let Some(buf) = source.universes.get(uid) {
                    match source.strategy {
                        MergeStrategy::Htp => out.merge_htp(buf),
                        MergeStrategy::Ltp => out.merge_ltp(buf),
                    }
                }
            }
        }
    }

    pub fn output(&self) -> &UniverseSet {
        &self.output
    }

    pub fn output_buffer(&self, universe: u16) -> Option<&DmxBuffer> {
        self.output.get(universe)
    }
}
