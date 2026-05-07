mod client;
mod worker;

use gfoc_proto::{Command, Response, State};
use iced::wgpu::wgc::command::CommandEncoderError;
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Task};
use tokio_serial::SerialPortInfo;

pub struct GFoc {
    baud_rate: String,
    connection: Option<worker::Connection>,
    serial_status: worker::Status,
    device_state: Option<State>,
    last_message: String,
    available_ports: Vec<SerialPortInfo>,
    selected_port: Option<SerialPortInfo>,
}

#[derive(Debug, Clone)]
pub enum Message {
    BaudRateChanged(String),
    ToggleRun,
    Worker(worker::Event),
    AvailablePorts(Vec<SerialPortInfo>),
    SelectedPort(SerialPortInfo),
    Connect,
    ClosePort,
    Run,
    Stop,
}

impl GFoc {
    fn new() -> GFoc {
        Self {
            baud_rate: "115200".into(),
            connection: None,
            serial_status: worker::Status::Disconnected,
            device_state: None,
            last_message: String::from("Waiting for serial connection"),
            selected_port: None,
            available_ports: Vec::new(),
        }
    }

    fn serial_config(&self) -> Result<client::Config, String> {
        let baud_rate = self.baud_rate.parse::<u32>().map_err(|e| e.to_string())?;
        if let Some(port) = &self.selected_port {
            Ok(client::Config {
                port: port.port_name.clone(),
                baud_rate,
            })
        } else {
            Err("No Selected Port".into())
        }
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::BaudRateChanged(baud_rate) => {
                self.baud_rate = baud_rate;
                self.connection = None;
                self.device_state = None;
                self.last_message = String::from("Baud rate changed");
            }
            Message::Connect => {
                if let Ok(config) = self.serial_config() {
                    let Some(connection) = &mut self.connection else {
                        return Task::none();
                    };
                    connection.send(worker::Input::OpenPort(config));
                }
            }
            Message::ClosePort => {
                if let Some(ref mut connection) = self.connection {
                    connection.send(worker::Input::ClosePort);
                }
                return Task::none();
            }
            Message::ToggleRun => {
                let Some(connection) = &mut self.connection else {
                    self.last_message = String::from("Serial client is not ready");
                    return Task::none();
                };

                let command = if matches!(self.device_state, Some(State::Running)) {
                    Command::Stop
                } else {
                    Command::Start
                };

                if connection.send_command(command) {
                    self.last_message = format!("Sent {command:?}");
                } else {
                    self.connection = None;
                    self.last_message = String::from("Serial command channel is full");
                }
            }
            Message::AvailablePorts(ports) => self.available_ports = ports,
            Message::SelectedPort(port) => self.selected_port = Some(port),
            Message::Run => {
                if self.serial_status.is_connected()
                    && let Some(connection) = &mut self.connection
                {
                    connection.send_command(Command::Start);
                }
                return Task::none();
            }
            Message::Stop => {
                if self.serial_status.is_connected()
                    && let Some(connection) = &mut self.connection
                {
                    connection.send_command(Command::Stop);
                }
                return Task::none();
            }
            Message::Worker(event) => match event {
                worker::Event::Ready(connection) => {
                    self.connection = Some(connection);
                    self.last_message = String::from("Serial client ready");
                }
                worker::Event::Status(status) => {
                    println!("Status: {:?}", status);
                    if !status.is_connected() {
                        self.device_state = None;
                    }

                    self.last_message = status.label();
                    self.serial_status = status;
                }
                worker::Event::Response(response) => match response {
                    Response::CyclicStatus { state } => {
                        self.device_state = Some(state);
                        self.last_message = format!("State updated: {}", state_label(state));
                    }
                    Response::Ack => {
                        self.last_message = String::from("Device acknowledged command");
                    }
                },
                worker::Event::Error(error) => {
                    self.last_message = error;
                }
            },
        }

        Task::none()
    }

    fn connection(&self) -> Element<'_, Message> {
        println!("{:?}", self.serial_status);
        let connect_button = if self.serial_status.is_connected() {
            button("Disconnect")
                .on_press(Message::ClosePort)
                .padding([10, 18])
                .style(iced::widget::button::danger)
        } else if self.serial_status == worker::Status::Disconnected {
            button("Connect")
                .on_press(Message::Connect)
                .padding([10, 18])
        } else {
            button("Connecting").padding([10, 18])
        };

        let controls = row![
            view_ports(self.available_ports.clone(), self.selected_port.clone()),
            text_input("Baud", &self.baud_rate)
                .on_input(Message::BaudRateChanged)
                .padding(10)
                .width(Length::Fixed(120.0)),
            connect_button,
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        controls.into()
    }

    fn controls(&self) -> Element<'_, Message> {
        if self.serial_status.is_connected() {
            return row![
                iced::widget::button("Run").on_press(Message::Run),
                iced::widget::button("Stop").on_press(Message::Stop),
            ]
            .spacing(12)
            .into();
        }
        row![].into()
    }

    fn view(&self) -> Element<'_, Message> {
        let state = self.device_state.map(state_label).unwrap_or("Unknown");

        let status = column![
            text("GFOC Serial").size(28),
            row![
                text("Connection").width(Length::Fixed(110.0)),
                text(self.serial_status.label())
            ]
            .spacing(12),
            row![text("State").width(Length::Fixed(110.0)), text(state)].spacing(12),
            row![
                text("Last event").width(Length::Fixed(110.0)),
                text(&self.last_message)
            ]
            .spacing(12),
        ]
        .spacing(10);

        container(
            column![status, self.connection(), self.controls()]
                .padding(24)
                .spacing(24),
        )
        .center(Length::Fill)
        .into()
    }
}

fn main() -> iced::Result {
    iced::application(|| GFoc::new(), GFoc::update, GFoc::view)
        .subscription(|_| {
            iced::Subscription::batch(vec![
                worker::gfoc_subscription().map(Message::Worker),
                iced::time::every(std::time::Duration::from_secs(1)).map(|_| {
                    if let Ok(ports) = tokio_serial::available_ports() {
                        Message::AvailablePorts(
                            ports
                                .into_iter()
                                .filter(|x| x.port_name.contains("tty"))
                                .collect(),
                        )
                    } else {
                        Message::AvailablePorts(vec![])
                    }
                }),
            ])
        })
        .antialiasing(true)
        .run()
}

fn state_label(state: State) -> &'static str {
    match state {
        State::Idle => "Idle",
        State::Running => "Running",
    }
}

fn toggle_label(state: Option<State>) -> &'static str {
    match state {
        Some(State::Running) => "Stop",
        _ => "Start",
    }
}

fn view_ports<'a>(
    ports: Vec<SerialPortInfo>,
    selected: Option<SerialPortInfo>,
) -> iced::Element<'a, Message> {
    iced::widget::pick_list(selected, ports, |x| x.port_name.clone())
        .on_select(Message::SelectedPort)
        .placeholder("Select Port")
        .into()
}
