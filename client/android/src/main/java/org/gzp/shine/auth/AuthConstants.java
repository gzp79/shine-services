package org.gzp.shine.auth;

public class AuthConstants {
    public static final String API_URL_BASE = "https://cloud.scytta.com/identity";

    public static final String WEB_URL_BASE = "https://scytta.com";
    public static final String DEEP_LINK_REDIRECT_URL = "/link/shine-open";
    public static final String LOGIN_URL = WEB_URL_BASE + DEEP_LINK_REDIRECT_URL;

    public static final String SESSION_COOKIE_NAME = "sid";
    public static final String REFRESH_COOKIE_NAME = "tid";
}
