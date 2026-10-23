//! PopCap image fonts: a `.txt` descriptor script plus glyph sheet images.
//!
//! The descriptor is a tiny language of `Define`s and `LayerSet…` commands
//! that give each layer an image, glyph rectangles, widths, offsets and
//! kerning. Fonts with several layers (e.g. a black outline under the main
//! glyphs) draw every layer for each character.
//!
//! Behaviour per ruslan831 decomp, `DescParser`, `FontData::HandleCommand`,
//! `ImageFont::CharWidthKern` and `ImageFont::DrawStringEx`. Only the default
//! point size at scale 1 is supported; that's all the game's fonts use so far.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::files::Files;
use crate::image::{DecodedImage, load_image};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum FontError {
    #[error("line {line}: {msg}")]
    Syntax { line: usize, msg: String },
    #[error("{command}: {msg}")]
    Command { command: String, msg: String },
}

/// Decodes font and string files, which are Latin-1.
pub fn latin1_to_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    /// Unquoted word: a number, layer name, command or `Define` name.
    Atom(String),
    /// Quoted string.
    Str(String),
    List(Vec<Value>),
}

/// Splits a descriptor into statements (`;`-terminated, `#` comment lines).
fn statements(text: &str) -> Result<Vec<(usize, Vec<Value>)>, FontError> {
    let mut out = Vec::new();
    let mut stack: Vec<Vec<Value>> = vec![Vec::new()];
    let mut start_line = 1;
    let mut line = 1;
    let mut at_line_start = true;
    let mut chars = text.chars().peekable();

    let syntax = |line, msg: &str| FontError::Syntax { line, msg: msg.to_owned() };

    while let Some(c) = chars.next() {
        match c {
            '\r' => {}
            '\n' => {
                line += 1;
                at_line_start = true;
            }
            ' ' | '\t' | ',' => {}
            '#' if at_line_start => {
                while chars.next_if(|&c| c != '\n').is_some() {}
            }
            ';' => {
                if stack.len() != 1 {
                    return Err(syntax(line, "unbalanced parentheses"));
                }
                let stmt = std::mem::take(&mut stack[0]);
                if !stmt.is_empty() {
                    out.push((start_line, stmt));
                }
            }
            '(' => stack.push(Vec::new()),
            ')' => {
                let list = stack.pop().filter(|_| !stack.is_empty()).ok_or_else(|| syntax(line, "unexpected ')'"))?;
                stack.last_mut().unwrap().push(Value::List(list));
            }
            '\'' | '"' => {
                let mut s = String::new();
                loop {
                    match chars.next() {
                        None => return Err(syntax(line, "unterminated string")),
                        Some('\\') => s.extend(chars.next()),
                        Some(q) if q == c => break,
                        Some(ch) => s.push(ch),
                    }
                }
                stack.last_mut().unwrap().push(Value::Str(s));
            }
            _ => {
                let mut s = String::from(c);
                while let Some(ch) = chars.next_if(|ch| !" \t\r\n,;()'\"".contains(*ch)) {
                    s.push(ch);
                }
                stack.last_mut().unwrap().push(Value::Atom(s));
            }
        }
        if !matches!(c, '\n' | ' ' | '\t' | '\r') {
            if at_line_start && stack.len() == 1 && stack[0].len() <= 1 {
                start_line = line;
            }
            at_line_start = false;
        }
    }
    if stack.len() != 1 || !stack[0].is_empty() {
        return Err(syntax(line, "missing ';' at end of file"));
    }
    Ok(out)
}

/// RGBA with 0..=255 components.
pub type Rgba = [u8; 4];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CharData {
    pub width: i32,
    pub image_rect: IRect,
    pub offset: (i32, i32),
    pub order: i32,
    pub kerning: HashMap<char, i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontLayer {
    pub name: String,
    /// Glyph sheet path inside the game files, without extension.
    pub image: Option<String>,
    pub draw_mode: i32,
    pub offset: (i32, i32),
    pub spacing: i32,
    pub point_size: i32,
    pub ascent: i32,
    pub ascent_padding: i32,
    pub height: i32,
    pub default_height: i32,
    pub line_spacing_offset: i32,
    pub color_mult: Rgba,
    pub color_add: Rgba,
    pub base_order: i32,
    pub chars: HashMap<char, CharData>,
}

impl FontLayer {
    fn new(name: String) -> Self {
        Self {
            name,
            image: None,
            draw_mode: -1,
            offset: (0, 0),
            spacing: 0,
            point_size: 0,
            ascent: 0,
            ascent_padding: 0,
            height: 0,
            default_height: 0,
            line_spacing_offset: 0,
            color_mult: [255; 4],
            color_add: [0; 4],
            base_order: 0,
            chars: HashMap::new(),
        }
    }

    fn char_data(&self, c: char) -> Option<&CharData> {
        self.chars.get(&c)
    }
}

/// A parsed font. Image sizes are needed to validate glyph rectangles, so
/// they're passed in via `image_size` while parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    pub layers: Vec<FontLayer>,
    pub default_point_size: i32,
    pub char_map: HashMap<char, char>,
    /// Metrics combined over all layers, as `GenerateActiveFontLayers` does.
    pub ascent: i32,
    pub ascent_padding: i32,
    pub height: i32,
    pub line_spacing_offset: i32,
}

/// One glyph to draw: the `src` rectangle of layer `layer`'s image at `dest`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub layer: usize,
    pub dest: (i32, i32),
    pub src: IRect,
    pub color: Rgba,
}

impl Font {
    /// Parses a font descriptor at `path` (e.g. `data/BrianneTod16.txt`).
    /// `image_size` returns the size of a layer image path, or `None` if it
    /// doesn't exist.
    pub fn parse(text: &str, path: &str, image_size: &dyn Fn(&str) -> Option<(u32, u32)>) -> Result<Self, FontError> {
        let dir = path.rfind(['/', '\\']).map_or("", |i| &path[..=i]);
        let mut defines: HashMap<String, Value> = HashMap::new();
        let mut font = Font {
            layers: Vec::new(),
            default_point_size: 0,
            char_map: HashMap::new(),
            ascent: 0,
            ascent_padding: 0,
            height: 0,
            line_spacing_offset: 0,
        };

        for (line, stmt) in statements(text)? {
            let Some(Value::Atom(cmd)) = stmt.first() else {
                return Err(FontError::Syntax { line, msg: "statement must start with a command".into() });
            };
            let cmd = cmd.to_ascii_lowercase();
            let err = |msg: &str| FontError::Command { command: format!("line {line}: {cmd}"), msg: msg.to_owned() };
            // Arguments with `Define`d names substituted.
            let args: Vec<Value> = stmt[1..]
                .iter()
                .map(|v| match v {
                    Value::Atom(a) => defines.get(&a.to_ascii_lowercase()).cloned().unwrap_or_else(|| v.clone()),
                    _ => v.clone(),
                })
                .collect();
            let want = |n: usize| if args.len() == n { Ok(()) } else { Err(err(&format!("expects {n} arguments"))) };

            match cmd.as_str() {
                "define" => {
                    want(2)?;
                    let Value::Atom(name) = &stmt[1] else { return Err(err("name must be a word")) };
                    defines.insert(name.to_ascii_lowercase(), args[1].clone());
                }
                "setdefaultpointsize" => {
                    want(1)?;
                    font.default_point_size = int(&args[0]).ok_or_else(|| err("bad point size"))?;
                }
                "createlayer" => {
                    want(1)?;
                    let name = string(&args[0]).ok_or_else(|| err("bad layer name"))?;
                    if font.layer_index(&name).is_some() {
                        return Err(err("layer already exists"));
                    }
                    font.layers.push(FontLayer::new(name));
                }
                _ if cmd.starts_with("layer") => {
                    let name = args.first().and_then(string).ok_or_else(|| err("missing layer name"))?;
                    let idx = font.layer_index(&name).ok_or_else(|| err(&format!("unknown layer {name:?}")))?;
                    let layer = &mut font.layers[idx];
                    match cmd.as_str() {
                        "layersetimage" => {
                            want(2)?;
                            let file = string(&args[1]).ok_or_else(|| err("bad image name"))?;
                            let image = format!("{dir}{file}");
                            if image_size(&image).is_none() {
                                return Err(err(&format!("image {image:?} not found")));
                            }
                            layer.image = Some(image);
                        }
                        "layersetdrawmode" => layer.draw_mode = int_arg(&args, 1, &err)?,
                        "layersetspacing" => layer.spacing = int_arg(&args, 1, &err)?,
                        "layersetpointsize" => layer.point_size = int_arg(&args, 1, &err)?,
                        "layersetascent" => layer.ascent = int_arg(&args, 1, &err)?,
                        "layersetascentpadding" => layer.ascent_padding = int_arg(&args, 1, &err)?,
                        "layersetheight" => layer.height = int_arg(&args, 1, &err)?,
                        "layersetlinespacingoffset" => layer.line_spacing_offset = int_arg(&args, 1, &err)?,
                        "layersetbaseorder" => layer.base_order = int_arg(&args, 1, &err)?,
                        "layersetoffset" => {
                            want(2)?;
                            layer.offset = pair(&args[1]).ok_or_else(|| err("bad offset"))?;
                        }
                        "layersetcolormult" => {
                            want(2)?;
                            layer.color_mult = color(&args[1]).ok_or_else(|| err("bad color"))?;
                        }
                        "layersetcoloradd" => {
                            want(2)?;
                            layer.color_add = color(&args[1]).ok_or_else(|| err("bad color"))?;
                        }
                        "layersetcharwidths" | "layersetcharorders" => {
                            want(3)?;
                            let chars = char_list(&args[1]).ok_or_else(|| err("bad character list"))?;
                            let values = int_list(&args[2]).ok_or_else(|| err("bad number list"))?;
                            if chars.len() != values.len() {
                                return Err(err("list sizes differ"));
                            }
                            for (c, v) in chars.into_iter().zip(values) {
                                let data = layer.chars.entry(c).or_default();
                                if cmd == "layersetcharwidths" { data.width = v } else { data.order = v }
                            }
                        }
                        "layersetimagemap" => {
                            want(3)?;
                            let image = layer.image.as_deref().ok_or_else(|| err("layer image not set"))?;
                            let (iw, ih) = image_size(image).unwrap_or_default();
                            let chars = char_list(&args[1]).ok_or_else(|| err("bad character list"))?;
                            let Value::List(rects) = &args[2] else { return Err(err("bad rectangle list")) };
                            if chars.len() != rects.len() {
                                return Err(err("list sizes differ"));
                            }
                            for (c, r) in chars.into_iter().zip(rects) {
                                let [x, y, w, h] = int_list(r)
                                    .and_then(|v| <[i32; 4]>::try_from(v).ok())
                                    .ok_or_else(|| err("bad rectangle"))?;
                                if x < 0 || y < 0 || x + w > iw as i32 || y + h > ih as i32 {
                                    return Err(err("image rectangle out of bounds"));
                                }
                                layer.chars.entry(c).or_default().image_rect = IRect { x, y, w, h };
                            }
                            layer.default_height =
                                layer.chars.values().map(|d| d.image_rect.h + d.offset.1).max().unwrap_or(0).max(0);
                        }
                        "layersetcharoffsets" => {
                            want(3)?;
                            let chars = char_list(&args[1]).ok_or_else(|| err("bad character list"))?;
                            let Value::List(offsets) = &args[2] else { return Err(err("bad offset list")) };
                            if chars.len() != offsets.len() {
                                return Err(err("list sizes differ"));
                            }
                            for (c, o) in chars.into_iter().zip(offsets) {
                                layer.chars.entry(c).or_default().offset = pair(o).ok_or_else(|| err("bad offset"))?;
                            }
                        }
                        "layersetkerningpairs" => {
                            want(3)?;
                            let Value::List(pairs) = &args[1] else { return Err(err("bad pair list")) };
                            let values = int_list(&args[2]).ok_or_else(|| err("bad number list"))?;
                            if pairs.len() != values.len() {
                                return Err(err("list sizes differ"));
                            }
                            for (p, v) in pairs.iter().zip(values) {
                                let s = string(p).ok_or_else(|| err("bad kerning pair"))?;
                                let mut it = s.chars();
                                let (Some(a), Some(b), None) = (it.next(), it.next(), it.next()) else {
                                    return Err(err("kerning pairs must be two characters"));
                                };
                                layer.chars.entry(a).or_default().kerning.insert(b, v);
                            }
                        }
                        // Point ranges and tags select layers by size/style; no
                        // PvZ font uses them, so all layers are always active.
                        "layerpointrange" | "layerrequiretags" | "layerexcludetags" | "layersetexinfo" => {}
                        _ => return Err(err("unknown command")),
                    }
                }
                _ => return Err(err("unknown command")),
            }
        }

        font.compute_metrics();
        Ok(font)
    }

    fn layer_index(&self, name: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.name.eq_ignore_ascii_case(name))
    }

    fn compute_metrics(&mut self) {
        for (i, layer) in self.layers.iter().enumerate() {
            self.ascent = self.ascent.max(layer.ascent);
            let h = if layer.height != 0 { layer.height } else { layer.default_height };
            self.height = self.height.max(h);
            if i == 0 || layer.ascent_padding < self.ascent_padding {
                self.ascent_padding = layer.ascent_padding;
            }
            if i == 0 || layer.line_spacing_offset > self.line_spacing_offset {
                self.line_spacing_offset = layer.line_spacing_offset;
            }
        }
    }

    fn mapped(&self, c: char) -> char {
        self.char_map.get(&c).copied().unwrap_or(c)
    }

    /// Advance for `c` when it follows `prev`, the widest over all layers.
    pub fn char_width_kern(&self, c: char, prev: Option<char>) -> i32 {
        let c = self.mapped(c);
        let prev = prev.map(|p| self.mapped(p));
        self.layers
            .iter()
            .map(|layer| {
                let width = layer.char_data(c).map_or(0, |d| d.width);
                let spacing = prev.map_or(0, |p| {
                    layer.spacing + layer.char_data(p).and_then(|d| d.kerning.get(&c)).copied().unwrap_or(0)
                });
                width + spacing
            })
            .fold(0, i32::max)
    }

    pub fn string_width(&self, text: &str) -> i32 {
        let mut prev = None;
        text.chars()
            .map(|c| {
                let w = self.char_width_kern(c, prev);
                prev = Some(c);
                w
            })
            .sum()
    }

    /// `Font::GetDescent`.
    pub fn descent(&self) -> i32 {
        self.height - self.ascent
    }

    /// `Font::GetLineSpacing`: the distance between baselines.
    pub fn line_spacing(&self) -> i32 {
        self.height + self.line_spacing_offset
    }

    /// Breaks `text` into lines no wider than `width`, like
    /// `Graphics::WriteWordWrapped`: at the last space before the line gets
    /// too wide (or mid-word if there's none), and at every `\n`. Returns
    /// each line with its baseline relative to the top of the text, and the
    /// text's total height. Colour codes (`^RRGGBB^`) aren't supported.
    ///
    /// Behaviour per ruslan831 decomp, `Graphics::WriteWordWrapped`.
    pub fn word_wrap(&self, text: &str, width: i32, line_spacing: i32) -> (Vec<(String, i32)>, i32) {
        let chars: Vec<char> = text.chars().collect();
        let mut lines = Vec::new();
        let mut y = self.ascent - self.ascent_padding;
        let (mut pos, mut line_start, mut cur_width) = (0, 0, 0);
        let mut space_pos: Option<usize> = None;
        let mut prev = None;
        let mut cur = None;

        while pos < chars.len() {
            let c = chars[pos];
            cur = Some(c);
            if c == ' ' {
                space_pos = Some(pos);
            } else if c == '\n' {
                // Forces a wrap; the newline itself is skipped next time round.
                cur_width = width + 1;
                space_pos = Some(pos);
                pos += 1;
            }
            cur_width += self.char_width_kern(c, prev);
            prev = Some(c);

            if cur_width > width {
                match space_pos {
                    Some(space) => {
                        lines.push((chars[line_start..space].iter().collect(), y));
                        pos = space + 1;
                        if c != '\n' {
                            while pos < chars.len() && chars[pos] == ' ' {
                                pos += 1;
                            }
                        }
                    }
                    None => {
                        // At least one character per line.
                        if pos < line_start + 1 {
                            pos += 1;
                        }
                        lines.push((chars[line_start..pos].iter().collect(), y));
                    }
                }
                line_start = pos;
                space_pos = None;
                cur_width = 0;
                prev = None;
                y += line_spacing;
            } else {
                pos += 1;
            }
        }

        if line_start < chars.len() {
            lines.push((chars[line_start..].iter().collect(), y));
            y += line_spacing;
        } else if cur == Some('\n') {
            y += line_spacing;
        }
        (lines, y + self.descent() - line_spacing)
    }

    /// Lays out `text` with its baseline at `y`, returning glyphs in draw
    /// order and the total advance.
    pub fn layout(&self, x: i32, y: i32, text: &str, color: Rgba) -> (Vec<Glyph>, i32) {
        let chars: Vec<char> = text.chars().map(|c| self.mapped(c)).collect();
        let mut glyphs = Vec::new();
        let mut cur_x = x;
        for (i, &c) in chars.iter().enumerate() {
            let next = chars.get(i + 1).copied();
            let mut max_x = cur_x;
            for (li, layer) in self.layers.iter().enumerate() {
                let data = layer.char_data(c).cloned().unwrap_or_default();
                let dest_x = cur_x + layer.offset.0 + data.offset.0;
                let dest_y = y - (layer.ascent - layer.offset.1 - data.offset.1);
                let spacing = next.map_or(0, |n| layer.spacing + data.kerning.get(&n).copied().unwrap_or(0));
                let blend = |k: usize| {
                    (color[k] as i32 * layer.color_mult[k] as i32 / 255 + layer.color_add[k] as i32).min(255) as u8
                };
                let order = (layer.base_order + data.order + 128).clamp(0, 255);
                glyphs.push((
                    order,
                    Glyph { layer: li, dest: (dest_x, dest_y), src: data.image_rect, color: [0, 1, 2, 3].map(blend) },
                ));
                max_x = max_x.max(cur_x + data.width + spacing);
            }
            cur_x = max_x;
        }
        // Stable: equal orders keep character-then-layer order, like the original's buckets.
        glyphs.sort_by_key(|(order, _)| *order);
        (glyphs.into_iter().map(|(_, g)| g).filter(|g| g.src.w > 0 && g.src.h > 0).collect(), cur_x - x)
    }
}

/// A font plus each layer's decoded glyph sheet (same order as `font.layers`).
#[derive(Debug)]
pub struct LoadedFont {
    pub font: Font,
    pub images: Vec<DecodedImage>,
}

/// Reads and parses a font descriptor, decoding its layer images with the
/// usual alpha-mask rules (most glyph sheets are a lone `_Name.png` mask).
pub fn load_font(files: &dyn Files, path: &str) -> Result<LoadedFont, FontError> {
    let not_found = || FontError::Command { command: path.to_owned(), msg: "font file not found".into() };
    let text = latin1_to_string(&files.read(path).ok_or_else(not_found)?);
    let decoded: RefCell<HashMap<String, Option<DecodedImage>>> = RefCell::default();
    let decode = |image: &str| {
        decoded
            .borrow_mut()
            .entry(image.to_owned())
            .or_insert_with(|| load_image(files, image, true, 0xFF_FFFF))
            .as_ref()
            .map(|i| (i.width, i.height))
    };
    let font = Font::parse(&text, path, &decode)?;
    let mut decoded = decoded.into_inner();
    let images = font
        .layers
        .iter()
        .map(|l| {
            let path = l.image.as_deref().unwrap_or_default();
            decoded.get_mut(path).and_then(Option::take).or_else(|| load_image(files, path, true, 0xFF_FFFF))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| FontError::Command { command: path.to_owned(), msg: "a layer has no image".into() })?;
    Ok(LoadedFont { font, images })
}

fn string(v: &Value) -> Option<String> {
    match v {
        Value::Atom(s) | Value::Str(s) => Some(s.clone()),
        Value::List(_) => None,
    }
}

fn int(v: &Value) -> Option<i32> {
    match v {
        Value::Atom(s) => s.parse().ok(),
        _ => None,
    }
}

fn int_arg(args: &[Value], i: usize, err: &dyn Fn(&str) -> FontError) -> Result<i32, FontError> {
    if args.len() != i + 1 {
        return Err(err(&format!("expects {} arguments", i + 1)));
    }
    int(&args[i]).ok_or_else(|| err("expected a number"))
}

fn int_list(v: &Value) -> Option<Vec<i32>> {
    match v {
        Value::List(items) => items.iter().map(int).collect(),
        _ => None,
    }
}

fn pair(v: &Value) -> Option<(i32, i32)> {
    let [a, b] = <[i32; 2]>::try_from(int_list(v)?).ok()?;
    Some((a, b))
}

fn char_list(v: &Value) -> Option<Vec<char>> {
    let Value::List(items) = v else { return None };
    items
        .iter()
        .map(|item| {
            let s = string(item)?;
            let mut it = s.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => Some(c),
                _ => None,
            }
        })
        .collect()
}

/// `GetColorFromDataElement`: a packed ARGB int (alpha 0 means opaque) or a
/// list of four 0..1 factors.
fn color(v: &Value) -> Option<Rgba> {
    match v {
        Value::List(items) => {
            let f: Vec<f64> = items.iter().map(|i| string(i)?.parse().ok()).collect::<Option<_>>()?;
            let [r, g, b, a] = <[f64; 4]>::try_from(f).ok()?;
            Some([r, g, b, a].map(|c| (c * 255.0) as i32 as u8))
        }
        Value::Atom(s) => {
            let n = s.parse::<i64>().ok()? as u32;
            let [a, r, g, b] = n.to_be_bytes();
            Some([r, g, b, if a == 0 { 255 } else { a }])
        }
        Value::Str(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESC: &str = "\
# A comment line ; with a semicolon
Define CharList ('A', 'B', \"'\");
Define WidthList (10, 12, 3);
Define RectList ((0, 0, 10, 20), (10, 0, 12, 20), (22, 0, 3, 5));
CreateLayer Outline;
LayerSetImage Outline 'FontOutline';
LayerSetAscent Outline 15;
LayerSetCharWidths Outline CharList WidthList;
LayerSetImageMap Outline CharList RectList;
LayerSetColorMult Outline 0;
CreateLayer Main;
LayerSetImage Main 'Font';
LayerSetAscent Main 16;
LayerSetCharWidths Main CharList WidthList;
LayerSetCharWidths Main (' ') (5);
LayerSetImageMap Main CharList RectList;
LayerSetCharOffsets Main ('B') ((1, -2));
LayerSetKerningPairs Main (\"AB\") (-3);
LayerSetLineSpacingOffset Main 2;
SetDefaultPointSize 16;
";

    fn parse() -> Font {
        Font::parse(DESC, "data/Font.txt", &|p| matches!(p, "data/Font" | "data/FontOutline").then_some((30, 20)))
            .unwrap()
    }

    #[test]
    fn parses_layers_defines_and_metrics() {
        let f = parse();
        assert_eq!(f.layers.len(), 2);
        assert_eq!(f.layers[1].image.as_deref(), Some("data/Font"));
        assert_eq!(f.layers[0].color_mult, [0, 0, 0, 255]);
        assert_eq!(f.layers[1].chars[&'\''].image_rect, IRect { x: 22, y: 0, w: 3, h: 5 });
        assert_eq!(f.layers[1].chars[&'B'].offset, (1, -2));
        assert_eq!((f.ascent, f.height, f.line_spacing_offset, f.default_point_size), (16, 20, 2, 16));
    }

    #[test]
    fn widths_use_kerning_and_widest_layer() {
        let f = parse();
        assert_eq!(f.char_width_kern('A', None), 10);
        // Main kerns A→B by -3; Outline doesn't, so the outline's width wins.
        assert_eq!(f.char_width_kern('B', Some('A')), 12);
        assert_eq!(f.string_width("AB "), 10 + 12 + 5);
    }

    #[test]
    fn layout_draws_each_layer_per_character_in_order() {
        let f = parse();
        let (glyphs, width) = f.layout(100, 50, "AB", [255, 255, 255, 128]);
        assert_eq!(width, 22);
        let summary: Vec<_> = glyphs.iter().map(|g| (g.layer, g.dest)).collect();
        // A: outline then main; B: outline then main (offset (1, -2) on main).
        assert_eq!(summary, [(0, (100, 35)), (1, (100, 34)), (0, (110, 35)), (1, (111, 32))]);
        assert_eq!(glyphs[0].color, [0, 0, 0, 128]);
        assert_eq!(glyphs[1].color, [255, 255, 255, 128]);
    }

    #[test]
    fn word_wrap_breaks_at_spaces_and_newlines() {
        let f = parse();
        let spacing = f.line_spacing();
        assert_eq!(spacing, 22);
        let (lines, height) = f.word_wrap("AB AB  AB", 30, spacing);
        // It breaks at the last space, so a double space keeps one at the end.
        assert_eq!(lines, [("AB".to_owned(), 16), ("AB ".to_owned(), 38), ("AB".to_owned(), 60)]);
        assert_eq!(height, 16 + 3 * 22 + f.descent() - 22);

        let (lines, _) = f.word_wrap("A\nB", 30, spacing);
        assert_eq!(lines, [("A".to_owned(), 16), ("B".to_owned(), 38)]);
        // No space to break at: break mid-word.
        let (lines, _) = f.word_wrap("ABAB", 30, spacing);
        assert_eq!(lines, [("AB".to_owned(), 16), ("AB".to_owned(), 38)]);
    }

    #[test]
    fn reports_errors_with_context() {
        let missing = Font::parse("CreateLayer Main; LayerSetImage Main 'X';", "data/f.txt", &|_| None);
        assert!(matches!(missing, Err(FontError::Command { msg, .. }) if msg.contains("not found")));
        assert!(matches!(Font::parse("CreateLayer Main", "f.txt", &|_| None), Err(FontError::Syntax { .. })));
    }
}
