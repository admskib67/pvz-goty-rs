//! TodLib particle systems: dirt puffs, sparkles, arrows, flying zombie
//! heads and so on.
//!
//! A particle definition (`particles/*.xml`, or its compiled form on
//! Windows) is a list of emitters. Each emitter spawns particles over the
//! system's lifetime; almost every property (spawn rate, colour, scale,
//! spin, ...) is a [`FloatTrack`]: a curve over the system's or particle's
//! lifetime, with a random spread chosen per system or per particle.
//!
//! Behaviour per ruslan831 decomp, `Sexy.TodLib/TodParticle.cpp` (the
//! simulation) and `Definition.cpp` (`FloatTrackEvaluate`, the field tables
//! and the compiled reader). The original's float track parser
//! (`DefinitionReadFloatTrackField`) is missing from the decompilation; the
//! track syntax here is worked out from the game's particle files.

use std::sync::Arc;

use crate::data_array::{DataArray, Id};
use crate::definition::{CompiledError, CompiledReader, decompress, u32_at};
use crate::rand::MtRand;
use crate::tod::{TodCurve, curve_evaluate};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ParticleError {
    #[error("invalid XML: {0}")]
    Xml(String),
    #[error("compiled definition: {0}")]
    Compiled(&'static str),
    #[error("<{element}>: can't parse {value:?}")]
    BadValue { element: String, value: String },
}

impl From<CompiledError> for ParticleError {
    fn from(e: CompiledError) -> Self {
        Self::Compiled(e.0)
    }
}

/// One point of a [`FloatTrack`] (`FloatParameterTrackNode`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatTrackNode {
    /// 0..=1 through the lifetime.
    pub time: f32,
    pub low: f32,
    pub high: f32,
    /// How the value moves from this node to the next.
    pub curve: TodCurve,
    /// How the random spread between `low` and `high` is distributed.
    pub distribution: TodCurve,
}

/// A value over time with a random spread (`FloatParameterTrack`).
///
/// Text form, as in the particle files: nodes separated by spaces, each a
/// number or `[low high]` (optionally `[low Curve high]` for a non-linear
/// spread), optionally followed by `,time` (percent of the lifetime). A
/// curve name between two nodes sets how the value moves between them.
/// Nodes without a time are spread evenly; the first defaults to 0% and the
/// last to 100%.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FloatTrack {
    pub nodes: Vec<FloatTrackNode>,
}

impl FloatTrack {
    /// Parses the text form.
    pub fn parse(text: &str) -> Option<Self> {
        let mut nodes: Vec<FloatTrackNode> = Vec::new();
        let mut times: Vec<Option<f32>> = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        // Like scanf's %f: a sign, digits and one decimal point, so the
        // game's "[.3.8]" reads as .3 then .8.
        let number = |i: &mut usize| -> Option<f32> {
            let start = *i;
            if *i < chars.len() && matches!(chars[*i], '-' | '+') {
                *i += 1;
            }
            let mut seen_dot = false;
            while *i < chars.len() && (chars[*i].is_ascii_digit() || (chars[*i] == '.' && !seen_dot)) {
                seen_dot |= chars[*i] == '.';
                *i += 1;
            }
            chars[start..*i].iter().collect::<String>().parse().ok()
        };
        let word = |i: &mut usize| -> String {
            let start = *i;
            while *i < chars.len() && chars[*i].is_ascii_alphabetic() {
                *i += 1;
            }
            chars[start..*i].iter().collect()
        };
        let skip_ws = |i: &mut usize| {
            while *i < chars.len() && chars[*i].is_whitespace() {
                *i += 1;
            }
        };

        loop {
            skip_ws(&mut i);
            if i >= chars.len() {
                break;
            }
            let c = chars[i];
            let mut node =
                FloatTrackNode { time: 0.0, low: 0.0, high: 0.0, curve: TodCurve::Linear, distribution: TodCurve::Linear };
            if c == '[' {
                i += 1;
                let mut values = Vec::new();
                loop {
                    skip_ws(&mut i);
                    match chars.get(i) {
                        None => return None,
                        Some(']') => {
                            i += 1;
                            break;
                        }
                        Some(c) if c.is_ascii_alphabetic() => node.distribution = TodCurve::from_name(&word(&mut i))?,
                        Some(_) => values.push(number(&mut i)?),
                    }
                }
                match values[..] {
                    [v] => (node.low, node.high) = (v, v),
                    [lo, hi] => (node.low, node.high) = (lo, hi),
                    _ => return None,
                }
            } else if c.is_ascii_alphabetic() {
                let curve = TodCurve::from_name(&word(&mut i))?;
                if let Some(prev) = nodes.last_mut() {
                    prev.curve = curve;
                }
                continue;
            } else {
                let v = number(&mut i)?;
                (node.low, node.high) = (v, v);
            }
            let mut time = None;
            if chars.get(i) == Some(&',') {
                i += 1;
                time = Some(number(&mut i)? / 100.0);
            }
            nodes.push(node);
            times.push(time);
        }

        // Spread the nodes without a time between those with one.
        let n = nodes.len();
        if n > 0 {
            if times[0].is_none() {
                times[0] = Some(0.0);
            }
            if n > 1 && times[n - 1].is_none() {
                times[n - 1] = Some(1.0);
            }
            let mut prev = 0;
            for next in 1..n {
                if let Some(t1) = times[next] {
                    let t0 = times[prev].unwrap();
                    for (k, time) in times.iter_mut().enumerate().take(next).skip(prev + 1) {
                        *time = Some(t0 + (t1 - t0) * (k - prev) as f32 / (next - prev) as f32);
                    }
                    prev = next;
                }
            }
        }
        for (node, time) in nodes.iter_mut().zip(times) {
            node.time = time.unwrap_or(0.0);
        }
        Some(Self { nodes })
    }

    /// `FloatTrackEvaluate`: the value at `time` (0..=1) for a random spread
    /// position `interp` (0..1).
    pub fn evaluate(&self, time: f32, interp: f32) -> f32 {
        let Some(first) = self.nodes.first() else { return 0.0 };
        let value = |n: &FloatTrackNode| curve_evaluate(interp, n.low, n.high, n.distribution);
        if time < first.time {
            return value(first);
        }
        for pair in self.nodes.windows(2) {
            let (cur, next) = (&pair[0], &pair[1]);
            if time <= next.time {
                let fraction = (time - cur.time) / (next.time - cur.time);
                return curve_evaluate(fraction, value(cur), value(next), cur.curve);
            }
        }
        value(self.nodes.last().unwrap())
    }

    /// `FloatTrackEvaluateFromLastTime`: 0 before the first update.
    pub fn evaluate_from_last_time(&self, time: f32, interp: f32) -> f32 {
        if time < 0.0 { 0.0 } else { self.evaluate(time, interp) }
    }

    /// `FloatTrackIsSet`: given in the definition (not a default).
    pub fn is_set(&self) -> bool {
        self.nodes.first().is_some_and(|n| n.curve != TodCurve::Constant)
    }

    /// `FloatTrackIsConstantZero`.
    pub fn is_constant_zero(&self) -> bool {
        match &self.nodes[..] {
            [] => true,
            [n] => n.low == 0.0 && n.high == 0.0,
            _ => false,
        }
    }

    /// `FloatTrackSetDefault`: a constant for tracks the definition leaves out.
    fn set_default(&mut self, value: f32) {
        if self.nodes.is_empty() && value != 0.0 {
            self.nodes.push(FloatTrackNode {
                time: 0.0,
                low: value,
                high: value,
                curve: TodCurve::Constant,
                distribution: TodCurve::Linear,
            });
        }
    }
}

/// `ParticleFlags`, as bit numbers.
pub mod flags {
    pub const RANDOM_LAUNCH_SPIN: u32 = 1 << 0;
    pub const ALIGN_LAUNCH_SPIN: u32 = 1 << 1;
    pub const ALIGN_TO_PIXELS: u32 = 1 << 2;
    pub const SYSTEM_LOOPS: u32 = 1 << 3;
    pub const PARTICLE_LOOPS: u32 = 1 << 4;
    pub const PARTICLES_DONT_FOLLOW: u32 = 1 << 5;
    pub const RANDOM_START_TIME: u32 = 1 << 6;
    pub const DIE_IF_OVERLOADED: u32 = 1 << 7;
    pub const ADDITIVE: u32 = 1 << 8;
    pub const FULLSCREEN: u32 = 1 << 9;
    pub const SOFTWARE_ONLY: u32 = 1 << 10;
    pub const HARDWARE_ONLY: u32 = 1 << 11;
}

/// Flag element names (`gParticleFlagSymbols`).
const FLAG_NAMES: [(&str, u32); 12] = [
    ("RandomLaunchSpin", 0),
    ("AlignLaunchSpin", 1),
    ("AlignToPixel", 2),
    ("ParticleLoops", 4),
    ("SystemLoops", 3),
    ("ParticlesDontFollow", 5),
    ("RandomStartTime", 6),
    ("DieIfOverloaded", 7),
    ("Additive", 8),
    ("FullScreen", 9),
    ("SoftwareOnly", 10),
    ("HardwareOnly", 11),
];

/// `EmitterType`: where particles start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitterType {
    Circle,
    Box,
    BoxPath,
    CirclePath,
    CircleEvenSpacing,
}

impl EmitterType {
    fn from_index(i: u32) -> Option<Self> {
        [Self::Circle, Self::Box, Self::BoxPath, Self::CirclePath, Self::CircleEvenSpacing].get(i as usize).copied()
    }

    fn from_name(name: &str) -> Option<Self> {
        ["Circle", "Box", "BoxPath", "CirclePath", "CircleEvenSpacing"]
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))
            .and_then(|i| Self::from_index(i as u32))
    }
}

/// `ParticleFieldType`: a force or motion applied every tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Invalid,
    Friction,
    Acceleration,
    Attractor,
    MaxVelocity,
    Velocity,
    Position,
    SystemPosition,
    GroundConstraint,
    Shake,
    Circle,
    Away,
}

impl FieldType {
    const ALL: [Self; 12] = [
        Self::Invalid,
        Self::Friction,
        Self::Acceleration,
        Self::Attractor,
        Self::MaxVelocity,
        Self::Velocity,
        Self::Position,
        Self::SystemPosition,
        Self::GroundConstraint,
        Self::Shake,
        Self::Circle,
        Self::Away,
    ];

    fn from_name(name: &str) -> Option<Self> {
        let names = [
            "Friction",
            "Acceleration",
            "Attractor",
            "MaxVelocity",
            "Velocity",
            "Position",
            "SystemPosition",
            "GroundConstraint",
            "Shake",
            "Circle",
            "Away",
        ];
        names.iter().position(|n| n.eq_ignore_ascii_case(name)).map(|i| Self::ALL[i + 1])
    }
}

/// `ParticleField`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleField {
    pub field_type: FieldType,
    pub x: FloatTrack,
    pub y: FloatTrack,
}

/// `TodEmitterDefinition`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EmitterDefinition {
    /// Image id (`IMAGE_...`), if any.
    pub image: Option<String>,
    pub image_col: i32,
    pub image_row: i32,
    pub image_frames: i32,
    pub animated: bool,
    pub flags: u32,
    pub emitter_type: Option<EmitterType>,
    pub name: String,
    /// Emitter to cross-fade particles to when they end.
    pub on_duration: String,
    pub system_duration: FloatTrack,
    pub cross_fade_duration: FloatTrack,
    pub spawn_rate: FloatTrack,
    pub spawn_min_active: FloatTrack,
    pub spawn_max_active: FloatTrack,
    pub spawn_max_launched: FloatTrack,
    pub emitter_radius: FloatTrack,
    pub emitter_offset_x: FloatTrack,
    pub emitter_offset_y: FloatTrack,
    pub emitter_box_x: FloatTrack,
    pub emitter_box_y: FloatTrack,
    pub emitter_skew_x: FloatTrack,
    pub emitter_skew_y: FloatTrack,
    pub emitter_path: FloatTrack,
    pub particle_duration: FloatTrack,
    pub launch_speed: FloatTrack,
    pub launch_angle: FloatTrack,
    pub system_red: FloatTrack,
    pub system_green: FloatTrack,
    pub system_blue: FloatTrack,
    pub system_alpha: FloatTrack,
    pub system_brightness: FloatTrack,
    pub particle_fields: Vec<ParticleField>,
    pub system_fields: Vec<ParticleField>,
    pub particle_red: FloatTrack,
    pub particle_green: FloatTrack,
    pub particle_blue: FloatTrack,
    pub particle_alpha: FloatTrack,
    pub particle_brightness: FloatTrack,
    pub particle_spin_angle: FloatTrack,
    pub particle_spin_speed: FloatTrack,
    pub particle_scale: FloatTrack,
    pub particle_stretch: FloatTrack,
    pub collision_reflect: FloatTrack,
    pub collision_spin: FloatTrack,
    pub clip_top: FloatTrack,
    pub clip_bottom: FloatTrack,
    pub clip_left: FloatTrack,
    pub clip_right: FloatTrack,
    pub animation_rate: FloatTrack,
}

/// The emitter's float tracks in the original's field-table order, with
/// their XML names (`gEmitterDefFields`).
macro_rules! emitter_tracks {
    ($def:expr) => {
        [
            ("SystemDuration", &mut $def.system_duration),
            ("CrossFadeDuration", &mut $def.cross_fade_duration),
            ("SpawnRate", &mut $def.spawn_rate),
            ("SpawnMinActive", &mut $def.spawn_min_active),
            ("SpawnMaxActive", &mut $def.spawn_max_active),
            ("SpawnMaxLaunched", &mut $def.spawn_max_launched),
            ("EmitterRadius", &mut $def.emitter_radius),
            ("EmitterOffsetX", &mut $def.emitter_offset_x),
            ("EmitterOffsetY", &mut $def.emitter_offset_y),
            ("EmitterBoxX", &mut $def.emitter_box_x),
            ("EmitterBoxY", &mut $def.emitter_box_y),
            ("EmitterPath", &mut $def.emitter_path),
            ("EmitterSkewX", &mut $def.emitter_skew_x),
            ("EmitterSkewY", &mut $def.emitter_skew_y),
            ("ParticleDuration", &mut $def.particle_duration),
            ("SystemRed", &mut $def.system_red),
            ("SystemGreen", &mut $def.system_green),
            ("SystemBlue", &mut $def.system_blue),
            ("SystemAlpha", &mut $def.system_alpha),
            ("SystemBrightness", &mut $def.system_brightness),
            ("LaunchSpeed", &mut $def.launch_speed),
            ("LaunchAngle", &mut $def.launch_angle),
            ("ParticleRed", &mut $def.particle_red),
            ("ParticleGreen", &mut $def.particle_green),
            ("ParticleBlue", &mut $def.particle_blue),
            ("ParticleAlpha", &mut $def.particle_alpha),
            ("ParticleBrightness", &mut $def.particle_brightness),
            ("ParticleSpinAngle", &mut $def.particle_spin_angle),
            ("ParticleSpinSpeed", &mut $def.particle_spin_speed),
            ("ParticleScale", &mut $def.particle_scale),
            ("ParticleStretch", &mut $def.particle_stretch),
            ("CollisionReflect", &mut $def.collision_reflect),
            ("CollisionSpin", &mut $def.collision_spin),
            ("ClipTop", &mut $def.clip_top),
            ("ClipBottom", &mut $def.clip_bottom),
            ("ClipLeft", &mut $def.clip_left),
            ("ClipRight", &mut $def.clip_right),
            ("AnimationRate", &mut $def.animation_rate),
        ]
    };
}

impl EmitterDefinition {
    fn new() -> Self {
        Self { image_frames: 1, emitter_type: Some(EmitterType::Box), ..Default::default() }
    }

    /// `TodParticleLoadADef`'s defaults for tracks left out.
    fn set_defaults(&mut self) {
        let defaults = [
            (&mut self.spawn_min_active, -1.0),
            (&mut self.spawn_max_active, -1.0),
            (&mut self.spawn_max_launched, -1.0),
            (&mut self.particle_duration, 100.0),
            (&mut self.system_red, 1.0),
            (&mut self.system_green, 1.0),
            (&mut self.system_blue, 1.0),
            (&mut self.system_alpha, 1.0),
            (&mut self.system_brightness, 1.0),
            (&mut self.particle_red, 1.0),
            (&mut self.particle_green, 1.0),
            (&mut self.particle_blue, 1.0),
            (&mut self.particle_alpha, 1.0),
            (&mut self.particle_brightness, 1.0),
            (&mut self.particle_scale, 1.0),
            (&mut self.particle_stretch, 1.0),
        ];
        for (track, value) in defaults {
            track.set_default(value);
        }
    }

    pub fn has_flag(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}

/// `TodParticleDefinition`: one or more emitters.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParticleDefinition {
    pub emitters: Vec<EmitterDefinition>,
}

impl ParticleDefinition {
    /// Loads `path` (e.g. `particles/Planting.xml`), preferring the compiled
    /// `compiled/<path>.compiled` when present, like the original.
    pub fn load(files: &impl crate::files::Files, path: &str) -> Result<Self, String> {
        if let Some(bytes) = files.read(&format!("compiled/{path}.compiled")) {
            return Self::parse_compiled(&bytes).map_err(|e| e.to_string());
        }
        let bytes = files.read(path).ok_or_else(|| "missing".to_owned())?;
        Self::parse(&crate::font::latin1_to_string(&bytes)).map_err(|e| e.to_string())
    }

    /// Every image the emitters use.
    pub fn image_ids(&self) -> impl Iterator<Item = &str> {
        self.emitters.iter().filter_map(|e| e.image.as_deref())
    }

    /// Parses `particles/*.xml`: top-level `<Emitter>` elements.
    pub fn parse(text: &str) -> Result<Self, ParticleError> {
        let wrapped = format!("<particle>{text}</particle>");
        let doc = roxmltree::Document::parse(&wrapped).map_err(|e| ParticleError::Xml(e.to_string()))?;
        let mut def = Self::default();
        for el in doc.root_element().children().filter(|n| n.is_element()) {
            if el.tag_name().name().eq_ignore_ascii_case("Emitter") {
                def.emitters.push(parse_emitter(&el)?);
            }
        }
        Ok(def)
    }

    /// Parses `compiled/particles/*.xml.compiled`.
    pub fn parse_compiled(bytes: &[u8]) -> Result<Self, ParticleError> {
        const EMITTER_SIZE: usize = 0x164;
        const FIELD_SIZE: usize = 0x14;
        let data = decompress(bytes)?;
        let mut r = CompiledReader::new(&data);
        r.u32()?; // schema hash
        let raw = r.skip(8)?;
        let count = u32_at(raw, 4) as usize;
        let mut def = Self::default();
        if r.u32()? as usize != EMITTER_SIZE {
            return Err(ParticleError::Compiled("emitter size mismatch"));
        }
        if count > 0 {
            let raw = r.skip(EMITTER_SIZE * count)?;
            for i in 0..count {
                let raw = &raw[EMITTER_SIZE * i..EMITTER_SIZE * (i + 1)];
                let mut e = EmitterDefinition::new();
                e.image_col = u32_at(raw, 0x4) as i32;
                e.image_row = u32_at(raw, 0x8) as i32;
                e.image_frames = u32_at(raw, 0xC) as i32;
                e.animated = u32_at(raw, 0x10) != 0;
                e.flags = u32_at(raw, 0x14);
                e.emitter_type = EmitterType::from_index(u32_at(raw, 0x18));
                let image = r.string()?;
                e.image = (!image.is_empty()).then_some(image);
                e.name = r.string()?;
                // DefMapReadFromCache: strings, tracks and arrays in field-table order.
                e.system_duration = read_track(&mut r)?;
                e.on_duration = r.string()?;
                let fields_count = u32_at(raw, 0xD8) as usize;
                let system_fields_count = u32_at(raw, 0xE0) as usize;
                let mut tracks = emitter_tracks!(e);
                for (name, track) in tracks.iter_mut().skip(1) {
                    **track = read_track(&mut r)?;
                    if *name == "LaunchAngle" {
                        // Field and SystemField come right after LaunchAngle.
                        break;
                    }
                }
                e.particle_fields = read_fields(&mut r, fields_count, FIELD_SIZE)?;
                e.system_fields = read_fields(&mut r, system_fields_count, FIELD_SIZE)?;
                let mut tracks = emitter_tracks!(e);
                for (_, track) in tracks.iter_mut().skip_while(|(n, _)| *n != "ParticleRed") {
                    **track = read_track(&mut r)?;
                }
                e.set_defaults();
                def.emitters.push(e);
            }
        }
        if !r.at_end() {
            return Err(ParticleError::Compiled("trailing data"));
        }
        Ok(def)
    }
}

/// `DefReadFromCacheFloatTrack`.
fn read_track(r: &mut CompiledReader) -> Result<FloatTrack, ParticleError> {
    let count = r.u32()? as usize;
    let mut nodes = Vec::with_capacity(count);
    for _ in 0..count {
        let (time, low, high) = (r.f32()?, r.f32()?, r.f32()?);
        let curve = TodCurve::from_index(r.u32()?).ok_or(ParticleError::Compiled("bad curve"))?;
        let distribution = TodCurve::from_index(r.u32()?).ok_or(ParticleError::Compiled("bad curve"))?;
        nodes.push(FloatTrackNode { time, low, high, curve, distribution });
    }
    Ok(FloatTrack { nodes })
}

/// `DefReadFromCacheArray` of `ParticleField`s.
fn read_fields(r: &mut CompiledReader, count: usize, size: usize) -> Result<Vec<ParticleField>, ParticleError> {
    if r.u32()? as usize != size {
        return Err(ParticleError::Compiled("field size mismatch"));
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let raw = r.skip(size * count)?;
    let mut fields = Vec::with_capacity(count);
    for i in 0..count {
        let field_type = FieldType::ALL.get(u32_at(raw, size * i) as usize).copied().unwrap_or(FieldType::Invalid);
        let x = read_track(r)?;
        let y = read_track(r)?;
        fields.push(ParticleField { field_type, x, y });
    }
    Ok(fields)
}

fn element_text(el: &roxmltree::Node) -> String {
    el.text().unwrap_or_default().trim().to_owned()
}

fn bad(el: &roxmltree::Node) -> ParticleError {
    ParticleError::BadValue { element: el.tag_name().name().to_owned(), value: element_text(el) }
}

fn parse_track(el: &roxmltree::Node) -> Result<FloatTrack, ParticleError> {
    FloatTrack::parse(&element_text(el)).ok_or_else(|| bad(el))
}

fn parse_field(el: &roxmltree::Node) -> Result<ParticleField, ParticleError> {
    let mut field = ParticleField { field_type: FieldType::Invalid, x: FloatTrack::default(), y: FloatTrack::default() };
    for child in el.children().filter(|n| n.is_element()) {
        let name = child.tag_name().name();
        if name.eq_ignore_ascii_case("FieldType") {
            field.field_type = FieldType::from_name(&element_text(&child)).ok_or_else(|| bad(&child))?;
        } else if name.eq_ignore_ascii_case("x") {
            field.x = parse_track(&child)?;
        } else if name.eq_ignore_ascii_case("y") {
            field.y = parse_track(&child)?;
        }
    }
    Ok(field)
}

/// `DefinitionReadField` for one `<Emitter>`.
fn parse_emitter(el: &roxmltree::Node) -> Result<EmitterDefinition, ParticleError> {
    let mut e = EmitterDefinition::new();
    for child in el.children().filter(|n| n.is_element()) {
        let name = child.tag_name().name();
        let text = element_text(&child);
        let int = || text.parse::<f32>().map(|v| v as i32).map_err(|_| bad(&child));
        let is = |n: &str| name.eq_ignore_ascii_case(n);
        if is("Image") {
            e.image = (!text.is_empty()).then_some(text.clone());
        } else if is("ImageRow") {
            e.image_row = int()?;
        } else if is("ImageCol") {
            e.image_col = int()?;
        } else if is("ImageFrames") {
            e.image_frames = int()?;
        } else if is("Animated") {
            e.animated = int()? != 0;
        } else if is("EmitterType") {
            e.emitter_type = Some(EmitterType::from_name(&text).ok_or_else(|| bad(&child))?);
        } else if is("Name") {
            e.name = text;
        } else if is("OnDuration") {
            e.on_duration = text;
        } else if is("Field") {
            e.particle_fields.push(parse_field(&child)?);
        } else if is("SystemField") {
            e.system_fields.push(parse_field(&child)?);
        } else if let Some(&(_, bit)) = FLAG_NAMES.iter().find(|(n, _)| is(n)) {
            if int()? != 0 {
                e.flags |= 1 << bit;
            } else {
                e.flags &= !(1 << bit);
            }
        } else {
            let mut tracks = emitter_tracks!(e);
            match tracks.iter_mut().find(|(n, _)| is(n)) {
                Some((_, track)) => **track = parse_track(&child)?,
                None => return Err(bad(&child)),
            }
        }
    }
    e.set_defaults();
    Ok(e)
}

/// Max fields per emitter (`MAX_PARTICLE_FIELDS`).
const MAX_FIELDS: usize = 4;

/// `ParticleSystemTracks`: per-system random spread slots.
mod system_track {
    pub const SPAWN_RATE: usize = 0;
    pub const SPAWN_MIN_ACTIVE: usize = 1;
    pub const SPAWN_MAX_ACTIVE: usize = 2;
    pub const SPAWN_MAX_LAUNCHED: usize = 3;
    pub const EMITTER_PATH: usize = 4;
    pub const RED: usize = 5;
    pub const GREEN: usize = 6;
    pub const BLUE: usize = 7;
    pub const ALPHA: usize = 8;
    pub const BRIGHTNESS: usize = 9;
    pub const COUNT: usize = 10;
}

/// `ParticleTracks`: per-particle random spread slots.
mod particle_track {
    pub const RED: usize = 0;
    pub const GREEN: usize = 1;
    pub const BLUE: usize = 2;
    pub const ALPHA: usize = 3;
    pub const BRIGHTNESS: usize = 4;
    pub const SPIN_SPEED: usize = 5;
    pub const SPIN_ANGLE: usize = 6;
    pub const SCALE: usize = 7;
    pub const STRETCH: usize = 8;
    pub const COLLISION_REFLECT: usize = 9;
    pub const COLLISION_SPIN: usize = 10;
    pub const CLIP_TOP: usize = 11;
    pub const CLIP_BOTTOM: usize = 12;
    pub const CLIP_LEFT: usize = 13;
    pub const CLIP_RIGHT: usize = 14;
    pub const ANIMATION_RATE: usize = 15;
    pub const COUNT: usize = 16;
}

/// `TodParticle`.
#[derive(Debug, Clone)]
pub struct Particle {
    /// Index of its emitter in the system.
    pub emitter: usize,
    pub duration: i32,
    pub age: i32,
    pub time_value: f32,
    last_time_value: f32,
    animation_time_value: f32,
    pub velocity: (f32, f32),
    pub position: (f32, f32),
    pub image_frame: i32,
    pub spin_position: f32,
    spin_velocity: f32,
    /// The particle this one is cross-fading from.
    cross_fade_particle: Option<ParticleId>,
    cross_fade_duration: i32,
    interp: [f32; particle_track::COUNT],
    field_interp: [[f32; 2]; MAX_FIELDS],
}

pub type ParticleId = Id<Particle>;

/// `TodParticleEmitter`.
#[derive(Debug, Clone)]
pub struct Emitter {
    /// Index into the definition's emitters.
    pub def: usize,
    /// Newest first, like the original's list.
    particles: Vec<ParticleId>,
    spawn_accum: f32,
    pub system_center: (f32, f32),
    particles_spawned: i32,
    system_age: i32,
    system_duration: i32,
    system_time_value: f32,
    system_last_time_value: f32,
    pub dead: bool,
    /// 0..=255 per channel.
    pub color_override: [f32; 4],
    pub scale_override: f32,
    pub image_override: Option<String>,
    pub frame_override: Option<i32>,
    cross_fade_emitter: Option<usize>,
    emitter_cross_fade_count_down: i32,
    track_interp: [f32; system_track::COUNT],
    system_field_interp: [[f32; 2]; MAX_FIELDS],
}

/// How to draw one particle (`ParticleRenderParams` plus `RenderParticle`'s
/// choice of image and cel).
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleDraw {
    pub image: String,
    /// Cel column and row of the image.
    pub col: i32,
    pub row: i32,
    /// Part of the cel to show, as fractions clipped off each side.
    pub clip: [f32; 4],
    pub x: f32,
    pub y: f32,
    pub spin: f32,
    pub scale: f32,
    pub stretch: f32,
    /// 0..=255 per channel, already clamped.
    pub color: [u8; 4],
    pub additive: bool,
    pub extra_additive: bool,
    pub align_to_pixels: bool,
    pub fullscreen: bool,
}

/// `MAX_PARTICLES_SIZE`: past this many systems, emitters or particles, an
/// emitter flagged `DieIfOverloaded` refuses to start.
pub const MAX_PARTICLES_SIZE: usize = 900;

/// `TodParticleSystem`: a running effect.
pub struct ParticleSystem {
    pub definition: Arc<ParticleDefinition>,
    pub emitters: Vec<Emitter>,
    particles: DataArray<Particle>,
    pub dead: bool,
    pub is_attachment: bool,
    pub render_order: i32,
    pub dont_update: bool,
}

impl ParticleSystem {
    /// `TodParticleInitializeFromDef`. `overloaded` is the holder's
    /// `IsOverLoaded`: an emitter flagged `DieIfOverloaded` then kills the
    /// whole effect instead of adding itself.
    pub fn new(
        x: f32,
        y: f32,
        render_order: i32,
        definition: Arc<ParticleDefinition>,
        rng: &mut MtRand,
        overloaded: bool,
    ) -> Self {
        let mut system = Self {
            definition,
            emitters: Vec::new(),
            particles: DataArray::default(),
            dead: false,
            is_attachment: false,
            render_order,
            dont_update: false,
        };
        let definition = system.definition.clone();
        for i in 0..definition.emitters.len() {
            if !definition.emitters[i].cross_fade_duration.is_set() {
                if overloaded && definition.emitters[i].has_flag(flags::DIE_IF_OVERLOADED) {
                    system.dead = true;
                    break;
                }
                system.add_emitter(x, y, i, rng);
            }
        }
        system
    }

    /// How many particles are alive right now (`TodParticleHolder`'s count).
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    /// `TodEmitterInitialize` for a new emitter.
    fn add_emitter(&mut self, x: f32, y: f32, def_index: usize, rng: &mut MtRand) -> usize {
        let def = &self.definition.emitters[def_index];
        let mut system_duration = if def.system_duration.is_set() {
            def.system_duration.evaluate(0.0, rng.next_f32(1.0)) as i32
        } else {
            def.particle_duration.evaluate(0.0, 1.0) as i32
        };
        system_duration = system_duration.max(1);
        let mut system_field_interp = [[0.0; 2]; MAX_FIELDS];
        for interp in system_field_interp.iter_mut().take(def.system_fields.len()) {
            *interp = [rng.next_f32(1.0), rng.next_f32(1.0)];
        }
        let mut track_interp = [0.0; system_track::COUNT];
        for t in &mut track_interp {
            *t = rng.next_f32(1.0);
        }
        self.emitters.push(Emitter {
            def: def_index,
            particles: Vec::new(),
            spawn_accum: 0.0,
            system_center: (x, y),
            particles_spawned: 0,
            system_age: -1,
            system_duration,
            system_time_value: -1.0,
            system_last_time_value: -1.0,
            dead: false,
            color_override: [255.0; 4],
            scale_override: 1.0,
            image_override: None,
            frame_override: None,
            cross_fade_emitter: None,
            emitter_cross_fade_count_down: 0,
            track_interp,
            system_field_interp,
        });
        let index = self.emitters.len() - 1;
        self.update_emitter(index, rng);
        index
    }

    /// `ParticleSystemDie`.
    pub fn die(&mut self) {
        self.emitters.clear();
        self.particles.clear();
        self.dead = true;
    }

    /// `TodParticleSystem::Update`: one 100 Hz tick.
    pub fn update(&mut self, rng: &mut MtRand) {
        if self.dont_update || self.dead {
            return;
        }
        let mut alive = false;
        for i in 0..self.emitters.len() {
            self.update_emitter(i, rng);
            let e = &self.emitters[i];
            let def = &self.definition.emitters[e.def];
            if (def.cross_fade_duration.is_set() && !e.particles.is_empty()) || !e.dead {
                alive = true;
            }
        }
        if !alive {
            self.dead = true;
        }
    }

    /// `TodParticleSystem::SystemMove`: moves the effect (and its particles,
    /// unless they're set not to follow).
    pub fn system_move(&mut self, x: f32, y: f32) {
        for e in &mut self.emitters {
            let (dx, dy) = (x - e.system_center.0, y - e.system_center.1);
            if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
                continue;
            }
            e.system_center = (x, y);
            if !self.definition.emitters[e.def].has_flag(flags::PARTICLES_DONT_FOLLOW) {
                for &id in &e.particles {
                    if let Some(p) = self.particles.get_mut(id) {
                        p.position.0 += dx;
                        p.position.1 += dy;
                    }
                }
            }
        }
    }

    /// `OverrideColor` (`None` for every emitter); 0..=255 per channel.
    pub fn override_color(&mut self, emitter: Option<&str>, color: [u8; 4]) {
        for e in self.emitters_named(emitter) {
            e.color_override = color.map(|c| c as f32);
        }
    }

    /// `OverrideScale`.
    pub fn override_scale(&mut self, emitter: Option<&str>, scale: f32) {
        for e in self.emitters_named(emitter) {
            e.scale_override = scale;
        }
    }

    /// `OverrideImage`.
    pub fn override_image(&mut self, emitter: Option<&str>, image: Option<&str>) {
        for e in self.emitters_named(emitter) {
            e.image_override = image.map(str::to_owned);
        }
    }

    /// `OverrideFrame`.
    pub fn override_frame(&mut self, emitter: Option<&str>, frame: i32) {
        for e in self.emitters_named(emitter) {
            e.frame_override = Some(frame);
        }
    }

    fn emitters_named<'a>(&'a mut self, name: Option<&'a str>) -> impl Iterator<Item = &'a mut Emitter> + 'a {
        let defs = &self.definition.emitters;
        self.emitters
            .iter_mut()
            .filter(move |e| name.is_none_or(|n| defs[e.def].name.eq_ignore_ascii_case(n)))
    }

    fn find_emitter_def(&self, name: &str) -> Option<usize> {
        self.definition.emitters.iter().position(|d| d.name.eq_ignore_ascii_case(name))
    }

    /// `TodParticleSystem::CrossFade`: fades every emitter into the named one.
    pub fn cross_fade(&mut self, emitter_name: &str, rng: &mut MtRand) {
        let Some(def_index) = self.find_emitter_def(emitter_name) else { return };
        if !self.definition.emitters[def_index].cross_fade_duration.is_set() {
            return;
        }
        for i in 0..self.emitters.len() {
            if self.emitters[i].def != def_index {
                let (x, y) = self.emitters[i].system_center;
                let to = self.add_emitter(x, y, def_index, rng);
                self.cross_fade_emitter(i, to, rng);
            }
        }
    }

    /// `TodParticleEmitter::CrossFadeEmitter`.
    fn cross_fade_emitter(&mut self, from: usize, to: usize, rng: &mut MtRand) {
        if self.emitters[from].emitter_cross_fade_count_down > 0 {
            return;
        }
        let def = Arc::clone(&self.definition);
        let to_def = &def.emitters[self.emitters[to].def];
        if !to_def.cross_fade_duration.is_set() {
            return;
        }
        let duration = to_def.cross_fade_duration.evaluate(self.emitters[from].system_time_value, rng.next_f32(1.0));
        let duration = (duration as i32).max(1);
        let system_duration_set = to_def.system_duration.is_set();
        let e = &mut self.emitters[from];
        e.emitter_cross_fade_count_down = duration;
        e.cross_fade_emitter = Some(to);
        if !system_duration_set {
            self.emitters[to].system_duration = duration;
        }
        let particles = self.emitters[from].particles.clone();
        for id in particles {
            self.cross_fade_particle(from, id, to, rng);
        }
    }

    /// `TodParticleEmitter::CrossFadeParticle`: a particle in `to` takes over
    /// from `particle`, which fades out meanwhile.
    fn cross_fade_particle(&mut self, from: usize, particle: ParticleId, to: usize, rng: &mut MtRand) -> bool {
        if self.particles.get(particle).is_none_or(|p| p.cross_fade_duration > 0) {
            return false;
        }
        let def = Arc::clone(&self.definition);
        let to_def = &def.emitters[self.emitters[to].def];
        if !to_def.cross_fade_duration.is_set() || to == from {
            return false;
        }
        let Some(to_particle) = self.spawn_particle(to, 0, 1, rng) else { return false };
        let duration = if self.emitters[from].emitter_cross_fade_count_down > 0 {
            self.emitters[from].emitter_cross_fade_count_down
        } else {
            let d = to_def.cross_fade_duration.evaluate(self.emitters[from].system_time_value, rng.next_f32(1.0));
            (d as i32).max(1)
        };
        let particle_duration_set = to_def.particle_duration.is_set();
        if let Some(p) = self.particles.get_mut(particle) {
            p.cross_fade_duration = duration;
        }
        if let Some(tp) = self.particles.get_mut(to_particle) {
            if !particle_duration_set {
                tp.duration = duration;
            }
            tp.cross_fade_particle = Some(particle);
        }
        true
    }

    /// `TodParticleEmitter::Update`.
    fn update_emitter(&mut self, index: usize, rng: &mut MtRand) {
        let def = Arc::clone(&self.definition);
        let e = &mut self.emitters[index];
        if e.dead {
            return;
        }
        let edef = &def.emitters[e.def];
        e.system_age += 1;
        let mut die = false;
        if e.system_age >= e.system_duration {
            if edef.has_flag(flags::SYSTEM_LOOPS) {
                e.system_age = 0;
            } else {
                e.system_age = e.system_duration - 1;
                die = true;
            }
        }
        if e.emitter_cross_fade_count_down > 0 {
            e.emitter_cross_fade_count_down -= 1;
            if e.emitter_cross_fade_count_down == 0 {
                die = true;
            }
        }
        if let Some(to) = e.cross_fade_emitter
            && self.emitters.get(to).is_none_or(|t| t.dead)
        {
            die = true;
        }

        let e = &mut self.emitters[index];
        e.system_time_value = e.system_age as f32 / (e.system_duration - 1) as f32;
        // UpdateSystemField: only SystemPosition moves the emitter.
        for (i, field) in edef.system_fields.iter().enumerate().take(MAX_FIELDS) {
            if field.field_type == FieldType::SystemPosition {
                let [ix, iy] = e.system_field_interp[i];
                let x = field.x.evaluate(e.system_time_value, ix);
                let y = field.y.evaluate(e.system_time_value, iy);
                let last_x = field.x.evaluate_from_last_time(e.system_last_time_value, ix);
                let last_y = field.y.evaluate_from_last_time(e.system_last_time_value, iy);
                e.system_center.0 += x - last_x;
                e.system_center.1 += y - last_y;
            }
        }
        let particles = e.particles.clone();
        for id in particles {
            if !self.update_particle(id, rng) {
                self.delete_particle(id);
            }
        }
        self.update_spawning(index, rng);

        let e = &mut self.emitters[index];
        if die {
            // DeleteNonCrossFading.
            let ids = e.particles.clone();
            for id in ids {
                if self.particles.get(id).is_some_and(|p| p.cross_fade_duration <= 0) {
                    self.delete_particle(id);
                }
            }
            let e = &mut self.emitters[index];
            if e.particles.is_empty() {
                e.dead = true;
                return;
            }
        }
        let e = &mut self.emitters[index];
        e.system_last_time_value = e.system_time_value;
    }

    /// `TodParticleEmitter::SystemTrackEvaluate`.
    fn system_track(&self, emitter: usize, track: &FloatTrack, slot: usize) -> f32 {
        let e = &self.emitters[emitter];
        track.evaluate(e.system_time_value, e.track_interp[slot])
    }

    /// `TodParticleEmitter::UpdateSpawning`.
    fn update_spawning(&mut self, index: usize, rng: &mut MtRand) {
        let def = Arc::clone(&self.definition);
        let spawning = self.emitters[index].cross_fade_emitter.filter(|&i| i < self.emitters.len()).unwrap_or(index);
        let sdef = &def.emitters[self.emitters[spawning].def];
        let rate = self.system_track(spawning, &sdef.spawn_rate, system_track::SPAWN_RATE) * 0.01;
        let e = &mut self.emitters[index];
        e.spawn_accum += rate;
        let mut count = e.spawn_accum as i32;
        e.spawn_accum -= count as f32;
        let active = self.emitters[index].particles.len() as i32;

        let min_active = self.system_track(spawning, &sdef.spawn_min_active, system_track::SPAWN_MIN_ACTIVE) as i32;
        if min_active >= 0 && count < min_active - active {
            count = min_active - active;
        }
        let max_active = self.system_track(spawning, &sdef.spawn_max_active, system_track::SPAWN_MAX_ACTIVE) as i32;
        if max_active >= 0 && count > max_active - active {
            count = max_active - active;
        }
        if sdef.spawn_max_launched.is_set() {
            let max = self.system_track(spawning, &sdef.spawn_max_launched, system_track::SPAWN_MAX_LAUNCHED) as i32;
            let spawned = self.emitters[index].particles_spawned;
            if count > max - spawned {
                count = max - spawned;
            }
        }
        let cross_fade = self.emitters[index].cross_fade_emitter;
        for i in 0..count.max(0) {
            let particle = self.spawn_particle(index, i, count, rng);
            if let (Some(to), Some(p)) = (cross_fade, particle) {
                self.cross_fade_particle(index, p, to, rng);
            }
        }
    }

    /// `TodParticleEmitter::SpawnParticle`.
    fn spawn_particle(&mut self, index: usize, spawn_index: i32, spawn_count: i32, rng: &mut MtRand) -> Option<ParticleId> {
        use std::f32::consts::PI;
        let def = Arc::clone(&self.definition);
        let e = &self.emitters[index];
        let edef = &def.emitters[e.def];
        let t = e.system_time_value;

        let mut field_interp = [[0.0; 2]; MAX_FIELDS];
        for interp in field_interp.iter_mut().take(edef.particle_fields.len()) {
            *interp = [rng.next_f32(1.0), rng.next_f32(1.0)];
        }
        let mut interp = [0.0; particle_track::COUNT];
        for v in &mut interp {
            *v = rng.next_f32(1.0);
        }
        let duration_interp = rng.next_f32(1.0);
        let launch_speed_interp = rng.next_f32(1.0);
        let offset_x_interp = rng.next_f32(1.0);
        let offset_y_interp = rng.next_f32(1.0);
        let duration = (edef.particle_duration.evaluate(t, duration_interp) as i32).max(1);
        let mut age = 0;
        if edef.has_flag(flags::RANDOM_START_TIME) {
            age = rng.next_range(duration as u32) as i32;
        }
        let launch_speed = edef.launch_speed.evaluate(t, launch_speed_interp) * 0.01;
        let launch_angle_interp = rng.next_f32(1.0);

        let ty = edef.emitter_type.unwrap_or(EmitterType::Box);
        let launch_angle = match ty {
            EmitterType::CirclePath => {
                let path = edef.emitter_path.evaluate(t, e.track_interp[system_track::EMITTER_PATH]) * 2.0 * PI;
                path + edef.launch_angle.evaluate(t, launch_angle_interp).to_radians()
            }
            EmitterType::CircleEvenSpacing => {
                2.0 * PI * spawn_index as f32 / spawn_count as f32
                    + edef.launch_angle.evaluate(t, launch_angle_interp).to_radians()
            }
            _ if edef.launch_angle.is_constant_zero() => rng.next_f32(2.0 * PI),
            _ => edef.launch_angle.evaluate(t, launch_angle_interp).to_radians(),
        };

        let (pos_x, pos_y) = match ty {
            EmitterType::Circle | EmitterType::CirclePath | EmitterType::CircleEvenSpacing => {
                let radius = edef.emitter_radius.evaluate(t, rng.next_f32(1.0));
                // Angle 0 points straight down.
                (launch_angle.sin() * radius, launch_angle.cos() * radius)
            }
            EmitterType::Box => {
                let (ix, iy) = (rng.next_f32(1.0), rng.next_f32(1.0));
                (edef.emitter_box_x.evaluate(t, ix), edef.emitter_box_y.evaluate(t, iy))
            }
            EmitterType::BoxPath => {
                let path = edef.emitter_path.evaluate(t, e.track_interp[system_track::EMITTER_PATH]);
                let (min_x, max_x) = (edef.emitter_box_x.evaluate(t, 0.0), edef.emitter_box_x.evaluate(t, 1.0));
                let (min_y, max_y) = (edef.emitter_box_y.evaluate(t, 0.0), edef.emitter_box_y.evaluate(t, 1.0));
                let (dx, dy) = (max_x - min_x, max_y - min_y);
                let p = path * (dy + dx + dy + dx);
                if p < dy {
                    (min_x, min_y + p)
                } else if p < dy + dx {
                    (min_x + (p - dy), max_y)
                } else if p < dy + dx + dy {
                    (max_x, max_y - (p - dy - dx))
                } else {
                    (max_x - (p - dy - dx - dy), min_y)
                }
            }
        };
        let skew_x = edef.emitter_skew_x.evaluate(t, rng.next_f32(1.0));
        let skew_y = edef.emitter_skew_y.evaluate(t, rng.next_f32(1.0));
        let mut position = (e.system_center.0 + pos_x + pos_y * skew_x, e.system_center.1 + pos_y + pos_x * skew_y);
        let velocity = (launch_angle.sin() * launch_speed, launch_angle.cos() * launch_speed);
        position.0 += edef.emitter_offset_x.evaluate(t, offset_x_interp);
        position.1 += edef.emitter_offset_y.evaluate(t, offset_y_interp);

        let image_frame = if edef.animated || edef.animation_rate.is_set() {
            0
        } else {
            rng.next_range(edef.image_frames.max(1) as u32) as i32
        };
        let spin_position = if edef.has_flag(flags::RANDOM_LAUNCH_SPIN) {
            rng.next_f32(2.0 * PI)
        } else if edef.has_flag(flags::ALIGN_LAUNCH_SPIN) {
            launch_angle
        } else {
            0.0
        };

        let id = self.particles.alloc(Particle {
            emitter: index,
            duration,
            age,
            time_value: -1.0,
            last_time_value: -1.0,
            animation_time_value: 0.0,
            velocity,
            position,
            image_frame,
            spin_position,
            spin_velocity: 0.0,
            cross_fade_particle: None,
            cross_fade_duration: 0,
            interp,
            field_interp,
        });
        let e = &mut self.emitters[index];
        e.particles.insert(0, id);
        e.particles_spawned += 1;
        // A particle that ends on its first update is still returned, like the original.
        self.update_particle(id, rng);
        Some(id)
    }

    /// `TodParticleEmitter::UpdateParticle`. Returns false when the particle
    /// should be deleted.
    fn update_particle(&mut self, id: ParticleId, rng: &mut MtRand) -> bool {
        let def = Arc::clone(&self.definition);
        let Some(p) = self.particles.get(id) else { return false };
        let index = p.emitter;
        let edef = &def.emitters[self.emitters[index].def];

        let p = self.particles.get_mut(id).unwrap();
        if p.age >= p.duration {
            if edef.has_flag(flags::PARTICLE_LOOPS) {
                p.age = 0;
            } else if p.cross_fade_duration > 0 {
                p.age = p.duration - 1;
            } else if edef.on_duration.is_empty() {
                return false;
            } else {
                // CrossFadeParticleToName.
                let Some(to_def) = self.find_emitter_def(&edef.on_duration) else { return false };
                let center = self.emitters[index].system_center;
                let to = self.add_emitter(center.0, center.1, to_def, rng);
                if !self.cross_fade_particle(index, id, to, rng) {
                    return false;
                }
            }
        }
        let cross_fade_gone =
            self.particles.get(id).and_then(|p| p.cross_fade_particle).is_some_and(|c| self.particles.get(c).is_none());
        if cross_fade_gone {
            return false;
        }

        let center = self.emitters[index].system_center;
        let p = self.particles.get_mut(id).unwrap();
        p.time_value = p.age as f32 / (p.duration - 1) as f32;
        for (i, field) in edef.particle_fields.iter().enumerate().take(MAX_FIELDS) {
            update_particle_field(p, field, i, center, edef);
        }
        p.position.0 += p.velocity.0;
        p.position.1 += p.velocity.1;
        let spin_speed = edef.particle_spin_speed.evaluate(p.time_value, p.interp[particle_track::SPIN_SPEED]) * 0.01;
        let spin_angle = edef.particle_spin_angle.evaluate(p.time_value, p.interp[particle_track::SPIN_ANGLE]);
        let last_spin_angle =
            edef.particle_spin_angle.evaluate_from_last_time(p.last_time_value, p.interp[particle_track::SPIN_ANGLE]);
        p.spin_position += (spin_speed + spin_angle - last_spin_angle).to_radians() + p.spin_velocity;
        if edef.animation_rate.is_set() {
            let rate = edef.animation_rate.evaluate(p.time_value, p.interp[particle_track::ANIMATION_RATE]) * 0.01;
            p.animation_time_value = (p.animation_time_value + rate).rem_euclid(1.0);
        }
        p.age += 1;
        p.last_time_value = p.time_value;
        true
    }

    /// `TodParticleEmitter::DeleteParticle`, with whatever it was fading from.
    fn delete_particle(&mut self, id: ParticleId) {
        let Some(p) = self.particles.get_mut(id) else { return };
        let emitter = p.emitter;
        if let Some(from) = p.cross_fade_particle.take() {
            self.delete_particle(from);
        }
        self.emitters[emitter].particles.retain(|&i| i != id);
        self.particles.free(id);
    }

    /// What to draw, back to front (`TodParticleSystem::Draw`), one quad
    /// each. Like the original with 3D acceleration, software-only emitters
    /// are skipped.
    pub fn draw_list(&self) -> Vec<ParticleDraw> {
        let mut out = Vec::new();
        for e in &self.emitters {
            let edef = &self.definition.emitters[e.def];
            if edef.has_flag(flags::SOFTWARE_ONLY) {
                continue;
            }
            for &id in &e.particles {
                if let Some(p) = self.particles.get(id)
                    && let Some(d) = self.draw_particle(e, p)
                {
                    out.push(d);
                }
            }
        }
        out
    }

    /// `GetRenderParams`: (color, x, y, scale, stretch, spin, and which of
    /// those the definition sets, for cross-fades).
    fn render_params(&self, p: &Particle) -> RenderParams {
        let e = &self.emitters[p.emitter];
        let d = &self.definition.emitters[e.def];
        let sys = |track: &FloatTrack, slot: usize| track.evaluate(e.system_time_value, e.track_interp[slot]);
        let part = |track: &FloatTrack, slot: usize| track.evaluate(p.time_value, p.interp[slot]);
        let brightness = part(&d.particle_brightness, particle_track::BRIGHTNESS)
            * sys(&d.system_brightness, system_track::BRIGHTNESS);
        let channel = |s: &FloatTrack, ss: usize, pt: &FloatTrack, ps: usize, o: f32| part(pt, ps) * sys(s, ss) * o * brightness;
        let mut params = RenderParams {
            color: [
                channel(&d.system_red, system_track::RED, &d.particle_red, particle_track::RED, e.color_override[0]),
                channel(&d.system_green, system_track::GREEN, &d.particle_green, particle_track::GREEN, e.color_override[1]),
                channel(&d.system_blue, system_track::BLUE, &d.particle_blue, particle_track::BLUE, e.color_override[2]),
                channel(&d.system_alpha, system_track::ALPHA, &d.particle_alpha, particle_track::ALPHA, e.color_override[3]),
            ],
            // The original compares 0..=255 overrides to 1.0, so colours always count as set.
            color_set: [
                d.system_red.is_set() || d.particle_red.is_set() || e.color_override[0] != 1.0,
                d.system_green.is_set() || d.particle_green.is_set() || e.color_override[1] != 1.0,
                d.system_blue.is_set() || d.particle_blue.is_set() || e.color_override[2] != 1.0,
                d.system_alpha.is_set() || d.particle_alpha.is_set() || e.color_override[3] != 1.0,
            ],
            pos: p.position,
            pos_set: !d.particle_fields.is_empty()
                || d.emitter_radius.is_set()
                || d.emitter_offset_x.is_set()
                || d.emitter_offset_y.is_set()
                || d.emitter_box_x.is_set()
                || d.emitter_box_y.is_set(),
            scale: part(&d.particle_scale, particle_track::SCALE) * e.scale_override,
            scale_set: d.particle_scale.is_set() || e.scale_override != 1.0,
            stretch: part(&d.particle_stretch, particle_track::STRETCH),
            stretch_set: d.particle_stretch.is_set(),
            spin: p.spin_position,
            spin_set: d.particle_spin_speed.is_set()
                || d.particle_spin_angle.is_set()
                || d.has_flag(flags::RANDOM_LAUNCH_SPIN)
                || d.has_flag(flags::ALIGN_LAUNCH_SPIN),
        };
        if let Some(from) = p.cross_fade_particle.and_then(|id| self.particles.get(id)) {
            let f = self.render_params(from);
            let fraction = p.age as f32 / (from.cross_fade_duration - 1) as f32;
            let lerp = |a: f32, b: f32, a_set: bool, b_set: bool| {
                if !a_set {
                    b
                } else if !b_set {
                    a
                } else {
                    a + (b - a) * fraction
                }
            };
            for c in 0..4 {
                params.color[c] = lerp(f.color[c], params.color[c], f.color_set[c], params.color_set[c]);
                params.color_set[c] |= f.color_set[c];
            }
            params.scale = lerp(f.scale, params.scale, f.scale_set, params.scale_set);
            params.stretch = lerp(f.stretch, params.stretch, f.stretch_set, params.stretch_set);
            params.spin = lerp(f.spin, params.spin, f.spin_set, params.spin_set);
            params.pos.0 = lerp(f.pos.0, params.pos.0, f.pos_set, params.pos_set);
            params.pos.1 = lerp(f.pos.1, params.pos.1, f.pos_set, params.pos_set);
            params.scale_set |= f.scale_set;
            params.stretch_set |= f.stretch_set;
            params.spin_set |= f.spin_set;
            params.pos_set |= f.pos_set;
        }
        params
    }

    /// `DrawParticle` + `RenderParticle` (without the image size, which the
    /// caller knows: clipping is returned as fractions).
    fn draw_particle(&self, e: &Emitter, p: &Particle) -> Option<ParticleDraw> {
        if p.cross_fade_duration > 0 {
            return None;
        }
        let params = self.render_params(p);
        let color = params.color.map(|c| crate::tod::float_round_to_int(c).clamp(0, 255) as u8);
        if color[3] == 0 {
            return None;
        }
        let edef = &self.definition.emitters[e.def];
        // Without an image, a cross-fading particle shows the one it's fading from.
        let (source, source_emitter) = if e.image_override.is_some() || edef.image.is_some() {
            (p, e)
        } else {
            let from = p.cross_fade_particle.and_then(|id| self.particles.get(id))?;
            (from, &self.emitters[from.emitter])
        };
        let sdef = &self.definition.emitters[source_emitter.def];
        let image = source_emitter.image_override.clone().or_else(|| sdef.image.clone())?;
        let frames = sdef.image_frames.max(1);
        let frame = match source_emitter.frame_override {
            Some(f) => f,
            None if sdef.animation_rate.is_set() => ((source.animation_time_value * frames as f32) as i32).clamp(0, frames - 1),
            None if sdef.animated => ((source.time_value * frames as f32) as i32).clamp(0, frames - 1),
            None => source.image_frame,
        };
        let part = |track: &FloatTrack, slot: usize| track.evaluate(source.time_value, source.interp[slot]);
        let clip = [
            part(&sdef.clip_top, particle_track::CLIP_TOP),
            part(&sdef.clip_bottom, particle_track::CLIP_BOTTOM),
            part(&sdef.clip_left, particle_track::CLIP_LEFT),
            part(&sdef.clip_right, particle_track::CLIP_RIGHT),
        ];
        Some(ParticleDraw {
            image,
            col: frame + sdef.image_col,
            row: sdef.image_row,
            clip,
            x: params.pos.0,
            y: params.pos.1,
            spin: params.spin,
            scale: params.scale,
            stretch: params.stretch,
            color,
            additive: sdef.has_flag(flags::ADDITIVE),
            extra_additive: false,
            align_to_pixels: sdef.has_flag(flags::ALIGN_TO_PIXELS),
            fullscreen: sdef.has_flag(flags::FULLSCREEN),
        })
    }
}

struct RenderParams {
    color: [f32; 4],
    color_set: [bool; 4],
    pos: (f32, f32),
    pos_set: bool,
    scale: f32,
    scale_set: bool,
    stretch: f32,
    stretch_set: bool,
    spin: f32,
    spin_set: bool,
}

/// `TodParticleEmitter::UpdateParticleField`.
fn update_particle_field(p: &mut Particle, field: &ParticleField, index: usize, center: (f32, f32), edef: &EmitterDefinition) {
    let [ix, iy] = p.field_interp[index];
    let x = field.x.evaluate(p.time_value, ix);
    let y = field.y.evaluate(p.time_value, iy);
    match field.field_type {
        FieldType::Invalid | FieldType::SystemPosition => {}
        FieldType::Friction => {
            p.velocity.0 *= 1.0 - x;
            p.velocity.1 *= 1.0 - y;
        }
        FieldType::Acceleration => {
            p.velocity.0 += 0.01 * x;
            p.velocity.1 += 0.01 * y;
        }
        FieldType::Attractor => {
            p.velocity.0 += 0.01 * (x - (p.position.0 - center.0));
            p.velocity.1 += 0.01 * (y - (p.position.1 - center.1));
        }
        FieldType::MaxVelocity => {
            p.velocity.0 = p.velocity.0.clamp(-x, x);
            p.velocity.1 = p.velocity.1.clamp(-y, y);
        }
        FieldType::Velocity => {
            p.position.0 += 0.01 * x;
            p.position.1 += 0.01 * y;
        }
        FieldType::Position => {
            p.position.0 += x - field.x.evaluate_from_last_time(p.last_time_value, ix);
            p.position.1 += y - field.y.evaluate_from_last_time(p.last_time_value, iy);
        }
        FieldType::GroundConstraint => {
            if p.position.1 > center.1 + y {
                p.position.1 = center.1 + y;
                let reflect =
                    edef.collision_reflect.evaluate(p.time_value, p.interp[particle_track::COLLISION_REFLECT]);
                let spin = edef.collision_spin.evaluate(p.time_value, p.interp[particle_track::COLLISION_SPIN]) / 1000.0;
                p.spin_velocity = p.velocity.1 * spin;
                p.velocity.0 *= reflect;
                p.velocity.1 *= -reflect;
            }
        }
        FieldType::Shake => {
            // The original undoes last tick's random offset and applies a new
            // one, seeding C's rand() from the age; any repeatable noise works.
            let last_x = field.x.evaluate_from_last_time(p.last_time_value, ix);
            let last_y = field.y.evaluate_from_last_time(p.last_time_value, iy);
            let last_age = if p.age == 0 { p.duration - 1 } else { p.age - 1 };
            let (lx, ly) = shake_noise(last_age, ix);
            let (nx, ny) = shake_noise(p.age, ix);
            p.position.0 += x * nx - last_x * lx;
            p.position.1 += y * ny - last_y * ly;
        }
        FieldType::Circle | FieldType::Away => {
            let to_center = (p.position.0 - center.0, p.position.1 - center.1);
            let radius = (to_center.0 * to_center.0 + to_center.1 * to_center.1).sqrt();
            if radius > 0.0 {
                let dir = if field.field_type == FieldType::Circle {
                    // SexyVector2::Perp: (-y, x).
                    (-to_center.1 / radius, to_center.0 / radius)
                } else {
                    (to_center.0 / radius, to_center.1 / radius)
                };
                let amount = 0.01 * (x + radius * y);
                p.position.0 += dir.0 * amount;
                p.position.1 += dir.1 * amount;
            }
        }
    }
}

/// Two numbers in -1..1 fixed by `age` and a per-particle seed.
fn shake_noise(age: i32, seed: f32) -> (f32, f32) {
    let mut h = (age as u32).wrapping_mul(0x9E37_79B9) ^ seed.to_bits();
    let mut next = || {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        (h % 20001) as f32 / 10000.0 - 1.0
    };
    (next(), next())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(t: &FloatTrack, i: usize) -> (f32, f32, f32) {
        let n = t.nodes[i];
        (n.time, n.low, n.high)
    }

    #[test]
    fn parses_track_syntax() {
        let t = FloatTrack::parse("1,80 0").unwrap();
        assert_eq!((node(&t, 0), node(&t, 1)), ((0.8, 1.0, 1.0), (1.0, 0.0, 0.0)));

        let t = FloatTrack::parse("[.7 .9]").unwrap();
        assert_eq!(node(&t, 0), (0.0, 0.7, 0.9));

        let t = FloatTrack::parse("0 EaseInOutWeak 10 EaseInOutWeak 0").unwrap();
        assert_eq!(t.nodes.iter().map(|n| n.time).collect::<Vec<_>>(), [0.0, 0.5, 1.0]);
        assert_eq!(t.nodes[0].curve, TodCurve::EaseInOutWeak);

        let t = FloatTrack::parse("[.1 EaseIn 0.5]").unwrap();
        assert_eq!(t.nodes[0].distribution, TodCurve::EaseIn);

        let t = FloatTrack::parse("1 [.4 .8],80 0").unwrap();
        assert_eq!(t.nodes.iter().map(|n| n.time).collect::<Vec<_>>(), [0.0, 0.8, 1.0]);
        assert!(FloatTrack::parse("[1 2").is_none());
        // SnowPeaPuff.xml's typo.
        assert_eq!(node(&FloatTrack::parse("[.3.8],80 0").unwrap(), 0), (0.8, 0.3, 0.8));
    }

    #[test]
    fn evaluates_tracks() {
        let t = FloatTrack::parse("1,80 0").unwrap();
        assert_eq!(t.evaluate(0.5, 0.0), 1.0);
        assert!((t.evaluate(0.9, 0.0) - 0.5).abs() < 1e-5);
        assert_eq!(t.evaluate(1.0, 0.0), 0.0);
        let spread = FloatTrack::parse("[10 20]").unwrap();
        assert_eq!(spread.evaluate(0.3, 0.5), 15.0);
        assert!(spread.is_set());
    }

    const PLANTING: &str = "<Emitter>
  <Name>Planting</Name>
  <SpawnMinActive>8</SpawnMinActive>
  <SystemAlpha>1,85 0</SystemAlpha>
  <ParticleScale>[.7 .9]</ParticleScale>
  <LaunchSpeed>[300 500]</LaunchSpeed>
  <ParticleDuration>30</ParticleDuration>
  <SystemDuration>30</SystemDuration>
  <Image>IMAGE_DIRTSMALL</Image>
  <ImageFrames>8</ImageFrames>
  <LaunchAngle>[110 250]</LaunchAngle>
  <RandomLaunchSpin>1</RandomLaunchSpin>
  <Field>
    <FieldType>Acceleration</FieldType>
    <Y>8.0</Y>
  </Field>
</Emitter>";

    #[test]
    fn parses_an_emitter_and_runs_it_out() {
        let def = ParticleDefinition::parse(PLANTING).unwrap();
        let e = &def.emitters[0];
        assert_eq!((e.name.as_str(), e.image_frames), ("Planting", 8));
        assert!(e.has_flag(flags::RANDOM_LAUNCH_SPIN));
        assert_eq!(e.particle_fields[0].field_type, FieldType::Acceleration);
        // Defaults fill in what's left out.
        assert_eq!(e.particle_red.evaluate(0.0, 0.0), 1.0);
        assert!(!e.particle_red.is_set());

        let mut rng = MtRand::new(1);
        let mut system = ParticleSystem::new(100.0, 100.0, 0, Arc::new(def), &mut rng, false);
        assert_eq!(system.draw_list().len(), 8);
        let falling = system.draw_list()[0].y;
        for _ in 0..10 {
            system.update(&mut rng);
        }
        assert!(system.draw_list()[0].y != falling);
        for _ in 0..40 {
            system.update(&mut rng);
        }
        assert!(system.dead);
    }

    #[test]
    fn an_overloaded_die_if_overloaded_emitter_kills_the_effect() {
        let mut def = ParticleDefinition::parse(PLANTING).unwrap();
        def.emitters[0].flags |= flags::DIE_IF_OVERLOADED;
        def.emitters[0].cross_fade_duration = FloatTrack::default();
        let def = Arc::new(def);

        let mut rng = MtRand::new(1);
        let system = ParticleSystem::new(0.0, 0.0, 0, def.clone(), &mut rng, true);
        assert!(system.dead, "the effect dies instead of starting");
        assert!(system.emitters.is_empty(), "and adds no emitter");

        let mut rng = MtRand::new(1);
        let system = ParticleSystem::new(0.0, 0.0, 0, def, &mut rng, false);
        assert!(!system.dead);
        assert_eq!(system.emitters.len(), 1);
    }
}
