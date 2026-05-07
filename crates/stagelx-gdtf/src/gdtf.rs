use std::io::Read;
use zip::ZipArchive;
use crate::error::GdtfError;

// ─── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GdtfFixtureType {
    pub fixture_type_id: String,
    pub name: String,
    pub short_name: String,
    pub manufacturer: String,
    pub description: String,
    pub dmx_modes: Vec<DmxMode>,
    pub geometries: Vec<Geometry>,
    pub wheels: Vec<Wheel>,
    /// Raw bytes of each embedded 3DS model file, keyed by filename (e.g. "models/3ds/Body.3ds").
    pub models: Vec<(String, Vec<u8>)>,
}

#[derive(Debug, Clone)]
pub struct DmxMode {
    pub name: String,
    /// Top-level geometry this mode is wired to.
    pub root_geometry: String,
    pub channels: Vec<DmxChannel>,
}

#[derive(Debug, Clone)]
pub struct DmxChannel {
    /// 1-based DMX offset from fixture base address (first byte for 16-bit channels).
    pub offset: u16,
    /// Number of DMX bytes: 1 = 8-bit, 2 = 16-bit.
    pub resolution: u8,
    /// GDTF attribute name, e.g. "Pan", "Tilt", "Dimmer", "ColorAdd_R".
    pub attribute: String,
    /// Name of the geometry this channel controls (used for articulation).
    pub geometry: String,
    pub default_value: u8,
    /// Physical minimum (e.g. -270.0 for Pan).
    pub physical_from: f32,
    /// Physical maximum (e.g. 270.0 for Pan).
    pub physical_to: f32,
}

impl DmxChannel {
    /// Map a raw 8-bit value (or MSB of a 16-bit value) to a 0.0–1.0 ratio.
    pub fn normalize(&self, msb: u8, lsb: u8) -> f32 {
        if self.resolution >= 2 {
            let raw = ((msb as u32) << 8 | lsb as u32) as f32 / 65535.0;
            raw
        } else {
            msb as f32 / 255.0
        }
    }

    /// Map a normalized 0.0–1.0 value to physical units using this channel's range.
    pub fn to_physical(&self, normalized: f32) -> f32 {
        self.physical_from + normalized * (self.physical_to - self.physical_from)
    }
}

#[derive(Debug, Clone)]
pub struct Geometry {
    pub name: String,
    pub geometry_type: GeometryType,
    pub children: Vec<Geometry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GeometryType {
    General,
    Beam { beam_angle: f32, field_angle: f32, beam_type: BeamType },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BeamType {
    Wash,
    Spot,
    None,
}

#[derive(Debug, Clone)]
pub struct Wheel {
    pub name: String,
    pub slots: Vec<WheelSlot>,
}

#[derive(Debug, Clone)]
pub struct WheelSlot {
    pub name: String,
    /// CIE xyY colour [x, y, Y] if this is a colour slot.
    pub color: Option<[f32; 3]>,
    /// Path inside the GDTF ZIP to the gobo/gel image asset.
    pub media_file: Option<String>,
}

// ─── Private XML serde structs ────────────────────────────────────────────────

mod xml {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct Gdtf {
        #[serde(rename = "FixtureType")]
        pub fixture_type: FixtureType,
    }

    #[derive(Deserialize)]
    pub struct FixtureType {
        #[serde(rename = "@Name", default)]
        pub name: String,
        #[serde(rename = "@ShortName", default)]
        pub short_name: String,
        #[serde(rename = "@Manufacturer", default)]
        pub manufacturer: String,
        #[serde(rename = "@Description", default)]
        pub description: String,
        #[serde(rename = "@FixtureTypeID", default)]
        pub fixture_type_id: String,
        #[serde(rename = "Wheels")]
        pub wheels: Option<Wheels>,
        #[serde(rename = "Geometries")]
        pub geometries: Option<Geometries>,
        #[serde(rename = "DMXModes")]
        pub dmx_modes: Option<DmxModes>,
    }

    #[derive(Deserialize, Default)]
    pub struct Wheels {
        #[serde(rename = "Wheel", default)]
        pub wheels: Vec<Wheel>,
    }

    #[derive(Deserialize)]
    pub struct Wheel {
        #[serde(rename = "@Name", default)]
        pub name: String,
        #[serde(rename = "Slot", default)]
        pub slots: Vec<WheelSlot>,
    }

    #[derive(Deserialize)]
    pub struct WheelSlot {
        #[serde(rename = "@Name", default)]
        pub name: String,
        #[serde(rename = "@Color", default)]
        pub color: Option<String>,
        #[serde(rename = "@MediaFileName", default)]
        pub media_file: Option<String>,
    }

    #[derive(Deserialize, Default)]
    pub struct Geometries {
        #[serde(rename = "Geometry", default)]
        pub geometries: Vec<Geometry>,
        #[serde(rename = "BeamGeometry", default)]
        pub beam_geometries: Vec<BeamGeometry>,
    }

    #[derive(Deserialize)]
    pub struct Geometry {
        #[serde(rename = "@Name", default)]
        pub name: String,
        #[serde(rename = "Geometry", default)]
        pub children: Vec<Geometry>,
        #[serde(rename = "BeamGeometry", default)]
        pub beam_children: Vec<BeamGeometry>,
    }

    #[derive(Deserialize)]
    pub struct BeamGeometry {
        #[serde(rename = "@Name", default)]
        pub name: String,
        #[serde(rename = "@BeamAngle", default)]
        pub beam_angle: String,
        #[serde(rename = "@FieldAngle", default)]
        pub field_angle: String,
        #[serde(rename = "@BeamType", default)]
        pub beam_type: String,
    }

    #[derive(Deserialize, Default)]
    pub struct DmxModes {
        #[serde(rename = "DMXMode", default)]
        pub modes: Vec<DmxMode>,
    }

    #[derive(Deserialize)]
    pub struct DmxMode {
        #[serde(rename = "@Name", default)]
        pub name: String,
        #[serde(rename = "@Geometry", default)]
        pub geometry: String,
        #[serde(rename = "DMXChannels")]
        pub channels: Option<DmxChannels>,
    }

    #[derive(Deserialize)]
    pub struct DmxChannels {
        #[serde(rename = "DMXChannel", default)]
        pub channels: Vec<DmxChannel>,
    }

    #[derive(Deserialize)]
    pub struct DmxChannel {
        #[serde(rename = "@Offset", default)]
        pub offset: String,
        #[serde(rename = "@Default", default)]
        pub default: String,
        #[serde(rename = "@Geometry", default)]
        pub geometry: String,
        #[serde(rename = "LogicalChannel", default)]
        pub logical_channels: Vec<LogicalChannel>,
    }

    #[derive(Deserialize)]
    pub struct LogicalChannel {
        #[serde(rename = "@Attribute", default)]
        pub attribute: String,
        #[serde(rename = "ChannelFunction", default)]
        pub functions: Vec<ChannelFunction>,
    }

    #[derive(Deserialize)]
    pub struct ChannelFunction {
        #[serde(rename = "@PhysicalFrom", default)]
        pub physical_from: String,
        #[serde(rename = "@PhysicalTo", default)]
        pub physical_to: String,
    }
}

// ─── Parse entry point ────────────────────────────────────────────────────────

pub fn parse_gdtf(data: &[u8]) -> Result<GdtfFixtureType, GdtfError> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = ZipArchive::new(cursor)?;

    // Collect all embedded 3DS model filenames before borrowing description.xml.
    let model_names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            archive.by_index(i).ok().and_then(|f| {
                let n = f.name().to_string();
                if n.starts_with("models/") && n.ends_with(".3ds") { Some(n) } else { None }
            })
        })
        .collect();

    let xml_content = {
        let mut file = archive.by_name("description.xml")?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        content
    };

    let mut models = Vec::new();
    for name in model_names {
        if let Ok(mut file) = archive.by_name(&name) {
            let mut bytes = Vec::new();
            if file.read_to_end(&mut bytes).is_ok() {
                models.push((name, bytes));
            }
        }
    }

    let parsed: xml::Gdtf = quick_xml::de::from_str(&xml_content)?;
    let mut ft = convert_fixture_type(parsed.fixture_type);
    ft.models = models;
    Ok(ft)
}

// ─── Conversion helpers ───────────────────────────────────────────────────────

fn convert_fixture_type(ft: xml::FixtureType) -> GdtfFixtureType {
    let dmx_modes = ft.dmx_modes
        .map(|m| m.modes.into_iter().map(convert_dmx_mode).collect())
        .unwrap_or_default();

    let wheels = ft.wheels
        .map(|w| w.wheels.into_iter().map(convert_wheel).collect())
        .unwrap_or_default();

    let geometries = ft.geometries
        .map(|g| {
            let mut geoms: Vec<Geometry> =
                g.geometries.into_iter().map(convert_geometry).collect();
            geoms.extend(g.beam_geometries.into_iter().map(convert_beam_geometry));
            geoms
        })
        .unwrap_or_default();

    GdtfFixtureType {
        fixture_type_id: ft.fixture_type_id,
        name: ft.name,
        short_name: ft.short_name,
        manufacturer: ft.manufacturer,
        description: ft.description,
        dmx_modes,
        geometries,
        wheels,
        models: Vec::new(), // populated by parse_gdtf after convert_fixture_type returns
    }
}

fn convert_dmx_mode(mode: xml::DmxMode) -> DmxMode {
    let channels = mode.channels
        .map(|c| c.channels.into_iter().map(convert_dmx_channel).collect())
        .unwrap_or_default();
    DmxMode {
        name: mode.name,
        root_geometry: mode.geometry,
        channels,
    }
}

fn convert_dmx_channel(ch: xml::DmxChannel) -> DmxChannel {
    let (offset, resolution) = parse_offset(&ch.offset);
    let default_value = parse_default_value(&ch.default, resolution);

    let (attribute, physical_from, physical_to) = ch.logical_channels
        .into_iter()
        .next()
        .map(|lc| {
            let (pf, pt) = lc.functions.into_iter().next()
                .map(|cf| (
                    cf.physical_from.parse().unwrap_or(0.0),
                    cf.physical_to.parse().unwrap_or(1.0),
                ))
                .unwrap_or((0.0, 1.0));
            (lc.attribute, pf, pt)
        })
        .unwrap_or_default();

    DmxChannel {
        offset,
        resolution,
        attribute,
        geometry: ch.geometry,
        default_value,
        physical_from,
        physical_to,
    }
}

fn convert_geometry(g: xml::Geometry) -> Geometry {
    let mut children: Vec<Geometry> =
        g.children.into_iter().map(convert_geometry).collect();
    children.extend(g.beam_children.into_iter().map(convert_beam_geometry));
    Geometry {
        name: g.name,
        geometry_type: GeometryType::General,
        children,
    }
}

fn convert_beam_geometry(bg: xml::BeamGeometry) -> Geometry {
    let beam_angle = bg.beam_angle.parse().unwrap_or(10.0_f32);
    let field_angle = bg.field_angle.parse().unwrap_or(beam_angle * 1.5);
    let beam_type = match bg.beam_type.as_str() {
        "Spot" => BeamType::Spot,
        "Wash" => BeamType::Wash,
        _ => BeamType::None,
    };
    Geometry {
        name: bg.name,
        geometry_type: GeometryType::Beam { beam_angle, field_angle, beam_type },
        children: vec![],
    }
}

fn convert_wheel(w: xml::Wheel) -> Wheel {
    Wheel {
        name: w.name,
        slots: w.slots.into_iter().map(|s| WheelSlot {
            name: s.name,
            color: s.color.as_deref().and_then(parse_cie_color),
            media_file: s.media_file,
        }).collect(),
    }
}

// ─── Value parsing utilities ──────────────────────────────────────────────────

/// Parse GDTF Offset field: "1" → (1, 1-byte), "2,3" → (2, 2-byte).
fn parse_offset(s: &str) -> (u16, u8) {
    let parts: Vec<&str> = s.split(',').collect();
    let offset = parts.first()
        .and_then(|p| p.trim().parse().ok())
        .unwrap_or(1);
    let resolution = (parts.len() as u8).max(1);
    (offset, resolution)
}

/// Parse GDTF Default field: "128/1" or "32768/2" → 8-bit MSB equivalent.
fn parse_default_value(s: &str, resolution: u8) -> u8 {
    let mut parts = s.split('/');
    let value: u32 = parts.next().and_then(|p| p.trim().parse().ok()).unwrap_or(0);
    if resolution >= 2 {
        (value >> 8) as u8
    } else {
        value.min(255) as u8
    }
}

/// Parse CIE xyY colour string "0.3127,0.3290,100" → [x, y, Y].
fn parse_cie_color(s: &str) -> Option<[f32; 3]> {
    let parts: Vec<f32> = s.split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    (parts.len() >= 3).then(|| [parts[0], parts[1], parts[2]])
}

// ─── Convenience methods ──────────────────────────────────────────────────────

impl GdtfFixtureType {
    /// Find the first DMX mode with the given name, or the first mode if `name` is empty.
    pub fn find_mode(&self, name: &str) -> Option<&DmxMode> {
        if name.is_empty() {
            self.dmx_modes.first()
        } else {
            self.dmx_modes.iter().find(|m| m.name == name)
        }
    }

    /// Find the first beam geometry in the geometry tree.
    pub fn beam_angle(&self) -> f32 {
        fn search(geoms: &[Geometry]) -> Option<f32> {
            for g in geoms {
                if let GeometryType::Beam { beam_angle, .. } = g.geometry_type {
                    return Some(beam_angle);
                }
                if let Some(a) = search(&g.children) {
                    return Some(a);
                }
            }
            None
        }
        search(&self.geometries).unwrap_or(10.0)
    }
}

impl DmxMode {
    /// Find the first channel with the given GDTF attribute name.
    pub fn channel_for(&self, attribute: &str) -> Option<&DmxChannel> {
        self.channels.iter().find(|c| c.attribute == attribute)
    }
}
