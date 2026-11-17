//! Decisions on whether the audio path can carry a SYRO stream unchanged:
//! from a PipeWire [`GraphSnapshot`] on Linux, from the device mix format
//! elsewhere. Pure logic, no I/O.

use std::fmt;

use crate::graph::{ClockSettings, GraphSnapshot, NodeState, Volumes};

/// Sample rate of SYRO streams.
pub const STREAM_RATE: u32 = 44_100;

/// Command that forces the PipeWire graph to the stream rate.
pub const FORCE_RATE_COMMAND: &str = "pw-metadata -n settings 0 clock.force-rate 44100";

/// Reason to refuse a transfer.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    /// The graph cannot run at 44.1 kHz with the current clock settings.
    RateUnavailable(ClockSettings),
    /// Our stream is not (yet) in the graph.
    StreamNotFound,
    /// Our stream has no driver (device) yet.
    NotConnected,
    /// The stream or device format is not known yet.
    FormatUnknown { node: String },
    /// The device runs at another rate: PipeWire would resample.
    DeviceRate { device: String, rate: u32 },
    /// The stream itself was negotiated with another rate or channel count.
    StreamFormat { rate: u32, channels: u32 },
    /// The stream or device is muted.
    Muted { node: String },
    /// The shared-mode mix rate of the device (Windows) is not the rate the
    /// stream was prepared for (it changed after the stream was opened): the
    /// system would resample the stream.
    MixRate {
        device: String,
        rate: u32,
        expected: u32,
    },
}

impl Problem {
    /// Whether the problem may resolve by itself shortly after the stream
    /// starts (node creation, linking, format negotiation, rate switch).
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::StreamNotFound
                | Self::NotConnected
                | Self::FormatUnknown { .. }
                | Self::DeviceRate { .. }
        )
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RateUnavailable(clock) => {
                write!(f, "PipeWire cannot run at {STREAM_RATE} Hz (")?;
                match clock.force_rate {
                    Some(rate) => write!(f, "clock.force-rate is {rate}")?,
                    None => write!(f, "clock.allowed-rates is {:?}", clock.allowed_rates)?,
                }
                write!(
                    f,
                    "), the stream would be resampled. Run `{FORCE_RATE_COMMAND}` \
                     (and `... clock.force-rate 0` afterwards), or add {STREAM_RATE} \
                     to clock.allowed-rates in ~/.config/pipewire/pipewire.conf.d/"
                )
            }
            Self::StreamNotFound => write!(f, "the output stream did not appear in PipeWire"),
            Self::NotConnected => write!(f, "the output stream is not connected to a device"),
            Self::FormatUnknown { node } => write!(f, "the format of {node} is unknown"),
            Self::DeviceRate { device, rate } => write!(
                f,
                "{device} runs at {rate} Hz, not {STREAM_RATE} Hz; PipeWire would resample \
                 the stream. Run `{FORCE_RATE_COMMAND}` and stop other audio"
            ),
            Self::StreamFormat { rate, channels } => write!(
                f,
                "the stream was negotiated as {rate} Hz, {channels} channels instead of \
                 {STREAM_RATE} Hz stereo"
            ),
            Self::Muted { node } => write!(f, "{node} is muted"),
            Self::MixRate {
                device,
                rate,
                expected,
            } => write!(
                f,
                "{device} switched to {rate} Hz, the stream was prepared for {expected} Hz; \
                 the system would resample it. Try again"
            ),
        }
    }
}

/// Something that may alter the stream but does not stop the transfer.
#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    /// The device supports only other rates; PipeWire resamples the stream.
    Resampled { device: String, rate: u32 },
    /// Software volume of the stream or device is not 100%: samples are
    /// scaled.
    SoftVolume { node: String, volumes: Vec<f32> },
    /// Device output level is below 100%.
    DeviceVolume { device: String, volumes: Vec<f32> },
    /// The shared-mode mix rate of the device (Windows) is not 44.1 kHz:
    /// Volcasm resampled the stream to it before sending.
    MixResampled { device: String, rate: u32 },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let percent = |v: &[f32]| {
            v.iter()
                .map(|x| format!("{:.0}%", x * 100.0))
                .collect::<Vec<_>>()
                .join("/")
        };
        match self {
            Self::Resampled { device, rate } => write!(
                f,
                "{device} supports only {rate} Hz, the system resamples the stream from \
                 {STREAM_RATE} Hz (unavoidable on this device; a 44.1 kHz capable \
                 interface avoids it)"
            ),
            Self::SoftVolume { node, volumes } => write!(
                f,
                "software volume of {node} is {} (samples are scaled; 100% recommended)",
                percent(volumes)
            ),
            Self::DeviceVolume { device, volumes } => write!(
                f,
                "output volume of {device} is {} (100% recommended)",
                percent(volumes)
            ),
            Self::MixResampled { device, rate } => write!(
                f,
                "{device} runs at {rate} Hz: the stream was resampled from {STREAM_RATE} Hz. \
                 If the volca shows Err dcod, open the device in Sound settings and set \
                 Format (Advanced) to 16 bit, {STREAM_RATE} Hz"
            ),
        }
    }
}

/// Checks a device whose stream goes through a shared mixer running at
/// `mix_rate` (Windows). The stream was prepared (resampled if needed) for
/// `stream_rate`; any other mix rate would make the system resample it.
pub fn verify_mix_rate(
    device: &str,
    mix_rate: u32,
    stream_rate: u32,
) -> Result<Vec<Warning>, Problem> {
    if mix_rate != stream_rate {
        Err(Problem::MixRate {
            device: device.to_owned(),
            rate: mix_rate,
            expected: stream_rate,
        })
    } else if mix_rate != STREAM_RATE {
        Ok(vec![Warning::MixResampled {
            device: device.to_owned(),
            rate: mix_rate,
        }])
    } else {
        Ok(Vec::new())
    }
}

/// Can the graph clock run at 44.1 kHz with the current settings?
pub fn clock_reachable(clock: &ClockSettings) -> Result<(), Problem> {
    let reachable = match clock.force_rate {
        Some(rate) => rate == STREAM_RATE,
        None => clock.rate == Some(STREAM_RATE) || clock.allowed_rates.contains(&STREAM_RATE),
    };
    if reachable {
        Ok(())
    } else {
        Err(Problem::RateUnavailable(clock.clone()))
    }
}

/// Checks the running stream: the nodes matching `is_ours` must be connected
/// to a device running at 44.1 kHz, with nothing muted. A device that cannot
/// run at 44.1 kHz at all only gives a [`Warning::Resampled`]. One that can,
/// but runs at another rate, is refused: for good if the clock settings do
/// not allow 44.1 kHz, as transient otherwise (the graph may still switch).
pub fn verify(
    snapshot: &GraphSnapshot,
    is_ours: impl Fn(&NodeState) -> bool,
) -> Result<Vec<Warning>, Problem> {
    let streams: Vec<_> = snapshot.nodes.iter().filter(|n| is_ours(n)).collect();
    if streams.is_empty() {
        return Err(Problem::StreamNotFound);
    }
    let mut warnings = Vec::new();
    for stream in streams {
        let name = node_name(stream);
        let format = stream
            .format
            .ok_or_else(|| Problem::FormatUnknown { node: name.clone() })?;
        if format.rate != STREAM_RATE || format.channels != 2 {
            return Err(Problem::StreamFormat {
                rate: format.rate,
                channels: format.channels,
            });
        }
        let device = stream
            .prop("node.driver-id")
            .and_then(|id| id.parse().ok())
            .and_then(|id| snapshot.node(id))
            .ok_or(Problem::NotConnected)?;
        let device_name = node_name(device);
        let device_format = device.format.ok_or_else(|| Problem::FormatUnknown {
            node: device_name.clone(),
        })?;
        if device_format.rate != STREAM_RATE {
            if device.supports_rate(STREAM_RATE) == Some(false) {
                // The hardware cannot run at 44.1 kHz at all, so resampling
                // is unavoidable. SYRO transfers survived it on hardware
                // (docs/hardware.md, section 3).
                warnings.push(Warning::Resampled {
                    device: device_name.clone(),
                    rate: device_format.rate,
                });
            } else {
                clock_reachable(&snapshot.clock)?;
                return Err(Problem::DeviceRate {
                    device: device_name,
                    rate: device_format.rate,
                });
            }
        }
        check_volumes(stream, &name, false, &mut warnings)?;
        check_volumes(device, &device_name, true, &mut warnings)?;
    }
    Ok(warnings)
}

fn check_volumes(
    node: &NodeState,
    name: &str,
    is_device: bool,
    warnings: &mut Vec<Warning>,
) -> Result<(), Problem> {
    let Some(Volumes {
        channel,
        soft,
        mute,
        soft_mute,
    }) = &node.volumes
    else {
        return Ok(());
    };
    if *mute || *soft_mute {
        return Err(Problem::Muted {
            node: name.to_owned(),
        });
    }
    let unity = |v: &[f32]| v.iter().all(|&x| x == 1.0);
    if !unity(soft) {
        warnings.push(Warning::SoftVolume {
            node: name.to_owned(),
            volumes: soft.clone(),
        });
    }
    if is_device && !unity(channel) {
        warnings.push(Warning::DeviceVolume {
            device: name.to_owned(),
            volumes: channel.clone(),
        });
    }
    Ok(())
}

fn node_name(node: &NodeState) -> String {
    node.prop("node.description")
        .or_else(|| node.prop("node.name"))
        .map_or_else(|| format!("node {}", node.id), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{RateRange, RawFormat};

    fn clock(rate: u32, allowed: &[u32], force: Option<u32>) -> ClockSettings {
        ClockSettings {
            rate: Some(rate),
            allowed_rates: allowed.to_vec(),
            force_rate: force,
        }
    }

    #[test]
    fn clock_reachable_cases() {
        // State observed on the development machine (2026-10-13).
        assert!(clock_reachable(&clock(48_000, &[48_000], None)).is_err());
        assert!(clock_reachable(&clock(48_000, &[48_000], Some(44_100))).is_ok());
        assert!(clock_reachable(&clock(48_000, &[44_100, 48_000], None)).is_ok());
        assert!(clock_reachable(&clock(44_100, &[], None)).is_ok());
        assert!(clock_reachable(&clock(48_000, &[44_100, 48_000], Some(48_000))).is_err());
    }

    fn volumes(channel: f32, soft: f32) -> Option<Volumes> {
        Some(Volumes {
            channel: vec![channel; 2],
            soft: vec![soft; 2],
            mute: false,
            soft_mute: false,
        })
    }

    fn node(id: u32, props: &[(&str, &str)], rate: Option<u32>) -> NodeState {
        NodeState {
            id,
            props: props
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            format: rate.map(|rate| RawFormat { rate, channels: 2 }),
            rates: Vec::new(),
            volumes: volumes(1.0, 1.0),
        }
    }

    /// Our stream (id 91) on a device (id 76), as seen with `pw-dump`.
    fn graph(device_rate: Option<u32>) -> GraphSnapshot {
        GraphSnapshot {
            clock: clock(48_000, &[48_000], None),
            nodes: vec![
                node(
                    76,
                    &[
                        ("node.description", "Speaker"),
                        ("media.class", "Audio/Sink"),
                    ],
                    device_rate,
                ),
                node(
                    91,
                    &[("node.group", "cpal-1"), ("node.driver-id", "76")],
                    Some(STREAM_RATE),
                ),
            ],
        }
    }

    fn ours(node: &NodeState) -> bool {
        node.prop("node.group") == Some("cpal-1")
    }

    #[test]
    fn device_at_stream_rate_passes() {
        assert_eq!(verify(&graph(Some(STREAM_RATE)), ours), Ok(vec![]));
    }

    #[test]
    fn device_capable_of_44k_but_at_48k_without_clock_support_is_refused() {
        let mut g = graph(Some(48_000));
        g.nodes[0].rates = vec![RateRange {
            min: 32_000,
            max: 192_000,
            step: 0,
        }];
        let err = verify(&g, ours).unwrap_err();
        assert!(matches!(err, Problem::RateUnavailable(_)));
        assert!(!err.is_transient());
        // With the clock forced, the graph may still switch: transient.
        g.clock.force_rate = Some(STREAM_RATE);
        assert!(verify(&g, ours).unwrap_err().is_transient());
    }

    #[test]
    fn device_only_at_48k_gives_warning() {
        // Headphones of the development laptop: EnumFormat offers only
        // 48 kHz, also with clock.force-rate 44100 (2026-10-13).
        let mut g = graph(Some(48_000));
        g.nodes[0].rates = vec![RateRange::fixed(48_000)];
        assert_eq!(
            verify(&g, ours),
            Ok(vec![Warning::Resampled {
                device: "Speaker".into(),
                rate: 48_000
            }])
        );
    }

    #[test]
    fn device_at_48k_is_refused() {
        // Device formats unknown: treated as able to run at 44.1 kHz.
        let mut g = graph(Some(48_000));
        g.clock.force_rate = Some(STREAM_RATE);
        let err = verify(&g, ours).unwrap_err();
        assert_eq!(
            err,
            Problem::DeviceRate {
                device: "Speaker".into(),
                rate: 48_000
            }
        );
        assert!(err.is_transient());
    }

    #[test]
    fn missing_pieces_are_transient() {
        let mut g = graph(None);
        assert!(matches!(
            verify(&g, ours),
            Err(Problem::FormatUnknown { .. })
        ));
        g.nodes[1].props.remove("node.driver-id");
        assert_eq!(verify(&g, ours), Err(Problem::NotConnected));
        assert_eq!(verify(&g, |_| false), Err(Problem::StreamNotFound));
    }

    #[test]
    fn volumes_give_warnings_and_mute_refuses() {
        let mut g = graph(Some(STREAM_RATE));
        g.nodes[0].volumes = volumes(0.157_458, 0.965_299);
        let warnings = verify(&g, ours).unwrap();
        assert!(matches!(warnings[0], Warning::SoftVolume { .. }));
        assert!(matches!(warnings[1], Warning::DeviceVolume { .. }));

        g.nodes[1].volumes.as_mut().unwrap().mute = true;
        let err = verify(&g, ours).unwrap_err();
        assert!(matches!(err, Problem::Muted { .. }));
        assert!(!err.is_transient());
    }

    #[test]
    fn mix_rate() {
        assert_eq!(verify_mix_rate("Speakers", 44_100, 44_100), Ok(Vec::new()));
        assert_eq!(
            verify_mix_rate("Speakers", 48_000, 48_000),
            Ok(vec![Warning::MixResampled {
                device: "Speakers".into(),
                rate: 48_000
            }])
        );
        assert_eq!(
            verify_mix_rate("Speakers", 48_000, 44_100),
            Err(Problem::MixRate {
                device: "Speakers".into(),
                rate: 48_000,
                expected: 44_100
            })
        );
    }
}
