//! The rotary encoder implementation is from: https://github.com/leshow/rotary-encoder-hal/blob/master/src/lib.rs
//!

use embedded_hal::digital::InputPin;
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;

use crate::event::KeyEvent;
use crate::keyboard::KEY_EVENT_CHANNEL;

use super::InputDevice;

/// Holds current/old state and both [`InputPin`](https://docs.rs/embedded-hal/latest/embedded_hal/digital/trait.InputPin.html)
#[derive(Clone, Debug)]
// #[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RotaryEncoder<A, B, P> {
    pin_a: A,
    pin_b: B,
    state: u8,
    phase: P,
    /// (row, col) position in the keymap
    clockwise_pos: (u8, u8),
    /// (row, col) position in the keymap
    counter_clockwise_pos: (u8, u8),
}

/// The encoder direction is either `Clockwise`, `CounterClockwise`, or `None`
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// #[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Direction {
    /// A clockwise turn
    Clockwise,
    /// A counterclockwise turn
    CounterClockwise,
    /// No change
    None,
}

/// Allows customizing which Quadrature Phases should be considered movements
/// and in which direction or ignored.
pub trait Phase {
    /// Given the current state `s`, return the direction.
    fn direction(&mut self, s: u8) -> Direction;
}

/// Default implementation of `Phase`.
pub struct DefaultPhase;

/// The useful values of `s` are:
/// - 0b0001 | 0b0111 | 0b1000 | 0b1110
/// - 0b0010 | 0b0100 | 0b1011 | 0b1101
impl Phase for DefaultPhase {
    fn direction(&mut self, s: u8) -> Direction {
        match s {
            0b0001 | 0b0111 | 0b1000 | 0b1110 => Direction::Clockwise,
            0b0010 | 0b0100 | 0b1011 | 0b1101 => Direction::CounterClockwise,
            _ => Direction::None,
        }
    }
}

impl<A, B> RotaryEncoder<A, B, DefaultPhase>
where
    A: InputPin,
    B: InputPin,
{
    /// Accepts two [`InputPin`](https://docs.rs/embedded-hal/latest/embedded_hal/digital/trait.InputPin.html)s, these will be read on every `update()`.
    ///
    /// `clockwise_pos` and `counter_clockwise_pos` are the (row, col) positions in the keymap.
    pub fn new(
        pin_a: A,
        pin_b: B,
        clockwise_pos: (u8, u8),
        counter_clockwise_pos: (u8, u8),
    ) -> Self {
        Self {
            pin_a,
            pin_b,
            state: 0u8,
            phase: DefaultPhase,
            clockwise_pos,
            counter_clockwise_pos,
        }
    }
}

impl<A: InputPin, B: InputPin, P: Phase> RotaryEncoder<A, B, P> {
    /// Accepts two [`InputPin`](https://docs.rs/embedded-hal/latest/embedded_hal/digital/trait.InputPin.html)s, these will be read on every `update()`, while using `phase` to determine the direction.
    ///
    /// `clockwise_pos` and `counter_clockwise_pos` are the (row, col) positions in the keymap.
    pub fn with_phase(
        pin_a: A,
        pin_b: B,
        phase: P,
        clockwise_pos: (u8, u8),
        counter_clockwise_pos: (u8, u8),
    ) -> Self {
        Self {
            pin_a,
            pin_b,
            state: 0u8,
            phase,
            clockwise_pos,
            counter_clockwise_pos,
        }
    }

    /// Call `update` to evaluate the next state of the encoder, propagates errors from `InputPin` read
    pub fn update(&mut self) -> Direction {
        // use mask to get previous state value
        let mut s = self.state & 0b11;

        let (a_is_low, b_is_low) = (self.pin_a.is_low(), self.pin_b.is_low());

        // move in the new state
        match a_is_low {
            Ok(true) => s |= 0b0100,
            Ok(false) => {}
            Err(_) => return Direction::None,
        }
        match b_is_low {
            Ok(true) => s |= 0b1000,
            Ok(false) => {}
            Err(_) => return Direction::None,
        }

        // move new state in
        self.state = s >> 2;
        self.phase.direction(s)
    }

    /// Returns a reference to the first pin. Can be used to clear interrupt.
    pub fn pin_a(&mut self) -> &mut A {
        &mut self.pin_a
    }

    /// Returns a reference to the second pin. Can be used to clear interrupt.
    pub fn pin_b(&mut self) -> &mut B {
        &mut self.pin_b
    }

    /// Returns a reference to both pins. Can be used to clear interrupt.
    pub fn pins(&mut self) -> (&mut A, &mut B) {
        (&mut self.pin_a, &mut self.pin_b)
    }

    /// Consumes this `Rotary`, returning the underlying pins `A` and `B`.
    pub fn into_inner(self) -> (A, B) {
        (self.pin_a, self.pin_b)
    }
}

impl<
        #[cfg(feature = "async_matrix")] A: InputPin + Wait,
        #[cfg(not(feature = "async_matrix"))] A: InputPin,
        #[cfg(feature = "async_matrix")] B: InputPin + Wait,
        #[cfg(not(feature = "async_matrix"))] B: InputPin,
        P: Phase,
    > InputDevice for RotaryEncoder<A, B, P>
{
    async fn run(&mut self) {
        loop {
            #[cfg(feature = "async_matrix")]
            {
                let (pin_a, pin_b) = self.pins();
                embassy_futures::select::select(
                    pin_a.wait_for_any_edge(),
                    pin_b.wait_for_any_edge(),
                )
                .await;
            }
            // If not using async_matrix feature, scanning the encoder pins with 50HZ frequency
            #[cfg(not(feature = "async_matrix"))]
            embassy_time::Timer::after_millis(20).await;
            let direction = self.update();
            // TODO: Resolution
            let (row, col) = match direction {
                Direction::Clockwise => (self.clockwise_pos.0, self.clockwise_pos.1),
                Direction::CounterClockwise => {
                    (self.counter_clockwise_pos.0, self.counter_clockwise_pos.1)
                }
                Direction::None => continue,
            };

            // Send the key event, process it like a tap
            KEY_EVENT_CHANNEL
                .send(KeyEvent {
                    row,
                    col,
                    pressed: true,
                })
                .await;
            embassy_time::Timer::after_millis(10).await;
            KEY_EVENT_CHANNEL
                .send(KeyEvent {
                    row,
                    col,
                    pressed: false,
                })
                .await;
        }
    }
}
