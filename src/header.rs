use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// The byte length of the identifier, which is a prefix of the original document's sha512 hash.
pub const IDENTIFIER_LENGTH: usize = 5;

/// `Sha512Array` is a alias for an [`u8`] array that is the length of a sha512 output.
pub(crate) type Sha512Array = [u8; 64];

/// `MetaHeader` is a header that appears in a metadata chunk.
// This has a fixed "index" of `0xFFFF`
#[derive(Debug, PartialEq)]
pub struct MetaHeader {
    pub hash: Sha512Array,
    /// Number of original input blocks.  None of these are ever printed.
    pub original_count: u16,
    /// Number of total recovery blocks.
    pub recovery_count: u16,
    /// Number of bytes per shard, excluding headers.
    pub shard_bytes: u64,
}

impl MetaHeader {
    pub const LENGTH: usize =
        size_of::<Sha512Array>() + size_of::<u16>() + size_of::<u16>() + size_of::<u64>();
}

/// `PayloadHeader` is a header that appears in a payload chunk.
#[derive(Debug)]
pub struct PayloadHeader {
    /// Index for a recovery chunk; can be between 0 and 65534 inclusive.
    pub index: u16,
    /// Identifier for this document.
    pub identifier: [u8; IDENTIFIER_LENGTH],
}

impl PayloadHeader {
    pub const LENGTH: usize = size_of::<u16>() + IDENTIFIER_LENGTH;
}

/// Header that gets written to one QR code.
#[derive(Debug)]
pub enum Header {
    Meta(MetaHeader),
    Payload(PayloadHeader),
}

impl Header {
    pub fn read_from(reader: &mut impl Read) -> Result<Self> {
        let index = reader.read_u16::<LittleEndian>()?;
        if index == u16::MAX {
            // This is a metadata block
            let mut result = MetaHeader {
                hash: [0; 64],
                original_count: 0,
                recovery_count: 0,
                shard_bytes: 0,
            };
            reader.read_exact(result.hash.as_mut_slice())?;
            result.original_count = reader.read_u16::<LittleEndian>()?;
            result.recovery_count = reader.read_u16::<LittleEndian>()?;
            result.shard_bytes = reader.read_u64::<LittleEndian>()?;

            Ok(Header::Meta(result))
        } else {
            let mut identifier: [u8; IDENTIFIER_LENGTH] = [0; IDENTIFIER_LENGTH];
            reader.read_exact(&mut identifier)?;

            Ok(Header::Payload(PayloadHeader { index, identifier }))
        }
    }

    pub fn write_to(&self, writer: &mut impl Write) -> Result<()> {
        match self {
            Header::Meta(m) => {
                writer.write_u16::<LittleEndian>(u16::MAX)?;
                writer.write_all(m.hash.as_slice())?;
                writer.write_u16::<LittleEndian>(m.original_count)?;
                writer.write_u16::<LittleEndian>(m.recovery_count)?;
                writer.write_u64::<LittleEndian>(m.shard_bytes)?;
            }
            Header::Payload(p) => {
                writer.write_u16::<LittleEndian>(p.index)?;
                writer.write_all(&p.identifier)?;
            }
        }
        Ok(())
    }
}
