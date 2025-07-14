use crate::input_manager::{ButtonLike, InputSources, PressedButton, UserInput};
use bevy::time::Time;

/// A button chord that processes multiple buttons as a single input.
pub struct ButtonChord<B1, B2, B3, B4>
where
    B1: ButtonLike,
    B2: ButtonLike,
    B3: ButtonLike,
    B4: ButtonLike,
{
    chord: (B1, B2, B3, B4),
}

impl<B1, B2, B3, B4> ButtonChord<B1, B2, B3, B4>
where
    B1: ButtonLike,
    B2: ButtonLike,
    B3: ButtonLike,
    B4: ButtonLike,
{
    pub fn new4(b1: B1, b2: B2, b3: B3, b4: B4) -> Self {
        Self { chord: (b1, b2, b3, b4) }
    }
}

impl<B1, B2, B3> ButtonChord<B1, B2, B3, PressedButton>
where
    B1: ButtonLike,
    B2: ButtonLike,
    B3: ButtonLike,
{
    pub fn new3(b1: B1, b2: B2, b3: B3) -> Self {
        Self::new4(b1, b2, b3, PressedButton)
    }
}

impl<B1, B2> ButtonChord<B1, B2, PressedButton, PressedButton>
where
    B1: ButtonLike,
    B2: ButtonLike,
{
    pub fn new2(b1: B1, b2: B2) -> Self {
        Self::new4(b1, b2, PressedButton, PressedButton)
    }
}

impl<B1, B2, B3, B4> UserInput for ButtonChord<B1, B2, B3, B4>
where
    B1: ButtonLike,
    B2: ButtonLike,
    B3: ButtonLike,
    B4: ButtonLike,
{
    fn integrate(&mut self, input: &InputSources) {
        self.chord.0.integrate(input);
        self.chord.1.integrate(input);
        self.chord.2.integrate(input);
        self.chord.3.integrate(input);
    }
}

impl<B1, B2, B3, B4> ButtonLike for ButtonChord<B1, B2, B3, B4>
where
    B1: ButtonLike,
    B2: ButtonLike,
    B3: ButtonLike,
    B4: ButtonLike,
{
    fn process(&mut self, time: &Time) -> Option<bool> {
        let v0 = self.chord.0.process(time).unwrap_or(false);
        let v1 = self.chord.1.process(time).unwrap_or(false);
        let v2 = self.chord.2.process(time).unwrap_or(false);
        let v3 = self.chord.3.process(time).unwrap_or(false);
        Some(v0 & v1 & v2 & v3)
    }
}
