package org.gzp.shine.auth;

import org.jspecify.annotations.Nullable;

import java.io.IOException;


public interface AuthAPI {
    /// Get the user information based on the current cookies
    @Nullable
    CurrentUser getCurrentUser() throws IOException;

    /// Try to authenticate using the current cookie
    boolean authenticate() throws IOException;

    ///  Try to authenticate using the single access token
    boolean authenticateWithToken(String token) throws IOException;

    /// Logs out the user by clearing all cookies
    void logout();
}
