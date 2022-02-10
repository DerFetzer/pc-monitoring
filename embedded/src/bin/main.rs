#![no_main]
#![no_std]

use pc_monitoring_embedded as _; // global logger + panicking-behavior + memory layout

use pc_monitoring_embedded::adc::Adc;

use pc_monitoring_lib::heapless::String;
use pc_monitoring_lib::postcard::to_slice_cobs;
use pc_monitoring_lib::temperature::{Thermistor, ThermistorParameter};

use core::cell::RefCell;
use cortex_m::asm::wfi;
use cortex_m::{
    self,
    delay::Delay,
    interrupt::{free, Mutex},
    peripheral::NVIC,
};
use fixed::types::U20F12;
use stm32_hal2::clocks::Clocks;
use stm32_hal2::gpio::{Pin, PinMode, Port};
use stm32_hal2::pac::interrupt;
use stm32_hal2::rtc::{Rtc, RtcClockSource, RtcConfig};
use stm32_hal2::usart::{Usart, UsartConfig};
use stm32_hal2::{access_global, make_globals, pac};

make_globals!((RTC, Rtc));

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Entry");

    // Set up CPU peripherals
    let cp = cortex_m::Peripherals::take().unwrap();
    // Set up microcontroller peripherals
    let mut dp = pac::Peripherals::take().unwrap();

    dp.RCC.ahbenr.modify(|_, w| w.dmaen().set_bit());

    let clock_cfg = Clocks::default();
    clock_cfg.setup().unwrap();

    defmt::println!("ADC");

    let mut dbg_pin = Pin::new(Port::B, 6, PinMode::Output);
    dbg_pin.set_low();

    // Configure the ADC pin in analog mode. (This is the default state for some STM32 families,
    // but not all)
    let _adc_pin = Pin::new(Port::A, 0, PinMode::Analog);

    let mut adc = Adc::new(dp.ADC);

    defmt::println!("UART");

    // Configure pins for UART, according to the user manual.
    let _uart_tx = Pin::new(Port::A, 3, PinMode::Alt(1));
    let _uart_rx = Pin::new(Port::A, 2, PinMode::Alt(1));

    // Set up the USART1 peripheral.
    let mut uart = Usart::new(dp.USART2, 115200, UsartConfig::default(), &clock_cfg);

    defmt::println!("RTC");

    // Set up the realtime clock.
    let mut rtc = Rtc::new(
        dp.RTC,
        &mut dp.PWR,
        RtcConfig {
            clock_source: RtcClockSource::Lse,
            ..Default::default()
        },
    );

    rtc.set_wakeup(&mut dp.EXTI, 2.);

    free(|cs| {
        RTC.borrow(cs).replace(Some(rtc));
    });

    let mut delay = Delay::new(cp.SYST, clock_cfg.systick());

    defmt::println!("Unmask");

    // Unmask the interrupt line.
    unsafe {
        NVIC::unmask(pac::Interrupt::RTC_TAMP);
    }

    let mut ntc = Thermistor {
        name: String::from("radiator_in"),
        parameters: ThermistorParameter {
            b: 3950,
            tn: 25,
            r_tn: 10_000,
        },
        resistance: 0,
    };

    let mut buf: [u8; 1024] = [0; 1024];

    defmt::println!("Looping");

    loop {
        dbg_pin.set_high();
        adc.enable(&mut delay);
        let reading = adc.read(0);
        adc.disable();
        dbg_pin.set_low();

        let voltage = reading_to_voltage(reading, 3300);

        let res = reading_to_resistance(reading, 4_700);
        ntc.resistance = res;

        defmt::println!(
            "Reading: {:?}, Voltage: {:?}, Res: {:?}",
            reading,
            voltage,
            res
        );

        let data = to_slice_cobs(&ntc, &mut buf).unwrap();

        defmt::println!("Data length: {:?}", data.len());

        dbg_pin.set_high();
        uart.write(&data);
        dbg_pin.set_low();

        wfi();
    }
}

#[interrupt]
/// RTC wakeup handler
fn RTC_TAMP() {
    free(|cs| {
        access_global!(RTC, rtc, cs);
        rtc.clear_wakeup_flag();
    });
    defmt::println!("RTC_TAMP");
}

fn reading_to_resistance(reading: u16, pull_up_resistance: u32) -> u32 {
    let reading = U20F12::from_num(reading);
    let pull_up_resistance = U20F12::from_num(pull_up_resistance);
    let max_reading = U20F12::from_num(Adc::max_reading());

    let result = pull_up_resistance / ((max_reading / reading) - U20F12::ONE);
    result.to_num()
}

fn reading_to_voltage(reading: u16, v_ref_mv: u16) -> u16 {
    (v_ref_mv as u32 * reading as u32 / Adc::max_reading() as u32) as u16
}
