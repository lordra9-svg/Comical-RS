//! Reads the installer database inside an MSI package without Windows.
//!
//! An MSI is an OLE compound file. Its tables are stored in streams whose names
//! are obfuscated by the installer toolchain, and the collision-free product
//! loader lives in an embedded cabinet. Checking only that a file is an OLE
//! container misses the failures that matter: a build that drops the file
//! association component, renames the shortcut, packages the wrong version or
//! ships a different executable still produces a valid container.
//!
//! This module walks the container, concatenates the table stream bytes (their
//! strings are stored uncompressed) and lists the embedded cabinet payload, so
//! `verify` can assert on what the installer actually contains.

/// Compound file signature.
const MAGIC: [u8; 8] = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
/// Marks the end of a sector chain.
const END_OF_CHAIN: u32 = 0xffff_fffe;
/// The first id that is a marker rather than a sector number.
const FIRST_MARKER: u32 = 0xffff_fffc;
/// Upper bound on a single chain, so a corrupt table cannot loop forever.
const MAX_SECTORS: usize = 1 << 22;
/// Directory entries are fixed size.
const ENTRY_SIZE: usize = 128;
/// Smallest possible cabinet header: no optional reserved fields.
const CAB_HEADER: usize = 36;
/// A cabinet file entry has a fixed 16-byte header before its name.
const CAB_FILE_HEADER: usize = 16;

/// One file inside the embedded cabinet.
#[derive(Debug)]
pub struct PayloadFile {
    /// Name as the installer database knows it, for example `ComicalEXE`.
    pub name: String,
    /// Uncompressed size, which the installer database also records.
    pub size: u32,
}

/// What `verify` needs to know about a package.
#[derive(Debug)]
pub struct Package {
    /// Stream names found in the compound file.
    pub streams: Vec<String>,
    /// Table stream bytes, without the cabinet.
    pub tables: Vec<u8>,
    /// Files in the embedded cabinet.
    pub payload: Vec<PayloadFile>,
}

/// Reads `bytes` as an MSI package.
pub fn read(bytes: &[u8]) -> Result<Package, String> {
    let file = CompoundFile::parse(bytes)?;
    let mut tables = Vec::new();
    let mut payload = Vec::new();
    let mut cabinets = 0;

    for entry in file.streams() {
        let data = file.contents(entry)?;
        if data.starts_with(b"MSCF") {
            cabinets += 1;
            payload.extend(cabinet(&data)?);
        } else {
            tables.extend_from_slice(&data);
        }
    }

    if cabinets == 0 {
        return Err("no embedded cabinet, so the package carries no files".to_owned());
    }
    if payload.is_empty() {
        return Err("the embedded cabinet lists no files".to_owned());
    }
    if tables.is_empty() {
        return Err("no table streams, so the installer has no database".to_owned());
    }

    Ok(Package {
        streams: file.names(),
        tables,
        payload,
    })
}

/// Lists the files of an embedded cabinet.
///
/// Only the fixed header is decoded: the payload names and their uncompressed
/// sizes are enough to prove which executable the installer carries, and they
/// are stored uncompressed ahead of the (possibly MSZIP) data.
fn cabinet(bytes: &[u8]) -> Result<Vec<PayloadFile>, String> {
    if bytes.len() < CAB_HEADER {
        return Err("cabinet is shorter than its header".to_owned());
    }
    let claimed = read_u32(bytes, 0x08)? as usize;
    if claimed > bytes.len() {
        return Err(format!(
            "cabinet claims {claimed} bytes but only {} are present",
            bytes.len()
        ));
    }
    let count = read_u16(bytes, 0x1c)? as usize;
    if count == 0 {
        return Err("cabinet lists no files".to_owned());
    }

    let mut at = read_u32(bytes, 0x10)? as usize;
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        let size = read_u32(bytes, at)?;
        let name_at = at + CAB_FILE_HEADER;
        let rest = bytes
            .get(name_at..)
            .ok_or("cabinet file entry is truncated")?;
        let end = rest
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("cabinet file name is not terminated")?;
        files.push(PayloadFile {
            name: String::from_utf8_lossy(&rest[..end]).into_owned(),
            size,
        });
        at = name_at + end + 1;
    }
    Ok(files)
}

/// One directory entry of the compound file.
struct Entry {
    name: String,
    kind: u8,
    start: u32,
    size: u64,
}

/// The parts of a compound file needed to read its streams.
struct CompoundFile<'a> {
    bytes: &'a [u8],
    sector_size: usize,
    mini_sector_size: usize,
    cutoff: u64,
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    /// The root entry's data, which holds every mini stream.
    mini_stream: Vec<u8>,
    entries: Vec<Entry>,
}

impl<'a> CompoundFile<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() < 512 {
            return Err("too short to be a compound file".to_owned());
        }
        if !bytes.starts_with(&MAGIC) {
            return Err("not an OLE compound file".to_owned());
        }

        let sector_shift = read_u16(bytes, 0x1e)?;
        let mini_shift = read_u16(bytes, 0x20)?;
        if !(7..=12).contains(&sector_shift) || mini_shift > 12 {
            return Err(format!(
                "implausible sector sizes: 1<<{sector_shift} and 1<<{mini_shift}"
            ));
        }
        let sector_size = 1usize << sector_shift;
        let mini_sector_size = 1usize << mini_shift;
        let cutoff = u64::from(read_u32(bytes, 0x38)?);
        let first_directory = read_u32(bytes, 0x30)?;
        let first_mini_fat = read_u32(bytes, 0x3c)?;
        let mini_fat_count = read_u32(bytes, 0x40)?;

        // The allocation table is named by the header's DIFAT, which spills
        // into a chain of DIFAT sectors.
        let mut fat_sectors = Vec::new();
        for index in 0..109 {
            let id = read_u32(bytes, 0x4c + index * 4)?;
            if id < FIRST_MARKER {
                fat_sectors.push(id);
            }
        }
        let mut next = read_u32(bytes, 0x44)?;
        for _ in 0..1 << 16 {
            if next >= FIRST_MARKER {
                break;
            }
            let sector = sector(bytes, sector_size, next)?;
            for chunk in sector[..sector_size - 4].as_chunks::<4>().0 {
                let id = u32::from_le_bytes(*chunk);
                if id < FIRST_MARKER {
                    fat_sectors.push(id);
                }
            }
            next = read_u32(sector, sector_size - 4)?;
        }

        let mut fat = Vec::new();
        for id in fat_sectors {
            let sector = sector(bytes, sector_size, id)?;
            for chunk in sector.as_chunks::<4>().0 {
                fat.push(u32::from_le_bytes(*chunk));
            }
        }
        if fat.is_empty() {
            return Err("compound file has no allocation table".to_owned());
        }

        let mut entries = Vec::new();
        for id in chain(&fat, first_directory)? {
            let sector = sector(bytes, sector_size, id)?;
            for chunk in sector.as_chunks::<ENTRY_SIZE>().0 {
                let kind = chunk[0x42];
                if kind == 0 {
                    continue;
                }
                let name_length = u16::from_le_bytes([chunk[0x40], chunk[0x41]]) as usize;
                let name = if name_length >= 2 {
                    let units: Vec<u16> = chunk[..(name_length - 2).min(62)]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| u16::from_le_bytes(*pair))
                        .collect();
                    String::from_utf16_lossy(&units)
                } else {
                    String::new()
                };
                entries.push(Entry {
                    name,
                    kind,
                    start: u32::from_le_bytes(chunk[0x74..0x78].try_into().expect("4 bytes")),
                    size: u64::from_le_bytes(chunk[0x78..0x80].try_into().expect("8 bytes")),
                });
            }
        }

        // The root entry is the mini stream: the streams below the cutoff live
        // inside it, chained through the mini allocation table.
        let mut mini_stream = Vec::new();
        if let Some(root) = entries.iter().find(|entry| entry.kind == 5) {
            for id in chain(&fat, root.start)? {
                mini_stream.extend_from_slice(sector(bytes, sector_size, id)?);
            }
            mini_stream.truncate(root.size as usize);
        }

        let mut mini_fat = Vec::new();
        if mini_fat_count > 0 {
            for id in chain(&fat, first_mini_fat)? {
                let sector = sector(bytes, sector_size, id)?;
                for chunk in sector.as_chunks::<4>().0 {
                    mini_fat.push(u32::from_le_bytes(*chunk));
                }
            }
        }

        Ok(Self {
            bytes,
            sector_size,
            mini_sector_size,
            cutoff,
            fat,
            mini_fat,
            mini_stream,
            entries,
        })
    }

    /// Every stream entry, in directory order.
    fn streams(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == 2 && entry.size > 0)
    }

    /// Every stream name, in directory order.
    fn names(&self) -> Vec<String> {
        self.streams().map(|entry| entry.name.clone()).collect()
    }

    /// Reads a stream's bytes, truncating the padding of the last sector.
    fn contents(&self, entry: &Entry) -> Result<Vec<u8>, String> {
        let size = entry.size as usize;
        let mut data = Vec::with_capacity(size);
        // Without a mini allocation table every stream lives in regular
        // sectors, which is what a small package written by hand looks like.
        if entry.size < self.cutoff && !self.mini_fat.is_empty() {
            for id in chain(&self.mini_fat, entry.start)? {
                let at = id as usize * self.mini_sector_size;
                let end = at + self.mini_sector_size;
                data.extend_from_slice(
                    self.mini_stream
                        .get(at..end)
                        .ok_or("mini sector is outside the mini stream")?,
                );
            }
        } else {
            for id in chain(&self.fat, entry.start)? {
                data.extend_from_slice(sector(self.bytes, self.sector_size, id)?);
            }
        }
        data.truncate(size);
        Ok(data)
    }
}

/// Follows a sector chain in `table`.
fn chain(table: &[u32], start: u32) -> Result<Vec<u32>, String> {
    let mut ids = Vec::new();
    let mut next = start;
    while next != END_OF_CHAIN {
        let index = next as usize;
        if index >= table.len() {
            return Err(format!(
                "sector chain leaves the allocation table at 0x{next:08x}"
            ));
        }
        ids.push(next);
        if ids.len() > MAX_SECTORS {
            return Err("sector chain loops".to_owned());
        }
        next = table[index];
    }
    Ok(ids)
}

/// The sectors follow a header sized like one sector.
fn sector(bytes: &[u8], sector_size: usize, id: u32) -> Result<&[u8], String> {
    let start = (id as usize + 1) * sector_size;
    bytes
        .get(start..start + sector_size)
        .ok_or_else(|| format!("sector {id} is beyond the end of the file"))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("truncated header at 0x{offset:x}"))?;
    Ok(u16::from_le_bytes(
        slice.try_into().expect("two bytes were sliced"),
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated header at 0x{offset:x}"))?;
    Ok(u32::from_le_bytes(
        slice.try_into().expect("four bytes were sliced"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FREE: u32 = 0xffff_ffff;
    const SECTOR: usize = 512;

    /// Writes one directory entry.
    fn entry(into: &mut [u8], name: &str, kind: u8, start: u32, size: u64) {
        let units: Vec<u16> = name.encode_utf16().collect();
        for (index, unit) in units.iter().enumerate() {
            into[index * 2..index * 2 + 2].copy_from_slice(&unit.to_le_bytes());
        }
        let length = ((units.len() + 1) * 2) as u16;
        into[0x40..0x42].copy_from_slice(&length.to_le_bytes());
        into[0x42] = kind;
        into[0x74..0x78].copy_from_slice(&start.to_le_bytes());
        into[0x78..0x80].copy_from_slice(&size.to_le_bytes());
    }

    /// A cabinet holding the named files, all of the given sizes.
    fn cabinet(files: &[(&str, u32)]) -> Vec<u8> {
        // Header, one folder, then the file entries.
        let mut bytes = vec![0u8; CAB_HEADER + 8];
        bytes[..4].copy_from_slice(b"MSCF");
        bytes[0x18] = 3; // minor version
        bytes[0x19] = 1; // major version
        bytes[0x1a..0x1c].copy_from_slice(&1u16.to_le_bytes());
        bytes[0x1c..0x1e].copy_from_slice(&(files.len() as u16).to_le_bytes());
        bytes[0x10..0x14].copy_from_slice(&((CAB_HEADER + 8) as u32).to_le_bytes());

        for (name, size) in files {
            let mut header = [0u8; CAB_FILE_HEADER];
            header[..4].copy_from_slice(&size.to_le_bytes());
            header[14..16].copy_from_slice(&0x20u16.to_le_bytes());
            bytes.extend_from_slice(&header);
            bytes.extend_from_slice(name.as_bytes());
            bytes.push(0);
        }
        let total = bytes.len() as u32;
        bytes[0x08..0x0c].copy_from_slice(&total.to_le_bytes());
        bytes
    }

    /// Lays out a v3 compound file whose streams all live in regular sectors.
    fn compound(streams: &[(&str, &[u8])]) -> Vec<u8> {
        let mut fat: Vec<u32> = vec![FREE; SECTOR / 4];
        // Sector 0 holds the allocation table itself, sector 1 the directory.
        fat[0] = 0xffff_fffd;
        fat[1] = END_OF_CHAIN;
        let mut data: Vec<u8> = Vec::new();
        let mut directory = vec![0u8; SECTOR];
        entry(
            &mut directory[..ENTRY_SIZE],
            "Root Entry",
            5,
            END_OF_CHAIN,
            0,
        );

        for (index, (name, payload)) in streams.iter().enumerate() {
            let first = 2 + (data.len() / SECTOR) as u32;
            let count = payload.len().div_ceil(SECTOR).max(1);
            for offset in 0..count {
                let id = first + offset as u32;
                fat[id as usize] = if offset + 1 == count {
                    END_OF_CHAIN
                } else {
                    id + 1
                };
            }
            data.extend_from_slice(payload);
            data.resize(data.len().div_ceil(SECTOR) * SECTOR, 0);
            entry(
                &mut directory[(index + 1) * ENTRY_SIZE..(index + 2) * ENTRY_SIZE],
                name,
                2,
                first,
                payload.len() as u64,
            );
        }

        assemble(fat, &directory, &[], &data)
    }

    /// Lays out a compound file whose streams live in the mini stream.
    fn compound_with_mini(streams: &[(&str, &[u8])]) -> Vec<u8> {
        const MINI: usize = 64;
        let mut mini_fat: Vec<u32> = vec![FREE; SECTOR / 4];
        let mut mini_stream: Vec<u8> = Vec::new();
        let mut directory = vec![0u8; SECTOR];

        for (index, (name, payload)) in streams.iter().enumerate() {
            let first = (mini_stream.len() / MINI) as u32;
            let count = payload.len().div_ceil(MINI).max(1);
            for offset in 0..count {
                let id = first + offset as u32;
                mini_fat[id as usize] = if offset + 1 == count {
                    END_OF_CHAIN
                } else {
                    id + 1
                };
            }
            mini_stream.extend_from_slice(payload);
            mini_stream.resize(mini_stream.len().div_ceil(MINI) * MINI, 0);
            entry(
                &mut directory[(index + 1) * ENTRY_SIZE..(index + 2) * ENTRY_SIZE],
                name,
                2,
                first,
                payload.len() as u64,
            );
        }

        // Sector 0 FAT, 1 directory, 2 mini FAT, the rest the mini stream.
        let sectors = mini_stream.len().div_ceil(SECTOR).max(1);
        let mut fat: Vec<u32> = vec![FREE; SECTOR / 4];
        fat[0] = 0xffff_fffd;
        fat[1] = END_OF_CHAIN;
        fat[2] = END_OF_CHAIN;
        for offset in 0..sectors {
            let id = 3 + offset as u32;
            fat[id as usize] = if offset + 1 == sectors {
                END_OF_CHAIN
            } else {
                id + 1
            };
        }
        entry(
            &mut directory[..ENTRY_SIZE],
            "Root Entry",
            5,
            3,
            mini_stream.len() as u64,
        );

        let mut mini_fat_bytes = vec![0u8; SECTOR];
        for (index, value) in mini_fat.iter().enumerate() {
            mini_fat_bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        assemble(fat, &directory, &[mini_fat_bytes], &mini_stream)
    }

    /// Wraps a header, FAT sector, directory sector and data sectors.
    fn assemble(fat: Vec<u32>, directory: &[u8], extras: &[Vec<u8>], data: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0u8; SECTOR];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[0x18..0x1a].copy_from_slice(&0x003eu16.to_le_bytes());
        bytes[0x1a..0x1c].copy_from_slice(&3u16.to_le_bytes());
        bytes[0x1c..0x1e].copy_from_slice(&0xfffeu16.to_le_bytes());
        bytes[0x1e..0x20].copy_from_slice(&9u16.to_le_bytes());
        bytes[0x20..0x22].copy_from_slice(&6u16.to_le_bytes());
        bytes[0x2c..0x30].copy_from_slice(&1u32.to_le_bytes());
        bytes[0x30..0x34].copy_from_slice(&1u32.to_le_bytes());
        bytes[0x38..0x3c].copy_from_slice(&4096u32.to_le_bytes());
        let first_mini_fat = if extras.is_empty() { END_OF_CHAIN } else { 2 };
        bytes[0x3c..0x40].copy_from_slice(&first_mini_fat.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(&(extras.len() as u32).to_le_bytes());
        bytes[0x44..0x48].copy_from_slice(&END_OF_CHAIN.to_le_bytes());

        // Only the first FAT sector is named in the header; the rest of the
        // DIFAT is unused because these test files have a single FAT sector.
        bytes[0x4c..0x50].copy_from_slice(&0u32.to_le_bytes());
        for index in 1..109 {
            bytes[0x4c + index * 4..0x50 + index * 4].copy_from_slice(&FREE.to_le_bytes());
        }

        let mut fat_sector = vec![0u8; SECTOR];
        for (index, value) in fat.iter().enumerate() {
            fat_sector[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&fat_sector);
        bytes.extend_from_slice(directory);
        for extra in extras {
            bytes.extend_from_slice(extra);
        }
        bytes.extend_from_slice(data);
        // Every sector is whole, and the last one may need padding.
        bytes.resize(bytes.len().div_ceil(SECTOR) * SECTOR, 0);
        bytes
    }

    #[test]
    fn collects_table_bytes_and_the_cabinet_payload() {
        let cab = cabinet(&[("ComicalEXE", 30_987_233)]);
        let tables = b"reg0EF4C9B41Comical.CBZComic Book ZIP Archive\0";
        let bytes = compound_with_mini(&[("mangled", tables), ("cabinet", &cab)]);
        let package = read(&bytes).expect("read package");

        assert_eq!(package.payload.len(), 1);
        assert_eq!(package.payload[0].name, "ComicalEXE");
        assert_eq!(package.payload[0].size, 30_987_233);
        assert!(package.streams.contains(&"mangled".to_owned()));
        assert!(
            package
                .tables
                .windows(b"Comical.CBZ".len())
                .any(|window| window == b"Comical.CBZ"),
            "table bytes are kept verbatim"
        );
        assert!(
            !package.tables.windows(4).any(|window| window == b"MSCF"),
            "the cabinet is not part of the tables"
        );
    }

    #[test]
    fn reads_a_stream_larger_than_one_sector() {
        let cab = cabinet(&[("ComicalEXE", 1234)]);
        let mut tables = b"start".to_vec();
        tables.resize(5000, b'x');
        let bytes = compound(&[("tables", &tables), ("cabinet", &cab)]);
        let package = read(&bytes).expect("read package");

        assert_eq!(package.tables.len(), 5000);
        assert_eq!(package.payload[0].size, 1234);
    }

    #[test]
    fn lists_every_cabinet_file() {
        let cab = cabinet(&[("ComicalEXE", 10), ("OtherEXE", 20)]);
        let bytes = compound(&[("tables", b"table bytes"), ("cabinet", &cab)]);
        let package = read(&bytes).expect("read package");

        let sizes: Vec<u32> = package.payload.iter().map(|file| file.size).collect();
        assert_eq!(sizes, vec![10, 20]);
    }

    #[test]
    fn rejects_a_file_that_is_not_a_compound_file() {
        let error = read(b"PK\x03\x04 not an installer").expect_err("must fail");
        assert!(
            error.contains("too short") || error.contains("OLE"),
            "{error}"
        );
    }

    #[test]
    fn rejects_a_package_without_a_cabinet() {
        let bytes = compound(&[("tables", b"table bytes")]);
        let error = read(&bytes).expect_err("must fail");
        assert!(error.contains("no embedded cabinet"), "{error}");
    }

    #[test]
    fn rejects_a_cabinet_that_lists_no_files() {
        let cab = cabinet(&[]);
        let bytes = compound(&[("tables", b"table bytes"), ("cabinet", &cab)]);
        let error = read(&bytes).expect_err("must fail");
        assert!(error.contains("no files"), "{error}");
    }

    #[test]
    fn rejects_a_cabinet_that_claims_more_than_it_holds() {
        let mut cab = cabinet(&[("ComicalEXE", 10)]);
        cab[0x08..0x0c].copy_from_slice(&u32::MAX.to_le_bytes());
        let bytes = compound(&[("tables", b"table bytes"), ("cabinet", &cab)]);
        let error = read(&bytes).expect_err("must fail");
        assert!(error.contains("claims"), "{error}");
    }
}
