use stagelx_core::universe::{DmxBuffer, UniverseSet};
use crate::merge::MergeStrategy;

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
}

impl DmxEngine {
    pub fn add_source(&mut self, source: DmxSource) {
        self.sources.push(source);
        self.sources.sort_by_key(|s| s.priority);
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
        // Collect universe IDs across all sources
        let universe_ids: Vec<u16> = self.sources
            .iter()
            .flat_map(|s| s.universes.universes().map(|(id, _)| id))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for uid in universe_ids {
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
