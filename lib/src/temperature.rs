use heapless::String;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct Thermistor {
    pub name: String<64>,
    pub parameters: ThermistorParameter,
    pub resistance: u32,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
pub struct ThermistorParameter {
    pub b: u16,
    pub tn: i16,
    pub r_tn: u32,
}

#[cfg(feature = "std")]
impl Thermistor {
    pub fn get_temperature(&self) -> f32 {
        1f32 / (1f32 / (self.parameters.tn as f32 + 273.15f32)
            + 1f32 / self.parameters.b as f32
                * (self.resistance as f32 / self.parameters.r_tn as f32).ln())
            - 273.15f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[cfg(feature = "std")]
    #[test]
    fn thermistor() {
        let parameters = ThermistorParameter {
            b: 3950,
            tn: 25,
            r_tn: 10_000,
        };
        let test_data = [(10_000, 25.0), (2_472, 60.0), (31_770, 0.0)];

        for test_datum in test_data {
            let th = Thermistor {
                name: String::from("test"),
                resistance: test_datum.0,
                parameters,
            };

            assert_relative_eq!(th.get_temperature(), test_datum.1, max_relative = 1.0);
        }
    }
}
