//! HTTP/2 stream state machine (RFC 7540 Section 5.1).

use torque_core::error::{Result, TorqueError};

/// Stream lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamState {
    Idle,
    ReservedLocal,
    ReservedRemote,
    Open,
    HalfClosedLocal,
    HalfClosedRemote,
    Closed,
}

/// Representation of an active or transitioning HTTP/2 stream.
#[derive(Debug, Clone)]
pub struct Http2Stream {
    pub id: u32,
    pub state: StreamState,
    pub weight: u8,
    pub dependency: u32,
    pub exclusive: bool,
}

impl Http2Stream {
    /// Create a new stream in Idle state.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            state: StreamState::Idle,
            weight: 16,
            dependency: 0,
            exclusive: false,
        }
    }

    /// Process transmission of an outgoing HEADERS frame.
    pub fn send_headers(&mut self, end_stream: bool) -> Result<()> {
        match self.state {
            StreamState::Idle => {
                if end_stream {
                    self.state = StreamState::HalfClosedLocal;
                } else {
                    self.state = StreamState::Open;
                }
                Ok(())
            }
            StreamState::ReservedLocal => {
                if end_stream {
                    self.state = StreamState::Closed;
                } else {
                    self.state = StreamState::HalfClosedRemote;
                }
                Ok(())
            }
            StreamState::Open => {
                if end_stream {
                    self.state = StreamState::HalfClosedLocal;
                }
                Ok(())
            }
            StreamState::HalfClosedRemote => {
                if end_stream {
                    self.state = StreamState::Closed;
                }
                Ok(())
            }
            other => Err(TorqueError::Http2Stream {
                stream_id: self.id,
                error_code: 1, // PROTOCOL_ERROR
                detail: format!("Cannot send HEADERS in {:?} state", other),
            }),
        }
    }

    /// Process receipt of an incoming HEADERS frame.
    pub fn recv_headers(&mut self, end_stream: bool) -> Result<()> {
        match self.state {
            StreamState::Idle => {
                if end_stream {
                    self.state = StreamState::HalfClosedRemote;
                } else {
                    self.state = StreamState::Open;
                }
                Ok(())
            }
            StreamState::ReservedRemote => {
                if end_stream {
                    self.state = StreamState::Closed;
                } else {
                    self.state = StreamState::HalfClosedLocal;
                }
                Ok(())
            }
            StreamState::Open => {
                if end_stream {
                    self.state = StreamState::HalfClosedRemote;
                }
                Ok(())
            }
            StreamState::HalfClosedLocal => {
                if end_stream {
                    self.state = StreamState::Closed;
                }
                Ok(())
            }
            other => Err(TorqueError::Http2Stream {
                stream_id: self.id,
                error_code: 1,
                detail: format!("Cannot receive HEADERS in {:?} state", other),
            }),
        }
    }

    /// Process transmission of an outgoing DATA frame.
    pub fn send_data(&mut self, end_stream: bool) -> Result<()> {
        match self.state {
            StreamState::Open => {
                if end_stream {
                    self.state = StreamState::HalfClosedLocal;
                }
                Ok(())
            }
            StreamState::HalfClosedRemote => {
                if end_stream {
                    self.state = StreamState::Closed;
                }
                Ok(())
            }
            other => Err(TorqueError::Http2Stream {
                stream_id: self.id,
                error_code: 1,
                detail: format!("Cannot send DATA in {:?} state", other),
            }),
        }
    }

    /// Process receipt of an incoming DATA frame.
    pub fn recv_data(&mut self, end_stream: bool) -> Result<()> {
        match self.state {
            StreamState::Open => {
                if end_stream {
                    self.state = StreamState::HalfClosedRemote;
                }
                Ok(())
            }
            StreamState::HalfClosedLocal => {
                if end_stream {
                    self.state = StreamState::Closed;
                }
                Ok(())
            }
            other => Err(TorqueError::Http2Stream {
                stream_id: self.id,
                error_code: 1,
                detail: format!("Cannot receive DATA in {:?} state", other),
            }),
        }
    }

    /// Reset stream via RST_STREAM frame.
    pub fn reset(&mut self) {
        self.state = StreamState::Closed;
    }

    /// Check if stream is fully closed and can be reclaimed.
    pub fn is_closed(&self) -> bool {
        self.state == StreamState::Closed
    }
}
