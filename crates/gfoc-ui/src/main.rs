mod client;

use gfoc_proto::{Command, Response, State};
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Task};
use tokio_serial::SerialPortInfo;

pub struct GFoc {
    port: String,
    baud_rate: String,
    connection: Option<client::Connection>,
    serial_status: client::Status,
    device_state: Option<State>,
    last_message: String,
    available_ports: Vec<SerialPortInfo>,
    selected_port: Option<SerialPortInfo>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PortChanged(String),
    BaudRateChanged(String),
    ToggleRun,
    Serial(client::Event),
    AvailablePorts(Vec<SerialPortInfo>),
    SelectedPort(SerialPortInfo),
}

impl GFoc {
    fn new() -> GFoc {
        Self {
            port: "".into(),
            baud_rate: "115200".into(),
            connection: None,
            serial_status: client::Status::Disconnected,
            device_state: None,
            last_message: String::from("Waiting for serial connection"),
            selected_port: None,
            available_ports: Vec::new(),
        }
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::PortChanged(port) => {
                self.port = port;
                self.connection = None;
                self.device_state = None;
                self.serial_status = client::Status::Disconnected;
                self.last_message = String::from("Serial port changed");
            }
            Message::BaudRateChanged(baud_rate) => {
                self.baud_rate = baud_rate;
                self.connection = None;
                self.device_state = None;
                self.serial_status = client::Status::Disconnected;
                self.last_message = String::from("Baud rate changed");
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

                if connection.send(command) {
                    self.last_message = format!("Sent {command:?}");
                } else {
                    self.connection = None;
                    self.last_message = String::from("Serial command channel is full");
                }
            }
            Message::AvailablePorts(ports) => self.available_ports = ports,
            Message::SelectedPort(port) => self.selected_port = Some(port),
            Message::Serial(event) => match event {
                client::Event::Ready(connection) => {
                    self.connection = Some(connection);
                    self.last_message = String::from("Serial client ready");
                }
                client::Event::Status(status) => {
                    if !status.is_connected() {
                        self.device_state = None;
                    }

                    self.last_message = status.label();
                    self.serial_status = status;
                }
                client::Event::Response(response) => match response {
                    Response::CyclicStatus { state } => {
                        self.device_state = Some(state);
                        self.last_message = format!("State updated: {}", state_label(state));
                    }
                    Response::Ack => {
                        self.last_message = String::from("Device acknowledged command");
                    }
                },
                client::Event::Error(error) => {
                    self.last_message = error;
                }
            },
        }

        Task::none()
    }

    // fn subscription(&self) -> Subscription<Message> {
    //     client::subscription(self.config()).map(Message::Serial)
    // }

    // fn config(&self) -> Option<client::Config> {
    //     let port = self.port.trim();

    //     if port.is_empty() {
    //         return None;
    //     }

    //     let Ok(baud_rate) = self.baud_rate.trim().parse::<u32>() else {
    //         return None;
    //     };

    //     if baud_rate == 0 {
    //         return None;
    //     }

    //     Some(client::Config {
    //         port: port.to_string(),
    //         baud_rate,
    //     })
    // }

    fn view(&self) -> Element<'_, Message> {
        let state = self.device_state.map(state_label).unwrap_or("Unknown");

        let mut run_button =
            button(text(toggle_label(self.device_state)).width(Length::Fixed(88.0)))
                .padding([10, 18]);

        if !self.serial_status.is_connected() {
            run_button = run_button.on_press(Message::ToggleRun);
        }

        let controls = row![
            view_ports(self.available_ports.clone(), self.selected_port.clone()),
            text_input("Baud", &self.baud_rate)
                .on_input(Message::BaudRateChanged)
                .padding(10)
                .width(Length::Fixed(120.0)),
            run_button,
        ]
        .spacing(12)
        .align_y(Alignment::Center);

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

        container(column![status, controls].padding(24).spacing(24))
            .center(Length::Fill)
            .into()
    }
}

fn main() -> iced::Result {
    iced::application(|| GFoc::new(), GFoc::update, GFoc::view)
        .subscription(|_| {
            iced::Subscription::batch(vec![
                client::gfoc_subscription().map(Message::Serial),
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
