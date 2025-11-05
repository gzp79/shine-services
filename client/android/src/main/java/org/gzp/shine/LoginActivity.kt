package org.gzp.shine

import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.view.View
import android.widget.Button
import android.widget.ProgressBar
import android.widget.TextView
import androidx.annotation.StringRes
import androidx.appcompat.app.AppCompatActivity
import androidx.browser.customtabs.CustomTabsIntent
import androidx.constraintlayout.widget.ConstraintLayout
import androidx.core.net.toUri
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.gzp.shine.auth.AuthConstants
import org.gzp.shine.auth.CurrentUser

class LoginActivity : AppCompatActivity() {
    companion object {
        private const val TAG = "Login"
        private const val KEY_BROWSER_FLOW_STARTED = "isBrowserFlowStarted"
    }

    private lateinit var progressBar: ProgressBar
    private lateinit var statusText: TextView
    private lateinit var retryButton: Button
    private lateinit var successLayout: ConstraintLayout
    private lateinit var errorLayout: ConstraintLayout
    private lateinit var errorText: TextView

    private var isFlowRunning: Boolean = false
    private var isBrowserFlowStarted: Boolean = false
    private var singleAccessToken: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_login)

        progressBar = findViewById(R.id.progress_bar)
        statusText = findViewById(R.id.status_text)
        retryButton = findViewById(R.id.retry_button)
        successLayout = findViewById(R.id.success_layout)
        errorLayout = findViewById(R.id.error_layout)
        errorText = findViewById(R.id.error_text)

        if (savedInstanceState != null) {
            isBrowserFlowStarted = savedInstanceState.getBoolean(KEY_BROWSER_FLOW_STARTED, false)
        }

        retryButton.setOnClickListener {
            startLoginFlow()
        }

        handleIntent(intent)
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putBoolean(KEY_BROWSER_FLOW_STARTED, isBrowserFlowStarted)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIntent(intent)
    }

    override fun onResume() {
        super.onResume()
        if (isBrowserFlowStarted && intent?.action != Intent.ACTION_VIEW) {
            setErrorProgress(R.string.login_cancelled)
            isBrowserFlowStarted = false
        }
    }

    private fun handleIntent(intent: Intent?) {
        //if (!BuildConfig.REQUIRES_AUTHENTICATION) {
        //  goToGame()
        //  return
        //}

        if (intent?.action == Intent.ACTION_VIEW && intent.data != null) {
            isBrowserFlowStarted = false
            singleAccessToken = intent.data?.getQueryParameter("token")
        }

        startLoginFlow()
    }

    private fun startLoginFlow() {
        if (isFlowRunning) return
        isFlowRunning = true

        resetProgress()
        Log.d(TAG, "Starting login flow" + if (singleAccessToken != null) " with token" else "")

        lifecycleScope.launch {
            try {
                if (singleAccessToken != null) {
                    setProgress(R.string.authenticating_with_token, 10)
                    val authResult = authenticateWithToken(singleAccessToken)
                    if (authResult.isError) {
                        setErrorProgress(R.string.authentication_failed)
                        return@launch
                    }
                    singleAccessToken = null
                }

                setProgress(R.string.fetching_user, 25)
                val userResult = getCurrentUser()
                if (userResult.isSuccess) {
                    goToGame()
                } else if (userResult.isError) {
                    setErrorProgress(R.string.failed_to_fetch_user)
                } else {
                    setProgress(R.string.refreshing_token, 50)
                    val authResult = authenticateWithRefresh()
                    if (authResult.isError) {
                        setErrorProgress(R.string.failed_to_refresh_token)
                        return@launch
                    }

                    setProgress(R.string.fetching_user, 75)
                    val newUserResult = getCurrentUser()
                    if (newUserResult.isSuccess) {
                        goToGame()
                    } else if (newUserResult.isError) {
                        setErrorProgress(R.string.failed_to_fetch_user)
                    } else {
                        launchBrowserForLogin()
                    }
                }
            } finally {
                isFlowRunning = false
            }
        }
    }

    private suspend fun getCurrentUser(): ResultWrapper<CurrentUser?> = withContext(Dispatchers.IO) {
        val startTime = System.currentTimeMillis()
        try {
            val user = (application as MyApp).refreshCurrentUser()
            ResultWrapper.success(user)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to fetch current user", e)
            ResultWrapper.error(e)
        } finally {
            val elapsedTime = System.currentTimeMillis() - startTime
            if (elapsedTime < 500) {
                delay(500 - elapsedTime)
            }
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
            val result = (application as MyApp).authAPI.authenticateWithToken(token)
            ResultWrapper.success(result)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to authenticate with token", e)
            ResultWrapper.error(e)
        }
    }

    private fun launchBrowserForLogin() {
        setProgress(R.string.redirecting_to_login, 100)
        isBrowserFlowStarted = true
        val loginUri = AuthConstants.LOGIN_URL.toUri().buildUpon()
            .appendQueryParameter("redirectUrl", AuthConstants.DEEP_LINK_REDIRECT_URL)
            .appendQueryParameter("prompt", "true")
            .build()
        Log.i(TAG, "No login information, starting web login: $loginUri")
        val customTabsIntent = CustomTabsIntent.Builder()
            .setShowTitle(false)
            .setUrlBarHidingEnabled(true)
            .setShareState(CustomTabsIntent.SHARE_STATE_OFF)
            .build()

        customTabsIntent.intent.setPackage("com.android.chrome")
        customTabsIntent.launchUrl(this, loginUri)
    }

    private fun goToGame() {
        setProgress(R.string.login_successful, 100)
        isBrowserFlowStarted = false
        val intent = Intent(this, MainActivity::class.java)
        startActivity(intent)
        finish()
    }

    private fun resetProgress() {
        Log.d(TAG, "Progress reset")
        successLayout.visibility = View.VISIBLE
        errorLayout.visibility = View.GONE
        progressBar.progress = 0
        statusText.text = ""
    }

    private fun setProgress(@StringRes text: Int, progress: Int) {
        val status = getString(text)
        Log.d(TAG, "Status: $status, progress: $progress%")
        statusText.text = status
        if (progress > progressBar.progress) {
            progressBar.progress = progress
        }
    }

    private fun setErrorProgress(@StringRes messageRes: Int) {
        val status = getString(messageRes)
        Log.d(TAG, "Error: $status")
        successLayout.visibility = View.GONE
        errorLayout.visibility = View.VISIBLE
        errorText.text = status
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
