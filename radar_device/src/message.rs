use crate::error::ParseError;

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
        let item_size = T::size_of();
        if bytes.len() % item_size != 0 || (bytes.len() / item_size) <= 0 {
            return Err(ParseError::DataLengthMismatch);
        }
        let mut items = Vec::new();
        for i in 0..(bytes.len() / item_size) - 1 {
            let element = T::from_bytes(
                &bytes
                    .get(i * item_size..(i + 1) * item_size)
                    .ok_or(ParseError::DataLengthMismatch)?,
            )?;
            items.push(element);
        }
        Ok(items)
    }

    fn size_of_val(&self) -> usize {
        // Get the size of all vals and sum them, instead of returning the standard vec size!
        self.iter().map(|t| t.size_of_val()).sum()
    }
}

#[derive(Debug)]
pub struct Frame {
    pub frame_header: FrameHeader,
    pub frame_body: FrameBody,
}

#[derive(Debug)]
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

impl FromBytes for FrameHeader {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        let mut index: usize = 0;
        Ok(FrameHeader {
            magic_word: {
                let parsed = <[u16; 4] as FromBytes>::from_bytes(
                    &bytes[index..index + <[u16; 4]>::size_of()],
                )?;
                index += <[u16; 4]>::size_of();
                parsed
            },
            version: {
                let parsed =
                    <u32 as FromBytes>::from_bytes(&bytes[index..index + <u32>::size_of()])?;
                index += <u32>::size_of();
                parsed
            },
            packet_length: {
                let parsed =
                    <u32 as FromBytes>::from_bytes(&bytes[index..index + <u32>::size_of()])?;
                index += <u32>::size_of();
                parsed
            },
            platform: {
                let parsed =
                    <u32 as FromBytes>::from_bytes(&bytes[index..index + <u32>::size_of()])?;
                index += <u32>::size_of();
                parsed
            },
            frame_number: {
                let parsed =
                    <u32 as FromBytes>::from_bytes(&bytes[index..index + <u32>::size_of()])?;
                index += <u32>::size_of();
                parsed
            },
            time: {
                let parsed =
                    <u32 as FromBytes>::from_bytes(&bytes[index..index + <u32>::size_of()])?;
                index += <u32>::size_of();
                parsed
            },
            num_detected: {
                let parsed =
                    <u32 as FromBytes>::from_bytes(&bytes[index..index + <u32>::size_of()])?;
                index += <u32>::size_of();
                parsed
            },
            num_tlvs: {
                let parsed =
                    <u32 as FromBytes>::from_bytes(&bytes[index..index + <u32>::size_of()])?;
                index += <u32>::size_of();
                parsed
            },
            subframe_num: {
                let parsed = <u32 as FromBytes>::from_bytes(&bytes[index..])?;
                index += parsed.size_of_val();
                parsed
            },
        })
    }
}

#[derive(Debug)]
pub struct FrameBody {
    pub tlvs: Vec<Tlv>,
}

impl FrameBody {
    pub fn from_bytes(bytes: &[u8], num_tlvs: usize) -> Result<FrameBody, ParseError> {
        let mut tlvs = Vec::new();
        let mut offset: usize = 0;
        for _ in 0..num_tlvs {
            let tlv_header = TlvHeader::from_bytes(
                &bytes
                    .get(offset..offset + TlvHeader::size_of())
                    .ok_or(ParseError::DataLengthMismatch)?,
            )?;
            offset += std::mem::size_of::<TlvHeader>();

            let bytes = &bytes
                .get(offset..offset + tlv_header.length as usize)
                .ok_or(ParseError::DataLengthMismatch)?;

            let tlv_body = match tlv_header.tlv_type {
                TlvType::PointCloud => TlvBody::PointCloud(Vec::from_bytes(&bytes)?),
                TlvType::RangeProfile => TlvBody::RangeProfile(Vec::from_bytes(&bytes)?),
                TlvType::NoiseProfile => TlvBody::NoiseProfile(Vec::from_bytes(&bytes)?),
                TlvType::StaticAzimuthHeatmap => {
                    TlvBody::StatisticAzimuthHeatmap(Vec::from_bytes(&bytes)?)
                }
                TlvType::RangeDopplerHeatmap => {
                    return Err(ParseError::UnimplementedTlvType(
                        "RangeDopplerHeatMap".to_owned(),
                    ))
                }
                TlvType::Statistics => TlvBody::Statistics(<_>::from_bytes(&bytes)?),
                TlvType::SideInfo => TlvBody::SideInfo(Vec::from_bytes(&bytes)?),
                TlvType::AzimuthElevationStaticHeatmap => {
                    TlvBody::AzimuthElevationStaticHeatmap(Vec::from_bytes(&bytes)?)
                }
                TlvType::Temperature => {
                    return Err(ParseError::UnimplementedTlvType("Temperature".to_owned()))
                }
            };

            offset += tlv_header.length as usize;
            tlvs.push(Tlv {
                tlv_header,
                tlv_body,
            });
        }

        Ok(Self { tlvs })
    }
}

#[derive(Debug)]
pub struct Tlv {
    pub tlv_header: TlvHeader,
    pub tlv_body: TlvBody,
}

#[derive(Debug)]
pub enum TlvBody {
    PointCloud(Vec<[f32; 4]>),
    RangeProfile(Vec<[u8; 2]>),
    NoiseProfile(Vec<u32>),
    StatisticAzimuthHeatmap(Vec<[u8; 4]>),
    RangeDopplerHeatmap,
    Statistics([u32; 24 / std::mem::size_of::<u32>()]),
    SideInfo(Vec<[u16; 2]>),
    AzimuthElevationStaticHeatmap(Vec<[u8; 4]>),
    Temperature,
}

#[derive(Debug)]
pub struct TlvHeader {
    tlv_type: TlvType,
    length: u32,
}

impl FromBytes for TlvHeader {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        Ok(TlvHeader {
            tlv_type: TlvType::from_bytes(
                &bytes
                    .get(0..TlvType::size_of())
                    .ok_or(ParseError::DataLengthMismatch)?,
            )?,
            length: u32::from_bytes(
                &bytes
                    .get(TlvType::size_of()..TlvType::size_of() + 4)
                    .ok_or(ParseError::DataLengthMismatch)?,
            )?,
        })
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

    fn size_of() -> usize {
        4
    }
}
