package org.gzp.shine.auth;

public final class AuthResult {


    public final String sessionCookie;
    public final String refreshCookie;

    public AuthResult() {
        sessionCookie = null;
        refreshCookie = null;
    }

    public AuthResult(String session, String refresh) {
        sessionCookie = session;
        refreshCookie = refresh;
    }

    public boolean isSuccess() {
        return sessionCookie != null;
    }
}
