//! Status parsing trait and implementation

use crate::{reader::Yieldable, writer::MidiWriteable};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A MIDI Message Event
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MidiEvent {
    /// Turn Off event
    /// This message is sent whena  note is released
    NoteOff(u8, NoteMeta),
    /// Turn On event
    /// This message is sent when a note is depressed
    NoteOn(u8, NoteMeta),
    /// Polyphonic Key Pressure
    /// This message is most often sent by pressing down a key after it "bottoms out"
    PolyphonicKeyPressure(u8, NoteMeta),
    /// Control change
    /// This message is sent when a controller value changes. Controllers include devices such as
    /// pedals and levers. Certain controller numbers are reserved.
    ControlChange(u8, ControlChange),
    /// Program change.
    /// This message is sent when the patch number changes
    ProgramChange(u8, u8),
    /// Channel Pressure
    /// This message is most often sent by pressing down on a key after it "bottoms out"
    ChannelPressure(u8, u8),
    /// Pitch Wheel Change
    /// This message is sent to indicate a change in the pitch wheel as measured by a fourteen bit
    /// value.
    PitchWheelChange(u8, u16),
}

impl MidiWriteable for MidiEvent {
    fn to_midi_bytes(self) -> Vec<u8> {
        let status_byte = self.get_status_channel_combo();
        let mut bytes = vec![status_byte];

        let extra = match self {
            Self::NoteOff(_, notemeta)
            | Self::NoteOn(_, notemeta)
            | Self::PolyphonicKeyPressure(_, notemeta) => notemeta.to_midi_bytes(),
            Self::ControlChange(_, control_change) => control_change.to_midi_bytes(),
            Self::ProgramChange(_, val) | Self::ChannelPressure(_, val) => val.to_midi_bytes(),
            Self::PitchWheelChange(_, val) => val.to_midi_bytes(),
        };

        bytes.extend(extra.iter());

        bytes
    }
}

impl MidiEvent {
    /// Combines the channel and current type's status identifier into a single byte
    pub fn get_status_channel_combo(&self) -> u8 {
        match self {
            Self::NoteOff(channel, _) => 0b10000000 | channel,
            Self::NoteOn(channel, _) => 0b10010000 | channel,
            Self::PolyphonicKeyPressure(channel, _) => 0b10100000 | channel,
            Self::ControlChange(channel, _) => 0b10110000 | channel,
            Self::ProgramChange(channel, _) => 0b11000000 | channel,
            Self::ChannelPressure(channel, _) => 0b11010000 | channel,
            Self::PitchWheelChange(channel, _) => 0b11100000 | channel,
        }
    }
}

/// Error type for an unsupported error type
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnsupportedStatusCode(u8);

impl core::error::Error for UnsupportedStatusCode {}
impl core::fmt::Display for UnsupportedStatusCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write![f, "Unsupported Status Code {}", self.0]
    }
}
/// Wrapper around iterator to prevent trait implementation sillyness
pub struct IteratorWrapper<T>(pub T);
impl<ITER> TryFrom<IteratorWrapper<&mut ITER>> for MidiEvent
where
    ITER: Iterator<Item = u8>,
{
    type Error = UnsupportedStatusCode;
    fn try_from(value: IteratorWrapper<&mut ITER>) -> Result<Self, Self::Error> {
        let value = value.0;
        let status = value.get(1)[0];
        let channel = status & 0x0F;
        let status = status >> 4;

        match status {
            0b1000 => {
                let reads = value.get(2);
                Ok(Self::NoteOff(
                    channel,
                    NoteMeta {
                        key: reads[0],
                        velocity: reads[1],
                    },
                ))
            }

            0b1001 => {
                let reads = value.get(2);
                Ok(Self::NoteOn(
                    channel,
                    NoteMeta {
                        key: reads[0],
                        velocity: reads[1],
                    },
                ))
            }

            0b1011 => {
                let reads = value.get(2);
                Ok(Self::ControlChange(
                    channel,
                    ControlChange {
                        controller_number: reads[0],
                        new_value: reads[1],
                    },
                ))
            }

            0b1100 => {
                let reads = value.get(1);
                Ok(Self::ProgramChange(channel, reads[0]))
            }

            0b1101 => {
                let reads = value.get(1);
                Ok(Self::ChannelPressure(channel, reads[0]))
            }

            0b1110 => {
                let reads = value.get(2);

                const MASK: u8 = 0x7;

                let mut result: u16 = 0;
                for byte in reads.iter().rev() {
                    result <<= 7;
                    result |= (byte & MASK) as u16;
                }

                Ok(Self::PitchWheelChange(channel, result))
            }

            code => Err(UnsupportedStatusCode(code)),
        }
    }
}

/// Metadata for a note's relative info. Including channel, key and velocity
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NoteMeta {
    /// Note key
    key: u8,
    /// Note velocity
    velocity: u8,
}

impl MidiWriteable for NoteMeta {
    fn to_midi_bytes(self) -> Vec<u8> {
        vec![self.key, self.velocity]
    }
}

/// Metadata for changing a controller
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ControlChange {
    /// Controller number
    controller_number: u8,
    /// New value
    new_value: u8,
}

impl MidiWriteable for ControlChange {
    fn to_midi_bytes(self) -> Vec<u8> {
        vec![self.controller_number, self.new_value]
    }
}

#[cfg(test)]
mod tests {
    use crate::{chunk::track::event::UnsupportedStatusCode, writer::MidiWriteable};

    use super::{IteratorWrapper, MidiEvent, NoteMeta};

    #[test]
    fn midi_event_status_parsing() {
        let status_channel = 0b10001111;
        let key = 0b01010101;
        let velocity = 0b11111111;

        let mut stream = [status_channel, key, velocity].into_iter();
        let status =
            MidiEvent::try_from(IteratorWrapper(&mut stream)).expect("Parse off note signal");

        let expected = MidiEvent::NoteOff(0x0F, NoteMeta { key, velocity });

        assert_eq!(status, expected)
    }

    #[test]
    fn midi_event_status_parsing_fails_on_invalid_status() {
        let status_channel = 0b00101111;
        let key = 0b01010101;
        let velocity = 0b11111111;

        let mut stream = [status_channel, key, velocity].into_iter();
        let status = MidiEvent::try_from(IteratorWrapper(&mut stream));
        assert_eq!(status, Err(UnsupportedStatusCode(0b0010)));
    }

    #[test]
    fn midi_event_backwards_parses_to_bytes() {
        let key = 0b01010101;
        let velocity = 0b11111111;

        let expected = MidiEvent::NoteOff(0x0F, NoteMeta { key, velocity });

        let mut stream = expected.clone().to_midi_bytes().into_iter();
        let bytes =
            MidiEvent::try_from(IteratorWrapper(&mut stream)).expect("Parse from serialized bytes");

        assert_eq!(bytes, expected)
    }
}
