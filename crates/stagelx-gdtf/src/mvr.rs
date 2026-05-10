//! MVR (My Virtual Rig) parser — ANSI E1.54.
//!
//! An MVR file is a ZIP archive containing:
//! - `GeneralSceneDescription.xml` — fixture list with 3D transforms and DMX addresses
//! - One or more embedded `.gdtf` fixture files
//! - Optional venue geometry files (`.obj`, `.3ds`, `.glb`)
//!
//! Coordinate system: MVR uses Z-up, X-right, Y-depth (mm).
//! Converted to Bevy (Y-up, X-right, -Z-forward, metres):
//!   Bevy.X =  MVR.X / 1000
//!   Bevy.Y =  MVR.Z / 1000
//!   Bevy.Z = -MVR.Y / 1000

use std::io::{Read, Seek};
use zip::ZipArchive;
use quick_xml::{Reader as XmlReader, events::Event as XmlEvent};
use stagelx_core::{fixture::FixtureInstance, types::{DmxAddress, FixtureId}};
use crate::error::GdtfError;

// ─── Public types ─────────────────────────────────────────────────────────────

/// A 3D geometry reference inside a SceneObject or Truss.
#[derive(Debug, Clone, Default)]
pub struct MvrGeometry3D {
    pub file_name: String,
}

/// A generic scene object (stage element, scenery, etc.).
#[derive(Debug, Clone, Default)]
pub struct MvrSceneObject {
    pub name: String,
    pub uuid: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub geometries: Vec<MvrGeometry3D>,
}

/// A truss structure element.
#[derive(Debug, Clone, Default)]
pub struct MvrTruss {
    pub name: String,
    pub uuid: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub geometries: Vec<MvrGeometry3D>,
}

/// A parsed MVR scene.
#[derive(Debug, Default)]
pub struct MvrScene {
    pub name: String,
    pub fixture_instances: Vec<FixtureInstance>,
    /// Raw bytes of each embedded GDTF file, keyed by filename.
    pub gdtf_files: Vec<(String, Vec<u8>)>,
    /// Optional venue geometry file paths inside the MVR ZIP.
    pub geometry_files: Vec<String>,
    /// Scene objects (stage elements, scenery).
    pub scene_objects: Vec<MvrSceneObject>,
    /// Truss structures.
    pub trusses: Vec<MvrTruss>,
}

// ─── Parse entry point ────────────────────────────────────────────────────────

pub fn parse_mvr(data: &[u8]) -> Result<MvrScene, GdtfError> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = ZipArchive::new(cursor)?;

    // Collect all embedded file names first to avoid borrow-while-iterating.
    let all_names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();

    let mut gdtf_files = Vec::new();
    let mut geometry_files = Vec::new();

    for name in &all_names {
        if name.eq_ignore_ascii_case("GeneralSceneDescription.xml") {
            continue;
        }
        let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        match ext.as_str() {
            "gdtf" => {
                if let Ok(mut f) = archive.by_name(name) {
                    let mut bytes = Vec::new();
                    if f.read_to_end(&mut bytes).is_ok() {
                        gdtf_files.push((name.clone(), bytes));
                    }
                }
            }
            "obj" | "3ds" | "glb" | "gltf" => {
                geometry_files.push(name.clone());
            }
            _ => {}
        }
    }

    // Parse GeneralSceneDescription.xml.
    let xml_content = read_case_insensitive(&mut archive, "GeneralSceneDescription.xml")?;
    let ParsedElements { fixture_instances, scene_objects, trusses } = parse_scene_xml(&xml_content)?;

    Ok(MvrScene {
        name: String::new(),
        fixture_instances,
        gdtf_files,
        geometry_files,
        scene_objects,
        trusses,
    })
}

// ─── XML parsing ─────────────────────────────────────────────────────────────

/// Intermediate fixture data accumulated from XML events.
#[derive(Default)]
struct MvrFixtureData {
    name:            String,
    gdtf_spec:       String,
    gdtf_mode:       String,
    fixture_type_id: String,
    matrix_str:      String,
    address_str:     String,
}

/// Intermediate scene-object data accumulated from XML events.
#[derive(Default)]
struct MvrSceneObjectData {
    name:       String,
    uuid:       String,
    matrix_str: String,
    geometries: Vec<MvrGeometry3D>,
}

/// Intermediate truss data accumulated from XML events.
#[derive(Default)]
struct MvrTrussData {
    name:       String,
    uuid:       String,
    matrix_str: String,
    geometries: Vec<MvrGeometry3D>,
}

#[derive(PartialEq)]
enum TextTarget { None, GdtfSpec, GdtfMode, FixtureTypeId, Matrix, Address }

/// Tracks which kind of element we are currently inside.
enum ActiveElement {
    None,
    Fixture(MvrFixtureData),
    SceneObject(MvrSceneObjectData),
    Truss(MvrTrussData),
}

struct ParsedElements {
    fixture_instances: Vec<FixtureInstance>,
    scene_objects: Vec<MvrSceneObject>,
    trusses: Vec<MvrTruss>,
}

fn parse_scene_xml(xml: &str) -> Result<ParsedElements, GdtfError> {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut instances = Vec::new();
    let mut scene_objects = Vec::new();
    let mut trusses = Vec::new();

    let mut active = ActiveElement::None;
    let mut target = TextTarget::None;
    let mut depth: u32 = 0;          // nesting depth inside the active element
    let mut in_geometries: bool = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(ref e)) => {
                match e.name().as_ref() {
                    b"Fixture" => {
                        let mut data = MvrFixtureData::default();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" | b"Name" => {
                                    data.name = String::from_utf8_lossy(&attr.value).into_owned();
                                }
                                _ => {}
                            }
                        }
                        active = ActiveElement::Fixture(data);
                        depth = 1;
                    }
                    b"SceneObject" => {
                        let mut data = MvrSceneObjectData::default();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" | b"Name" => {
                                    data.name = String::from_utf8_lossy(&attr.value).into_owned();
                                }
                                b"uuid" | b"UUID" => {
                                    data.uuid = String::from_utf8_lossy(&attr.value).into_owned();
                                }
                                _ => {}
                            }
                        }
                        active = ActiveElement::SceneObject(data);
                        depth = 1;
                    }
                    b"Truss" => {
                        let mut data = MvrTrussData::default();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" | b"Name" => {
                                    data.name = String::from_utf8_lossy(&attr.value).into_owned();
                                }
                                b"uuid" | b"UUID" => {
                                    data.uuid = String::from_utf8_lossy(&attr.value).into_owned();
                                }
                                _ => {}
                            }
                        }
                        active = ActiveElement::Truss(data);
                        depth = 1;
                    }
                    b"Geometries" => {
                        if matches!(active, ActiveElement::SceneObject(_) | ActiveElement::Truss(_)) {
                            in_geometries = true;
                        } else if !matches!(active, ActiveElement::None) {
                            depth = depth.saturating_add(1);
                        }
                    }
                    b"Geometry3D" => {
                        if in_geometries {
                            let mut file_name = String::new();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"fileName" | b"FileName" => {
                                        file_name = String::from_utf8_lossy(&attr.value).into_owned();
                                    }
                                    _ => {}
                                }
                            }
                            let geo = MvrGeometry3D { file_name };
                            match &mut active {
                                ActiveElement::SceneObject(d) => d.geometries.push(geo),
                                ActiveElement::Truss(d) => d.geometries.push(geo),
                                _ => {}
                            }
                        } else if !matches!(active, ActiveElement::None) {
                            depth = depth.saturating_add(1);
                        }
                    }
                    b"GDTFSpec"       if !matches!(active, ActiveElement::None) => { target = TextTarget::GdtfSpec; }
                    b"GDTFMode"       if !matches!(active, ActiveElement::None) => { target = TextTarget::GdtfMode; }
                    b"FixtureTypeId"  if matches!(active, ActiveElement::Fixture(_)) => { target = TextTarget::FixtureTypeId; }
                    b"Matrix"         if !matches!(active, ActiveElement::None) => { target = TextTarget::Matrix; }
                    b"Address"        if matches!(active, ActiveElement::Fixture(_)) => { target = TextTarget::Address; }
                    _ => {
                        if !matches!(active, ActiveElement::None) {
                            depth = depth.saturating_add(1);
                        }
                    }
                }
            }

            Ok(XmlEvent::Text(ref e)) => {
                if !matches!(active, ActiveElement::None) {
                    let text = e.unescape().map(|s| s.into_owned()).unwrap_or_default();
                    match &mut active {
                        ActiveElement::Fixture(data) => {
                            match target {
                                TextTarget::GdtfSpec      => data.gdtf_spec       = text,
                                TextTarget::GdtfMode      => data.gdtf_mode       = text,
                                TextTarget::FixtureTypeId => data.fixture_type_id = text,
                                TextTarget::Matrix        => data.matrix_str      = text,
                                TextTarget::Address       => data.address_str     = text,
                                TextTarget::None          => {}
                            }
                        }
                        ActiveElement::SceneObject(data) => {
                            match target {
                                TextTarget::Matrix => data.matrix_str = text,
                                _ => {}
                            }
                        }
                        ActiveElement::Truss(data) => {
                            match target {
                                TextTarget::Matrix => data.matrix_str = text,
                                _ => {}
                            }
                        }
                        ActiveElement::None => {}
                    }
                }
            }

            Ok(XmlEvent::End(ref e)) => {
                target = TextTarget::None;
                match e.name().as_ref() {
                    b"Fixture" => {
                        if let ActiveElement::Fixture(data) = std::mem::replace(&mut active, ActiveElement::None) {
                            if let Some(inst) = convert_fixture(data, instances.len() as u32) {
                                instances.push(inst);
                            }
                        }
                        depth = 0;
                    }
                    b"SceneObject" => {
                        if let ActiveElement::SceneObject(data) = std::mem::replace(&mut active, ActiveElement::None) {
                            if let Some(obj) = convert_scene_object(data) {
                                scene_objects.push(obj);
                            }
                        }
                        depth = 0;
                    }
                    b"Truss" => {
                        if let ActiveElement::Truss(data) = std::mem::replace(&mut active, ActiveElement::None) {
                            if let Some(t) = convert_truss(data) {
                                trusses.push(t);
                            }
                        }
                        depth = 0;
                    }
                    b"Geometries" => {
                        in_geometries = false;
                        if !matches!(active, ActiveElement::None) {
                            depth = depth.saturating_sub(1);
                        }
                    }
                    _ => {
                        if !matches!(active, ActiveElement::None) {
                            depth = depth.saturating_sub(1);
                        }
                    }
                }
            }

            Ok(XmlEvent::Eof) => break,
            Err(e) => return Err(GdtfError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(ParsedElements { fixture_instances: instances, scene_objects, trusses })
}

// ─── Conversions ──────────────────────────────────────────────────────────────

fn convert_fixture(data: MvrFixtureData, seq: u32) -> Option<FixtureInstance> {
    let fixture_type_id = if !data.fixture_type_id.is_empty() {
        data.fixture_type_id
    } else {
        data.gdtf_spec
            .rsplit('/')
            .next()
            .unwrap_or(&data.gdtf_spec)
            .trim_end_matches(".gdtf")
            .to_string()
    };

    let (universe, channel) = parse_address(&data.address_str).unwrap_or((1, 1));
    let (position, rotation) = parse_matrix(&data.matrix_str);

    let name = if data.name.is_empty() {
        format!("Fixture {}", seq + 1)
    } else {
        data.name
    };

    Some(FixtureInstance {
        id: FixtureId(seq),
        name,
        fixture_type_id,
        dmx_mode: if data.gdtf_mode.is_empty() { "Default".into() } else { data.gdtf_mode },
        address: DmxAddress::new(universe, channel),
        position,
        rotation,
        channel_map: Default::default(),
    })
}

fn convert_scene_object(data: MvrSceneObjectData) -> Option<MvrSceneObject> {
    let (position, rotation) = parse_matrix(&data.matrix_str);
    Some(MvrSceneObject {
        name: data.name,
        uuid: data.uuid,
        position,
        rotation,
        geometries: data.geometries,
    })
}

fn convert_truss(data: MvrTrussData) -> Option<MvrTruss> {
    let (position, rotation) = parse_matrix(&data.matrix_str);
    Some(MvrTruss {
        name: data.name,
        uuid: data.uuid,
        position,
        rotation,
        geometries: data.geometries,
    })
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Parse MVR address string "universe.channel" (both 1-based).
fn parse_address(s: &str) -> Option<(u16, u16)> {
    let mut parts = s.trim().splitn(2, '.');
    let u: u16 = parts.next()?.trim().parse().ok()?;
    let c: u16 = parts.next()?.trim().parse().ok()?;
    if u == 0 || c == 0 || c > 512 { return None; }
    Some((u, c))
}

/// Parse MVR 4×4 column-major matrix string "{f0,f1,…,f15}" (positions in mm).
/// Returns (position_metres, rotation_degrees) in Bevy coordinate space.
///
/// MVR coordinate system (MA3): X=right, Y=depth, Z=up.
/// Bevy coordinate system: X=right, Y=up, Z=-depth.
pub fn parse_matrix(s: &str) -> ([f32; 3], [f32; 3]) {
    let inner = s.trim().trim_start_matches('{').trim_end_matches('}');
    let floats: Vec<f32> = inner
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();

    if floats.len() < 16 {
        return ([0.0; 3], [0.0; 3]);
    }

    // Column-major: m[12..15] is the translation column.
    let tx = floats[12];
    let ty = floats[13];
    let tz = floats[14];

    // Convert mm → metres, swap axes: Bevy.Y = MVR.Z, Bevy.Z = -MVR.Y
    let position = [tx / 1000.0, tz / 1000.0, -ty / 1000.0];

    // Extract rotation from upper-left 3×3.
    let r = [
        [floats[0], floats[4], floats[8]],
        [floats[1], floats[5], floats[9]],
        [floats[2], floats[6], floats[10]],
    ];

    // Map to Bevy-space rotation matrix.
    let rb = [
        [ r[0][0],  r[0][2], -r[0][1]],
        [ r[2][0],  r[2][2], -r[2][1]],
        [-r[1][0], -r[1][2],  r[1][1]],
    ];

    // ZYX euler decomposition (radians → degrees).
    let pitch = rb[2][0].clamp(-1.0, 1.0).asin();
    let (yaw, roll) = if pitch.cos().abs() > 1e-6 {
        (rb[2][1].atan2(rb[2][2]), rb[1][0].atan2(rb[0][0]))
    } else {
        (0.0_f32, rb[0][1].atan2(rb[1][1]))
    };

    let deg = 180.0 / std::f32::consts::PI;
    let rotation = [roll * deg, yaw * deg, pitch * deg];

    (position, rotation)
}

/// Find a file in the archive case-insensitively and read its contents.
fn read_case_insensitive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    target: &str,
) -> Result<String, GdtfError> {
    let target_lower = target.to_ascii_lowercase();
    let name = (0..archive.len())
        .find_map(|i| {
            archive.by_index(i).ok().filter(|f| {
                f.name().to_ascii_lowercase() == target_lower
            }).map(|f| f.name().to_string())
        })
        .ok_or(GdtfError::MissingField("GeneralSceneDescription.xml"))?;

    let mut file = archive.by_name(&name)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}
