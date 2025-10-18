package org.gzp.shine;

import android.app.Application;

import org.gzp.shine.auth.AuthAPI;
import org.gzp.shine.auth.CurrentUser;
import org.gzp.shine.auth.OkHttpAuthAPI;
import org.gzp.shine.auth.SessionCookieJar;

import java.io.IOException;
import java.security.GeneralSecurityException;

import okhttp3.OkHttpClient;

public class MyApp extends Application {
    private AuthAPI authAPI;
    private CurrentUser currentUser;

    @Override
    public void onCreate() {
        super.onCreate();

        var cookieJar = new SessionCookieJar(this);
        var client = new OkHttpClient.Builder()
                .cookieJar(cookieJar)
                .build();

        authAPI = new OkHttpAuthAPI(client);
    }

    public AuthAPI getAuthAPI()  {
        return authAPI;
    }

    public CurrentUser refreshCurrentUser() throws IOException {
        currentUser = authAPI.getCurrentUser();
        return currentUser;
    }
}
