//! Compiled definitions (`compiled/<path>.compiled`): what the Windows pak
//! ships instead of the reanimation and particle XML.
//!
//! A file is a header (cookie `0xDEADFED4`, uncompressed size) and zlib data.
//! Uncompressed, it holds a schema hash and then the definition's structs as
//! the 32-bit game laid them out, each followed by its strings, arrays and
//! float tracks inline, in field-table order.
//!
//! Behaviour per ruslan831 decomp, `Sexy.TodLib/Definition.cpp`
//! (`DefinitionReadCompiledFile`, `DefinitionUncompressCompiledBuffer`,
//! `DefMapReadFromCache` and the `DefReadFromCache*` helpers).

/// A compiled file that can't be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompiledError(pub &'static str);

/// Checks the header and inflates the contents.
pub(crate) fn decompress(bytes: &[u8]) -> Result<Vec<u8>, CompiledError> {
    const COOKIE: u32 = 0xDEAD_FED4;
    if bytes.len() < 8 || u32::from_le_bytes(bytes[0..4].try_into().unwrap()) != COOKIE {
        return Err(CompiledError("bad header"));
    }
    let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let data = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&bytes[8..], size.max(1))
        .map_err(|_| CompiledError("zlib error"))?;
    if data.len() != size {
        return Err(CompiledError("wrong uncompressed size"));
    }
    Ok(data)
}

/// Reads through an uncompressed compiled definition.
pub(crate) struct CompiledReader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> CompiledReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn skip(&mut self, n: usize) -> Result<&'a [u8], CompiledError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or(CompiledError("truncated"))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    pub fn u32(&mut self) -> Result<u32, CompiledError> {
        Ok(u32::from_le_bytes(self.skip(4)?.try_into().unwrap()))
    }

    pub fn f32(&mut self) -> Result<f32, CompiledError> {
        self.u32().map(f32::from_bits)
    }

    /// Length-prefixed string (Latin-1).
    pub fn string(&mut self) -> Result<String, CompiledError> {
        let len = self.u32()? as usize;
        Ok(self.skip(len)?.iter().map(|&b| b as char).collect())
    }

    pub fn at_end(&self) -> bool {
        self.pos == self.data.len()
    }
}

/// A little-endian `u32` at `offset` in a raw struct.
pub(crate) fn u32_at(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap())
}
