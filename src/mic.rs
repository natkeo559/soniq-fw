use embassy_rp::Peripheral;
use embassy_rp::dma::{AnyChannel, Channel};
use embassy_rp::pio::{
    Common, Direction, FifoJoin, Instance, LoadedProgram, PioPin, ShiftConfig, ShiftDirection,
    StateMachine,
};
use embassy_rp::{PeripheralRef, into_ref};
use fixed::traits::ToFixed;

pub struct PioI2sMicProgram<'a, PIO: Instance> {
    prg: LoadedProgram<'a, PIO>,
}

impl<'a, PIO: Instance> PioI2sMicProgram<'a, PIO> {
    /// Load the program into the given pio
    pub fn new(common: &mut Common<'a, PIO>) -> Self {
        let prg = pio::pio_asm!(
            ".side_set 2",
            "    set x, 14              side 0b01",
            "left_start:",
            "    set x, 23              side 0b00",
            "left_loop:",
            "    in pins, 1             side 0b01",
            "    jmp x--, left_loop     side 0b00",
            "    set y, 7               side 0b00",
            "dummy_left:",
            "    nop                    side 0b01",
            "    jmp y--, dummy_left    side 0b00",
            "    set x, 23              side 0b10",
            "right_loop:"
            "    in pins, 1             side 0b11",
            "    jmp x--, right_loop    side 0b10",
            "    set y, 7               side 0b10",
            "dummy_right:",
            "    nop                    side 0b11",
            "    jmp y--, dummy_right   side 0b10",
            "    jmp left_start         side 0b00",
        );

        let prg = common.load_program(&prg.program);

        Self { prg }
    }
}

/// I2S microphone input driver using PIO + DMA.
/// It reads 32-bit words (with 24-bit samples in lower bits) from the RX FIFO.
pub struct PioI2sMic<'a, P: Instance, const SM: usize> {
    dma: PeripheralRef<'a, AnyChannel>,
    sm: StateMachine<'a, P, SM>,
}

#[allow(clippy::too_many_arguments)]
impl<'a, P: Instance, const SM: usize> PioI2sMic<'a, P, SM> {
    /// Create a new `PioI2sMic` instance for capturing I2S audio from the mic.
    pub fn new(
        common: &mut Common<'a, P>,
        mut sm: StateMachine<'a, P, SM>,
        dma: impl Peripheral<P = impl Channel> + 'a,
        data_pin: impl PioPin,
        bit_clock_pin: impl PioPin,
        lr_clock_pin: impl PioPin,
        sample_rate: u32,
        program: &PioI2sMicProgram<'a, P>,
    ) -> Self {
        into_ref!(dma);

        let data_pin = common.make_pio_pin(data_pin);
        let bit_clock_pin = common.make_pio_pin(bit_clock_pin);
        let lr_clock_pin = common.make_pio_pin(lr_clock_pin);

        let mut cfg = embassy_rp::pio::Config::default();
        cfg.use_program(&program.prg, &[&bit_clock_pin, &lr_clock_pin]);
        cfg.set_in_pins(&[&data_pin]);

        // PIO clock = sample_rate * 64 (BCLK cycles) * 2 (clock phases).
        let clk = f64::from(sample_rate * 64) * 2.0;
        let pio_clk = f64::from(embassy_rp::clocks::clk_sys_freq());
        cfg.clock_divider = (pio_clk / clk).to_fixed();

        cfg.shift_in = ShiftConfig {
            threshold: 24,
            direction: ShiftDirection::Left,
            auto_fill: true,
        };
        cfg.fifo_join = FifoJoin::RxOnly;

        sm.set_config(&cfg);
        sm.set_pin_dirs(Direction::In, &[&data_pin]);
        sm.set_pin_dirs(Direction::Out, &[&lr_clock_pin, &bit_clock_pin]);
        sm.set_enable(true);

        Self {
            dma: dma.map_into(),
            sm,
        }
    }

    /// Start a DMA transfer to read audio samples from the RX FIFO.
    pub fn read<'b>(
        &'b mut self,
        buff: &'b mut [u32],
    ) -> embassy_rp::dma::Transfer<'b, embassy_rp::dma::AnyChannel> {
        self.sm.rx().dma_pull(self.dma.reborrow(), buff, false)
    }
}
