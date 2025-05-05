// Client flow:
//   1. Client temporarily applies the command locally
//   2. Client sends command to server using the store_operation
//   3. Poll/Get new commands(events) from server (websocket)
//   4. Client applies persisted commands locally and finalize the local update

// Stream flow (no bevy, very lightweight)
//   1. Server receives command from client (Client/2.)
//   2. Server validates the command schema and access to chunk. Send immediate rejection to the client if invalid
//   3. Server saves the command to the event store and sends it to all clients (Client/3.)
// Note: Commands are persisted independent if it can be applied. Stream server checks only the schema, but not the semantics.
//   State playbacks (Client, Replay flows) should skip outdated commands.

// Replay flow:
//   1. Server listen to event source events (for example through PG Notification)
//   1.b Server listens to snapshot events 
//   2. On a new event, apply the commands
//   2.b On snapshot event, reload the chunk.
//   3. For every nth command or in every n minutes, create a snapshot
// Question: what chunks should it track ?

// todo:
//  - How to prevent multiple State instance saving the same same snapshot. Unique id will prevent it, is that enough?
//  - Client can detect if states are drifted by comparing some hash of the state at a given version. But how should servers detect any drift
//    to the snapshots ?
//    A: When a new snapshot is created (similar to event listener), check the hash of the snapshot
//      and compare to the tracked version. If the hash is different, then the snapshot is drifted, reload the aggregate from DB.
//      But before creating the we snapshot, we should check if the snapshot is drifted even if no nfo exists about the prev snapshot.

//  - Client should now when a local operation was persisted or rejected, how should the client be informed ? What could be the unique id of a (pending) operation ?

mod es_chunk_id;
//pub use self::ev_chunk_id::*;
mod es_store;
pub use self::es_store::*;
