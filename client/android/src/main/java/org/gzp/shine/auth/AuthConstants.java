package org.gzp.shine.auth;

import android.net.Uri;

public class AuthConstants {
    public static final String API_URL_BASE = "https://cloud.scytta.com/identity";

    public static final String WEB_URL_BASE = "https://scytta.com";
    public static final String RELATIVE_LOGIN_URL = "/link/shine-open";
    public static final String LOGIN_URL = WEB_URL_BASE + RELATIVE_LOGIN_URL;
    public static final Uri LOGIN_URI = Uri.parse(LOGIN_URL);
    public static final String DEEP_LINK_URL = WEB_URL_BASE + "/public/mobile";

    public static final String SESSION_COOKIE_NAME = "sid";
    public static final String REFRESH_COOKIE_NAME = "tid";
}
