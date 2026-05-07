#![cfg_attr(not(feature = "std"), no_std)]

use serde::{Deserialize, Serialize};

pub const MAX_FRAME: usize = 256;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Command {
    Start,
    SetCyclic(bool),
    Stop,
    Status,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum State {
    Idle,
    Running,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Status {
    pub state: State,
    pub angle: Option<f32>,
    pub velocity: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Response {
    CyclicStatus(Status),
    Ack,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ProtocolError {
    Encode(postcard::Error),
    Decode(postcard::Error),
}

impl core::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProtocolError::Encode(e) => write!(f, "encode error: {e}"),
            ProtocolError::Decode(e) => write!(f, "decode error: {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ProtocolError {}

pub fn encode_frame<T: Serialize>(value: &T, out: &mut [u8]) -> Result<usize, ProtocolError> {
    postcard::to_slice_cobs(value, out)
        .map(|frame| frame.len())
        .map_err(ProtocolError::Encode)
}

pub fn decode_frame<'a, T: Deserialize<'a>>(frame: &'a mut [u8]) -> Result<T, ProtocolError> {
    postcard::from_bytes_cobs(frame).map_err(ProtocolError::Decode)
}
