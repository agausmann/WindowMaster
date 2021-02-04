# WindowMaster PCB

This circuit board is the main board for the WindowMaster physcial controller.
It supports 6 control channels which can be individually assigned to different
channels via the WindowMaster software.

The MCU powering this board is an [STM32F072C8], which can read the encoder
states and set indicators via GPIO, and communicate with the host computer over
USB. You can flash the provided firmware to communicate with and control the
WindowMaster host software, or write and flash your own if you have another
usecase for the control surface.  In the future, I may provide other firmwares
to turn it into a generic input device like a joystick or MIDI. Feel free to
contact me if you'd like to commission a custom firmware - I'd love to hear
about your idea!

This board is developed with KiCad version 5.1.9. Components, especially those
that can or must be surface-mounted, are picked from the [JLCPCB SMT Parts
Library](https://jlcpcb.com/parts), if there is a suitable part there. This
makes it more convenient to fabricate and assemble, as they will do most of the
work. Everything else is picked to be easy to hand-solder with an iron, which
generally MEans through-hole or large tabs.

## Documentation/Pinout

TODO

## Ordering

TODO

[STM32F072C8]: https://www.st.com/en/microcontrollers-microprocessors/stm32f072c8.html
