package org.gzp.shine.auth;

public class AuthConstants {
    public static final String API_URL_BASE = "https://scytta.com";
    public static final String TOKEN_LOGIN_URL = API_URL_BASE + "/auth/token/login";

    public static final String WEB_URL_BASE = "https://scytta.com";
    public static final String LOGIN_URL = WEB_URL_BASE + "/login";

    public static final String DEEP_LINK_URL_BASE = "shine://scytta.com";
    public static final String DEEP_LINK_RELATIVE_GAME_URL = "/game/mobile";
    public static final String DEEP_LINK_GAME_URL = DEEP_LINK_URL_BASE + DEEP_LINK_RELATIVE_GAME_URL;

    public static final String SESSION_COOKIE_NAME = "sid";
    public static final String REFRESH_COOKIE_NAME = "tid";
}
