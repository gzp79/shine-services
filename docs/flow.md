# Flows to entering the site

**There is no distinct sign in or register page, just a simple "enter" page.**
User opens the `Enter` page, and meets the following  options:
  - "Just let me in"
  - External, social linking: github, discord, etc
    - a remember me option

### "Just let me in"

#### New user

It always creates a new user (with the addition describe in the `Returning user` section)
 - A new user is created 
   - with some generated name 
   - no email
   - no link to any external provider
 - A token is created and stored in a persistent storage 
   - in browser it is a cookie
   - in non-browser it can be any persistent (secure?) storage.
 - Player is let into the game and he/she is ready to play. Some information is displayed for the user, where some action have to be done (Also see: `Complete user creation` section)
   - Encourage to link account to a social provider not to loose progress
   - Warn about the time left before the token expires (at most 5 weeks) and without a social link, the account will be inaccessible
   - After linking, to an external provider the flow will be the same as a returning user in the `Social Linking`chapter.

#### Returning user

Using the persisted token, when opening the `Enter` page, the user is redirected to the game without any action.
 
#### Sign out

The feature should be disabled, as a sign out would lock out the user. Instead some warning should be display similar to the one shown to the returning user.

### Social linking

#### New user

After completing the authorization of the external provider
  - A new user is generated
    - Username will be the name returned by provider, if it is not possible, a default generated name is used.
    - Email will be the email returned by the provider (or empty).
    - When the remember me is clicked a token is created in the same way as for the `"just let me in"` flow.
  - Enters the game

If user of the an external provider or the email has already been linked to an account, the user is directed to some error page suggesting to sign in and perform a link operation. (An email can be assigned to exactly one user)

#### Returning user

If there is no remember me token, the user completes the authorization of the external provider and than he is let into the game, if there is a token, there is no interaction required from the user. Opening the `Enter` page will redirect the user into the game without any further action.

#### Sign out

As user account is linked to some external provider, the sign out is a valid operation. We may include a sign out from all host option
as in this flow a user can be signed in from multiple sites.

## Complete user creation

After user is let into the game we may display some page to complete some missing information:
 - Encourage to link to social provider
 - Set or confirm email
 - Accepting End User Agreements
 - etc.

## Account modification

### Alter user name and email

  *Note*: It could be connected to some user progress to increase retention

### Manage linking
 
 - Add new provider. Only the unique id of the provider is checked during linking no email user username change is considered.
 - Delete existing link. Extra caution have to be made not to lock out user by deleting all the linked providers.

### Deleting user

When requested the identity and login credentials will be deleted. All the confirmation shall happen on the UI, server just perform a hard delete.

**Note**: There is a difference between a locked out user, that cannot sign in due to missing credentials and users purged from the identity DB.
