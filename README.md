The mainboard in my PC provides no possibility to connect external temperature sensors.

This repo includes firmware for a small `STM32G0` development board where you can connect one (or more) NTC thermistors
as well as a host side service that provides a simple fan control using `hwmon` `sysfs` paths.

Communication between embedded and host is done via USB and the virtual serial port of the embedded programmer.

## Credits

The embedded crate is based on the [Knurling app-template][template] template.

# License

These crates are licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)

- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Code of Conduct

Contribution to this crates is organized under the terms of the [Rust Code of
Conduct][CoC], the maintainer of this crates, [DerFetzer][team], promises
to intervene to uphold that code of conduct.

[CoC]: https://www.rust-lang.org/policies/code-of-conduct
[team]: https://github.com/DerFetzer
[template]: https://github.com/knurling-rs/app-template
