# Cooperative Edit

The cooperative server does not strictly control the client. Clients can perform any actions as long as the replay functions correctly and all 
participants are satisfied. Clients may drift as they wish, while the server assists in synchronizing them with the latest authentic version. Clients 
are responsible for respecting this synchronization; however, events from a drifted client are often ignored.

## High-Level Overview

### Parties

- **Client**
  - Made of a replayed (authentic) state and local pending change
  - On edit operations: Send update operations to the Stream server and apply to local pending changes
  - Get (authentic) commands
    - Snapshot to use as the base for the (authentic) state
    - Update operation: apply on the (authentic) state and send sync events to the server 
      - sync event contains the last seen version and a checksum for drift detection

- **Stream** server
  - Read events from the clients with schema validation
    - Invalid events are ignored silently, later some dashboard or monitoring can be added
  - Serialize events to the DB (implements deterministic ordering)
    - Last writer (operation) wins for conflict resolution
  
- **Replay** server
  - Made of the replayed (authentic) state
  - Keep track of clients and distribute update operations
    - Send disconnect information for unauthorized tracking request
    - Send missing operations (from all parties) - provides operation acknowledgments
    - Send re-sync data for new or drifted clients
      - When server detects the client has drifted from the authentic version using the client's local hash, 
      the client receives all data for resync      
  - When a new client is tracked, read data from DB
    - Keep some history: last n events, last n checksum to allow fast responses
    - Some idea: the last (stored) snapshot and events are stored as serialized bytes in memory, 
    thus there is no need to access the model for client message
  - Watch for DB changes:
    - When a new operation is stored, update the state, calculate new checksum
      - Periodically save the snapshot into DB
    - When a new snapshot is stored, check checksum for drift, and rebuild the state
