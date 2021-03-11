# WindowMaster PCB

This circuit board is the main board for the WindowMaster physcial controller.
It supports 6 control channels which can be individually assigned to different
channels via the WindowMaster software.

The MCU powering this board is an [STM32F072C8], which can read the encoder
states and set indicators via GPIO, and communicate with the host computer over
USB. You can flash the provided firmware to communicate with and control the
WindowMaster host software, or write and flash your own if you have another
usecase for the control surface. (In the future, I may provide other firmwares
to turn it into a generic input device like a joystick or MIDI. Feel free to
contact me if you'd like to commission a custom firmware - I'd love to hear
about your idea!)

This board is developed with KiCad version 5.1.9. Components, especially those
that can or must be surface-mounted, are picked from the [JLCPCB SMT Parts
Library](https://jlcpcb.com/parts), if there is a suitable part there. This
makes it more convenient to fabricate and assemble, as they will do most of the
work. Everything else is picked to be easy to hand-solder with an iron, which
generally means through-hole or large tabs.

## Ordering

TODO

## Programming Reference & Pinout

The PCB is laid out into six channels. Each channel has its own quadrature
encoder (PEC11R), a pushbutton switch (built into the encoder), and an
indicator LED. These peripherals are controlled by the onboard [STM32F072C8]
microcontroller, which is also given a USB port so it can communicate with a
host system.

Here is the pinout of the STM32 chip to all of its connected peripherals, for
quick reference while programming:

| Physical | Logical | Description          | Direction | Logic      |
|----------|---------|----------------------|-----------|------------|
| 25       | PB12    | Status LED           | Output    | Active Low |
| 32       | PA11    | USB D-               |           |            |
| 33       | PA12    | USB D+               |           |            |
| 3        | PC14    | Channel 1 Encoder A  | Input     | Active Low |
| 2        | PC13    | Channel 1 Encoder B  | Input     | Active Low |
| 39       | PB3     | Channel 1 Pushbutton | Input     | Active Low |
| 40       | PB4     | Channel 1 Indicator  | Output    | Active Low |
| 46       | PB9     | Channel 2 Encoder A  | Input     | Active Low |
| 45       | PB8     | Channel 2 Encoder B  | Input     | Active Low |
| 38       | PA15    | Channel 2 Pushbutton | Input     | Active Low |
| 41       | PB5     | Channel 2 Indicator  | Output    | Active Low |
| 43       | PB7     | Channel 3 Encoder A  | Input     | Active Low |
| 42       | PB6     | Channel 3 Encoder B  | Input     | Active Low |
| 27       | PB14    | Channel 3 Pushbutton | Input     | Active Low |
| 28       | PB15    | Channel 3 Indicator  | Output    | Active Low |
| 18       | PB0     | Channel 4 Encoder A  | Input     | Active Low |
| 17       | PA7     | Channel 4 Encoder B  | Input     | Active Low |
| 6        | PF1     | Channel 4 Pushbutton | Input     | Active Low |
| 10       | PA0     | Channel 4 Indicator  | Output    | Active Low |
| 16       | PA6     | Channel 5 Encoder A  | Input     | Active Low |
| 15       | PA5     | Channel 5 Encoder B  | Input     | Active Low |
| 5        | PF0     | Channel 5 Pushbutton | Input     | Active Low |
| 11       | PA1     | Channel 5 Indicator  | Output    | Active Low |
| 14       | PA4     | Channel 6 Encoder A  | Input     | Active Low |
| 13       | PA3     | Channel 6 Encoder B  | Input     | Active Low |
| 4        | PC15    | Channel 6 Pushbutton | Input     | Active Low |
| 12       | PA2     | Channel 6 Indicator  | Output    | Active Low |

## Flashing

The focus of this section will be on the specifics of flashing on this board. I expect the reader to know generally how to program STM32 devices; there's plenty of other resources on the Internet that explain it better than I ever could.

There are two ways that the STM32 chip can be flashed: the USB DFU protocol,
and the ST-Link debug interface.

### USB DFU

To start the device in DFU mode:  While holding down the DFU button, connect
the USB cable so that it powers on and connects to the computer. Once it has
turned on, you can let go of DFU.

(_Note_: If the device is already connected, you can enter DFU mode without disconnecting by holding down DFU and pressing and releasing RESET. Again, you can release DFU once it has reset.)

Once the device is in DFU mode, you can use a tool like [DfuSe] or [dfu-util] on the host computer to connect to the device and flash firmware binaries.

### ST-Link

There is a spot for an ST-Link / SWD breakout header at the top of the PCB, in between the reset switch and USB port. The pinout of that port is designed to match the SWD header on Discovery/Nucleo boards, so from left to right:

1. 3v3 power (not connected in rev1, you have to supply power over USB)
2. SWCLK
3. GND
4. SWDIO
5. NRST
6. Not connected (normally SWO, which is unsupported by this chip).

Connect an ST-Link to that header, and connect both the ST-Link and the PCB to the computer via USB. Then you can flash using whatever ST-Link compatible tool or method you prefer. (openOCD, cargo-flash, etc)

[STM32F072C8]: https://www.st.com/en/microcontrollers-microprocessors/stm32f072c8.html
[DfuSe]: https://www.st.com/en/development-tools/stsw-stm32080.html#overview
[dfu-util]: http://dfu-util.sourceforge.net/