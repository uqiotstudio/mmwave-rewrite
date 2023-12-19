extern crate derive_from_bytes;

use derive_from_bytes::FromBytes;

enum ParseError {
    DataLengthMismatch,
    MalformedData,
}

trait FromBytes
where
    Self: Sized,
{
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError>;

    fn size_of() -> usize {
        std::mem::size_of::<Self>()
    }

    fn size_of_val(&self) -> usize {
        std::mem::size_of_val(&self)
    }
}

impl FromBytes for u8 {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        if bytes.len() != 1 {
            Err(ParseError::DataLengthMismatch)
        } else {
            Ok(bytes[0])
        }
    }
}

impl FromBytes for u16 {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        if bytes.len() != 2 {
            Err(ParseError::DataLengthMismatch)
        } else {
            Ok(u16::from_ne_bytes([bytes[0], bytes[1]]))
        }
    }
}

impl FromBytes for u32 {
    fn from_bytes(bytes: &[u8]) -> Result<u32, ParseError> {
        if bytes.len() != 4 {
            Err(ParseError::DataLengthMismatch)
        } else {
            Ok(u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
    }
}

impl FromBytes for f32 {
    fn from_bytes(bytes: &[u8]) -> Result<f32, ParseError> {
        if bytes.len() != 4 {
            Err(ParseError::DataLengthMismatch)
        } else {
            Ok(f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
    }
}

impl<T: Default + Copy + FromBytes, const N: usize> FromBytes for [T; N] {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        let item_size = T::size_of();
        if bytes.len() < item_size * N {
            return Err(ParseError::DataLengthMismatch); // Not enough bytes to fill the array
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

impl<T: FromBytes> FromBytes for Vec<T> {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError>
    where
        Self: Sized,
    {
        let item_size = T::size_of();
        if bytes.len() % item_size != 0 {
            return Err(ParseError::DataLengthMismatch);
        }
        let mut items = Vec::new();
        for i in 0..(bytes.len() / item_size) {
            let element = T::from_bytes(&bytes[i * item_size..(i + 1) * item_size])?;
            items.push(element);
        }
        Ok(items)
    }

    fn size_of_val(&self) -> usize {
        // Get the size of all vals and sum them, instead of returning the standard vec size!
        self.iter().map(|t| t.size_of_val()).sum()
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
        let mut offset = 0;
        let frame_header =
            FrameHeader::from_bytes(&bytes[offset..offset + FrameHeader::size_of()])?;

        offset += FrameHeader::size_of();

        let frame_body =
            FrameBody::from_bytes(&bytes[offset..frame_header.packet_length as usize])?;

        if frame_body.tlvs.len() != frame_header.num_tlvs as usize {
            Err(ParseError::DataLengthMismatch)
        } else {
            Ok(Frame {
                frame_header,
                frame_body,
            })
        }
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

impl FromBytes for FrameBody {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError>
    where
        Self: Sized,
    {
        let mut tlvs = Vec::new();
        let mut offset = 0;

        while offset + TlvHeader::size_of() < bytes.len() {
            let tlv_header = TlvHeader::from_bytes(&bytes[offset..offset + TlvHeader::size_of()])?;

            offset += TlvHeader::size_of();

            // Slice the bytes to only the length of the tlv remaining!
            let bytes = &bytes[offset..offset + tlv_header.length as usize];

            let tlv_body = match tlv_header.tlv_type {
                TlvType::PointCloud => TlvBody::PointCloud(Vec::from_bytes(&bytes)?),
                TlvType::RangeProfile => TlvBody::RangeProfile(Vec::from_bytes(&bytes)?),
                TlvType::NoiseProfile => TlvBody::NoiseProfile(Vec::from_bytes(&bytes)?),
                TlvType::StaticAzimuthHeatmap => {
                    TlvBody::StatisticAzimuthHeatmap(Vec::from_bytes(&bytes)?)
                }
                TlvType::RangeDopplerHeatmap => TlvBody::RangeDopplerHeatmap,
                TlvType::Statistics => TlvBody::Statistics,
                TlvType::SideInfo => TlvBody::SideInfo(Vec::from_bytes(&bytes)?),
                TlvType::AzimuthElevationStaticHeatmap => {
                    TlvBody::AzimuthElevationStaticHeatmap(Vec::from_bytes(&bytes)?)
                }
                TlvType::Temperature => TlvBody::Temperature,
            };

            tlvs.push(Tlv {
                tlv_header,
                tlv_body,
            })
        }

        Ok(FrameBody { tlvs })
    }
}

struct Tlv {
    tlv_header: TlvHeader,
    tlv_body: TlvBody,
}

enum TlvBody {
    PointCloud(Vec<[f32; 4]>),
    RangeProfile(Vec<[u8; 2]>),
    NoiseProfile(Vec<u32>),
    StatisticAzimuthHeatmap(Vec<[u8; 4]>),
    RangeDopplerHeatmap,
    Statistics,
    SideInfo(Vec<[u16; 2]>),
    AzimuthElevationStaticHeatmap(Vec<[u8; 4]>),
    Temperature,
}

#[derive(FromBytes)]
struct TlvHeader {
    tlv_type: TlvType,
    length: u32,
}

// The full list of TLVTypes can be found at https://dev.ti.com/tirex/explore/node?node=A__ADnbI7zK9bSRgZqeAxprvQ__radar_toolbox__1AslXXD__LATEST in case you need to implement more later on.
// Note, the number assigned is IMPORTANT for the binary reading
enum TlvType {
    PointCloud = 1,
    RangeProfile = 2,
    NoiseProfile = 3,
    StaticAzimuthHeatmap = 4,
    RangeDopplerHeatmap = 5,
    Statistics = 6,
    SideInfo = 7,
    AzimuthElevationStaticHeatmap = 8,
    Temperature = 9,
}

impl FromBytes for TlvType {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError>
    where
        Self: Sized,
    {
        if bytes.len() != 4 {
            Err(ParseError::DataLengthMismatch)
        } else {
            let code = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            match code {
                1 => Ok(TlvType::PointCloud),
                2 => Ok(TlvType::RangeProfile),
                3 => Ok(TlvType::NoiseProfile),
                4 => Ok(TlvType::StaticAzimuthHeatmap),
                5 => Ok(TlvType::RangeDopplerHeatmap),
                6 => Ok(TlvType::Statistics),
                7 => Ok(TlvType::SideInfo),
                8 => Ok(TlvType::AzimuthElevationStaticHeatmap),
                9 => Ok(TlvType::Temperature),
                _ => Err(ParseError::MalformedData),
            }
        }
    }
}
