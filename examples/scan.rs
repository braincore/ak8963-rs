extern crate ak8963;
use ak8963::{
    Ak8963,
    SampleRate,
    Sensitivity,
};

pub fn main() {
    let mut ak = Ak8963::new(
        1, None, Sensitivity::Opt16bit, SampleRate::Opt100Hz).unwrap();

    loop {
        println!("Measurement: {:?}", ak.read_sample());
    }
}
