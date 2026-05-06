use defmt::*;
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

// #[embassy_executor::task]
// pub async fn communcation_task(uart: Uart<'static, Async>) {
//     let command_sender = COMMAND_CHANNEL.sender();
//     let response_receiver = RESPONSE_CHANNEL.receiver();
//     let (mut tx_handle, rx_handle) = uart.split();
//     let mut buffer: [u8; 256] = [0u8; 256];
//     //let buffer: StaticCell<[u8; 256]> = StaticCell::new();
//     // let rx_buffer = buffer.init([0u8; 256]);
//     let rb_rx = rx_handle.into_ring_buffered(&mut buffer);
//     loop {
//         if let Ok(response) = response_receiver.try_receive() {
//             tx_handle.write(&[0, 1, 2, 3, 4]).await;
//         }
//     }
// }

#[embassy_executor::task]
pub async fn communcation_task(uart: Uart<'static, Async>) {
    let command_sender = COMMAND_CHANNEL.sender();
    let response_receiver = RESPONSE_CHANNEL.receiver();

    let (mut tx_handle, rx_handle) = uart.split();

    let mut rb_buffer = [0u8; 256];
    let mut rb_rx = rx_handle.into_ring_buffered(&mut rb_buffer);

    let mut rx_frame = [0u8; 256];
    let mut rx_len = 0usize;

    let mut tx_frame = [0u8; 256];

    loop {
        let mut byte = [0u8; 1];

        if let Ok(1) = rb_rx.read(&mut byte).await {
            let b = byte[0];

            if b == 0x00 {
                if rx_len > 0 {
                    match decode_frame::<Command>(&mut rx_frame[..rx_len]) {
                        Ok(command) => {
                            info!("RX command: {:?}", command);
                            command_sender.send(command).await;
                        }
                        Err(e) => warn!("RX decode failed: {:?}", e),
                    }

                    rx_len = 0;
                }
            } else if rx_len < rx_frame.len() {
                rx_frame[rx_len] = b;
                rx_len += 1;
            } else {
                warn!("RX overflow");
                rx_len = 0;
            }
        }

        if let Ok(response) = response_receiver.try_receive() {
            match encode_frame(&response, &mut tx_frame) {
                Ok(frame_len) => {
                    tx_handle.write(&tx_frame[..frame_len]).await.ok();
                }
                Err(e) => warn!("TX encode failed: {:?}", e),
            }
        }
    }
}
