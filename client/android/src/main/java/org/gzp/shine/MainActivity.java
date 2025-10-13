package org.gzp.shine;

import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.app.NativeActivity;

public class MainActivity extends NativeActivity {
        static {
            System.loadLibrary("shine");
        }

        @Override
        public void onWindowFocusChanged(boolean hasFocus) {
            super.onWindowFocusChanged(hasFocus);

            if (hasFocus) {
                hideSystemUi();
            }
        }

        private void hideSystemUi() {
            // Use the new WindowInsetsController API (available from API 30+, which matches our minSdk)
            WindowInsetsController controller = getWindow().getInsetsController();
            if (controller != null) {
                controller.hide(WindowInsets.Type.statusBars() | WindowInsets.Type.navigationBars());
                controller.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        }
    }