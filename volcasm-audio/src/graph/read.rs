//! Reading the PipeWire graph into a [`GraphSnapshot`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use pipewire as pw;
use pw::context::ContextRc;
use pw::core::PW_ID_CORE;
use pw::main_loop::MainLoopRc;
use pw::metadata::Metadata;
use pw::node::Node;
use pw::spa;
use pw::types::ObjectType;
use spa::param::ParamType;
use spa::param::audio::AudioInfoRaw;
use spa::param::format::{MediaSubtype, MediaType};
use spa::param::format_utils::parse_format;
use spa::pod::deserialize::PodDeserializer;
use spa::pod::{ChoiceValue, Pod, Value, ValueArray};
use spa::utils::{Choice, ChoiceEnum};

use super::{ClockSettings, GraphError, GraphSnapshot, NodeState, RateRange, RawFormat, Volumes};

impl From<pw::Error> for GraphError {
    fn from(err: pw::Error) -> Self {
        Self(err.to_string())
    }
}

#[derive(Default)]
struct Collected {
    clock: ClockSettings,
    nodes: HashMap<u32, NodeState>,
}

/// Takes a snapshot of the PipeWire graph. Blocks until PipeWire has answered
/// all requests (a few round trips).
pub fn snapshot() -> Result<GraphSnapshot, GraphError> {
    pw::init();
    let mainloop = MainLoopRc::new(None)?;
    let context = ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;
    let registry = core.get_registry_rc()?;

    let collected = Rc::new(RefCell::new(Collected::default()));
    // Bound proxies and their listeners must live until the loop ends.
    let keep_alive: Rc<RefCell<Vec<Box<dyn std::any::Any>>>> = Rc::default();
    // Outstanding `core.sync` requests; the loop quits when all are done.
    let pending = Rc::new(RefCell::new(vec![core.sync(0)?]));

    let _core_listener = core
        .add_listener_local()
        .done({
            let pending = pending.clone();
            let mainloop = mainloop.clone();
            move |id, seq| {
                if id != PW_ID_CORE {
                    return;
                }
                let mut pending = pending.borrow_mut();
                pending.retain(|s| *s != seq);
                if pending.is_empty() {
                    mainloop.quit();
                }
            }
        })
        .register();

    let _registry_listener = registry
        .add_listener_local()
        .global({
            let registry = registry.clone();
            let core = core.clone();
            let collected = collected.clone();
            let keep_alive = keep_alive.clone();
            let pending = pending.clone();
            move |global| {
                let props = global.props;
                match global.type_ {
                    ObjectType::Metadata => {
                        if props.and_then(|p| p.get("metadata.name")) != Some("settings") {
                            return;
                        }
                        let Ok(metadata) = registry.bind::<Metadata, _>(global) else {
                            return;
                        };
                        let collected = collected.clone();
                        let listener = metadata
                            .add_listener_local()
                            .property(move |_, key, _, value| {
                                if let (Some(key), Some(value)) = (key, value) {
                                    apply_clock_setting(
                                        &mut collected.borrow_mut().clock,
                                        key,
                                        value,
                                    );
                                }
                                0
                            })
                            .register();
                        keep_alive.borrow_mut().push(Box::new((metadata, listener)));
                    }
                    ObjectType::Node => {
                        let is_audio = props
                            .and_then(|p| p.get("media.class"))
                            .is_some_and(|class| class.contains("Audio"));
                        if !is_audio {
                            return;
                        }
                        let Ok(node) = registry.bind::<Node, _>(global) else {
                            return;
                        };
                        let id = global.id;
                        collected.borrow_mut().nodes.insert(
                            id,
                            NodeState {
                                id,
                                ..Default::default()
                            },
                        );
                        let listener = node
                            .add_listener_local()
                            .info({
                                let collected = collected.clone();
                                move |info| {
                                    let Some(props) = info.props() else {
                                        return;
                                    };
                                    if let Some(node) = collected.borrow_mut().nodes.get_mut(&id) {
                                        node.props = props
                                            .iter()
                                            .map(|(k, v)| (k.to_owned(), v.to_owned()))
                                            .collect();
                                    }
                                }
                            })
                            .param({
                                let collected = collected.clone();
                                move |_, param_type, _, _, pod| {
                                    let Some(pod) = pod else {
                                        return;
                                    };
                                    let mut collected = collected.borrow_mut();
                                    let Some(node) = collected.nodes.get_mut(&id) else {
                                        return;
                                    };
                                    match param_type {
                                        ParamType::Format => node.format = parse_raw_format(pod),
                                        ParamType::EnumFormat => {
                                            for range in parse_rate_ranges(pod) {
                                                if !node.rates.contains(&range) {
                                                    node.rates.push(range);
                                                }
                                            }
                                        }
                                        ParamType::Props => {
                                            if let Some(volumes) = parse_volumes(pod) {
                                                node.volumes = Some(volumes);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            })
                            .register();
                        node.subscribe_params(&[
                            ParamType::EnumFormat,
                            ParamType::Format,
                            ParamType::Props,
                        ]);
                        keep_alive.borrow_mut().push(Box::new((node, listener)));
                    }
                    _ => return,
                }
                // Wait for the answers to the requests made above.
                if let Ok(seq) = core.sync(0) {
                    pending.borrow_mut().push(seq);
                }
            }
        })
        .register();

    mainloop.run();
    drop(keep_alive);

    let collected = collected.take();
    let mut nodes: Vec<_> = collected.nodes.into_values().collect();
    nodes.sort_by_key(|node| node.id);
    Ok(GraphSnapshot {
        clock: collected.clock,
        nodes,
    })
}

fn apply_clock_setting(clock: &mut ClockSettings, key: &str, value: &str) {
    match key {
        "clock.rate" => clock.rate = value.trim().parse().ok(),
        "clock.force-rate" => clock.force_rate = value.trim().parse().ok().filter(|&r| r != 0),
        "clock.allowed-rates" => clock.allowed_rates = parse_rate_list(value),
        _ => {}
    }
}

/// Parses a PipeWire rate list such as `[ 44100 48000 ]` or `[44100,48000]`.
fn parse_rate_list(value: &str) -> Vec<u32> {
    value
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .collect()
}

fn parse_raw_format(pod: &Pod) -> Option<RawFormat> {
    let (media_type, media_subtype) = parse_format(pod).ok()?;
    if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
        return None;
    }
    let mut info = AudioInfoRaw::new();
    info.parse(pod).ok()?;
    Some(RawFormat {
        rate: info.rate(),
        channels: info.channels(),
    })
}

/// Rates of a raw audio `EnumFormat` entry: a fixed rate or a choice.
fn parse_rate_ranges(pod: &Pod) -> Vec<RateRange> {
    let Ok((_, Value::Object(object))) = PodDeserializer::deserialize_any_from(pod.as_bytes())
    else {
        return Vec::new();
    };
    let Some(rate) = object
        .properties
        .into_iter()
        .find(|p| p.key == spa::sys::SPA_FORMAT_AUDIO_rate)
    else {
        return Vec::new();
    };
    let to_u32 = |v: i32| u32::try_from(v).unwrap_or(0);
    match rate.value {
        Value::Int(r) => vec![RateRange::fixed(to_u32(r))],
        Value::Choice(ChoiceValue::Int(Choice(_, choice))) => match choice {
            ChoiceEnum::None(r) => vec![RateRange::fixed(to_u32(r))],
            ChoiceEnum::Range { min, max, .. } => vec![RateRange {
                min: to_u32(min),
                max: to_u32(max),
                step: 0,
            }],
            ChoiceEnum::Step { min, max, step, .. } => vec![RateRange {
                min: to_u32(min),
                max: to_u32(max),
                step: to_u32(step),
            }],
            ChoiceEnum::Enum {
                default,
                alternatives,
            } => std::iter::once(default)
                .chain(alternatives)
                .map(|r| RateRange::fixed(to_u32(r)))
                .collect(),
            ChoiceEnum::Flags { .. } => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn parse_volumes(pod: &Pod) -> Option<Volumes> {
    let (_, Value::Object(object)) = PodDeserializer::deserialize_any_from(pod.as_bytes()).ok()?
    else {
        return None;
    };
    let mut volumes = Volumes::default();
    let mut found = false;
    for property in object.properties {
        match (property.key, property.value) {
            (spa::sys::SPA_PROP_channelVolumes, Value::ValueArray(ValueArray::Float(v))) => {
                volumes.channel = v;
                found = true;
            }
            (spa::sys::SPA_PROP_softVolumes, Value::ValueArray(ValueArray::Float(v))) => {
                volumes.soft = v;
                found = true;
            }
            (spa::sys::SPA_PROP_mute, Value::Bool(b)) => volumes.mute = b,
            (spa::sys::SPA_PROP_softMute, Value::Bool(b)) => volumes.soft_mute = b,
            _ => {}
        }
    }
    found.then_some(volumes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_lists() {
        assert_eq!(parse_rate_list("[ 48000 ]"), [48_000]);
        assert_eq!(parse_rate_list("[ 44100 48000 ]"), [44_100, 48_000]);
        assert_eq!(parse_rate_list("[44100,48000]"), [44_100, 48_000]);
    }

    #[test]
    fn rate_ranges() {
        let fixed = RateRange::fixed(48_000);
        assert!(fixed.contains(48_000) && !fixed.contains(44_100));
        let range = RateRange {
            min: 32_000,
            max: 192_000,
            step: 0,
        };
        assert!(range.contains(44_100));
        let node = NodeState {
            rates: vec![fixed],
            ..Default::default()
        };
        assert_eq!(node.supports_rate(44_100), Some(false));
        assert_eq!(NodeState::default().supports_rate(44_100), None);
    }

    #[test]
    fn clock_settings() {
        let mut clock = ClockSettings::default();
        apply_clock_setting(&mut clock, "clock.rate", "48000");
        apply_clock_setting(&mut clock, "clock.allowed-rates", "[ 48000 ]");
        apply_clock_setting(&mut clock, "clock.force-rate", "0");
        assert_eq!(
            clock,
            ClockSettings {
                rate: Some(48_000),
                allowed_rates: vec![48_000],
                force_rate: None,
            }
        );
        apply_clock_setting(&mut clock, "clock.force-rate", "44100");
        assert_eq!(clock.force_rate, Some(44_100));
    }
}
