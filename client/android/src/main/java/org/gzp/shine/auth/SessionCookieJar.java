package org.gzp.shine.auth;

import android.content.Context;
import android.content.SharedPreferences;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;

import androidx.annotation.NonNull;
import androidx.security.crypto.EncryptedSharedPreferences;
import androidx.security.crypto.MasterKey;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.security.GeneralSecurityException;
import java.security.InvalidAlgorithmParameterException;
import java.security.InvalidKeyException;
import java.security.KeyStore;
import java.security.KeyStoreException;
import java.security.NoSuchAlgorithmException;
import java.security.NoSuchProviderException;
import java.security.UnrecoverableEntryException;
import java.security.cert.CertificateException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.List;

import javax.crypto.BadPaddingException;
import javax.crypto.Cipher;
import javax.crypto.IllegalBlockSizeException;
import javax.crypto.KeyGenerator;
import javax.crypto.NoSuchPaddingException;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

import kotlin.jvm.Volatile;
import okhttp3.Cookie;
import okhttp3.CookieJar;
import okhttp3.HttpUrl;

/// Store session and refresh cookies securely Session cookie is stored in
/// memory only to mimic the browsers session behavior Refresh cookie is stored
/// encrypted in SharedPreferences
public class SessionCookieJar implements CookieJar {
    @Volatile
    private String sessionCookie = null;

    private final SharedPreferences prefs;
    private final SecretKey secretKey;

    private final String REFRESH_TOKEN_KEY = "refresh_token";
    private static final int IV_SIZE = 12;

    public SessionCookieJar(Context context) {
        prefs = context.getSharedPreferences("secure_cookies", Context.MODE_PRIVATE);
        secretKey = getOrCreateSecretKey();
    }

    public void clear() {
        sessionCookie = null;
        prefs.edit().remove(REFRESH_TOKEN_KEY).apply();
    }

    public void setSessionCookie(String value) {
        sessionCookie = value;
    }

    public void setRefreshCookie(String value) {
        if (value != null) {
            String encrypted = encrypt(value);
            prefs.edit().putString(REFRESH_TOKEN_KEY, encrypted).apply();
        } else {
            prefs.edit().remove(REFRESH_TOKEN_KEY).apply();
        }
    }

    public boolean hasSession() {
        return sessionCookie != null;
    }

    public boolean hasRefreshToken() {
        return prefs.contains(REFRESH_TOKEN_KEY);
    }

    @Override
    public void saveFromResponse(@NonNull HttpUrl url, List<Cookie> cookies) {
        for (Cookie c : cookies) {
            if (AuthConstants.SESSION_COOKIE_NAME.equalsIgnoreCase(c.name())) {
                setSessionCookie(c.expiresAt() > System.currentTimeMillis() ? c.value() : null);
            } else if (AuthConstants.REFRESH_COOKIE_NAME.equalsIgnoreCase(c.name())) {
                setRefreshCookie(c.expiresAt() > System.currentTimeMillis() ? c.value() : null);
            }
        }
    }

    @NonNull
    @Override
    public List<Cookie> loadForRequest(@NonNull HttpUrl url) {
        List<Cookie> result = new ArrayList<>();

        String s = sessionCookie;
        if (s != null) {
            result.add(new Cookie.Builder()
                    .name(AuthConstants.SESSION_COOKIE_NAME)
                    .value(s)
                    .build());
        }

        String refreshToken = prefs.getString(REFRESH_TOKEN_KEY, null);
        if (refreshToken != null) {
            var decrypted = decrypt(refreshToken);
            Cookie rememberCookie = new Cookie.Builder()
                    .name(AuthConstants.REFRESH_COOKIE_NAME)
                    .value(decrypted)
                    .build();
            result.add(rememberCookie);
        }

        return result;
    }

    private SecretKey getOrCreateSecretKey() {
        try {
            KeyStore keyStore = KeyStore.getInstance("AndroidKeyStore");
            keyStore.load(null);

            String alias = "cookie_master_key";

            if (!keyStore.containsAlias(alias)) {
                KeyGenerator keyGenerator = KeyGenerator.getInstance("AES", "AndroidKeyStore");
                keyGenerator.init(
                        new android.security.keystore.KeyGenParameterSpec.Builder(
                                alias,
                                android.security.keystore.KeyProperties.PURPOSE_ENCRYPT |
                                        android.security.keystore.KeyProperties.PURPOSE_DECRYPT)
                                .setBlockModes(android.security.keystore.KeyProperties.BLOCK_MODE_GCM)
                                .setEncryptionPaddings(android.security.keystore.KeyProperties.ENCRYPTION_PADDING_NONE)
                                .setKeySize(256)
                                .build());
                return keyGenerator.generateKey();
            } else {
                KeyStore.SecretKeyEntry entry = (KeyStore.SecretKeyEntry) keyStore.getEntry(alias, null);
                return entry.getSecretKey();
            }
        } catch (InvalidAlgorithmParameterException | UnrecoverableEntryException | CertificateException
                | KeyStoreException | IOException | NoSuchAlgorithmException | NoSuchProviderException e) {
            throw new RuntimeException(e);
        }
    }

    private String encrypt(String plainText) {
        try {
            Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
            cipher.init(Cipher.ENCRYPT_MODE, secretKey);

            byte[] iv = cipher.getIV();
            byte[] ciphertext = cipher.doFinal(plainText.getBytes(StandardCharsets.UTF_8));

            byte[] combined = new byte[iv.length + ciphertext.length];
            System.arraycopy(iv, 0, combined, 0, iv.length);
            System.arraycopy(ciphertext, 0, combined, iv.length, ciphertext.length);

            return Base64.getEncoder().encodeToString(combined);
        } catch (NoSuchPaddingException | IllegalBlockSizeException | NoSuchAlgorithmException | BadPaddingException
                | InvalidKeyException e) {
            throw new RuntimeException(e);
        }
    }

    private String decrypt(String encrypted) {
        try {
            byte[] combined = Base64.getDecoder().decode(encrypted);
            byte[] iv = Arrays.copyOfRange(combined, 0, IV_SIZE);
            byte[] ciphertext = Arrays.copyOfRange(combined, IV_SIZE, combined.length);

            Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
            GCMParameterSpec spec = new GCMParameterSpec(128, iv);
            cipher.init(Cipher.DECRYPT_MODE, secretKey, spec);

            byte[] plainBytes = cipher.doFinal(ciphertext);
            return new String(plainBytes, StandardCharsets.UTF_8);
        } catch (NoSuchPaddingException | IllegalBlockSizeException | NoSuchAlgorithmException
                | InvalidAlgorithmParameterException | BadPaddingException | InvalidKeyException e) {
            throw new RuntimeException(e);
        }
    }
}
