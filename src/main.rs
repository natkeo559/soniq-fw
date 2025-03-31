#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::mem;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::spi::{Config, Spi};
use embassy_sync::channel::{Receiver, Sender};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use mic::{PioI2sMic, PioI2sMicProgram};
use panic_probe as _;
use sd::DummyTime;

mod mic;
mod sd;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

const SAMPLE_RATE: u32 = 48_000;
const BUFFER_SIZE: usize = 960;
const CHANNEL_CAP: usize = 1;

static mut DMA_BUFFER: [u32; BUFFER_SIZE * 2] = [0; BUFFER_SIZE * 2];

static BUFFER_CHANNEL: Channel<CriticalSectionRawMutex, [u32; BUFFER_SIZE], CHANNEL_CAP> =
    Channel::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(embassy_rp::config::Config::default());

    // MIC PINS
    let bit_clock_pin = p.PIN_18;
    let left_right_clock_pin = p.PIN_19;
    let data_pin = p.PIN_20;

    // SD CARD PINS
    let sck = p.PIN_10;
    let mosi = p.PIN_11;
    let miso = p.PIN_12;
    let cs_pin = Output::new(p.PIN_13, Level::High);

    // Configure SPI for SD card init.
    let mut spi_config = Config::default();
    spi_config.frequency = 400_000; // 400 kHz
    spi_config.phase = embassy_rp::spi::Phase::CaptureOnFirstTransition;
    spi_config.polarity = embassy_rp::spi::Polarity::IdleLow;

    let spi_bus = Spi::new_blocking(p.SPI1, sck, mosi, miso, spi_config);
    let spi_dev = ExclusiveDevice::new(spi_bus, cs_pin, Delay).unwrap();

    sd::try_sd_init(spi_dev, Delay, DummyTime).unwrap();

    // Configure MIC PIO state machine and driver.
    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO0, Irqs);

    let program: PioI2sMicProgram<'_, PIO0> = PioI2sMicProgram::new(&mut common);

    let i2s_mic: PioI2sMic<PIO0, 0> = PioI2sMic::new(
        &mut common,
        sm0,
        p.DMA_CH0,
        data_pin,
        bit_clock_pin,
        left_right_clock_pin,
        SAMPLE_RATE,
        &program,
    );

    spawner.must_spawn(mic_task(i2s_mic, BUFFER_CHANNEL.sender()));
    spawner.must_spawn(sd_task(BUFFER_CHANNEL.receiver()));
}

#[embassy_executor::task]
async fn mic_task(
    mut mic: PioI2sMic<'static, PIO0, 0>,
    sender: Sender<'static, CriticalSectionRawMutex, [u32; BUFFER_SIZE], CHANNEL_CAP>,
) {
    let (mut back_buffer, mut front_buffer) = unsafe {
        let slice = core::slice::from_raw_parts_mut(DMA_BUFFER.as_mut_ptr(), BUFFER_SIZE * 2);
        slice.split_at_mut(BUFFER_SIZE)
    };

    loop {
        let dma_future = mic.read(front_buffer);
        dma_future.await;
        mem::swap(&mut back_buffer, &mut front_buffer);
        sender.send(back_buffer.try_into().unwrap()).await;
    }
}

#[embassy_executor::task]
async fn sd_task(
    receiver: Receiver<'static, CriticalSectionRawMutex, [u32; BUFFER_SIZE], CHANNEL_CAP>,
) {
    loop {
        let recv_buf: [u32; BUFFER_SIZE] = receiver.receive().await;

        // TODO: Update WAV file on the mounted volume (sd card) via `embedded_sdmmc`
        defmt::info!("First sample in back buffer: {=u32}", recv_buf[0]);
    }
}
