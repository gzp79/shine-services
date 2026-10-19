//! A throwaway replay state that exercises every feature of the replay core. It exists only in the tests.

use serde::{Deserialize, Serialize};
use shine_game::replay::{CommandContext, CommandEnvelope, ReplayState, StableHash, StableHasher};
use uuid::Uuid;

pub const SLOTS: usize = 10;
pub const MAX_BATCH_LEN: usize = 32;

/// A value with the user and the command that wrote it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    pub value: i64,
    pub writer: Uuid,
    pub command_id: Uuid,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DemoState {
    pub slots: [Slot; SLOTS],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DemoCommand {
    Set {
        idx: u32,
        value: i64,
    },
    Add {
        idx: u32,
        delta: i64,
    },
    Swap {
        a: u32,
        b: u32,
    },
    CompareAndSet {
        idx: u32,
        expected: i64,
        value: i64,
    },
    /// Clears a slot written by `command_id`; only its writer may do it.
    Revert {
        idx: u32,
        command_id: Uuid,
    },
    /// All-or-nothing; written by one command, so every slot gets the batch's id.
    Batch {
        commands: Vec<DemoCommand>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DemoReject {
    IndexOutOfRange(u32),
    Overflow,
    Mismatch,
    /// The slot was not written by that command.
    NotFound,
    /// The slot was written by another user.
    NotOwner,
}

impl DemoState {
    fn slot(idx: u32) -> Result<usize, DemoReject> {
        let slot = idx as usize;
        if slot < SLOTS {
            Ok(slot)
        } else {
            Err(DemoReject::IndexOutOfRange(idx))
        }
    }

    fn write(&mut self, ctx: &CommandContext, slot: usize, value: i64) {
        self.slots[slot] = Slot {
            value,
            writer: ctx.user_id,
            command_id: ctx.command_id,
        };
    }

    fn apply_in_place(&mut self, ctx: &CommandContext, command: &DemoCommand) -> Result<(), DemoReject> {
        match command {
            DemoCommand::Set { idx, value } => {
                let slot = Self::slot(*idx)?;
                self.write(ctx, slot, *value);
            }
            DemoCommand::Add { idx, delta } => {
                let slot = Self::slot(*idx)?;
                let value = self.slots[slot].value.checked_add(*delta).ok_or(DemoReject::Overflow)?;
                self.write(ctx, slot, value);
            }
            DemoCommand::Swap { a, b } => {
                self.slots.swap(Self::slot(*a)?, Self::slot(*b)?);
            }
            DemoCommand::CompareAndSet { idx, expected, value } => {
                let slot = Self::slot(*idx)?;
                if self.slots[slot].value != *expected {
                    return Err(DemoReject::Mismatch);
                }
                self.write(ctx, slot, *value);
            }
            DemoCommand::Revert { idx, command_id } => {
                let slot = Self::slot(*idx)?;
                if self.slots[slot].command_id != *command_id {
                    return Err(DemoReject::NotFound);
                }
                if self.slots[slot].writer != ctx.user_id {
                    return Err(DemoReject::NotOwner);
                }
                self.slots[slot] = Slot::default();
            }
            DemoCommand::Batch { commands } => {
                for command in commands {
                    self.apply_in_place(ctx, command)?;
                }
            }
        }
        Ok(())
    }
}

impl StableHash for DemoState {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        (SLOTS as u64).stable_hash(hasher);
        for slot in &self.slots {
            slot.value.stable_hash(hasher);
            hasher.write(slot.writer.as_bytes());
            hasher.write(slot.command_id.as_bytes());
        }
    }
}

impl ReplayState for DemoState {
    type Command = DemoCommand;
    type Reject = DemoReject;

    const KIND: &'static str = "demo";

    fn apply(&mut self, ctx: &CommandContext, command: &DemoCommand) -> Result<(), DemoReject> {
        // Work on a copy so a failing batch leaves no partial change.
        let mut next = self.clone();
        next.apply_in_place(ctx, command)?;
        *self = next;
        Ok(())
    }

    /// A batch is flat and writes only: a revert inside it would skip the ownership check at ingest.
    fn validate(command: &DemoCommand) -> bool {
        match command {
            DemoCommand::Batch { commands } => {
                commands.len() <= MAX_BATCH_LEN
                    && !commands
                        .iter()
                        .any(|c| matches!(c, DemoCommand::Batch { .. } | DemoCommand::Revert { .. }))
            }
            _ => true,
        }
    }

    fn refers_to(command: &DemoCommand) -> Option<Uuid> {
        match command {
            DemoCommand::Revert { command_id, .. } => Some(*command_id),
            _ => None,
        }
    }
}

/// A user id for the tests.
pub fn user(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// The envelope of `command`, written by `user_id`; the command id is the version.
pub fn envelope(version: i64, user_id: Uuid, command: DemoCommand) -> CommandEnvelope<DemoCommand> {
    CommandEnvelope {
        version,
        command_id: Uuid::from_u128(1000 + version as u128),
        user_id,
        command: Some(command),
    }
}

/// The context of the envelope.
pub fn context_of(envelope: &CommandEnvelope<DemoCommand>) -> CommandContext {
    CommandContext {
        version: envelope.version,
        command_id: envelope.command_id,
        user_id: envelope.user_id,
    }
}

pub fn set(idx: u32, value: i64) -> DemoCommand {
    DemoCommand::Set { idx, value }
}
