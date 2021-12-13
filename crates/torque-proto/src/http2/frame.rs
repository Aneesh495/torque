//! HTTP/2 (RFC 7540) frame structures, binary serialization, and frame parser.

use bytes::Bytes;
use torque_core::error::{Result, TorqueError};

/// HTTP/2 frame type identifier (RFC 7540 Section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Data = 0x0,
    Headers = 0x1,
    Priority = 0x2,
    RstStream = 0x3,
    Settings = 0x4,
    PushPromise = 0x5,
    Ping = 0x6,
    GoAway = 0x7,
    WindowUpdate = 0x8,
    Continuation = 0x9,
    Unknown(u8),
}

impl From<u8> for FrameType {
    fn from(val: u8) -> Self {
        match val {
            0x0 => FrameType::Data,
            0x1 => FrameType::Headers,
            0x2 => FrameType::Priority,
            0x3 => FrameType::RstStream,
            0x4 => FrameType::Settings,
            0x5 => FrameType::PushPromise,
            0x6 => FrameType::Ping,
            0x7 => FrameType::GoAway,
            0x8 => FrameType::WindowUpdate,
            0x9 => FrameType::Continuation,
            other => FrameType::Unknown(other),
        }
    }
}

impl From<FrameType> for u8 {
    fn from(ft: FrameType) -> Self {
        match ft {
            FrameType::Data => 0x0,
            FrameType::Headers => 0x1,
            FrameType::Priority => 0x2,
            FrameType::RstStream => 0x3,
            FrameType::Settings => 0x4,
            FrameType::PushPromise => 0x5,
            FrameType::Ping => 0x6,
            FrameType::GoAway => 0x7,
            FrameType::WindowUpdate => 0x8,
            FrameType::Continuation => 0x9,
            FrameType::Unknown(val) => val,
        }
    }
}

/// HTTP/2 standard frame header (9 octets: Length 24b, Type 8b, Flags 8b, R 1b + StreamID 31b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub length: u32,
    pub frame_type: FrameType,
    pub flags: u8,
    pub stream_id: u32,
}

impl FrameHeader {
    pub const SIZE: usize = 9;

    /// Parse a 9-byte slice into an HTTP/2 frame header.
    pub fn parse(buf: &[u8]) -> Result<Self> {
        if buf.len() < Self::SIZE {
            return Err(TorqueError::protocol("Buffer too small for frame header"));
        }

        let length = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32);
        let frame_type = FrameType::from(buf[3]);
        let flags = buf[4];
        let stream_id = (((buf[5] as u32) & 0x7F) << 24)
            | ((buf[6] as u32) << 16)
            | ((buf[7] as u32) << 8)
            | (buf[8] as u32);

        Ok(Self {
            length,
            frame_type,
            flags,
            stream_id,
        })
    }

    /// Serialize header into a 9-byte array.
    pub fn encode(&self) -> [u8; 9] {
        let mut buf = [0u8; 9];
        buf[0] = ((self.length >> 16) & 0xFF) as u8;
        buf[1] = ((self.length >> 8) & 0xFF) as u8;
        buf[2] = (self.length & 0xFF) as u8;
        buf[3] = self.frame_type.into();
        buf[4] = self.flags;
        buf[5] = ((self.stream_id >> 24) & 0x7F) as u8;
        buf[6] = ((self.stream_id >> 16) & 0xFF) as u8;
        buf[7] = ((self.stream_id >> 8) & 0xFF) as u8;
        buf[8] = (self.stream_id & 0xFF) as u8;
        buf
    }

    /// Check if END_STREAM flag (0x1) is set.
    pub fn has_end_stream(&self) -> bool {
        (self.flags & 0x1) != 0
    }

    /// Check if END_HEADERS flag (0x4) is set.
    pub fn has_end_headers(&self) -> bool {
        (self.flags & 0x4) != 0
    }

    /// Check if ACK flag (0x1) is set on Settings or Ping.
    pub fn has_ack(&self) -> bool {
        (self.flags & 0x1) != 0
    }
}

/// Parsed HTTP/2 frame payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FramePayload {
    Data {
        data: Bytes,
        end_stream: bool,
    },
    Headers {
        header_block: Bytes,
        end_stream: bool,
        end_headers: bool,
    },
    Priority {
        exclusive: bool,
        stream_dependency: u32,
        weight: u8,
    },
    RstStream {
        error_code: u32,
    },
    Settings {
        ack: bool,
        settings: Vec<(u16, u32)>,
    },
    Ping {
        ack: bool,
        opaque_data: [u8; 8],
    },
    GoAway {
        last_stream_id: u32,
        error_code: u32,
        debug_data: Bytes,
    },
    WindowUpdate {
        window_size_increment: u32,
    },
    Continuation {
        header_block: Bytes,
        end_headers: bool,
    },
    Unknown {
        frame_type: u8,
        data: Bytes,
    },
}

/// Fully assembled HTTP/2 frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http2Frame {
    pub header: FrameHeader,
    pub payload: FramePayload,
}

impl Http2Frame {
    /// Construct a DATA frame.
    pub fn data(stream_id: u32, data: Bytes, end_stream: bool) -> Self {
        let flags = if end_stream { 0x1 } else { 0x0 };
        Self {
            header: FrameHeader {
                length: data.len() as u32,
                frame_type: FrameType::Data,
                flags,
                stream_id,
            },
            payload: FramePayload::Data { data, end_stream },
        }
    }

    /// Construct a HEADERS frame.
    pub fn headers(stream_id: u32, header_block: Bytes, end_stream: bool, end_headers: bool) -> Self {
        let mut flags = 0;
        if end_stream {
            flags |= 0x1;
        }
        if end_headers {
            flags |= 0x4;
        }
        Self {
            header: FrameHeader {
                length: header_block.len() as u32,
                frame_type: FrameType::Headers,
                flags,
                stream_id,
            },
            payload: FramePayload::Headers {
                header_block,
                end_stream,
                end_headers,
            },
        }
    }

    /// Construct a SETTINGS frame.
    pub fn settings(settings: Vec<(u16, u32)>, ack: bool) -> Self {
        let length = if ack { 0 } else { (settings.len() * 6) as u32 };
        let flags = if ack { 0x1 } else { 0x0 };
        Self {
            header: FrameHeader {
                length,
                frame_type: FrameType::Settings,
                flags,
                stream_id: 0,
            },
            payload: FramePayload::Settings { ack, settings },
        }
    }

    /// Construct a WINDOW_UPDATE frame.
    pub fn window_update(stream_id: u32, increment: u32) -> Self {
        Self {
            header: FrameHeader {
                length: 4,
                frame_type: FrameType::WindowUpdate,
                flags: 0,
                stream_id,
            },
            payload: FramePayload::WindowUpdate {
                window_size_increment: increment,
            },
        }
    }

    /// Construct a PING frame.
    pub fn ping(opaque_data: [u8; 8], ack: bool) -> Self {
        let flags = if ack { 0x1 } else { 0x0 };
        Self {
            header: FrameHeader {
                length: 8,
                frame_type: FrameType::Ping,
                flags,
                stream_id: 0,
            },
            payload: FramePayload::Ping { ack, opaque_data },
        }
    }

    /// Construct a GOAWAY frame.
    pub fn goaway(last_stream_id: u32, error_code: u32, debug_data: Bytes) -> Self {
        let length = (8 + debug_data.len()) as u32;
        Self {
            header: FrameHeader {
                length,
                frame_type: FrameType::GoAway,
                flags: 0,
                stream_id: 0,
            },
            payload: FramePayload::GoAway {
                last_stream_id,
                error_code,
                debug_data,
            },
        }
    }

    /// Parse an HTTP/2 frame from raw bytes.
    /// Returns `Ok(Some((frame, consumed)))` if a complete frame was parsed,
    /// or `Ok(None)` if more data is required.
    pub fn parse(src: &[u8]) -> Result<Option<(Self, usize)>> {
        if src.len() < FrameHeader::SIZE {
            return Ok(None);
        }

        let header = FrameHeader::parse(&src[..FrameHeader::SIZE])?;
        let total_size = FrameHeader::SIZE + (header.length as usize);

        if src.len() < total_size {
            return Ok(None);
        }

        let payload_bytes = &src[FrameHeader::SIZE..total_size];
        let payload = match header.frame_type {
            FrameType::Data => FramePayload::Data {
                data: Bytes::copy_from_slice(payload_bytes),
                end_stream: header.has_end_stream(),
            },
            FrameType::Headers => FramePayload::Headers {
                header_block: Bytes::copy_from_slice(payload_bytes),
                end_stream: header.has_end_stream(),
                end_headers: header.has_end_headers(),
            },
            FrameType::Priority => {
                if payload_bytes.len() < 5 {
                    return Err(TorqueError::protocol("Priority frame payload < 5 bytes"));
                }
                let raw = u32::from_be_bytes(payload_bytes[..4].try_into().unwrap());
                let exclusive = (raw & 0x8000_0000) != 0;
                let stream_dependency = raw & 0x7FFF_FFFF;
                let weight = payload_bytes[4];
                FramePayload::Priority {
                    exclusive,
                    stream_dependency,
                    weight,
                }
            }
            FrameType::RstStream => {
                if payload_bytes.len() < 4 {
                    return Err(TorqueError::protocol("RST_STREAM payload < 4 bytes"));
                }
                let error_code = u32::from_be_bytes(payload_bytes[..4].try_into().unwrap());
                FramePayload::RstStream { error_code }
            }
            FrameType::Settings => {
                let ack = header.has_ack();
                let mut settings = Vec::new();
                if !ack {
                    if payload_bytes.len() % 6 != 0 {
                        return Err(TorqueError::protocol("Settings payload not multiple of 6"));
                    }
                    for chunk in payload_bytes.chunks_exact(6) {
                        let id = u16::from_be_bytes([chunk[0], chunk[1]]);
                        let val = u32::from_be_bytes([chunk[2], chunk[3], chunk[4], chunk[5]]);
                        settings.push((id, val));
                    }
                }
                FramePayload::Settings { ack, settings }
            }
            FrameType::Ping => {
                if payload_bytes.len() != 8 {
                    return Err(TorqueError::protocol("Ping payload must be exactly 8 bytes"));
                }
                let mut opaque_data = [0u8; 8];
                opaque_data.copy_from_slice(payload_bytes);
                FramePayload::Ping {
                    ack: header.has_ack(),
                    opaque_data,
                }
            }
            FrameType::GoAway => {
                if payload_bytes.len() < 8 {
                    return Err(TorqueError::protocol("GoAway payload < 8 bytes"));
                }
                let last_stream_id =
                    u32::from_be_bytes(payload_bytes[..4].try_into().unwrap()) & 0x7FFF_FFFF;
                let error_code = u32::from_be_bytes(payload_bytes[4..8].try_into().unwrap());
                let debug_data = Bytes::copy_from_slice(&payload_bytes[8..]);
                FramePayload::GoAway {
                    last_stream_id,
                    error_code,
                    debug_data,
                }
            }
            FrameType::WindowUpdate => {
                if payload_bytes.len() < 4 {
                    return Err(TorqueError::protocol("WindowUpdate payload < 4 bytes"));
                }
                let increment =
                    u32::from_be_bytes(payload_bytes[..4].try_into().unwrap()) & 0x7FFF_FFFF;
                FramePayload::WindowUpdate {
                    window_size_increment: increment,
                }
            }
            FrameType::Continuation => FramePayload::Continuation {
                header_block: Bytes::copy_from_slice(payload_bytes),
                end_headers: header.has_end_headers(),
            },
            FrameType::Unknown(val) => FramePayload::Unknown {
                frame_type: val,
                data: Bytes::copy_from_slice(payload_bytes),
            },
            FrameType::PushPromise => FramePayload::Unknown {
                frame_type: 0x5,
                data: Bytes::copy_from_slice(payload_bytes),
            },
        };

        Ok(Some((Self { header, payload }, total_size)))
    }

    /// Serialize entire frame into byte buffer.
    pub fn encode(&self, dst: &mut Vec<u8>) {
        let header_bytes = self.header.encode();
        dst.extend_from_slice(&header_bytes);

        match &self.payload {
            FramePayload::Data { data, .. } => {
                dst.extend_from_slice(data);
            }
            FramePayload::Headers { header_block, .. } => {
                dst.extend_from_slice(header_block);
            }
            FramePayload::Priority {
                exclusive,
                stream_dependency,
                weight,
            } => {
                let mut raw = *stream_dependency & 0x7FFF_FFFF;
                if *exclusive {
                    raw |= 0x8000_0000;
                }
                dst.extend_from_slice(&raw.to_be_bytes());
                dst.push(*weight);
            }
            FramePayload::RstStream { error_code } => {
                dst.extend_from_slice(&error_code.to_be_bytes());
            }
            FramePayload::Settings { ack, settings } => {
                if !*ack {
                    for (id, val) in settings {
                        dst.extend_from_slice(&id.to_be_bytes());
                        dst.extend_from_slice(&val.to_be_bytes());
                    }
                }
            }
            FramePayload::Ping { opaque_data, .. } => {
                dst.extend_from_slice(opaque_data);
            }
            FramePayload::GoAway {
                last_stream_id,
                error_code,
                debug_data,
            } => {
                dst.extend_from_slice(&(last_stream_id & 0x7FFF_FFFF).to_be_bytes());
                dst.extend_from_slice(&error_code.to_be_bytes());
                dst.extend_from_slice(debug_data);
            }
            FramePayload::WindowUpdate {
                window_size_increment,
            } => {
                dst.extend_from_slice(&(window_size_increment & 0x7FFF_FFFF).to_be_bytes());
            }
            FramePayload::Continuation { header_block, .. } => {
                dst.extend_from_slice(header_block);
            }
            FramePayload::Unknown { data, .. } => {
                dst.extend_from_slice(data);
            }
        }
    }
}
