package org.gzp.shine.auth;

import android.util.Log;

import com.google.gson.Gson;

import org.jspecify.annotations.Nullable;

import java.io.IOException;

import okhttp3.HttpUrl;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;

public class OkHttpAuthAPI implements AuthAPI {
    private static final String TAG = "AuthAPI";
    private final OkHttpClient client;
    private final Gson gson = new Gson();

    public OkHttpAuthAPI(final OkHttpClient client) {
        this.client = client;
    }

    private SessionCookieJar getCookieJar() {
        return (SessionCookieJar) client.cookieJar();
    }

    @Override
    public @Nullable CurrentUser getCurrentUser() throws IOException {
        if (!getCookieJar().hasSession()) {
            return null;
        }

        Log.d(TAG, "Getting current user info");
        final Request request = new Request.Builder()
                .url(AuthConstants.API_URL_BASE + "/api/auth/user/info")
                .build();
        Log.d(TAG, "Request URL: " + request.url());

        try (final Response response = client.newCall(request).execute()) {
            final int responseCode = response.code();
            Log.d(TAG, "Get user info response code: " + responseCode);

            if (response.isSuccessful()) {
                final String body = response.body().string();
                Log.d(TAG, "Get user info response body: " + body);
                final CurrentUser user = gson.fromJson(body, CurrentUser.class);
                Log.d(TAG, "Parsed user: " + user);
                return user;
            }

            if (responseCode != 200 && responseCode != 401) {
                Log.e(TAG, "Server error fetching user info: " + responseCode);
                throw new IOException("Server error fetching user info: " + responseCode);
            }
        }
        Log.d(TAG, "Failed to get user info, continuing with login.");
        return null;
    }

    @Override
    public boolean authenticate() throws IOException {
        Log.d(TAG, "Authenticating with refresh token");
        if (!getCookieJar().hasRefreshToken()) {
            return false;
        }

        return performTokenLogin(null);
    }

    @Override
    public boolean authenticateWithToken(final String token) throws IOException {
        Log.d(TAG, "Authenticating with token");
        return performTokenLogin(token);
    }

    @Override
    public void logout() {
        Log.d(TAG, "Logging out");
        final Request request = new Request.Builder()
                .url(AuthConstants.API_URL_BASE + "/auth/logout")
                .build();
        Log.d(TAG, "Request URL: " + request.url());

        try (final Response response = client.newCall(request).execute()) {
            if (!response.isSuccessful()) {
                Log.e(TAG, "Server error during logout: " + response.code());
            }
        } catch (IOException e) {
            Log.e(TAG, "Error during logout", e);
        }

        getCookieJar().clear();
    }

    private boolean performTokenLogin(@Nullable final String token) throws IOException {
        final HttpUrl.Builder loginUrlBuilder = HttpUrl.get(AuthConstants.API_URL_BASE + "/auth/token/login").newBuilder();

        if (token != null) {
            loginUrlBuilder.addQueryParameter("token", token);
        }
        loginUrlBuilder.addQueryParameter("redirectUrl", AuthConstants.DEEP_LINK_REDIRECT_URL);

        final HttpUrl url = loginUrlBuilder.build();
        final Request.Builder requestBuilder = new Request.Builder().url(url);
        final Request request = requestBuilder.build();
        Log.d(TAG, "Request URL: " + request.url());

        try (final Response response = client.newCall(request).execute()) {
            final String locationHeader = response.header("Location", "");
            Log.d(TAG, "Token login response code: " + response.code() + ", location: " + locationHeader);
            // A successful login is a redirect to the game URL
            final boolean success = response.code() == 302 && AuthConstants.DEEP_LINK_REDIRECT_URL.equals(locationHeader);
            Log.d(TAG, "Token login success: " + success);
            return success;
        }
    }
}
