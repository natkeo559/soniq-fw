# I2S Microphone Capture on Raspberry Pi Pico W

This project captures audio from an I2S microphone using the Raspberry Pi Pico W. It utilizes the RP2040’s programmable I/O (PIO) to implement an I2S receiver and transfers captured audio to memory via DMA. The system is built using Rust in a `no_std` environment and structured around the async Embassy framework.

---

## Features

- **I2S Audio Capture** via PIO + DMA  
- **Async Task Architecture** using Embassy  
- **Double Buffering** for continuous data flow  
- **Planned SD Card Storage** of audio as `.wav` files  

---

## PIO + DMA Microphone Interface

At the heart of the system is a custom PIO state machine implementation that samples I2S input and streams it into memory through DMA. The PIO program implements left/right channel synchronization and clock signal decoding using the RP2040’s `side-set` and loop control features.

### PIO Assembly Details

The custom PIO program cycles between left and right channel reads based on the LR clock, capturing 24-bit samples per channel:

```assembly
.side_set 2
    set x, 14              side 0b01
left_start:
    set x, 23              side 0b00
left_loop:
    in pins, 1             side 0b01
    jmp x--, left_loop     side 0b00
    set y, 7               side 0b00
dummy_left:
    nop                    side 0b01
    jmp y--, dummy_left    side 0b00
    set x, 23              side 0b10
right_loop:
    in pins, 1             side 0b11
    jmp x--, right_loop    side 0b10
    set y, 7               side 0b10
dummy_right:
    nop                    side 0b11
    jmp y--, dummy_right   side 0b10
    jmp left_start         side 0b00
```
This sequence ensures 24 bits per channel are captured with dummy cycles for synchronization and alignment.

### Driver Configuration

The PioI2sMic struct sets up the state machine, configures PIO pins, and establishes shift-in behavior:
- `ShiftConfig` is set to auto-fill 24-bit samples shifted left.
- Clock Divider is computed as:
```PIO Clock = Sample Rate × 64 (BCLKs/frame) × 2 (phases)```
The divider adjusts PIO timing to match the I2S BCLK requirement.
- `FifoJoin` is configured to RxOnly for unified FIFO access.
- DMA Transfers are initiated through `dma_pull()` to transfer RX FIFO contents to memory buffers.

## Runtime Structure
Embassy uses a cooperative, single-threaded async runtime tailored for embedded systems. Each `#[embassy_executor::task]` function is compiled as a state machine that yields execution when awaiting, allowing other tasks to run. The `Spawner` in `main` schedules these tasks onto the executor. Under the hood, tasks are polled in a loop, and context switching is minimal—there's no preemption or thread stack switching. This approach makes task execution predictable and efficient, ideal for real-time operations like audio capture and buffer handoff. Because all tasks share the same core and run to completion between `await` points, timing-sensitive tasks (like DMA completion and buffer queuing) can be precisely coordinated without interrupt-heavy designs.

### `mic_task`

Captures audio data continuously:

- Waits for DMA to complete  
- Swaps the ping-pong buffers  
- Sends data through an Embassy channel  

### `sd_task`

Receives buffers and currently logs sample values. It is designed to be replaced with logic for writing to an SD card.

---

## Planned SD Card Integration

- **Filesystem Support** (e.g. FatFs or embedded SD drivers)  
- **Streaming `.wav` File Writes**
  - Handle proper WAV header creation and patching  
  - Write buffers in real-time  
- **Error Handling** for write failures and card removal  
