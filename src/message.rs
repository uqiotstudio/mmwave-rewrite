extern crate derive_from_bytes;

use derive_from_bytes::FromBytes;

#[derive(Debug)]
pub enum ParseError {
    DataLengthMismatch,
    MalformedData,
}

pub trait FromBytes
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

trait Header {
    // Returns the length in bytes of the associated body (excluding the header!!)
    fn get_body_length(&self) -> usize;
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
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        let mut index = 0;
        let mut results = Vec::new();
        while let Ok(t) = T::from_bytes(&bytes[index..]) {
            index += t.size_of_val();
            results.push(t);
        }
        Ok(results)
    }

    fn size_of_val(&self) -> usize {
        // Get the size of all vals and sum them, instead of returning the standard vec size!
        self.iter().map(|t| t.size_of_val()).sum()
    }
}

#[derive(FromBytes, Debug)]
pub struct Frame {
    #[Header(1)]
    pub frame_header: FrameHeader,
    #[Body(1)]
    pub frame_body: FrameBody,
}

#[derive(FromBytes, Debug)]
pub struct FrameHeader {
    pub magic_word: [u16; 4],
    pub version: u32,
    pub packet_length: u32,
    pub platform: u32,
    pub frame_number: u32,
    pub time: u32,
    pub num_detected: u32,
    pub num_tlvs: u32,
    pub subframe_num: u32,
}

impl Header for FrameHeader {
    fn get_body_length(&self) -> usize {
        self.packet_length as usize - std::mem::size_of::<Self>()
    }
}

#[derive(FromBytes, Debug)]
pub struct FrameBody {
    pub tlvs: Vec<Tlv>,
}

#[derive(Debug)]
pub struct Tlv {
    pub tlv_header: TlvHeader,
    pub tlv_body: TlvBody,
}

impl FromBytes for Tlv {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        let mut index = 0;
        let tlv_header = TlvHeader::from_bytes(&bytes[index..index + TlvHeader::size_of()])?;

        // Get the byte slice for the body
        index += TlvHeader::size_of();
        let bytes = &bytes[index..index + tlv_header.length as usize];

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

        Ok(Self {
            tlv_header,
            tlv_body,
        })
    }

    fn size_of_val(&self) -> usize {
        self.tlv_header.size_of_val() + self.tlv_body.size_of_val()
    }
}

#[derive(Debug)]
pub enum TlvBody {
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

impl TlvBody {
    fn size_of_val(&self) -> usize {
        match self {
            TlvBody::PointCloud(v) => v.size_of_val(),
            TlvBody::RangeProfile(v) => v.size_of_val(),
            TlvBody::NoiseProfile(v) => v.size_of_val(),
            TlvBody::StatisticAzimuthHeatmap(v) => v.size_of_val(),
            TlvBody::RangeDopplerHeatmap => todo!(),
            TlvBody::Statistics => todo!(),
            TlvBody::SideInfo(v) => v.size_of_val(),
            TlvBody::AzimuthElevationStaticHeatmap(v) => v.size_of_val(),
            TlvBody::Temperature => todo!(),
        }
    }
}

#[derive(FromBytes, Debug)]
pub struct TlvHeader {
    tlv_type: TlvType,
    length: u32,
}

impl Header for TlvHeader {
    fn get_body_length(&self) -> usize {
        self.length as usize
    }
}

// The full list of TLVTypes can be found at https://dev.ti.com/tirex/explore/node?node=A__ADnbI7zK9bSRgZqeAxprvQ__radar_toolbox__1AslXXD__LATEST in case you need to implement more later on.
// Note, the number assigned is IMPORTANT for the binary reading
#[derive(Debug)]
pub enum TlvType {
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
