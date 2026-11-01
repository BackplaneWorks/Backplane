package dev.backplane.mobile

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat

// The web client's surfaces and semantic colours (src/web/style.css).
// Wallpaper colours must not make ordinary thread controls look like errors.
private fun backplaneScheme(light: Boolean): ColorScheme {
    val bg = Color(if (light) 0xFFFAFAF8 else 0xFF0D0F12)
    val panel = Color(if (light) 0xFFF1F0EC else 0xFF13161B)
    val raised = Color(if (light) 0xFFE8E7E2 else 0xFF1A1E25)
    val line = Color(if (light) 0xFFD3D1CA else 0xFF262B34)
    val text = Color(if (light) 0xFF1B1D21 else 0xFFD9DDE4)
    val dim = Color(if (light) 0xFF5B6068 else 0xFF8A93A3)
    val accent = Color(if (light) 0xFF1F6FD1 else 0xFF5FB3FF)
    val onAccent = Color(if (light) 0xFFFFFFFF else 0xFF06121F)
    val bad = Color(if (light) 0xFFC93434 else 0xFFF06A6A)
    val scheme = if (light) lightColorScheme() else darkColorScheme()
    return scheme.copy(
        primary = accent, onPrimary = onAccent,
        primaryContainer = raised, onPrimaryContainer = text,
        inversePrimary = accent,
        secondary = dim, onSecondary = bg,
        secondaryContainer = raised, onSecondaryContainer = text,
        tertiary = accent, onTertiary = onAccent,
        tertiaryContainer = raised, onTertiaryContainer = text,
        background = bg, onBackground = text,
        surface = bg, onSurface = text,
        surfaceVariant = panel, onSurfaceVariant = dim,
        surfaceTint = Color.Transparent,
        inverseSurface = text, inverseOnSurface = bg,
        error = bad, onError = bg,
        errorContainer = Color(if (light) 0xFFFBE4E6 else 0xFF32131A), onErrorContainer = text,
        outline = dim, outlineVariant = line,
        scrim = Color.Black,
        surfaceBright = raised, surfaceDim = bg,
        surfaceContainerLowest = bg, surfaceContainerLow = panel,
        surfaceContainer = panel, surfaceContainerHigh = raised, surfaceContainerHighest = raised,
    )
}

private val lightScheme = backplaneScheme(true)
private val darkScheme = backplaneScheme(false)
private val square = RoundedCornerShape(0.dp)
private val shapes = Shapes(square, square, square, square, square)
private val typography = Typography().let { t ->
    val mono = FontFamily.Monospace
    t.copy(
        displayLarge = t.displayLarge.copy(fontFamily = mono),
        displayMedium = t.displayMedium.copy(fontFamily = mono),
        displaySmall = t.displaySmall.copy(fontFamily = mono),
        headlineLarge = t.headlineLarge.copy(fontFamily = mono),
        headlineMedium = t.headlineMedium.copy(fontFamily = mono),
        headlineSmall = t.headlineSmall.copy(fontFamily = mono),
        titleLarge = t.titleLarge.copy(fontFamily = mono),
        titleMedium = t.titleMedium.copy(fontFamily = mono),
        titleSmall = t.titleSmall.copy(fontFamily = mono),
        bodyLarge = t.bodyLarge.copy(fontFamily = mono),
        bodyMedium = t.bodyMedium.copy(fontFamily = mono),
        bodySmall = t.bodySmall.copy(fontFamily = mono),
        labelLarge = t.labelLarge.copy(fontFamily = mono),
        labelMedium = t.labelMedium.copy(fontFamily = mono),
        labelSmall = t.labelSmall.copy(fontFamily = mono),
    )
}

// This device's appearance: light, dark, or the system's. Both explicit
// modes keep their contrast even when the system uses the other mode.
@Composable
fun Theme(theme: String, content: @Composable () -> Unit) {
    val dark = when (theme) { "light" -> false; "dark" -> true; else -> isSystemInDarkTheme() }
    val view = LocalView.current
    SideEffect {
        (view.context as? Activity)?.window?.let { window ->
            WindowCompat.getInsetsController(window, view).apply {
                isAppearanceLightStatusBars = !dark
                isAppearanceLightNavigationBars = !dark
            }
        }
    }
    MaterialTheme(colorScheme = if (dark) darkScheme else lightScheme, typography = typography, shapes = shapes, content = content)
}
