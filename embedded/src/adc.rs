use embedded_hal::blocking::delay::DelayUs;
use stm32_hal2::pac::{ADC, RCC};

pub struct Adc {
    reg: ADC,
}

impl Adc {
    pub fn new(adc: ADC) -> Self {
        Adc { reg: adc }
    }

    pub fn max_reading() -> u16 {
        0x0FFF
    }

    pub fn enable<DELAY: DelayUs<u32>>(&mut self, delay: &mut DELAY) {
        let rcc = unsafe { &(*RCC::ptr()) };
        rcc.apbenr2.modify(|_, w| w.adcen().set_bit());
        rcc.apbrstr2.modify(|_, w| w.adcrst().set_bit());
        rcc.apbrstr2.modify(|_, w| w.adcrst().clear_bit());

        self.reg.cr.modify(|_, w| w.advregen().set_bit());
        delay.delay_us(20u32);

        self.reg.cr.modify(|_, w| w.adcal().set_bit());
        while self.reg.cr.read().adcal().bit_is_set() {}

        self.configure();

        self.reg.isr.write(|w| w.adrdy().set_bit());
        self.reg.cr.modify(|_, w| w.aden().set_bit());
        while self.reg.isr.read().adrdy().bit_is_clear() {}
    }

    fn configure(&self) {
        unsafe {
            self.reg.cfgr2.modify(|_, w| {
                w.ckmode()
                    .bits(0b01)
                    .ovse()
                    .set_bit()
                    .ovsr()
                    .bits(0b111)
                    .ovss()
                    .bits(0b1000)
            })
        };
        self.reg
            .cfgr1
            .modify(|_, w| w.chselrmod().clear_bit().cont().clear_bit());
        unsafe {
            self.reg
                .smpr
                .modify(|_, w| w.smp1().bits(0b111).smp2().bits(0b111));
        }
    }

    pub fn disable(&mut self) {
        self.reg.cr.modify(|_, w| w.addis().set_bit());
        while self.reg.cr.read().aden().bit_is_set() {}
        self.reg.isr.write(|w| w.adrdy().set_bit());
        self.reg.cr.modify(|_, w| w.advregen().clear_bit());

        let rcc = unsafe { &(*RCC::ptr()) };
        rcc.apbenr2.modify(|_, w| w.adcen().clear_bit());
    }

    pub fn read(&mut self, channel: u8) -> u16 {
        assert!(channel <= 18);
        unsafe { self.reg.chselr().write(|w| w.chsel().bits(1 << channel)) };
        self.reg.cr.modify(|_, w| w.adstart().set_bit());
        while self.reg.isr.read().eos().bit_is_clear() {}
        self.reg.isr.write(|w| w.eos().set_bit());
        self.reg.dr.read().regular_data().bits()
    }
}
