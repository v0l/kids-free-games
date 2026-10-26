package io.v0l.kids_games;

import android.graphics.Insets;
import android.os.Build;
import android.os.Bundle;
import android.util.DisplayMetrics;
import android.view.DisplayCutout;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowManager;

import com.google.androidgamesdk.GameActivity;

public class MainActivity extends GameActivity {
    static {
        System.loadLibrary("kids_games");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        if (Build.VERSION.SDK_INT >= 28) {
            getWindow().getAttributes().layoutInDisplayCutoutMode =
                    WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        }
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            getWindow().getDecorView().setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                            | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_FULLSCREEN);
        }
    }

    public int[] safeAreaInsets() {
        int[] out = new int[] { 0, 0, 0, 0 };
        WindowInsets insets = getWindow().getDecorView().getRootWindowInsets();
        if (insets == null) {
            return out;
        }
        if (Build.VERSION.SDK_INT >= 30) {
            Insets bars = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            Insets gestures = insets.getInsets(WindowInsets.Type.mandatorySystemGestures());
            out[0] = bars.left;
            out[1] = bars.top;
            out[2] = bars.right;
            out[3] = Math.max(bars.bottom, gestures.bottom);
            return out;
        }
        out[0] = insets.getSystemWindowInsetLeft();
        out[1] = insets.getSystemWindowInsetTop();
        out[2] = insets.getSystemWindowInsetRight();
        out[3] = insets.getSystemWindowInsetBottom();
        if (Build.VERSION.SDK_INT >= 28) {
            DisplayCutout cut = insets.getDisplayCutout();
            if (cut != null) {
                out[0] = Math.max(out[0], cut.getSafeInsetLeft());
                out[1] = Math.max(out[1], cut.getSafeInsetTop());
                out[2] = Math.max(out[2], cut.getSafeInsetRight());
                out[3] = Math.max(out[3], cut.getSafeInsetBottom());
            }
        }
        return out;
    }

    public float pixelsPerMm() {
        DisplayMetrics m = getResources().getDisplayMetrics();
        float dpi = (m.xdpi + m.ydpi) / 2.0f;
        if (dpi < 100.0f || dpi > 900.0f) {
            dpi = m.densityDpi;
        }
        return dpi / 25.4f;
    }
}
