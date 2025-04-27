// Client flow:
//   1. Client temporarily applies the command locally
//   2. Client sends command to server using the store_operation
//   3. Poll/Get new commands(events) from server
//   4. Client applies persisted commands locally and finalize the local update

// Stream flow:
//   1. Server receives command from client (Client/2.)
//   2. Start tracking of the target chunk (State/2.)
//   2. Server validates the command schema, and send rejection to the client if invalid
//   3. Server saves the command to the event store and sends it to all clients (Client/3.)
// Note: Commands are persisted independent if it can be applied. Stream server checks only the schema, ut not the semantics. State playbacks (Client, State flows) should
//   skip outdated commands.

// State flow:
//   1. Server listen to events (through PG Notification)
//   2. On tracked chunks, apply the commands
//   3. For every nth command create a snapshot

// todo:
//  - How to prevent multiple State instance saving the same same snapshot. Unique id will prevent it, is that enough?
//  - Client can detect if states are drifted by comparing some hash of the state at a given version. But how should servers detect any drift
//    to the snapshots ?
//  - Client should now when a local operation was persisted or rejected, how should the client be informed ? What could be the unique id of a (pending) operation ?

mod es_chunk_id;
//pub use self::ev_chunk_id::*;
mod es_store_dense;
pub use self::es_store_dense::*;
