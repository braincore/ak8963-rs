//! A library for the AK8963 magnetometer.

extern crate byteorder;
use byteorder::{
    ByteOrder,
    LittleEndian,
};
extern crate i2cdev;
use i2cdev::core::*;
use i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};
#[macro_use]
extern crate ndarray;
use ndarray::prelude::*;
use std::thread;
use std::time;

const MEAS_RANGE: f32 = 4912.0;  // UT = micro teslas

fn get_i2c_bus_path(i2c_bus: i32) -> String {
    format!("/dev/i2c-{}", i2c_bus)
}

#[derive(Clone, Copy)]
pub enum Ak8963Reg {
    St1 = 0x02,
    Hxl = 0x03,  // XoutL
    Cntl1 = 0x0a,
    Asax = 0x10,  // Sensitivity values
}

impl Ak8963Reg {
    fn addr(&self) -> u8 {
        *self as u8
    }
}

#[derive(Clone, Copy)]
enum RegCntl1 {
    PowerDn = 0,
    ContMeas1 = 0x02,  // 8hz sampling
    ContMeas2 = 0x06,  // 100hz sampling
    FuseRom = 0x0f,
    Sensitivity16bit = 1 << 4,
}

impl RegCntl1 {
    fn mask(&self) -> u8 {
        *self as u8
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SampleRate {
    /// Continuous measurement mode 1
    Opt8Hz,
    /// Continuous measurement mode 2
    Opt100Hz,
}

#[derive(Clone, Copy, Debug)]
pub enum Sensitivity {
    /// 0.6 uT/LSB
    Opt14bit,
    /// 0.15 uT/LSB
    Opt16bit,
}

impl Sensitivity {
    fn scalar(&self) -> f32 {
        MEAS_RANGE / match *self {
            Sensitivity::Opt14bit => 8192.0,
            Sensitivity::Opt16bit => 32768.0,
        }
    }
}

/// Output from the AK8963.
#[derive(Clone, Debug)]
pub struct Ak8963Sample {
    /// Unit is uT
    pub mag: Array1<f32>,
    /// The raw register value
    pub mag_raw: Array1<i16>,
    /// Whether a previous sample has been overwritten without being read.
    pub data_overrun: bool,
}

/// Errors that may occur when reading a sample.
#[derive(Debug)]
pub enum ReadSampleError {
    /// No new data is ready.
    DataNotReady,
    /// An i2c issue occurred.
    I2c(LinuxI2CError),
}

/// Magnetometer.
pub struct Ak8963 {
    i2c_dev: LinuxI2CDevice,
    pub factory_adjust: Array1<f32>,
    pub sensitivity: Sensitivity,
}

impl Ak8963 {

    /// Sets up and configures the AK8963.
    /// If i2c_addr isn't specified, defaults to 0x0c.
    pub fn new(
            i2c_bus: i32, i2c_addr: Option<u16>, sensitivity: Sensitivity,
            sample_rate: SampleRate)
            -> Result<Ak8963, LinuxI2CError> {
        let mut i2c_dev = LinuxI2CDevice::new(
            get_i2c_bus_path(i2c_bus), i2c_addr.unwrap_or(0x0c))?;

        let factory_adjust = Ak8963::read_sensitivity_adjustment(&mut i2c_dev)?;

        let mut ak = Ak8963 {
            i2c_dev,
            factory_adjust,
            sensitivity,
        };

        ak.initialize(sensitivity, sample_rate)?;

        Ok(ak)
    }

    /// Reads factory set sensitivity adjustment values from Fuse ROM.
    pub fn read_sensitivity_adjustment(i2c_dev: &mut LinuxI2CDevice) -> Result<Array1<f32>, LinuxI2CError> {
        // Power down mag
        i2c_dev.write(&[Ak8963Reg::Cntl1.addr(), RegCntl1::PowerDn.mask()])?;
        thread::sleep(time::Duration::from_millis(1));

        // Enter FUSE ROM mode
        i2c_dev.write(&[Ak8963Reg::Cntl1.addr(), RegCntl1::FuseRom.mask()])?;
        thread::sleep(time::Duration::from_millis(1));

        // Read sensitivity values from ROM
        let mut buf: [u8; 3] = [0u8; 3];
        i2c_dev.write(&[Ak8963Reg::Asax.addr()])?;
        i2c_dev.read(&mut buf)?;

        let factory_adjust = array![
            ((buf[0] - 128) as f32)/256.0 + 1.0,
            ((buf[1] - 128) as f32)/256.0 + 1.0,
            ((buf[2] - 128) as f32)/256.0 + 1.0,
        ];

        // Power down mag again
        i2c_dev.write(&[Ak8963Reg::Cntl1.addr(), RegCntl1::PowerDn.mask()])?;
        thread::sleep(time::Duration::from_micros(100));

        Ok(factory_adjust)
    }

    fn initialize(
            &mut self, sensitivity: Sensitivity, sample_rate: SampleRate)
            -> Result<(), LinuxI2CError> {

        let mut cntl1_byte = 0u8;
        match sensitivity {
            Sensitivity::Opt14bit => {},
            Sensitivity::Opt16bit => {
                cntl1_byte |= RegCntl1::Sensitivity16bit.mask()
            }
        }
        match sample_rate {
            SampleRate::Opt8Hz => {
                cntl1_byte |= RegCntl1::ContMeas1.mask()
            },
            SampleRate::Opt100Hz => {
                cntl1_byte |= RegCntl1::ContMeas2.mask()
            },
        }
        self.i2c_dev.write(&[Ak8963Reg::Cntl1.addr(), cntl1_byte])?;

        thread::sleep(time::Duration::from_micros(100));

        return Ok(())
    }

    /// Returns None if magnetometer reports magnetic field saturation.
    pub fn read_sample(&mut self) -> Result<Option<Ak8963Sample>, ReadSampleError> {
        let mut buf1: [u8; 1] = [0u8; 1];
        self.i2c_dev.write(&[Ak8963Reg::St1.addr()])
            .map_err(|e| ReadSampleError::I2c(e))?;
        self.i2c_dev.read(&mut buf1)
            .map_err(|e| ReadSampleError::I2c(e))?;

        // Check DRDY (data ready) bit
        if (buf1[0] & 1) == 0 {
            return Err(ReadSampleError::DataNotReady);
        }

        let mut buf: [u8; 7] = [0u8; 7];
        self.i2c_dev.write(&[Ak8963Reg::Hxl.addr()])
            .map_err(|e| ReadSampleError::I2c(e))?;
        self.i2c_dev.read(&mut buf)
            .map_err(|e| ReadSampleError::I2c(e))?;

        let mut sample = Ak8963::parse_sample_helper(
            &buf,
            self.sensitivity,
            &self.factory_adjust);

        if let Some(sample_unwrapped) = sample.as_mut() {
            // Check DOR (data overrun) bit
            if (buf1[0] & (1 << 1)) != 0 {
                sample_unwrapped.data_overrun = true;
            }
        }

        Ok(sample)
    }

    fn parse_sample_helper(
            data: &[u8], sensitivity: Sensitivity,
            factory_adjust: &Array1<f32>) -> Option<Ak8963Sample> {
        if (data[6] & (1 << 3)) > 0 {
            // Magnet saturation
            return None;
        }

        let mag_raw = array![
            LittleEndian::read_i16(&data[0 .. 2]),
            LittleEndian::read_i16(&data[2 .. 4]),
            LittleEndian::read_i16(&data[4 .. 6]),
        ];

        let mag = sensitivity.scalar() * factory_adjust *
            mag_raw.map(|e| *e as f32);

        Some(Ak8963Sample {
            mag_raw,
            mag,
            data_overrun: false,
        })
    }

    /// Returns None if magnetometer reports magnetic field saturation.
    pub fn parse_sample_data(&mut self, data: &[u8]) -> Option<Ak8963Sample> {
        Ak8963::parse_sample_helper(data, self.sensitivity, &self.factory_adjust)
    }
}

#[cfg(test)]
mod tests {
    use super::{Ak8963, SampleRate, Sensitivity};
    use std::env;

    fn get_i2c_bus() -> i32 {
        match env::var("AK8963_I2C_BUS") {
            Ok(bus_string) => {
                bus_string.parse().expect(
                    "Could not convert AK8963_I2C_BUS env var to i32.")
            },
            Err(_) => 1,
        }
    }

    fn get_i2c_addr() -> Option<u16> {
        match env::var("AK8963_I2C_ADDR") {
            Ok(addr_string) => {
                Some(addr_string.parse().expect(
                    "Could not convert AK8963_I2C_ADDR env var to u16."))
            },
            Err(_) => None,
        }
    }

    #[test]
    fn basic() {
        let mut ak8963 = Ak8963::new(
            get_i2c_bus(), get_i2c_addr(), Sensitivity::Opt16bit,
            SampleRate::Opt100Hz).unwrap();
        ak8963.read_sample().unwrap();
    }
}
