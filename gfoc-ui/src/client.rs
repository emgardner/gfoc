use gfoc_proto::{Command, MAX_FRAME, Response, decode_frame, encode_frame};
use serde::{Deserialize, Serialize};
use std::{io, time::Duration};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Config {
    pub port: String,
    pub baud_rate: u32,
}

pub trait Transport: AsyncRead + AsyncWrite {
    type Error: std::fmt::Debug;
}

impl Transport for SerialStream {
    type Error = tokio_serial::Error;
}

pub struct Client<T: Transport> {
    read: ReadHalf<T>,
    write: WriteHalf<T>,
}

pub type TransportError<T> = <T as Transport>::Error;
pub type ClientResult<T, R> = Result<R, ClientError<TransportError<T>>>;

#[derive(Debug, Error)]
pub enum ClientError<E> {
    #[error("Transport Error `{0}`")]
    TransportError(E),
    #[error("I/O Error `{0}`")]
    IoError(#[from] io::Error),
    #[error("Protocol Error `{0}`")]
    ProtocolError(#[from] gfoc_proto::ProtocolError),
    #[error("Timeout Error")]
    TimeoutError,
}

pub trait IntoClient {
    type Transport: Transport;

    fn open(config: &Config) -> ClientResult<Self::Transport, Client<Self::Transport>>;
}

pub struct Serial;

impl IntoClient for Serial {
    type Transport = SerialStream;

    fn open(config: &Config) -> ClientResult<SerialStream, Client<SerialStream>> {
        let port = tokio_serial::new(config.port.as_str(), config.baud_rate)
            .open_native_async()
            .map_err(ClientError::TransportError)?;
        Ok(Client::new(port))
    }
}

impl<T: Transport> Client<T> {
    pub fn new(rw: T) -> Client<T> {
        let (read, write) = tokio::io::split(rw);
        Self { read, write }
    }

    pub async fn write_command(&mut self, cmd: Command) -> ClientResult<T, ()> {
        let mut output_buffer = [0u8; 256];
        let buffer_size = encode_frame(&cmd, &mut output_buffer)?;
        self.write.write_all(&output_buffer[0..buffer_size]).await?;
        Ok(())
    }

    pub async fn transaction(&mut self, cmd: Command) -> ClientResult<T, Response> {
        self.write_command(cmd).await?;
        let resp = tokio::time::timeout(Duration::from_millis(100), self.read_response())
            .await
            .map_err(|_| ClientError::TimeoutError)?;
        return resp;
    }

    pub async fn read_response(&mut self) -> ClientResult<T, Response> {
        let mut frame = [0u8; MAX_FRAME];
        let mut len = 0;

        loop {
            if len == frame.len() {
                let error = io::Error::new(io::ErrorKind::InvalidData, "response frame too large");
                return Err(ClientError::IoError(error));
            }

            let bytes_read = self.read.read(&mut frame[len..len + 1]).await?;

            if bytes_read == 0 {
                let error = io::Error::new(io::ErrorKind::UnexpectedEof, "transport closed");
                return Err(ClientError::IoError(error));
            }

            len += bytes_read;

            if frame[len - 1] == 0 {
                break;
            }
        }

        Ok(decode_frame(&mut frame[..len])?)
    }
}
