//! Guitar Pro 6 (`.gpx`): the same XML score as Guitar Pro 7 has, kept in a container of
//! the program's own. The container is a small file system of 4096-byte sectors, usually
//! compressed as a stream of bits.

use super::{ImportError, Skipped};
use gtr_model::Score;

const SECTOR: usize = 0x1000;
const SCORE_FILE: &str = "score.gpif";
/// The most a container is allowed to unpack to: far more than any score needs.
const LIMIT: usize = 256 << 20;

fn corrupt(what: &str) -> ImportError {
    ImportError::Corrupt(what.into())
}

/// The bits of a compressed container, first bit of each byte first.
struct Bits<'a> {
    data: &'a [u8],
    position: usize,
}

impl Bits<'_> {
    fn bit(&mut self) -> Option<usize> {
        let byte = self.data.get(self.position / 8)?;
        let bit = (byte >> (7 - self.position % 8)) & 1;
        self.position += 1;
        Some(usize::from(bit))
    }

    /// A number written most significant bit first.
    fn number(&mut self, bits: usize) -> Option<usize> {
        (0..bits).try_fold(0, |value, _| Some(value << 1 | self.bit()?))
    }

    /// A number written least significant bit first.
    fn reversed(&mut self, bits: usize) -> Option<usize> {
        (0..bits).try_fold(0, |value, at| Some(value | self.bit()? << at))
    }
}

/// Unpacks a compressed container: runs of bytes given as they are, and runs copied from
/// what has been unpacked already.
fn unpack(data: &[u8]) -> Result<Vec<u8>, ImportError> {
    let length = data
        .get(..4)
        .ok_or_else(|| corrupt("the container is cut short"))?;
    let length = u32::from_le_bytes([length[0], length[1], length[2], length[3]]) as usize;
    if length > LIMIT {
        return Err(corrupt("the container claims to be implausibly large"));
    }
    let mut bits = Bits {
        data: &data[4..],
        position: 0,
    };
    let mut out: Vec<u8> = Vec::with_capacity(length);
    // The stream may end a little short of the length it gives; what there is, is used.
    let mut step = |out: &mut Vec<u8>| -> Option<()> {
        if bits.bit()? == 1 {
            let word = bits.number(4)?;
            let offset = bits.reversed(word)?;
            let size = bits.reversed(word)?;
            let from = out.len().checked_sub(offset)?;
            for at in from..from + size.min(offset) {
                out.push(out[at]);
            }
        } else {
            for _ in 0..bits.reversed(2)? {
                out.push(bits.number(8)? as u8);
            }
        }
        Some(())
    };
    while out.len() < length && step(&mut out).is_some() {}
    Ok(out)
}

fn int_at(data: &[u8], at: usize) -> Option<usize> {
    let bytes = data.get(at..at + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
}

/// Finds a file in an unpacked container. Each sector that starts with a 2 describes a
/// file: its name, its size, and the numbers of the sectors that hold it.
fn file(data: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut sector = SECTOR;
    while sector + 4 <= data.len() {
        let found = int_at(data, sector) == Some(2) && {
            let field = data.get(sector + 4..sector + 4 + 127)?;
            let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
            &field[..end] == name.as_bytes()
        };
        if found {
            let size = int_at(data, sector + 0x8C)?.min(LIMIT);
            let mut contents = Vec::with_capacity(size);
            let mut pointer = sector + 0x94;
            while let Some(index) = int_at(data, pointer).filter(|&index| index != 0) {
                let start = index.checked_mul(SECTOR)?;
                contents.extend(data.get(start..(start + SECTOR).min(data.len()))?);
                if contents.len() >= size {
                    break;
                }
                pointer += 4;
            }
            contents.truncate(size);
            return Some(contents);
        }
        sector += SECTOR;
    }
    None
}

pub(super) fn read(bytes: &[u8], skipped: &mut Skipped) -> Result<Score, ImportError> {
    let unpacked;
    let container = match bytes.get(..4) {
        Some(b"BCFZ") => {
            unpacked = unpack(&bytes[4..])?;
            unpacked.as_slice()
        }
        Some(b"BCFS") => bytes,
        _ => return Err(ImportError::UnknownFormat),
    };
    // The sectors are counted from after the four letters that name the container.
    let sectors = container
        .strip_prefix(b"BCFS")
        .ok_or_else(|| corrupt("the container did not unpack to a file system"))?;
    let score = file(sectors, SCORE_FILE).ok_or_else(|| corrupt("the container has no score"))?;
    let xml = String::from_utf8_lossy(&score);
    super::gp7::read_xml(&xml, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Packs bytes the plainest way the format allows: runs of up to three given as they are.
    fn pack(data: &[u8]) -> Vec<u8> {
        let mut bits: Vec<bool> = Vec::new();
        for run in data.chunks(3) {
            bits.push(false);
            bits.extend((0..2).map(|at| run.len() >> at & 1 == 1));
            for byte in run {
                bits.extend((0..8).rev().map(|at| byte >> at & 1 == 1));
            }
        }
        let mut out = (data.len() as u32).to_le_bytes().to_vec();
        out.extend(bits.chunks(8).map(|byte| {
            (byte.iter().enumerate())
                .fold(0u8, |value, (at, &bit)| value | u8::from(bit) << (7 - at))
        }));
        out
    }

    /// A file system holding one file, spread over two sectors that are not in order.
    fn file_system(name: &str, contents: &[u8]) -> Vec<u8> {
        let mut data = vec![0u8; SECTOR * 5];
        data[SECTOR..SECTOR + 4].copy_from_slice(&2u32.to_le_bytes());
        data[SECTOR + 4..SECTOR + 4 + name.len()].copy_from_slice(name.as_bytes());
        data[SECTOR + 0x8C..SECTOR + 0x90].copy_from_slice(&(contents.len() as u32).to_le_bytes());
        for (slot, sector) in [4u32, 3].into_iter().enumerate() {
            let at = SECTOR + 0x94 + 4 * slot;
            data[at..at + 4].copy_from_slice(&sector.to_le_bytes());
        }
        let (first, second) = contents.split_at(SECTOR.min(contents.len()));
        data[4 * SECTOR..4 * SECTOR + first.len()].copy_from_slice(first);
        data[3 * SECTOR..3 * SECTOR + second.len()].copy_from_slice(second);
        data
    }

    #[test]
    fn a_container_is_unpacked_and_its_files_found() {
        let text: Vec<u8> = (0..5000u32)
            .flat_map(|i| i.to_string().into_bytes())
            .collect();
        assert_eq!(unpack(&pack(&text)).unwrap(), text);

        // A run copied from earlier: after "abc", three bytes from three back, twice over.
        let mut bits = vec![false, true, true]; // three plain bytes
        for byte in b"abc" {
            bits.extend((0..8).rev().map(|at| byte >> at & 1 == 1));
        }
        for _ in 0..2 {
            bits.push(true);
            bits.extend([false, false, true, false]); // numbers of two bits
            bits.extend([true, true, true, true]); // offset 3, size 3, lowest bit first
        }
        let mut packed = 9u32.to_le_bytes().to_vec();
        packed.extend(bits.chunks(8).map(|byte| {
            (byte.iter().enumerate())
                .fold(0u8, |value, (at, &bit)| value | u8::from(bit) << (7 - at))
        }));
        assert_eq!(unpack(&packed).unwrap(), b"abcabcabc");

        let contents: Vec<u8> = (0..SECTOR + 700).map(|i| (i % 251) as u8).collect();
        let system = file_system("score.gpif", &contents);
        assert_eq!(file(&system, "score.gpif").unwrap(), contents);
        assert!(file(&system, "other").is_none());
        // Damage is an error or a short result, never a panic.
        assert!(unpack(&[1, 2]).is_err());
        assert!(unpack(&[255, 255, 255, 255, 0]).is_err());
        assert!(read(b"BCFZ\x10\x00\x00\x00\xff\xff", &mut Skipped::default()).is_err());
        assert!(file(&system[..SECTOR + 200], "score.gpif").is_none());
    }
}
