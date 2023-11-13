# Time handling in the service

In a single user interaction multiple components have to work together and their time can differ. To avoid any glitch, undefined behavior this time
difference have to be managed carefully. Some assumptions:
- The clocks in the hosted server are using some synchronization solution, thus their difference can be measured in seconds.
- The client may have arbitrary time configuration and clock speed.
  
Some high level guidelines:
- Don't mix the usage of different components, Even having a single component with multiple instance may introduce issues
- If mixed clock is required assume some maximum for the time difference and calculate with these maximums
- For cookie and token lifetime use the the lifetime for the validation and the ttl of the cookie should serve only as a quick pre-filter.


## Identity

Identity creation time
- Mainly for information, not really used in application logic.
- Based on the postgres server
- Usages:
  - Output of sql insert: `InsertIdentity`
  - Output of sql queries: `FindById::created`, `FindByLink::created`, `FindByToken::created`
  - Internal model: `Identity::created`
  - API model: `IdentityInfo`

Identity linking to external provider:
- Mainly for information, not really used in application logic.
- Based on the postgres server
- Usages:
  - Output of the sql insert: `InsertExternalLogin`
  - Output of sql queries: `FindByLink::linked`
  - Internal model: **TBD after (un)link issue has been done**
  - API model: **TBD after (un)link issue has been done**
 
## Token

Token creation time:
- Mainly for information, not really used in application logic.
- Based on the postgres server
- Usages:
  - Output of the sql insert: `InsertToken::created`
  - Output of sql queries: `FindByToken::token_created`
  - Internal model: `TokenInfo`
  - API model: **TBD after revoke issue has been done**

Token expire time:
- Critical time defining the token expiration
- Based on the postgres server and a relative time span.
- Usages:
  - Output of the sql insert: `InsertToken::expire`
  - Output of sql queries: `FindByToken::token_expire`
  - Internal model: `TokenInfo`
  - API `tid` cookie: `TokenLogin::expire`
  - API model: `CreatedToken`, **TBD after revoke issue has been done**
  - The time to live for the `tid` cookie is derived from this (see below)

Token cookie (`tid`) time to live:
- Based on the client clock and derived from the Token expiration time
- Validation flow:
  1. Token is created with the InsertToken sql command and uses the clock of the postgres server.
  2. Cookie expiration is set to a time slightly before the token expiration. It has no real security value just makes the login flow smoother by having a much smaller chance to fail the (token) login with a live `tid` cookie. The ttl of the cookie is irrelevant for the token validation, we just drop the dangling cookies on the client if possible.
  3. During (token) login the postgres time is considered in the sql query command to check the cookie expirations, thus to time of a single domain (postgres) is considered.
  4. Thus in the validation process only the postgres clock is used. Client clock may effect the liveness of cookie and the server time is not used at all.


## Session

Session sentinel creation date:
- Critical time, session expiration is derived from this through redis
- Based on the local server time 
- Usage:
  - Internal model: `SessionSentinel::created_at`, `CurrentUser::session_start`
  - These models are also present in the shared session handler codes
  - Redis key expiration settings is derived from this (see below)
  - Session length is derived from this value

Session length:
- For user notification, an approximate time for session length
- Based on the local server time
- Usage: 
  - Internal model: `CurrentUserInfo::session_length`

Redis session data TTL:
- Critical for session validation, when keys are removed the session expires
- Based on the redis clock.
- Validity:
  1. On session creation a new redis entry is added with a relative ttl.
  2. Session cookie has a scope of `session`, but this is only for convenience for the good clients. Cookie lifetime does not effect the validity of a session, but
  dropping a session on the client side reduces the chance for compromised cookies. Also whenever possible client should log out.
  3. During session validation only the redis key existence is considered. The `created_at` from the sentinel is used only for an approximate session length information.
  4. Session length uses only the server clocks (multiple instance may exist at once) and session validation uses only the redis clock through the key expiration.
  5. For multi region redis the existence of the key in the used node counts and servers should check it only once for a request at the beginning (entry point) even for a longer, multi-service operation. In this case pass the result of the session validation to the other parties as usual.


