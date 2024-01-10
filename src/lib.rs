#![no_std]

use embassy_futures::block_on;
use embedded_hal_async::spi::{ErrorType, SpiBus};
use smart_leds::{SmartLedsWrite, RGB8};

const PATTERNS: [u8; 4] = [0b1000_1000, 0b1000_1110, 0b1110_1000, 0b1110_1110];

/// N = 12 * NUM_LEDS
pub struct Ws2812<SPI: SpiBus<u8>, const N: usize> {
    spi: SPI,
    data: [u8; N],
}

impl<SPI: SpiBus<u8>, const N: usize> Ws2812<SPI, N> {
    // TODO: I think the size of this should be configurable based on the SPI frequency
    const BLANK: [u8; 140] = [0_u8; 140];

    pub fn new(spi: SPI) -> Self {
        Self { spi, data: [0; N] }
    }

    pub async fn write_colors(
        &mut self,
        iter: impl Iterator<Item = RGB8>,
    ) -> Result<(), <SPI as ErrorType>::Error> {
        for (led_bytes, RGB8 { r, g, b }) in self.data.chunks_mut(12).zip(iter) {
            for (i, mut color) in [r, g, b].into_iter().enumerate() {
                for ii in 0..4 {
                    led_bytes[i * 4 + ii] = PATTERNS[((color & 0b1100_0000) >> 6) as usize];
                    color <<= 2;
                }
            }
        }
        self.spi.write(&self.data).await?;

        self.flush().await
    }

    #[inline]
    pub async fn flush(&mut self) -> Result<(), <SPI as ErrorType>::Error> {
        self.spi.write(&Self::BLANK).await
    }
}

// TODO: needing block_on feels terrible, but embedded-graphics needs sync functions
impl<SPI: SpiBus<u8>, const N: usize> SmartLedsWrite for Ws2812<SPI, N> {
    type Color = RGB8;
    type Error = <SPI as ErrorType>::Error;

    fn write<T, I>(&mut self, iterator: T) -> Result<(), Self::Error>
    where
        T: Iterator<Item = I>,
        I: Into<Self::Color>,
    {
        // TODO: use spi transaction?

        let mut offset = [0_u8; 1];

        // We introduce an offset in the fifo here, so there's always one byte in transit
        // Some MCUs (like the stm32f1) only a one byte fifo, which would result
        // in overrun error if two bytes need to be stored
        block_on(self.spi.write(&offset))?;

        if cfg!(feature = "mosi_idle_high") {
            block_on(self.flush())?;
        }

        block_on(self.write_colors(iterator.map(Into::into)))?;

        // Now, resolve the offset we introduced at the beginning
        block_on(self.spi.read(&mut offset))?;

        Ok(())
    }
}
