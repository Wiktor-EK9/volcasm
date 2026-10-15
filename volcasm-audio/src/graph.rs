//! Read-only snapshot of the PipeWire graph: clock settings, audio nodes with
//! their negotiated format and volumes. Used to verify that the SYRO stream
//! reaches the device without resampling or level changes.

//!
//! The types and their checks ([`crate::check`]) build on every platform;
//! reading the graph ([`snapshot`]) only on Linux.

use std::collections::HashMap;

#[cfg(target_os = "linux")]
mod read;
#[cfg(target_os = "linux")]
pub use read::snapshot;

/// Clock settings from the PipeWire `settings` metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClockSettings {
    /// `clock.rate`: default graph rate.
    pub rate: Option<u32>,
    /// `clock.allowed-rates`: rates the graph may switch to.
    pub allowed_rates: Vec<u32>,
    /// `clock.force-rate`: forced graph rate, 0 or absent when not forced.
    pub force_rate: Option<u32>,
}

/// Negotiated raw audio format of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawFormat {
    pub rate: u32,
    pub channels: u32,
}

/// Sample rates a node accepts, from one `EnumFormat` entry: `min..=max`
/// in steps of `step` (0 = any rate in the range).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateRange {
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

impl RateRange {
    pub fn fixed(rate: u32) -> Self {
        Self {
            min: rate,
            max: rate,
            step: 0,
        }
    }

    pub fn contains(&self, rate: u32) -> bool {
        (self.min..=self.max).contains(&rate)
            && (self.step == 0 || (rate - self.min).is_multiple_of(self.step))
    }
}

/// Volume state of a node (`Props` param).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Volumes {
    /// Total per-channel volume (hardware and software part).
    pub channel: Vec<f32>,
    /// Part of the volume applied in software, i.e. to the samples.
    pub soft: Vec<f32>,
    pub mute: bool,
    pub soft_mute: bool,
}

/// One audio node.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeState {
    pub id: u32,
    pub props: HashMap<String, String>,
    pub format: Option<RawFormat>,
    /// Rates from `EnumFormat`; empty when unknown.
    pub rates: Vec<RateRange>,
    pub volumes: Option<Volumes>,
}

impl NodeState {
    pub fn prop(&self, key: &str) -> Option<&str> {
        self.props.get(key).map(String::as_str)
    }

    /// Whether the node accepts `rate`; `None` when its formats are unknown.
    pub fn supports_rate(&self, rate: u32) -> Option<bool> {
        (!self.rates.is_empty()).then(|| self.rates.iter().any(|r| r.contains(rate)))
    }
}

/// Audio part of the PipeWire graph at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GraphSnapshot {
    pub clock: ClockSettings,
    pub nodes: Vec<NodeState>,
}

impl GraphSnapshot {
    pub fn node(&self, id: u32) -> Option<&NodeState> {
        self.nodes.iter().find(|node| node.id == id)
    }
}

/// Error while reading the PipeWire graph.
#[derive(Debug)]
pub struct GraphError(pub String);

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot read the PipeWire graph: {}", self.0)
    }
}

impl std::error::Error for GraphError {}
