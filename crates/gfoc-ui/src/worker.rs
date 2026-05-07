use gfoc_proto::{Command, Response};
use iced::Subscription;
use iced::futures::FutureExt;
use iced::futures::Stream;
use iced::futures::channel::mpsc;
use iced::futures::sink::SinkExt;
use iced::futures::stream::StreamExt;
use std::fmt;
use std::time::Duration;

use crate::client::{Client, Config, IntoClient, Serial, TransportError};

#[derive(Debug, Clone)]
pub enum Event {
    Ready(Connection),
    Status(Status),
    Response(Response),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
#[allow(dead_code)]
pub struct Connection {
    commands: mpsc::Sender<Input>,
    // responses: std::sync::Arc<mpsc::Receiver<Event>>,
}

impl Connection {
    pub fn send(&mut self, command: Input) -> bool {
        self.commands.try_send(command).is_ok()
    }

    pub fn send_command(&mut self, command: Command) -> bool {
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

pub struct ClientWorker<C: IntoClient> {
    client: Option<Client<C::Transport>>,
    config: Option<Config>,
    receiver: mpsc::Receiver<Input>,
    sender: mpsc::Sender<Event>,
    status_poll: Duration,
    // reconnect_duration: Duration,
}

impl<C: IntoClient> ClientWorker<C> {
    pub fn new_with_config(
        config: Option<Config>,
        receiver: mpsc::Receiver<Input>,
        sender: mpsc::Sender<Event>,
    ) -> ClientWorker<C> {
        Self {
            client: None,
            config,
            receiver,
            sender,
            status_poll: Duration::from_millis(50),
            // reconnect_duration: Duration::from_secs(1),
        }
    }
}

impl<C> ClientWorker<C>
where
    C: IntoClient,
    TransportError<C::Transport>: fmt::Display,
{
    pub async fn run(&mut self) -> ! {
        loop {
            iced::futures::select! {
                _ = tokio::time::sleep(self.status_poll).fuse() => {
                            if let Some(ref mut client) = self.client {
                                match client.transaction(Command::Status).await {
                                    Ok(response) => {
                                        self.sender.send(Event::Response(response)).await.expect("Shouldn't Happen");
                                    }
                                    Err(s) => {
                                        self.sender.send(Event::Error(s.to_string())).await.expect("Shouldn't Happen");
                                    }
                                }
                            }
                }
                command = self.receiver.select_next_some() => {
                    match  command{
                        Input::ClosePort => {
                                let _ = self.sender.send(Event::Status(Status::Disconnected)).await;
                                self.client = None;
                        },
                        Input::OpenPort(config) => {
                            println!("Open: {:?}", config);
                            match C::open(&config) {
                                Ok(port) => {

                                self.config = Some(config.clone());
                                self.client = Some(port);
                                println!("Client Opened");
                                let _ = self.sender.send(Event::Status(Status::Connected { port: config.port, baud_rate: config.baud_rate })).await;
                                }
                                Err(e) => {
                                println!("Failed Open: {:?}", e);
                                self.config = Some(config);
                                }
                            }
                        }
                        Input::Command(cmd) => {
                            if let Some(ref mut client) = self.client {
                                match client.transaction(cmd).await {
                                    Ok(response) => {
                                        let _ = self.sender.send(Event::Response(response)).await;

                                    }
                                    Err(s) => {
                                        let _ = self.sender.send(Event::Error(s.to_string())).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn gfoc_worker() -> impl Stream<Item = Event> {
    iced::stream::channel(
        100,
        |mut output: iced::futures::channel::mpsc::Sender<Event>| async move {
            let (worker_tx, worker_rx) = mpsc::channel(100);
            // let (app_tx, app_rx) = mpsc::channel(100);
            output
                .send(Event::Ready(Connection {
                    commands: worker_tx,
                    // responses: std::sync::Arc::new(app_rx),
                }))
                .await
                .expect("Failed To Create Worker Subscription");
            let mut worker: ClientWorker<Serial> =
                ClientWorker::new_with_config(None, worker_rx, output);
            worker.run().await
        },
    )
}

pub fn gfoc_subscription() -> Subscription<Event> {
    Subscription::run(gfoc_worker)
}
