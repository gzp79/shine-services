package org.gzp.shine

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import androidx.browser.customtabs.CustomTabsIntent
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.launch
import org.gzp.shine.auth.AuthConstants
import org.gzp.shine.auth.CurrentUser
import androidx.core.net.toUri
import com.google.androidgamesdk.BuildConfig

class LoginActivity : AppCompatActivity() {
    private val TAG = "Login"

    private var isFlowRunning: Boolean = false;
    private var singleAccessToken: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        handleIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        if (!BuildConfig.REQUIRES_AUTHENTICATION) {
            goToGame()
            return
        }

        if (intent?.action == Intent.ACTION_VIEW && intent.data != null) {
            singleAccessToken = intent.data?.getQueryParameter("token")
        }

        startLoginFlow()
    }

    private fun startLoginFlow() {
        if (isFlowRunning) return
        isFlowRunning = true

        lifecycleScope.launch {
            try {
                if (singleAccessToken != null) {
                    val authResult = authenticateWithToken(singleAccessToken)
                    if (authResult.isError) {
                        showRetryButton()
                        return@launch
                    }
                    singleAccessToken = null;

                    val userResult = getCurrentUser()
                    if (userResult.isSuccess) {
                        goToGame()
                    } else if (userResult.isError) {
                        showRetryButton()
                    } else {
                        launchBrowserForLogin()
                    }
                } else {
                    val userResult = getCurrentUser()
                    if (userResult.isSuccess) {
                        goToGame()
                    } else if (userResult.isError) {
                        showRetryButton()
                    } else {
                        val authResult = authenticateWithRefresh()
                        if (authResult.isError) {
                            showRetryButton()
                            return@launch
                        }

                        val newUserResult = getCurrentUser()
                        if (newUserResult.isSuccess) {
                            goToGame()
                        } else if (newUserResult.isError) {
                            showRetryButton()
                        } else {
                            launchBrowserForLogin()
                        }
                    }
                }
            } finally {
                isFlowRunning = false
            }
        }
    }

    private suspend fun getCurrentUser(): ResultWrapper<CurrentUser?> = withContext(Dispatchers.IO) {
        try {
            val user = (application as MyApp).refreshCurrentUser()
            ResultWrapper.success(user)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to fetch current user", e)
            ResultWrapper.error(e)
        }
    }

    private suspend fun authenticateWithRefresh(): ResultWrapper<Boolean> = withContext(Dispatchers.IO) {
        try {
            val result = (application as MyApp).authAPI.authenticate()
            ResultWrapper.success(result)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to authenticate with cookies", e)
            ResultWrapper.error(e)
        }
    }

    private suspend fun authenticateWithToken(token: String?): ResultWrapper<Boolean?> = withContext(Dispatchers.IO) {
        try {
            val result = (application as MyApp).getAuthAPI().authenticateWithToken(token)
            ResultWrapper.success(result)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to authenticate with token", e)
            ResultWrapper.error(e)
        }
    }

    private fun launchBrowserForLogin() {
        val loginUri = AuthConstants.LOGIN_URL.toUri().buildUpon()
            .appendQueryParameter("redirectUrl", AuthConstants.DEEP_LINK_GAME_URL)
            .appendQueryParameter("prompt", "true")
            .build()
        Log.i(TAG, "No login information, starting web login: $loginUri")
        val intent = CustomTabsIntent.Builder()
            .setShowTitle(false)
            .setUrlBarHidingEnabled(true)
            .build()
        intent.launchUrl(this, loginUri)
    }

    private fun goToGame() {
        val intent = Intent(this, MainActivity::class.java)
        startActivity(intent)
        finish()
    }

    private fun showRetryButton() {
        // TODO: Show a retry button and set its click listener to call handleIntent(intent)
    }
}

// Helper wrapper to distinguish success, error, and null
class ResultWrapper<T>(val value: T?, val error: Exception?) {
    val isSuccess get() = error == null && value != null
    val isError get() = error != null
    companion object {
        fun <T> success(value: T?): ResultWrapper<T> = ResultWrapper(value, null)
        fun <T> error(e: Exception): ResultWrapper<T> = ResultWrapper(null, e)
    }
}