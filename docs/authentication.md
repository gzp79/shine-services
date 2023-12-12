# Authentication

## Tokens
 
- **access**: 
  - Usually used as a remember-me cookie (tid).
  - Connected to a site, fingerprint has to match
  - Has an expiration time of 1-2 weeks
  - On use it is rotated (see below at the *Token from the `tid` cookie* section)
  - On logout it can be revoked
    - On single logout, only the current token present in the cookie is revoked 
    - On logout from all site, all the **access** tokens are revoked
- **single access**
  - Used to transfer credential from one site to another
  - Not connected to a site, fingerprint is ignored
  - Can be used only once, expire after use
  - Has a short expiration time, a few minutes
- **api key**
  - Created for user request and server as a persistent token
  - Not connected to a site, fingerprint is ignored
  - Has a user defined lifetime, from a few seconds up to a year
  - Revoked only manually
  - Use with caution as it acts as a root password to an account

Supported functions:
- `GET /api/auth/user/{provider}/login` 
  - HTML response with set cookie headers
  - It converts external credentials to an **access** token and a user session.
  - It supports `OAuth2` flow
  - It supports `OpenIdConnect` flow
- `GET /api/auth/user/token/login`
  - HTML response with set cookie headers
  - It convert all kind of tokens to user sessions and **access** tokens. The conversion in priority order:
    1. In query parameter
      - It accepts only **single access** tokens as it has been exposed to public
      - Key should be revoked at all cost as soon as possible
      - On return, it generates (primary, without secondary - see key rotation) **access** key, only on request
      - It ignores and invalidates all the other token sources, clears cookies on return.
    2. In `Authorization` header as Bearer token
      - Accepted tokens:        
        - **api key** used, but not invalidated
      - **access** token is rejected as key rotation (see below) could be complicated and adds no value. 
      - On return, it generates (primary, without secondary - see key rotation) **access** key, only on request
      - It ignores and invalidates all the other token sources, clears cookies on return.
    3. In cookie (with `tid` name)
      - It accepts **access** token with key rotation:
        - There is a primary and an optional secondary key.
        - Any successful login with primary shall revoke the secondary key.
        - If primary key is expired, convert the secondary key to a primary (and hence not revoked). It usually means a failed rotation, where server completed the rotation, but failed to communicate it to the client. We may have some dangling **access** tokens those were never received, used by the clients, but the expiration time shall invalidate them occasionally. (VALIDATE if it really solves the non-updating client issue)
        - It generates a new (primary) key and turn primary into a secondary and update the client cookie.
        - Manual revoke ignores the key rotation, To revoke **access** token, it!s best to perform a log out from all site operation.
          - (VALIDATE if rotated keys shall be connected)
    4. No token:
      - It creates a new guest user, only on request.
      - On return, it generates (primary, without secondary - see key rotation) **access** key.
      - If no rememberMe is set, the response shall be some error.
- `GET /api/auth/user/logout`
  - HTML response with set (clear) cookie headers
  - It revokes the user sessions and **access** tokens. It can revoke either only the current or al the **access** tokens, user sessions
- `GET /api/auth/user/tokens`
  - REST JSON response
  - It lists all the tokens, tokens value are never exposed, they are not stored in the backend, just like any password
- `POST /api/auth/user/tokens`
  - REST JSON response
  - It creates a new **single access** or **api key** token
  - The token value is accessible only once. Only a hash of the token is stored in the DB, thus a lost token cannot be retrieved.
- `DELETE /api/auth/user/tokens/{tokenHash}`
  - REST JSON response
  - It revokes a single token with any type
  - Revoke can be done only by the hash
