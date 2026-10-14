//! A small, panic-free reader for ZIP archives (the container of `.docx` and legacy `.celtx`
//! files): stored and deflated entries, read from the central directory, with a fallback scan of
//! local headers for archives whose directory is missing or damaged. ZIP64 and encrypted entries
//! are refused.

use std::io::Read;

/// Largest entry we inflate.
pub const MAX_ENTRY: usize = 64 << 20;
/// Most entries we look at.
const MAX_ENTRIES: usize = 20_000;

/// One file in the archive.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    method: u16,
    flags: u16,
    comp_size: usize,
    /// Whether `comp_size` is known (a local-header scan may not know it).
    sized: bool,
    /// Offset of the local file header.
    local: usize,
}

/// An opened archive.
pub struct Zip<'a> {
    bytes: &'a [u8],
    pub entries: Vec<Entry>,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    let s = b.get(i..i.checked_add(2)?)?;
    Some(u16::from_le_bytes([*s.first()?, *s.get(1)?]))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    let s = b.get(i..i.checked_add(4)?)?;
    Some(u32::from_le_bytes([*s.first()?, *s.get(1)?, *s.get(2)?, *s.get(3)?]))
}

/// Whether the bytes look like a ZIP archive.
pub fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

impl<'a> Zip<'a> {
    /// Open an archive.
    pub fn open(bytes: &'a [u8]) -> Result<Zip<'a>, String> {
        let entries = match central(bytes) {
            Some(e) if !e.is_empty() => e,
            _ => scan_local(bytes),
        };
        if entries.is_empty() {
            return Err("not a readable ZIP archive".into());
        }
        Ok(Zip { bytes, entries })
    }

    /// The entry with this name (case-insensitive, `/` or `\` separators).
    pub fn find(&self, name: &str) -> Option<&Entry> {
        let want = name.replace('\\', "/").to_ascii_lowercase();
        self.entries.iter().find(|e| e.name.replace('\\', "/").to_ascii_lowercase() == want)
    }

    /// The contents of an entry.
    pub fn read(&self, e: &Entry) -> Result<Vec<u8>, String> {
        if e.flags & 1 != 0 {
            return Err(format!("{} is encrypted", e.name));
        }
        let b = self.bytes;
        if u32_at(b, e.local) != Some(0x0403_4b50) {
            return Err(format!("{}: damaged archive", e.name));
        }
        let name_len = usize::from(u16_at(b, e.local + 26).unwrap_or(0));
        let extra_len = usize::from(u16_at(b, e.local + 28).unwrap_or(0));
        let start = e.local.saturating_add(30).saturating_add(name_len).saturating_add(extra_len);
        let rest = b.get(start..).ok_or_else(|| format!("{}: damaged archive", e.name))?;
        // An unknown size (a data descriptor follows the data): inflate until the stream ends.
        let data = if e.sized { rest.get(..e.comp_size.min(rest.len())).unwrap_or(rest) } else { rest };
        match e.method {
            0 => Ok(data.get(..data.len().min(MAX_ENTRY)).unwrap_or(data).to_vec()),
            8 => {
                let mut out = Vec::new();
                let mut d = flate2::read::DeflateDecoder::new(data).take(MAX_ENTRY as u64 + 1);
                // A damaged stream still yields what was inflated before the damage.
                let r = d.read_to_end(&mut out);
                if out.len() > MAX_ENTRY {
                    return Err(format!("{} is too large", e.name));
                }
                if r.is_err() && out.is_empty() {
                    return Err(format!("{}: damaged archive", e.name));
                }
                Ok(out)
            }
            m => Err(format!("{}: unsupported compression method {m}", e.name)),
        }
    }

    /// Read an entry by name.
    pub fn read_named(&self, name: &str) -> Option<Vec<u8>> {
        let e = self.find(name)?;
        self.read(e).ok()
    }
}

/// Entries from the central directory.
fn central(b: &[u8]) -> Option<Vec<Entry>> {
    let tail = b.len().saturating_sub(65_557);
    let eocd = (tail..b.len().saturating_sub(21)).rev().find(|&i| u32_at(b, i) == Some(0x0605_4b50))?;
    let count = usize::from(u16_at(b, eocd + 10)?);
    let cd = u32_at(b, eocd + 16)?;
    if cd == u32::MAX || count == usize::from(u16::MAX) {
        return None; // ZIP64: fall back to the local headers
    }
    let mut i = cd as usize;
    let mut out = Vec::new();
    for _ in 0..count.min(MAX_ENTRIES) {
        if u32_at(b, i)? != 0x0201_4b50 {
            break;
        }
        let flags = u16_at(b, i + 8)?;
        let method = u16_at(b, i + 10)?;
        let comp_size = u32_at(b, i + 20)? as usize;
        let name_len = usize::from(u16_at(b, i + 28)?);
        let extra_len = usize::from(u16_at(b, i + 30)?);
        let comment_len = usize::from(u16_at(b, i + 32)?);
        let local = u32_at(b, i + 42)? as usize;
        let name = b.get(i + 46..(i + 46).checked_add(name_len)?)?;
        out.push(Entry { name: String::from_utf8_lossy(name).into_owned(), method, flags, comp_size, sized: true, local });
        i = i.checked_add(46 + name_len + extra_len + comment_len)?;
    }
    Some(out)
}

/// Entries found by scanning for local file headers.
fn scan_local(b: &[u8]) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 30 <= b.len() && out.len() < MAX_ENTRIES {
        if u32_at(b, i) != Some(0x0403_4b50) {
            i += 1;
            continue;
        }
        let flags = u16_at(b, i + 6).unwrap_or(0);
        let method = u16_at(b, i + 8).unwrap_or(0);
        let comp_size = u32_at(b, i + 18).unwrap_or(0) as usize;
        let name_len = usize::from(u16_at(b, i + 26).unwrap_or(0));
        let extra_len = usize::from(u16_at(b, i + 28).unwrap_or(0));
        let Some(name) = b.get(i + 30..i + 30 + name_len) else { break };
        // Sizes come after the data when bit 3 is set; they are then unknown here.
        let known = if flags & 8 != 0 { 0 } else { comp_size };
        out.push(Entry { name: String::from_utf8_lossy(name).into_owned(), method, flags, comp_size: known, sized: flags & 8 == 0, local: i });
        i += 30 + name_len + extra_len + known.max(1);
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write;

    /// Build a ZIP archive of deflated entries (for tests).
    pub fn build(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut cd = Vec::new();
        for (name, data) in files {
            let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            enc.write_all(data).unwrap();
            let comp = enc.finish().unwrap();
            let crc = crc(data);
            let off = out.len() as u32;
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&[20, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&(comp.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&comp);
            cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            cd.extend_from_slice(&[20, 0, 20, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
            cd.extend_from_slice(&crc.to_le_bytes());
            cd.extend_from_slice(&(comp.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
            cd.extend_from_slice(&[0; 12]);
            cd.extend_from_slice(&off.to_le_bytes());
            cd.extend_from_slice(name.as_bytes());
        }
        let cd_off = out.len() as u32;
        out.extend_from_slice(&cd);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&[0; 2]);
        out
    }

    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
            }
        }
        !c
    }

    #[test]
    fn reads_back_entries() {
        let z = build(&[("a.txt", b"hello"), ("dir/b.xml", b"<x/>")]);
        let zip = super::Zip::open(&z).unwrap();
        assert_eq!(zip.read_named("A.TXT").unwrap(), b"hello");
        assert_eq!(zip.read_named("dir/b.xml").unwrap(), b"<x/>");
        // Without the central directory, the local headers still work.
        let cut = z.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        let zip = super::Zip::open(&z[..cut]).unwrap();
        assert_eq!(zip.read_named("a.txt").unwrap(), b"hello");
    }
}
