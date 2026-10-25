//% IMPORTS
import android.view.DisplayCutout;
//% END
//% MAIN_ACTIVITY_ON_CREATE
        layout.setOnApplyWindowInsetsListener(new View.OnApplyWindowInsetsListener() {
            @Override
            public WindowInsets onApplyWindowInsets(View v, WindowInsets insets) {
                v.setPadding(0, 0, 0, 0);
                return insets;
            }
        });
        layout.setBackgroundColor(Color.TRANSPARENT);
//% END
//% MAIN_ACTIVITY_BODY
    public int[] safeAreaInsets() {
        int[] out = new int[] { 0, 0, 0, 0 };
        if (Build.VERSION.SDK_INT < 23) {
            return out;
        }
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
        android.util.DisplayMetrics m = getResources().getDisplayMetrics();
        float dpi = (m.xdpi + m.ydpi) / 2.0f;
        if (dpi < 100.0f || dpi > 900.0f) {
            dpi = m.densityDpi;
        }
        return dpi / 25.4f;
    }
//% END
