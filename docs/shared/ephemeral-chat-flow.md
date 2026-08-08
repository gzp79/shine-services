# Ephemeral Chat Flow

## Goal

Provide real-time chat with reconnect catch-up, no persistent database, and automatic room expiry.

- Redis Streams are the source of truth.
- Redis Pub/Sub is wake-up only.

## Inbound path

1. WS receives a chat message and forwards it to Hub.
2. Hub forwards inbound chat workload to ChatService.
3. ChatService appends the message to the room stream in Redis.
4. ChatService trims the stream to approximately the last N messages.
5. ChatService refreshes room TTL.
6. Redis publishes a notification that wakes outbound sync.

## Outbound path

1. ChatService dispatcher wakes on Pub/Sub notification or fallback timer tick.
2. Dispatcher reads stream once from the global minimum cursor across active connections (bounded batch).
3. Dispatcher filters per connection: only stream entries newer than that connection cursor.
4. Dispatcher emits dedicated Hub outbound messages targeted by connection id (or broadcast when intended).
5. WS sends only messages addressed to its own connection (plus broadcasts).
6. Cursor advances after outbound batch is enqueued to Hub.

## New connection path

1. WS connect triggers a Hub connect event.
2. ChatService registers cursor state for the new connection.
3. ChatService triggers immediate sync for that connection.
4. Connection receives initial history batch via Hub-targeted outbound message.

## Reconnect and consistency

- Cursor is tracked per connection id, not per user.
- At-least-once delivery is accepted (duplicates possible).
- Missed Pub/Sub notifications are recovered by timer-driven sync.

## Retention and failure model

- Keep approximately last N messages per room.
- Refresh room TTL on each write.
- Inactive rooms expire automatically.
- If Redis restarts, chat history may disappear by design.
