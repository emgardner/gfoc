use gfoc_proto::{Command, MAX_FRAME, Response, decode_frame, encode_frame};
use iced::Subscription;
use iced::futures::FutureExt;
use iced::futures::Stream;
use iced::futures::channel::mpsc;
use iced::futures::sink::SinkExt;
use iced::futures::stream::StreamExt;
use std::fmt;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

const DEFAULT_BAUD_RATE: u32 = 115_200;
const RECONNECT_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Config {
    pub port: String,
    pub baud_rate: u32,
}

#[derive(Debug, Clone)]
pub enum Event {
    Ready(Connection),
    Status(Status),
    Response(Response),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum Status {
    Disconnected,
    Connecting { port: String, baud_rate: u32 },
    Connected { port: String, baud_rate: u32 },
    Reconnecting { port: String, reason: String },
}

impl Status {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    pub fn label(&self) -> String {
        match self {
            Self::Disconnected => String::from("Disconnected"),
            Self::Connecting { port, baud_rate } => {
                format!("Connecting to {port} at {baud_rate} baud")
            }
            Self::Connected { port, baud_rate } => {
                format!("Connected to {port} at {baud_rate} baud")
            }
            Self::Reconnecting { port, reason } => {
                format!("Reconnecting to {port}: {reason}")
            }
        }
    }
}

#[derive(Clone)]
pub struct Connection {
    commands: mpsc::Sender<Input>,
    responses: std::sync::Arc<mpsc::Receiver<Event>>,
}

impl Connection {
    pub fn send(&mut self, command: Command) -> bool {
        self.commands.try_send(Input::Command(command)).is_ok()
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub enum Input {
    Command(Command),
    ClosePort,
    OpenPort(Config),
}

#[derive(Debug, Error)]
pub enum ClientWorkerError {}

#[derive(Debug)]
pub struct ClientWorker {
    connection: Option<SerialStream>,
    config: Option<Config>,
    receiver: mpsc::Receiver<Input>,
    sender: mpsc::Sender<Event>,
    status_poll: Duration,
    reconnect_duration: Duration,
}

impl ClientWorker {
    pub fn new_with_config(
        config: Option<Config>,
        receiver: mpsc::Receiver<Input>,
        sender: mpsc::Sender<Event>,
    ) -> Self {
        Self {
            connection: None,
            config,
            receiver,
            sender,
            status_poll: Duration::from_millis(100),
            reconnect_duration: Duration::from_secs(1),
        }
    }

    pub async fn run(&mut self) -> ! {
        let mut frame_buffer: Vec<u8> = Vec::new();
        loop {
            println!("Here");
            // tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            iced::futures::select! {
                _ = tokio::time::sleep(self.status_poll).fuse() => {
                            if let Some(ref mut conn) = self.connection {
                                match write_command(conn, Command::Status).await {
                                    Ok(_) => {
                                        // let response = receive_byte(byte, frame);
                                    }
                                    Err(s) => {
                                        self.sender.send(Event::Error(s)).await;
                                    }
                                }
                            }
                }
                command = self.receiver.select_next_some() => {
                    match  command{
                        Input::ClosePort => self.connection = None,
                        Input::OpenPort(config) => {
                            if let Ok(port) = open(&config) {
                                self.config = Some(config);
                                self.connection = Some(port)
                            } else {
                                self.config = Some(config);
                            }
                        }
                        Input::Command(cmd) => {
                            if let Some(ref mut conn) = self.connection {
                                match write_command(conn, cmd).await {
                                    Ok(_) => {
                                        //let response = receive_byte(byte, frame);
                                    }
                                    Err(s) => {
                                        self.sender.send(Event::Error(s)).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // if let Some(conf) = self.config {
            //     if let Ok(port) = open(&conf) {
            //         self.connection = Some(port)
            //     } else {
            //         tokio::time::sleep(self.reconnect_duration).await;
            //     }
            // } else {
            //     tokio::time::sleep(Duration::from_secs(1)).await;
            // }
            //}
        }
    }
}

fn open(config: &Config) -> tokio_serial::Result<SerialStream> {
    tokio_serial::new(config.port.as_str(), config.baud_rate).open_native_async()
}

// async fn run_connected(
//     serial: &mut SerialStream,
//     receiver: &mut mpsc::Receiver<Input>,
//     output: &mut mpsc::Sender<Event>,
// ) -> String {
//     let mut frame = Vec::with_capacity(MAX_FRAME);
//     let mut read_buffer = [0_u8; 64];
//
//     loop {
//         tokio::select! {
//             read = serial.read(&mut read_buffer) => {
//                 let count = match read {
//                     Ok(0) => return String::from("serial port closed"),
//                     Ok(count) => count,
//                     Err(error) => return error.to_string(),
//                 };
//
//                 for byte in &read_buffer[..count] {
//                     match receive_byte(*byte, &mut frame) {
//                         Some(Ok(response)) => {
//                             if output.send(Event::Response(response)).await.is_err() {
//                                 return String::from("UI subscription closed");
//                             }
//                         }
//                         Some(Err(error)) => {
//                             if output.send(Event::Error(error)).await.is_err() {
//                                 return String::from("UI subscription closed");
//                             }
//                         }
//                         None => {}
//                     }
//                 }
//             }
//             input = receiver.next() => {
//                 let Some(input) = input else {
//                     return String::from("command channel closed");
//                 };
//
//                 match input {
//                     Input::Command(command) => {
//                         if let Err(error) = write_command(serial, command).await {
//                             return error;
//                         }
//                     }
//                 }
//             }
//         }
//     }
// }
// async fn read_frame(serial: &mut SerialStream) -> Result<Response, ClientWorkerError> {
//     let mut frame_buffer: [u8; 256] = [0u8; 256];
//     serial.read()
// }
// fn read_frame_with_timeout<RW>(stream: &mut tokio::io::BufStream<RW>) -> Result<Response, String> {
//     tokio::time::timeout(tokio::time::Duration::from_millis(100), )
// }

// async fn read_frame<R: AsyncBufReadExt>(stream: &mut R) -> Result<Response, String> {
//     let mut frame_buffer: Vec<u8> = Vec::new();
//     stream.read_until(0, &mut frame_buffer).await;
//     Err("e".into())
// }

fn receive_byte(byte: u8, frame: &mut Vec<u8>) -> Option<Result<Response, String>> {
    if byte == 0 {
        if frame.is_empty() {
            return None;
        }

        frame.push(0);
        let result = decode_frame::<Response>(frame.as_mut_slice())
            .map_err(|error| format!("failed to decode response frame: {error:?}"));

        frame.clear();

        return Some(result);
    }

    if frame.len() >= MAX_FRAME - 1 {
        frame.clear();
        return Some(Err(String::from("response frame exceeded maximum size")));
    }

    frame.push(byte);
    None
}

async fn write_command(serial: &mut SerialStream, command: Command) -> Result<(), String> {
    let mut frame = [0_u8; MAX_FRAME];
    let len = encode_frame(&command, &mut frame)
        .map_err(|error| format!("failed to encode command frame: {error:?}"))?;

    serial
        .write_all(&frame[..len])
        .await
        .map_err(|error| error.to_string())?;

    serial.flush().await.map_err(|error| error.to_string())
}

fn gfoc_worker() -> impl Stream<Item = Event> {
    iced::stream::channel(
        100,
        |mut output: iced::futures::channel::mpsc::Sender<Event>| async move {
            let (worker_tx, worker_rx) = mpsc::channel(100);
            let (app_tx, app_rx) = mpsc::channel(100);
            output
                .send(Event::Ready(Connection {
                    commands: worker_tx,
                    responses: std::sync::Arc::new(app_rx),
                }))
                .await
                .expect("Failed To Create Worker Subscription");
            let mut worker = ClientWorker::new_with_config(None, worker_rx, app_tx);
            worker.run().await
        },
    )
}

pub fn gfoc_subscription() -> Subscription<Event> {
    Subscription::run(gfoc_worker)
}

// #[derive(Debug, thiserror::Error)]
// pub enum ProtocolError {
//     #[error("empty frame")]
//     EmptyFrame,
//
//     #[error("unknown command id: {0:#04x}")]
//     UnknownCommandId(u8),
//
//     #[error("unknown response id: {0:#04x}")]
//     UnknownResponseId(u8),
//
//     #[error("invalid bool value: {0}")]
//     InvalidBool(u8),
//
//     #[error("invalid state value: {0}")]
//     InvalidState(u8),
//
//     #[error("invalid command payload length for {id:?}: expected {expected}, got {actual}")]
//     InvalidCommandPayloadLength {
//         id: CommandId,
//         expected: usize,
//         actual: usize,
//     },
//
//     #[error("invalid response payload length for {id:?}: expected {expected}, got {actual}")]
//     InvalidResponsePayloadLength {
//         id: ResponseId,
//         expected: usize,
//         actual: usize,
//     },
// }
//
// #[derive(Debug, thiserror::Error)]
// pub enum TransportError {
//     #[error("serial open failed")]
//     SerialOpen(#[source] std::io::Error),
//
//     #[error("serial read failed")]
//     SerialRead(#[source] std::io::Error),
//
//     #[error("serial write failed")]
//     SerialWrite(#[source] std::io::Error),
//
//     #[error("COBS encode failed")]
//     CobsEncode,
//
//     #[error("COBS decode failed")]
//     CobsDecode,
//
//     #[error("frame exceeded maximum length: {max}")]
//     FrameTooLarge { max: usize },
//
//     #[error("transport closed")]
//     Closed,
//
//     #[error("protocol error")]
//     Protocol(#[from] ProtocolError),
// }
