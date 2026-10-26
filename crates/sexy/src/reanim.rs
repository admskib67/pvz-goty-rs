//! Reanimations: PopCap's skeletal-ish animation format (`reanim/*.reanim`).
//!
//! A definition is a list of named tracks with one keyframe per frame. A
//! keyframe positions an image with translation, skew and scale; empty
//! keyframes inherit the previous values. Tracks named `anim_*` carry no
//! image and only mark frame ranges (`f` = -1 outside the range).
//!
//! Behaviour per ruslan831 decomp, `Sexy.TodLib/Reanimator.cpp`
//! (`ReanimationLoadDefinition`, `Reanimation::Update`, `GetFrameTime`,
//! `GetTransformAtTime`, `GetCurrentTransform`, `MatrixFromTransform`,
//! `GetFramesForLayer`, `PlayReanim`, `StartBlend`).

use crate::files::Files;
use crate::definition::{CompiledError, CompiledReader, decompress};
use crate::matrix::Matrix3;
use crate::tod::float_round_to_int;

/// The original updates at 100 Hz.
const SECONDS_PER_UPDATE: f32 = 0.01;

#[derive(Debug, thiserror::Error)]
pub enum ReanimError {
    #[error("invalid XML: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("compiled definition: {0}")]
    Compiled(&'static str),
    #[error("track {track:?}: bad <{field}> value {value:?}")]
    BadValue { track: String, field: String, value: String },
    #[error("track {track:?} has {got} frames but the first track has {expected}")]
    FrameCountMismatch { track: String, got: usize, expected: usize },
}

/// One keyframe with every field filled in.
#[derive(Debug, Clone, PartialEq)]
pub struct ReanimTransform {
    pub x: f32,
    pub y: f32,
    /// Skew angles in degrees.
    pub kx: f32,
    pub ky: f32,
    pub sx: f32,
    pub sy: f32,
    /// Image cel; negative means "not shown".
    pub f: f32,
    pub a: f32,
    /// Image resource id, e.g. `IMAGE_REANIM_SPROUT_BODY`.
    pub image: Option<String>,
    pub font: Option<String>,
    pub text: String,
}

impl ReanimTransform {
    /// `MatrixFromTransform`.
    pub fn matrix(&self) -> Matrix3 {
        let skew_x = -self.kx.to_radians();
        let skew_y = -self.ky.to_radians();
        Matrix3 {
            m00: skew_x.cos() * self.sx,
            m10: -skew_x.sin() * self.sx,
            m01: skew_y.sin() * self.sy,
            m11: skew_y.cos() * self.sy,
            m02: self.x,
            m12: self.y,
        }
    }

    pub fn frame(&self) -> i32 {
        float_round_to_int(self.f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReanimTrack {
    pub name: String,
    pub transforms: Vec<ReanimTransform>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReanimDefinition {
    pub fps: f32,
    pub tracks: Vec<ReanimTrack>,
}

impl ReanimDefinition {
    /// Parses a `.reanim` file (a sequence of top-level XML elements).
    pub fn parse(text: &str) -> Result<Self, ReanimError> {
        let wrapped = format!("<reanim>{}</reanim>", text.trim_start_matches('\u{feff}'));
        let doc = roxmltree::Document::parse(&wrapped)?;
        let mut fps = 12.0;
        let mut tracks = Vec::new();

        for el in doc.root_element().children().filter(|n| n.is_element()) {
            match el.tag_name().name() {
                "fps" => fps = el.text().unwrap_or_default().trim().parse().unwrap_or(12.0),
                "track" => tracks.push(parse_track(&el)?),
                _ => {}
            }
        }

        if let Some(expected) = tracks.first().map(|t| t.transforms.len())
            && let Some(bad) = tracks.iter().find(|t| t.transforms.len() != expected)
        {
            return Err(ReanimError::FrameCountMismatch {
                track: bad.name.clone(),
                got: bad.transforms.len(),
                expected,
            });
        }
        Ok(Self { fps, tracks })
    }

    /// Loads `path` (e.g. `reanim/Foo.reanim`), preferring the compiled
    /// `compiled/<path>.compiled` when present, like the original.
    pub fn load(files: &impl Files, path: &str) -> Result<Self, String> {
        if let Some(bytes) = files.read(&format!("compiled/{path}.compiled")) {
            return Self::parse_compiled(&bytes).map_err(|e| e.to_string());
        }
        let bytes = files.read(path).ok_or_else(|| "missing".to_owned())?;
        let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        Self::parse(text).map_err(|e| e.to_string())
    }

    /// Parses a `compiled/*.reanim.compiled` file.
    ///
    /// Behaviour per ruslan831 decomp, `Definition.cpp`
    /// (`DefinitionUncompressCompiledBuffer`, `DefinitionReadCompiledFile`,
    /// `DefMapReadFromCache`, `DefReadFromCacheArray`/`String`/`Image`).
    /// The schema hash is skipped rather than checked.
    pub fn parse_compiled(bytes: &[u8]) -> Result<Self, ReanimError> {
        let err = ReanimError::Compiled;
        let data = decompress(bytes)?;

        let mut r = CompiledReader::new(&data);
        r.u32()?; // schema hash
        // ReanimatorDefinition: tracks ptr, track count, fps, atlas ptr.
        r.u32()?;
        let track_count = r.u32()? as usize;
        let fps = f32::from_bits(r.u32()?);
        r.u32()?;

        if r.u32()? != 12 {
            return Err(err("track size mismatch"));
        }
        // The raw track structs (name ptr, transform ptr, transform count)
        // come first, then each track's strings and transforms in order.
        let raw_tracks = r.pos;
        r.skip(12 * track_count)?;

        let mut tracks = Vec::with_capacity(track_count);
        for i in 0..track_count {
            let o = raw_tracks + 12 * i + 8;
            let count = u32::from_le_bytes(data[o..o + 4].try_into().unwrap()) as usize;
            let name = r.string()?;
            if r.u32()? != 44 {
                return Err(err("transform size mismatch"));
            }
            let raw_t = r.pos;
            r.skip(44 * count)?;

            let mut transforms = Vec::with_capacity(count);
            let mut prev = default_transform();
            for j in 0..count {
                let o = raw_t + 44 * j;
                let float = |k: usize, prev: f32| {
                    let v = f32::from_le_bytes(data[o + 4 * k..o + 4 * k + 4].try_into().unwrap());
                    if v == DEFAULT_FIELD_PLACEHOLDER { prev } else { v }
                };
                let image = r.string()?;
                let font = r.string()?;
                let text = r.string()?;
                let cur = ReanimTransform {
                    x: float(0, prev.x),
                    y: float(1, prev.y),
                    kx: float(2, prev.kx),
                    ky: float(3, prev.ky),
                    sx: float(4, prev.sx),
                    sy: float(5, prev.sy),
                    f: float(6, prev.f),
                    a: float(7, prev.a),
                    image: if image.is_empty() { prev.image.clone() } else { Some(image) },
                    font: if font.is_empty() { prev.font.clone() } else { Some(font) },
                    text: if text.is_empty() { prev.text.clone() } else { text },
                };
                prev = cur.clone();
                transforms.push(cur);
            }
            tracks.push(ReanimTrack { name, transforms });
        }
        if !r.at_end() {
            return Err(err("trailing data"));
        }
        Ok(Self { fps, tracks })
    }

    pub fn frame_count(&self) -> usize {
        self.tracks.first().map_or(0, |t| t.transforms.len())
    }

    /// Case-insensitive track lookup. The original falls back to track 0.
    pub fn find_track(&self, name: &str) -> Option<usize> {
        self.tracks.iter().position(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// `GetFramesForLayer`: the frame range where `track` is shown.
    pub fn frames_for_layer(&self, track: &str) -> (usize, usize) {
        if self.tracks.is_empty() {
            return (0, 0);
        }
        let track = &self.tracks[self.find_track(track).unwrap_or(0)];
        let shown: Vec<usize> =
            track.transforms.iter().enumerate().filter(|(_, t)| t.f >= 0.0).map(|(i, _)| i).collect();
        match (shown.first(), shown.last()) {
            (Some(&start), Some(&end)) => (start, end - start + 1),
            _ => (0, 1),
        }
    }

    /// Every image id the definition references.
    pub fn image_ids(&self) -> impl Iterator<Item = &str> {
        let mut ids: Vec<&str> =
            self.tracks.iter().flat_map(|t| &t.transforms).filter_map(|t| t.image.as_deref()).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
    }
}

impl From<CompiledError> for ReanimError {
    fn from(e: CompiledError) -> Self {
        Self::Compiled(e.0)
    }
}

/// Unset keyframe values in compiled files (`DEFAULT_FIELD_PLACEHOLDER`).
const DEFAULT_FIELD_PLACEHOLDER: f32 = -10000.0;

fn default_transform() -> ReanimTransform {
    ReanimTransform { x: 0.0, y: 0.0, kx: 0.0, ky: 0.0, sx: 1.0, sy: 1.0, f: 0.0, a: 1.0, image: None, font: None, text: String::new() }
}

fn parse_track(el: &roxmltree::Node) -> Result<ReanimTrack, ReanimError> {
    let name = el
        .children()
        .find(|n| n.has_tag_name("name"))
        .and_then(|n| n.text())
        .unwrap_or_default()
        .trim()
        .to_owned();

    // Missing values inherit from the previous keyframe (ReanimationFillInMissingData).
    let mut prev = ReanimTransform {
        x: 0.0,
        y: 0.0,
        kx: 0.0,
        ky: 0.0,
        sx: 1.0,
        sy: 1.0,
        f: 0.0,
        a: 1.0,
        image: None,
        font: None,
        text: String::new(),
    };
    let mut transforms = Vec::new();
    for t in el.children().filter(|n| n.has_tag_name("t")) {
        let mut cur = prev.clone();
        for field in t.children().filter(|n| n.is_element()) {
            let tag = field.tag_name().name();
            let value = field.text().unwrap_or_default().trim();
            let float = || {
                value.parse::<f32>().map_err(|_| ReanimError::BadValue {
                    track: name.clone(),
                    field: tag.to_owned(),
                    value: value.to_owned(),
                })
            };
            match tag {
                "x" => cur.x = float()?,
                "y" => cur.y = float()?,
                "kx" => cur.kx = float()?,
                "ky" => cur.ky = float()?,
                "sx" => cur.sx = float()?,
                "sy" => cur.sy = float()?,
                "f" => cur.f = float()?,
                "a" => cur.a = float()?,
                "i" if !value.is_empty() => cur.image = Some(value.to_owned()),
                "font" if !value.is_empty() => cur.font = Some(value.to_owned()),
                "text" if !value.is_empty() => cur.text = value.to_owned(),
                _ => {}
            }
        }
        prev = cur.clone();
        transforms.push(cur);
    }
    Ok(ReanimTrack { name, transforms })
}

/// Paths to try for an image id that isn't in the resource manifest, in
/// order (`DefinitionLoadImage`). Like the original, the prefix is cut by
/// length without checking it matches.
pub fn image_search_paths(id: &str) -> Vec<String> {
    const PATHS: [(&str, &str); 4] =
        [("IMAGE_", ""), ("IMAGE_", "particles/"), ("IMAGE_REANIM_", "reanim/"), ("IMAGE_REANIM_", "images/")];
    PATHS
        .iter()
        .filter(|(prefix, _)| prefix.len() < id.len())
        .map(|(prefix, dir)| format!("{dir}{}", &id[prefix.len()..]))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopType {
    Loop,
    LoopFullLastFrame,
    PlayOnce,
    PlayOnceFullLastFrame,
    PlayOnceAndHold,
    PlayOnceFullLastFrameAndHold,
}

impl LoopType {
    fn full_last_frame(self) -> bool {
        matches!(self, Self::LoopFullLastFrame | Self::PlayOnceFullLastFrame | Self::PlayOnceFullLastFrameAndHold)
    }
}

/// RGBA, 0..=255.
pub type Rgba = [u8; 4];

/// Tracks in this render group aren't drawn.
pub const RENDER_GROUP_HIDDEN: i32 = -1;
pub const RENDER_GROUP_NORMAL: i32 = 0;
/// `frame_base_pose` value meaning attachments use no base pose.
pub const NO_BASE_POSE: i32 = -2;

pub const WHITE: Rgba = [255; 4];

/// `ColorsMultiply`.
pub fn colors_multiply(a: Rgba, b: Rgba) -> Rgba {
    std::array::from_fn(|i| (a[i] as u32 * b[i] as u32 / 255) as u8)
}

/// Per-track playback state.
#[derive(Debug, Clone)]
pub struct TrackInstance {
    pub blend_counter: i32,
    pub blend_time: i32,
    pub blend_transform: Option<ReanimTransform>,
    pub render_group: i32,
    pub truncate_disappearing_frames: bool,
    pub track_color: Rgba,
    pub ignore_color_override: bool,
    pub ignore_clip_rect: bool,
    /// Image id drawn instead of the keyframe's image.
    pub image_override: Option<String>,
}

impl Default for TrackInstance {
    fn default() -> Self {
        Self {
            blend_counter: 0,
            blend_time: 0,
            blend_transform: None,
            render_group: 0,
            truncate_disappearing_frames: true,
            track_color: WHITE,
            ignore_color_override: false,
            ignore_clip_rect: false,
            image_override: None,
        }
    }
}

struct FrameTime {
    fraction: f32,
    before: usize,
    after: usize,
}

/// A playing instance of a [`ReanimDefinition`].
#[derive(Debug, Clone)]
pub struct Reanimation<D = std::sync::Arc<ReanimDefinition>> {
    pub definition: D,
    /// Position in the current range, 0..=1.
    pub anim_time: f32,
    /// Frames per second (negative plays backwards).
    pub anim_rate: f32,
    pub loop_type: LoopType,
    pub last_frame_time: f32,
    pub dead: bool,
    pub frame_start: usize,
    pub frame_count: usize,
    pub loop_count: i32,
    /// Position and scale applied on top of every track.
    pub overlay: Matrix3,
    /// `AttachEffect::mOffset`: where this sits on the track it's attached to.
    pub attach_offset: Matrix3,
    pub color_override: Rgba,
    /// A color added on top when drawing (the hit flash, the chill glow;
    /// `mExtraAdditiveColor`), if any.
    pub extra_additive: Option<Rgba>,
    /// Frame whose pose attachments are measured from; -1 means `frame_start`.
    pub frame_base_pose: i32,
    pub tracks: Vec<TrackInstance>,
    /// Where the board draws it among its objects (`mRenderOrder`).
    pub render_order: i32,
    /// Drawn by its owner rather than by the board (`mIsAttachment`).
    pub is_attachment: bool,
}

impl<D: std::ops::Deref<Target = ReanimDefinition>> Reanimation<D> {
    /// `ReanimationInitialize`: plays the whole file once at its own fps.
    pub fn new(x: f32, y: f32, definition: D) -> Self {
        let tracks = vec![TrackInstance::default(); definition.tracks.len()];
        Self {
            anim_rate: definition.fps,
            frame_count: definition.frame_count(),
            definition,
            anim_time: 0.0,
            loop_type: LoopType::PlayOnce,
            last_frame_time: -1.0,
            dead: false,
            frame_start: 0,
            loop_count: 0,
            overlay: Matrix3::translation(x, y),
            attach_offset: Matrix3::IDENTITY,
            color_override: WHITE,
            extra_additive: None,
            frame_base_pose: -1,
            tracks,
            render_order: 0,
            is_attachment: false,
        }
    }

    /// `FindTrackIndex`: like the original, an unknown name gives track 0.
    pub fn find_track(&self, name: &str) -> usize {
        self.definition.find_track(name).unwrap_or(0)
    }

    pub fn assign_render_group_to_track(&mut self, name: &str, group: i32) {
        if let Some(i) = self.definition.find_track(name) {
            self.tracks[i].render_group = group;
        }
    }

    /// Every track whose name starts with `prefix` (case-insensitive).
    pub fn assign_render_group_to_prefix(&mut self, prefix: &str, group: i32) {
        for (inst, track) in self.tracks.iter_mut().zip(&self.definition.tracks) {
            let name = track.name.as_bytes();
            if name.len() >= prefix.len() && name[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes()) {
                inst.render_group = group;
            }
        }
    }

    pub fn set_image_override(&mut self, track: &str, image: Option<&str>) {
        let i = self.find_track(track);
        self.tracks[i].image_override = image.map(str::to_owned);
    }

    /// `GetTrackBasePoseMatrix`.
    pub fn track_base_pose_matrix(&self, track: usize) -> Matrix3 {
        if self.frame_base_pose == NO_BASE_POSE {
            return Matrix3::IDENTITY;
        }
        let pose = if self.frame_base_pose == -1 { self.frame_start } else { self.frame_base_pose as usize };
        let last = self.definition.frame_count().saturating_sub(1);
        let time = FrameTime { fraction: 0.0, before: pose.min(last), after: (pose + 1).min(last) };
        self.transform_at(track, &time).matrix()
    }

    /// `GetAttachmentOverlayMatrix`: how far `track` has moved from its base
    /// pose, for things attached to it.
    pub fn attachment_overlay_matrix(&self, track: usize) -> Matrix3 {
        let current = self.overlay * self.current_transform(track).matrix();
        current * self.track_base_pose_matrix(track).inverse()
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.overlay.m02 = x;
        self.overlay.m12 = y;
    }

    pub fn override_scale(&mut self, sx: f32, sy: f32) {
        self.overlay.m00 = sx;
        self.overlay.m11 = sy;
    }

    /// One 100 Hz tick.
    pub fn update(&mut self) {
        if self.frame_count == 0 || self.dead {
            return;
        }
        self.last_frame_time = self.anim_time;
        self.anim_time += SECONDS_PER_UPDATE * self.anim_rate / self.frame_count as f32;

        let once = matches!(self.loop_type, LoopType::PlayOnce | LoopType::PlayOnceFullLastFrame);
        let looping = matches!(self.loop_type, LoopType::Loop | LoopType::LoopFullLastFrame);
        if self.anim_rate > 0.0 {
            if looping {
                while self.anim_time >= 1.0 {
                    self.loop_count += 1;
                    self.anim_time -= 1.0;
                }
            } else if self.anim_time >= 1.0 {
                self.loop_count = 1;
                self.anim_time = 1.0;
                self.dead = once;
            }
        } else if looping {
            while self.anim_time < 0.0 {
                self.loop_count += 1;
                self.anim_time += 1.0;
            }
        } else if self.anim_time < 0.0 {
            self.loop_count = 1;
            self.anim_time = 0.0;
            self.dead = once;
        }

        for track in &mut self.tracks {
            if track.blend_counter > 0 {
                track.blend_counter -= 1;
            }
        }
        // TODO: shake, attacher tracks and attachments once something uses them.
    }

    fn frame_time(&self) -> FrameTime {
        let frames = if self.loop_type.full_last_frame() { self.frame_count } else { self.frame_count.saturating_sub(1) };
        let pos = self.frame_start as f32 + self.anim_time * frames as f32;
        let before = pos.floor();
        let last = self.frame_start + self.frame_count.saturating_sub(1);
        let before_i = (float_round_to_int(before).max(0) as usize).min(last);
        FrameTime { fraction: pos - before, before: before_i, after: if before_i >= last { last } else { before_i + 1 } }
    }

    fn transform_at(&self, track: usize, time: &FrameTime) -> ReanimTransform {
        let frames = &self.definition.tracks[track].transforms;
        let (b, a) = (&frames[time.before], &frames[time.after]);
        let lerp = |x: f32, y: f32| x + (y - x) * time.fraction;
        let disappearing =
            b.f != -1.0 && a.f == -1.0 && time.fraction > 0.0 && self.tracks[track].truncate_disappearing_frames;
        ReanimTransform {
            x: lerp(b.x, a.x),
            y: lerp(b.y, a.y),
            kx: lerp(b.kx, a.kx),
            ky: lerp(b.ky, a.ky),
            sx: lerp(b.sx, a.sx),
            sy: lerp(b.sy, a.sy),
            a: lerp(b.a, a.a),
            f: if disappearing { -1.0 } else { b.f },
            image: b.image.clone(),
            font: b.font.clone(),
            text: b.text.clone(),
        }
    }

    /// `GetCurrentTransform`: interpolated, then blended from the previous animation.
    pub fn current_transform(&self, track: usize) -> ReanimTransform {
        let mut t = self.transform_at(track, &self.frame_time());
        let inst = &self.tracks[track];
        if let (true, Some(from)) = (t.frame() >= 0 && inst.blend_counter > 0, &inst.blend_transform) {
            let factor = inst.blend_counter as f32 / inst.blend_time as f32;
            let lerp = |x: f32, y: f32| x + (y - x) * factor;
            // The original snaps (rather than wraps) skews more than 180° apart.
            let near = |to: f32, from: f32| if (to - from).abs() > 180.0 { from } else { to };
            t.x = lerp(t.x, from.x);
            t.y = lerp(t.y, from.y);
            t.sx = lerp(t.sx, from.sx);
            t.sy = lerp(t.sy, from.sy);
            t.a = lerp(t.a, from.a);
            t.kx = lerp(t.kx, near(from.kx, t.kx));
            t.ky = lerp(t.ky, near(from.ky, t.ky));
        }
        t
    }

    /// `SetTruncateDisappearingFrames`: whether `track` (or every track, for
    /// `None`) stops at its last visible frame instead of holding it.
    pub fn set_truncate_disappearing_frames(&mut self, track: Option<&str>, truncate: bool) {
        match track {
            None => self.tracks.iter_mut().for_each(|t| t.truncate_disappearing_frames = truncate),
            Some(track) => {
                if let Some(i) = self.definition.find_track(track) {
                    self.tracks[i].truncate_disappearing_frames = truncate;
                }
            }
        }
    }

    /// `SetFramesForLayer`: restricts playback to the range where `track` is shown.
    pub fn set_frames_for_layer(&mut self, track: &str) {
        self.anim_time = if self.anim_rate >= 0.0 { 0.0 } else { 0.999_999_9 };
        self.last_frame_time = -1.0;
        (self.frame_start, self.frame_count) = self.definition.frames_for_layer(track);
    }

    /// `StartBlend`: remembers each visible track's pose to blend away from.
    pub fn start_blend(&mut self, blend_time: i32) {
        for i in 0..self.tracks.len() {
            let mut t = self.current_transform(i);
            if t.frame() >= 0 {
                (t.image, t.font, t.text) = (None, None, String::new());
                let inst = &mut self.tracks[i];
                inst.blend_transform = Some(t);
                inst.blend_time = blend_time;
                inst.blend_counter = blend_time;
            }
        }
    }

    /// `PlayReanim`. A rate of 0 keeps the current rate.
    pub fn play(&mut self, track: &str, loop_type: LoopType, blend_time: i32, anim_rate: f32) {
        if blend_time > 0 {
            self.start_blend(blend_time);
        }
        if anim_rate != 0.0 {
            self.anim_rate = anim_rate;
        }
        self.loop_type = loop_type;
        self.loop_count = 0;
        self.set_frames_for_layer(track);
    }

    pub fn is_anim_playing(&self, track: &str) -> bool {
        self.definition.frames_for_layer(track) == (self.frame_start, self.frame_count)
    }

    /// `GetTrackVelocity`: how far `track` moves along x per tick right now
    /// (zombies' `_ground` track drives how fast they walk).
    pub fn track_velocity(&self, track: &str) -> f32 {
        let i = self.find_track(track);
        let time = self.frame_time();
        let frames = &self.definition.tracks[i].transforms;
        (frames[time.after].x - frames[time.before].x) * SECONDS_PER_UPDATE * self.anim_rate
    }

    /// `IsTrackShowing`: whether the track's next keyframe draws an image.
    pub fn is_track_showing(&self, track: &str) -> bool {
        let i = self.find_track(track);
        let time = self.frame_time();
        self.definition.tracks[i].transforms[time.after].f >= 0.0
    }

    /// `GetTrackMatrix`: where `track` is right now, with the overlay applied.
    pub fn track_matrix(&self, track: usize) -> Matrix3 {
        self.overlay * self.current_transform(track).matrix()
    }

    /// `ShouldTriggerTimedEvent`: true on the tick that crosses `event_time` (0..=1).
    pub fn should_trigger_timed_event(&self, event_time: f32) -> bool {
        if self.frame_count == 0 || self.last_frame_time <= 0.0 || self.anim_rate <= 0.0 {
            return false;
        }
        if self.anim_time >= self.last_frame_time {
            event_time >= self.last_frame_time && event_time < self.anim_time
        } else {
            event_time >= self.last_frame_time || event_time < self.anim_time
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEF: &str = "<fps>10</fps>
<track><name>anim_idle</name><t><f>-1</f></t><t><f>0</f></t><t></t><t><f>-1</f></t></track>
<track><name>body</name>
<t><x>0</x><y>0</y><sx>1</sx><f>0</f><i>IMAGE_REANIM_BODY</i></t>
<t><x>10</x><kx>90</kx></t>
<t></t>
<t><f>-1</f></t>
</track>";

    fn def() -> std::sync::Arc<ReanimDefinition> {
        std::sync::Arc::new(ReanimDefinition::parse(DEF).unwrap())
    }

    #[test]
    fn parses_and_fills_missing_values() {
        let d = def();
        assert_eq!(d.fps, 10.0);
        assert_eq!(d.frame_count(), 4);
        let body = &d.tracks[1].transforms;
        assert_eq!(body[2].x, 10.0);
        assert_eq!(body[2].kx, 90.0);
        assert_eq!(body[2].image.as_deref(), Some("IMAGE_REANIM_BODY"));
        assert_eq!(body[3].f, -1.0);
        assert_eq!(d.frames_for_layer("ANIM_IDLE"), (1, 2));
        assert_eq!(d.image_ids().collect::<Vec<_>>(), ["IMAGE_REANIM_BODY"]);
    }

    #[test]
    fn interpolates_between_frames() {
        let mut r = Reanimation::new(0.0, 0.0, def());
        // 10 fps over 4 frames: each tick advances 0.01 * 10 / 4 = 0.025.
        for _ in 0..6 {
            r.update();
        }
        // Position 0.15 * 3 frames = 0.45 → between frames 0 and 1.
        let t = r.current_transform(1);
        assert!((t.x - 4.5).abs() < 1e-4, "{}", t.x);
        assert_eq!(t.frame(), 0);
    }

    #[test]
    fn play_once_dies_and_hold_holds() {
        let mut r = Reanimation::new(0.0, 0.0, def());
        for _ in 0..41 {
            r.update();
        }
        assert!(r.dead);

        let mut r = Reanimation::new(0.0, 0.0, def());
        r.loop_type = LoopType::PlayOnceAndHold;
        for _ in 0..100 {
            r.update();
        }
        assert!(!r.dead);
        assert_eq!(r.anim_time, 1.0);
    }

    #[test]
    fn play_restricts_range_and_loops() {
        let mut r = Reanimation::new(0.0, 0.0, def());
        r.play("anim_idle", LoopType::Loop, 0, 0.0);
        assert_eq!((r.frame_start, r.frame_count), (1, 2));
        assert!(r.is_anim_playing("anim_idle"));
        for _ in 0..30 {
            r.update();
        }
        assert!(!r.dead);
        assert!(r.loop_count >= 1);
    }

    #[test]
    fn image_search_paths_follow_the_original_order() {
        assert_eq!(
            image_search_paths("IMAGE_REANIM_SPROUT_BODY"),
            ["REANIM_SPROUT_BODY", "particles/REANIM_SPROUT_BODY", "reanim/SPROUT_BODY", "images/SPROUT_BODY"]
        );
    }

    #[test]
    fn matrix_from_unskewed_transform_is_scale_and_translate() {
        let t = ReanimTransform { x: 5.0, y: 6.0, sx: 2.0, sy: 3.0, ..def().tracks[1].transforms[0].clone() };
        assert_eq!(t.matrix().transform_point(1.0, 1.0), (7.0, 9.0));
    }
}
