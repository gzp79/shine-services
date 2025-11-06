package org.gzp.shine.auth;

public class CurrentUser {
    public String userId;
    public String name;
    public boolean isLinked;
    public boolean isEmailConfirmed;
    public String[] roles;
    public long sessionLength;
    public long remainingSessionTime;
}
