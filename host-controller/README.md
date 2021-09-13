# Host Controller Software

**This is a Work in Progress.** The core of the application is nearly complete, but I would not
consider it ready for general use, because it is lacking GUI elements that are very important for
a good user experience.

However, if you would like to try it out, you can run it on the command line. You will, of course,
need a WindowMaster device to interface with the application (though support for MIDI control
surfaces may come in the future).

Make sure you have the [Rust toolchain](https://www.rust-lang.org/tools/install) installed, and
then clone and run with:

```sh
git clone https://github.com/agausmann/windowmaster.git
cd windowmaster/host-controller
cargo run
```

You can set bindings on each channel by pressing the knob down until its LED starts blinking.
The menu will be printed on the console. Navigate by rotating the knob, and select by pressing.
Then, the knob can be used to control the volume and mute of that device or application.
Long-press again at any time to open the menu and re-bind the channel.