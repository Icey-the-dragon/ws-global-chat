# What this is
Dockerized app that makes a web app and api to serve a chat that goes trough websockets

## TODO:
- [x] Check if username exists before trying to create user
- [x] Record who is in which websocket sender (save user_id, senders should have a list of their ids)
- [x] Support /api/me?**id** Not supporting /api/logout?**id** since anyone could log anyone else out, maybe on the future if **id** changes from number to something else
- [x] Limit use of the /api/get_chat_history endpoint
- [ ] Make a full api helper so implementation is easier
- [x] Create endpoint for list of ids solving to list of usernames
- [ ] Add channels
  - [ ] Change DB to separate messages between all the channels
  - [ ] Change /ws endpoint to redirect to a list of dynamic endpoints for the channels. IE /ws -> /channel/[0-4] and have extra channels for other things like /channel/info or /channel/unformatted-[0-99] etc.


## WebSocket Protocol

### Message Format (Client → Server)
All WebSocket messages use a typed envelope:
```json
{
  "type": "broadcast | private | ephemeral",
  "metadata": {
    "session_id": "<session_token>",
    "to_username": "<target_user>",
    "sent_when_override": "<optional_timestamp>",
  },
  "content": "message text",
  "extra": {} // only if message is ephemereal
}
```

### Message Types

| Type | Description | Saved to DB | Routing |
|------|-------------|-------------|---------|
| `broadcast` | Normal chat message | yes | All connected clients via broadcast channel |
| `private` | Direct message to a user | No | Only to target user / server hosting user + echoed to sender. Who-probe if target offline (2s timeout, then voided) |
| `ephemeral` | Temporary message, supports arbitrary `extra` metadata for client-to-client custom comms | No | All connected clients via broadcast channel |

> Extra made for client-client custom things incase anyone might wanna use it, metadata field saved especifically for server fields.

### Message Format (Server → Client)
```json
{
  "type": "broadcast | private | ephemeral | who | error",
  "username": "sender_name",
  "content": "message text",
  "to_username": "recipient (private only)",
  "users": ["user1", "user2"],
  "extra": {}
}
```

### Frontend Slash Commands Javascript
- Normal message → `broadcast`
- `/pm @username message` → `private`
- `/ephemeral message` → `ephemeral`

## API Endpoints

### Endpoints:
- `/api/ws` → WebSocket connection (typed message envelope protocol)
- `/api/me` → **(GET)** `MeResponse` — Verify whether session is expired. Takes cookie as session_id. `200 OK` = valid session
  - Also suports `/api/me?id=<user_id>` for checking if user with id=<user_id> has an active session, useful for servers
- `/api/login` → **(POST)** `AuthResponse` — Returns cookie with session_id
  - Body: `LoginRequest`
- `/api/register` → **(POST)** `AuthResponse` — Returns cookie with session_id. Errors if user already exists (`409 Conflict`)
  - Body: `LoginRequest`
- `/api/logout` → **(POST)** Erases cookie and closes session (future: `?id=<sessid>` parameter)
- `/api/get_chat_history?limit=<number>` → **(GET)** Responds with the last N broadcast messages (currently exploitable, careful with bandwidth)
- `/api/online_users` → **(GET)** Returns array of usernames currently connected
- `/api/user_ids_by_sessions` → **(POST)** Returns array of user IDs for given session IDs. If no session_ids provided, uses cookie token
  - Body: `SessionIdsRequest`
- `/api/usernames_by_ids` → **(POST)** Returns array of usernames for given user IDs (in same order, null if not found)
  - Body: `UserIdsRequest`
- `/api/usernames_by_prefix` → **(POST)** Returns array of usernames starting with the given prefix (max 10 results)
  - Body: `UsernamePrefixRequest`

### Data Structures
```
LoginRequest {
    "username": "myname",
    "password": "pass"
}
MeResponse {
    "valid": bool,
    "session_token": "null_or_sess_id"
}
AuthResponse {
    "message": "successful auth (is for debugging and optional)",
    "session_token": "token_or_null"
}
SessionIdsRequest {
    "session_ids": ["token1", "token2"] // optional, if empty uses cookie
}
UserIdsRequest {
    "user_ids": [1, 2, 3]
}
UsernamePrefixRequest {
    "prefix": "user"
}
```
