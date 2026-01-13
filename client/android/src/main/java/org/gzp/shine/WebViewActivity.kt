package org.gzp.shine

import android.annotation.SuppressLint
import android.content.Intent
import android.os.Bundle
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.core.net.toUri
import org.gzp.shine.auth.AuthConstants

class WebViewActivity : AppCompatActivity() {

    private lateinit var webView: WebView
    private var finished = false // guard against double finish from lifecycle callbacks

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Security check: ensure the caller is from our own package
        val callingPackage = callingActivity?.packageName
        if (callingPackage != null && callingPackage != packageName) {
            setResult(RESULT_CANCELED)
            finish()
            return
        }

        webView = WebView(this)
        setContentView(webView)

        // Basic secure-ish settings
        webView.settings.apply {
            @SuppressLint("SetJavaScriptEnabled")
            javaScriptEnabled = true
            domStorageEnabled = true
            cacheMode = WebSettings.LOAD_DEFAULT
            allowFileAccess = false
            allowContentAccess = false
        }

        // Intercept custom scheme redirects
        webView.webViewClient = object : WebViewClient() {
            private val deepLinkUri = AuthConstants.DEEP_LINK_URL.toUri()

            // For API >= 24
            override fun shouldOverrideUrlLoading(
                view: WebView?,
                request: WebResourceRequest?
            ): Boolean {
                val url = request?.url?.toString() ?: return false
                return handleUrl(url)
            }

            // For older APIs
            override fun shouldOverrideUrlLoading(view: WebView?, url: String?): Boolean {
                return url?.let { handleUrl(it) } ?: false
            }

            private fun handleUrl(url: String): Boolean {
                val uri = url.toUri()
                // change this to your custom scheme / host as needed
                if (uri.scheme == deepLinkUri.scheme &&
                    uri.host == deepLinkUri.host &&
                    uri.path == deepLinkUri.path
                ) {
                    val token = uri.getQueryParameter("token")

                    // return token to caller
                    val data = Intent().apply {
                        putExtra("token", token)
                    }
                    setResult(RESULT_OK, data)

                    // clear cookies/storage and finish
                    clearWebViewData()
                    finishSafely()
                    return true
                }
                return false
            }
        }

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                clearWebViewData()
                finishSafely()
            }
        })

        intent.getStringExtra("w_url")?.let { webView.loadUrl(it) }
    }

    // Close the WebViewActivity when the app goes to background (user switches apps / home)
    override fun onStop() {
        super.onStop()
        // Only clear and finish if not already finished from login/back
        if (!finished) {
            clearWebViewData()
            finishSafely()
        }
    }

    override fun onDestroy() {
        // Properly destroy the WebView
        try {
            webView.apply {
                clearHistory()
                loadUrl("about:blank")
                removeAllViews()
                destroy()
            }
        } catch (_: Exception) {
        }
        super.onDestroy()
    }

    // Helper to clear cookies, web-storage and app cache
    private fun clearWebViewData() {
        try {
            CookieManager.getInstance().removeAllCookies(null)
            CookieManager.getInstance().flush()
        } catch (_: Exception) {
        }
    }

    // guard finish so we don't call it multiple times via lifecycle
    private fun finishSafely() {
        if (finished) return
        finished = true
        finish()
    }
}
