# Cooperative edit.

Cooperative as server is not keeping a strict control over the client. Clients can do anything and as long as the
the replay works everyone is happy. Client may drift however they want to, the server only helps to keep in sync with the
latest authentic version. It is up to the client to respect it or not, but it may happen that, the events from a drifted
client will be ignored more often.

## Client load

1. Client loads (tracks) chunk based on some (external) logic.
2. Send load/track request to the Replay server. (`Track(chunk_id)`)

## Client update

1. Apply an update command locally marked as uncommitted.
2. Send command to the `Stream server` (`Update(chunk_id, command)`)

## Client receive:

1. **Replay server** rejects the load request (`TrackRejected(chunk_id)`)
    - discard the chunk
    - Root cause: expired session, bogus or malicious client
2. **Stream server** rejects the command (`UpdateRejected(chunk_id, command)`)
    - discard the local changes 
    - Root cause: expired session, bogus or malicious client
3. **Replay server** sends a snapshot (`Chunk(data)`)
    - Sore chunk (unless already stored)
4. **Replay server** sends committed commands (`Vec<Update(chunk_id, version(!), command>)>`)
    - finalize or discard the local changes
    - If some version is missing request an update (`Refresh(chunk_id, version)`)
5. **Replay server** sends chunk hash commands (`Vec<(chunk_id, version, hash>)`), 
    - check drift and reload as required (**Client load**)

## Stream server (very lightweight)

1. Server receives command from client (`Update(chunk_id, command)`)
2. Server validates the command schema and the access to the chunk.
3. Send immediate rejection to the client if there is an error (`UpdateRejected(chunk_id, command)`)
4. Server saves the command to the event store

## Replay server:

1. Receive chunk to track from the client (`Track(chunk_id)`)
    a. Check access and reject if required (`TrackRejected(chunk_id)`)
    a. Start listening to events of the given chunk (if new)
    b. Load snapshot of the chunk (if new)
    c. Send the snapshot to the client (`Chunk(data)`)
    d. Start tracking the versions of the client
2. Receive a refresh from a client (`Refresh(chunk_id, version)`)
   a. Collect missing events based on the in-memory event queue
   b. If version is too old, send a (`Chunk(data)`) instead
3. Receive there are new snapshot in the DB
    a. Keep last n snapshot hashes in memory (hash is part of the DB notification)
    a. Check if the snapshot is drifted by comparing the hash
    b. If the snapshot is drifted, reload the chunk from DB and rebuild the chunk from the last authentic snapshot
4. Receive there are new events in the DB
    a. Keep last n commands in memory
    b. Load missing event
    b. Apply the commands to the chunk
    c. Using some heuristics (after every n command or time), store a new snapshot. If another snapshot was created meanwhile, check the hash and reload as required.
    d. Send the new events to the clients based on the tracked client versions (assume reliable communication) (`Vec<Update(chunk_id, version(!), command>)>`)
5. Periodically for some chunks send the authentic version to some clients (`Vec<(chunk_id, version, hash>)`)


## TODO:

- How should we now the operation was lost or committed ???
