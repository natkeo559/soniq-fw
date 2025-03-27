#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::mem;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::config::Config;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::{InterruptHandler, Pio};
use mic::{PioI2sMic, PioI2sMicProgram};
use panic_probe as _;

mod mic;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

const SAMPLE_RATE: u32 = 48_000;
const BUFFER_SIZE: usize = 960;
static mut DMA_BUFFER: [u32; BUFFER_SIZE * 2] = [0; BUFFER_SIZE * 2];

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    let p = embassy_rp::init(Config::default());

    // Setup pio state machine for i2s output
    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO0, Irqs);

    let bit_clock_pin = p.PIN_18;
    let left_right_clock_pin = p.PIN_19;
    let data_pin = p.PIN_20;

    let program: PioI2sMicProgram<'_, PIO0> = PioI2sMicProgram::new(&mut common);

    let mut i2s_mic: PioI2sMic<PIO0, 0> = PioI2sMic::new(
        &mut common,
        sm0,
        p.DMA_CH0,
        data_pin,
        bit_clock_pin,
        left_right_clock_pin,
        SAMPLE_RATE,
        &program,
    );

    let (mut back_buffer, mut front_buffer) = unsafe {
        let slice = core::slice::from_raw_parts_mut(DMA_BUFFER.as_mut_ptr(), BUFFER_SIZE * 2);
        slice.split_at_mut(BUFFER_SIZE)
    };

    loop {
        // Start DMA transfer into the front buffer.
        let dma_future = i2s_mic.read(front_buffer);

        // Wait for DMA transfer to finish.
        dma_future.await;

        // Swap the buffers for the next iteration.
        mem::swap(&mut back_buffer, &mut front_buffer);

        // Meanwhile, process or inspect data from the back buffer.
        // For demonstration, we log the first sample.
        defmt::info!("First sample in back buffer: {=u32}", back_buffer[0]);
    }
}
