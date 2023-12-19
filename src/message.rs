extern crate derive_from_bytes;

use derive_from_bytes::FromBytes;

enum ParseError {
    IncorrectByteCount,
}

trait FromBytes {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError>
    where
        Self: Sized;
}

impl FromBytes for u32 {
    fn from_bytes(bytes: &[u8]) -> Result<u32, ParseError> {
        if bytes.len() != 4 {
            Err(ParseError::IncorrectByteCount)
        } else {
            Ok(u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
    }
}

impl FromBytes for u16 {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError>
    where
        Self: Sized,
    {
        if bytes.len() != 2 {
            Err(ParseError::IncorrectByteCount)
        } else {
            Ok(u16::from_ne_bytes([bytes[0], bytes[1]]))
        }
    }
}

impl<T: Default + Copy + FromBytes, const N: usize> FromBytes for [T; N] {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        let item_size = std::mem::size_of::<T>();
        if bytes.len() < item_size * N {
            return Err(ParseError::IncorrectByteCount); // Not enough bytes to fill the array
        }

        let mut data: [T; N] = [T::default(); N]; // Initialize array with default values

        for (i, chunk) in bytes.chunks(item_size).enumerate().take(N) {
            match T::from_bytes(chunk) {
                Ok(item) => {
                    data[i] = item;
                }
                Err(e) => {
                    return Err(e); // Conversion failed
                }
            }
        }

        Ok(data)
    }
}

struct Frame {
    frame_header: FrameHeader,
    frame_body: FrameBody,
}

impl FromBytes for Frame {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError>
    where
        Self: Sized,
    {
        let index = 0;
        let frame_header =
            FrameHeader::from_bytes(&bytes[index..index + std::mem::size_of::<FrameHeader>()])?;

        // Parse the frame body, which itself cannot easily implement from_bytes because it needs header information!

        Err(ParseError::IncorrectByteCount)
    }
}

#[derive(FromBytes)]
struct FrameHeader {
    magic_word: [u16; 4],
    version: u32,
    packet_length: u32,
    platform: u32,
    frame_number: u32,
    time: u32,
    num_detected: u32,
    num_tlvs: u32,
    subframe_num: u32,
}

struct FrameBody {
    tlvs: Vec<Tlv>,
}

struct Tlv {
    tlv_header: TlvHeader,
    tlv_body: TlvBody,
}

struct TlvHeader {
    tlv_type: TlvType,
    length: usize,
}

struct TlvBody;

enum TlvType {}
