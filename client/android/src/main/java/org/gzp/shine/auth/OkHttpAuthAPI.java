package org.gzp.shine.auth;

import java.io.IOException;
import java.util.List;

import okhttp3.Cookie;
import okhttp3.HttpUrl;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;

public class OkHttpAuthAPI implements AuthAPI {
    private final OkHttpClient client;

    public OkHttpAuthAPI(OkHttpClient client) {
        this.client = new OkHttpClient.Builder()
                .followRedirects(false) // Important: handle redirects manually
                .build();
    }

    @Override
    public AuthResult authenticate(String token, TokenKind tokenKind) throws IOException {
        HttpUrl.Builder loginUrlBuilder = HttpUrl.get(AuthConstants.TOKEN_LOGIN_URL).newBuilder();

        if (tokenKind == TokenKind.SingleAccess) {
            loginUrlBuilder.addQueryParameter("token", token);
        }
        loginUrlBuilder.addQueryParameter("redirectUrl", AuthConstants.DEEP_LINK_GAME_URL);

        Request.Builder requestBuilder = new Request.Builder().url(loginUrlBuilder.build());
        if (tokenKind == TokenKind.RefreshCookie) {
            requestBuilder.addHeader("Cookie", AuthConstants.REFRESH_COOKIE_NAME + "=" + token);
        }
        Request request = requestBuilder.build();

        String location;
        List<Cookie> cookies;
        try (Response response = client.newCall(request).execute()) {
            if (response.code() != 302) {
                throw new IOException("Unexpected response code: " + response.code());
            }
            location = response.header("Location", "");
            cookies = Cookie.parseAll(HttpUrl.get(AuthConstants.API_URL_BASE), response.headers());
        }

        if (location == null || !location.equals(AuthConstants.DEEP_LINK_GAME_URL)) {
            // not a redirect to success -> login failure
            return new AuthResult();
        }

        cookies.removeIf(cookie -> cookie.expiresAt() <= System.currentTimeMillis());
        String sessionCookie = cookies.stream()
                .filter(cookie -> cookie.name().equals(AuthConstants.SESSION_COOKIE_NAME))
                .findAny()
                .map(Cookie::value)
                .orElse(null);
        String refreshCookie = cookies.stream()
                .filter(cookie -> cookie.name().equals(AuthConstants.REFRESH_COOKIE_NAME))
                .findAny()
                .map(Cookie::value)
                .orElse(null);

        if (sessionCookie == null) {
            // missing cookie -> login failure
            return new AuthResult();
        }

        return new AuthResult(sessionCookie, refreshCookie);
    }
}