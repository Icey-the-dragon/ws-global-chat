# What this is
Dockerized app that makes a web app and api to serve a chat that goes trough websockets
## TODO:
- [x] Change message type sent to have a format like {"type": "broadcast", "metadata":{"session_id":"<sess_id>","sent_when_override":"00_00_000"},"content":"i just use arch"}
- [x] Record who is in which websocket sender (save user_id, senders should have a list of their ids)
- [x] Support "private" messages (never truly private but not really anyone will see them just the server you connected to)
  - [x] Support "Who" messages to know and verify who is in which sender
- [ ] Make an fukk api helper so implementation is easier
- [ ] Support /api/me?**id** and /api/logout?**id** parameters
- Limit use of the /api/get_chat_history endpoint

## Temporal api help
### Endpoints: 
- /api/ws -> websocket connection
- /api/me?id=<sessid> -> (MeResponse) Verify wether session is expired, default takes cookie as session_id, in the future it will support id get parameter (400 OK = not expired)
- /api/login -> (AuthResponse) returns cookie with session_id
  - POST: Login_Request
- /api/register -> (AuthResponse) returns cookie with session_id and errors if user already exists
  - POST: Login_Request
- /api/logout?id=<sessid> -> errases cookie and closes session given by cookie or id parameter in the future
- /api/get_chat_history?limit=<number> -> responds with the x number of messages last sent (currently exploitable, careful can use a lot of bandwith)
### data_structures
```
LoginRequest {
    "username": "myname",
    "password": "pass",
}
MeResponse {
    "valid": bool,
    "session_token": "null_or_sess_id",
}
AuthResponse {
    "message": "successful auth (is for debugging and optional)",
    "session_token": "token_or_null",
}
```
