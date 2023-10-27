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
- Based on the Postgres server
- Usages:
  - Output of sql insert: `InsertIdentity`
  - Output of sql queries: `FindById::created`, `FindByLink::created`, `FindByToken::created`
  - Internal model: `Identity::created`
  - API model: `IdentityInfo`

Identity linking to external provider:
- Mainly for information, not really used in application logic.
- Based on the Postgres server
- Usages:
  - Output of the sql insert: `InsertExternalLogin`
  - Output of sql queries: `FindByLink::linked`
  - Internal model: **TBD after (un)link issue has been done**
  - API model: **TBD after (un)link issue has been done**
 
## Token

Token creation time:
- Mainly for information, not really used in application logic.
- Based on the Postgres server
- Usages:
  - Output of the sql insert: `InsertToken::created`
  - Output of sql queries: `FindByToken::token_created`
  - Internal model: `LoginTokenInfo`
  - API model: **TBD after revoke issue has been done**

Token expire time:
- Critical time defining the token expiration
- Based on the Postgres server and a relative time span.
- Usages:
  - Output of the sql insert: `InsertToken::expire`
  - Output of sql queries: `FindByToken::token_expire`
  - Internal model: `LoginTokenInfo`
  - API `tid` cookie: `TokenLogin::expire`
  - API model: `CreatedToken`, **TBD after revoke issue has been done**
  - Cookie (`tid`) expiration settings
  
Application logic, cookie and token validation:
1. Token is created with the InsertToken sql command and uses the clock of the postgres server.
2. Cookie expiration is set to the slightly before this time. It has no security value just makes the login flow smoother by having a much smaller chance to fail the (token) login with a live cookie. The ttl of the cookie is irrelevant for the token validation, we just drop the dangling cookies on the client if possible.
3. During (token) login the Postgres time is considered in the sql query command. 
4. Thus in the validation process only the postgres clock is used. Client clock may effect the liveness of cookie and the server time is not used at all.


## Session

Session sentinel creation date:
- Critical time defining the session expiration
- Based on the local server time, and 
- Usage:
  - Internal model: `CurrentUser::session_start`, `SessionSentinel::start_date`
  - These models are also present in the shared session handler codes
  - Redis key expiration settings

Application logic, cookie and session validation:
TBD!

