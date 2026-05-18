use defmt::*;
use embassy_futures::select::Either;
use embassy_stm32::mode::Async;
use embassy_stm32::usart::Uart;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use gfoc_proto::{Command, Response, decode_frame, encode_frame};

//  pub static COMMAND_CHANNEL: StaticCell<
//      embassy_sync::channel::Channel<CriticalSectionRawMutex, Command, 5>,
//  > = StaticCell::new();
//  // > = StaticCell::new(embassy_sync::channel::Channel::new());
//  pub static RESPONSE_CHANNEL: StaticCell<
//      embassy_sync::channel::Channel<CriticalSectionRawMutex, Response, 5>,
//  > = StaticCell::new();
//  //    embassy_sync::channel::Channel::new();
pub static COMMAND_CHANNEL: embassy_sync::channel::Channel<CriticalSectionRawMutex, Command, 5> =
    embassy_sync::channel::Channel::new();
pub static RESPONSE_CHANNEL: embassy_sync::channel::Channel<CriticalSectionRawMutex, Response, 5> =
    embassy_sync::channel::Channel::new();

#[embassy_executor::task]
pub async fn communcation_task(uart: Uart<'static, Async>) {
    let command_sender = COMMAND_CHANNEL.sender();
    let response_receiver = RESPONSE_CHANNEL.receiver();

    let (mut tx_handle, rx_handle) = uart.split();

    let mut rb_buffer = [0u8; 256];
    let mut rb_rx = rx_handle.into_ring_buffered(&mut rb_buffer);

    let mut rx_frame = [0u8; 256];
    let mut rx_len = 0usize;
    let mut rx_start_time: Option<embassy_time::Instant> = None;
    let rx_timeout = embassy_time::Duration::from_millis(100);

    let mut tx_frame = [0u8; 256];

    loop {
        let response_or_receive = embassy_futures::select::select(
            rb_rx.read(&mut rx_frame[rx_len..]),
            response_receiver.receive(),
        );
        match response_or_receive.await {
            Either::First(ser_res) => match ser_res {
                Ok(bytes_len) => {
                    rx_len += bytes_len;
                    if rx_len > rx_frame.len() - 1 {
                        defmt::error!("Rx Frame Overflow");
                        rx_frame = [0u8; 256];
                        rx_len = 0;
                        rx_start_time = None;
                        continue;
                    };

                    if let Some(rx_start) = rx_start_time
                        && embassy_time::Instant::now() - rx_start > rx_timeout
                    {
                        defmt::error!("Rx Frame Timeout");
                        rx_frame = [0u8; 256];
                        rx_len = 0;
                        rx_start_time = None;
                        continue;
                    } else {
                        rx_start_time = Some(embassy_time::Instant::now());
                    };

                    if rx_len > 1 && rx_frame[rx_len - 1] == 0 {
                        match decode_frame::<Command>(&mut rx_frame[..rx_len]) {
                            Ok(command) => {
                                command_sender.send(command).await;
                            }
                            Err(e) => warn!("RX decode failed: {:?}", e),
                        }
                        rx_frame = [0u8; 256];
                        rx_len = 0;
                        rx_start_time = None;
                    }
                }
                Err(e) => {
                    defmt::error!("Serial Error: {:?}", e);
                    rx_frame = [0u8; 256];
                    rx_len = 0;
                    rx_start_time = None;
                }
            },
            Either::Second(response) => match encode_frame(&response, &mut tx_frame) {
                Ok(frame_len) => {
                    tx_handle.write(&tx_frame[..frame_len]).await.ok();
                }
                Err(e) => warn!("TX encode failed: {:?}", e),
            },
        }
    }
}
