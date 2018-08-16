# AK8963 Library for Rust [![Latest Version]][crates.io] [![Documentation]][docs.rs] 

[Latest Version]: https://img.shields.io/crates/v/ak8963.svg
[crates.io]: https://crates.io/crates/ak8963
[Documentation]: https://docs.rs/ak8963/badge.svg
[docs.rs]: https://docs.rs/ak8963

A library for the AK8963 magnetometer. Only supports the i2c interface (no
SPI).

## Features

* Reads the sensitivity adjustment values from the Fuse ROM and applies them.
* Adjustable sensitivity and continuous measurement rate.
* Exposes data-not-ready, data overrun, and magnetic saturation cases.

## Usage

See basic test in `lib.rs` or `examples/scan.rs`.

## Testing

By default, uses i2c bus=1, addr=0x0c. To override, use these environment
variables:

```
MS5611_I2C_BUS2=1 MS5611_I2C_ADDR=12 cargo test
```

## Resources

* [Datasheet](https://www.akm.com/akm/en/file/datasheet/AK8963C.pdf)