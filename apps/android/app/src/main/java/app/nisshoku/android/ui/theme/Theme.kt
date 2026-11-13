package app.nisshoku.android.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight

/** The accents of the app (the web app's `accent` preference). */
private data class Accent(val light: Color, val onLightContainer: Color, val lightContainer: Color, val dark: Color, val darkContainer: Color)

private val accents = mapOf(
    "orange" to Accent(Color(0xFFB04C1B), Color(0xFF3A1300), Color(0xFFF7E4D7), Color(0xFFFFB590), Color(0xFF7A3510)),
    "vermilion" to Accent(Color(0xFFB3261E), Color(0xFF410002), Color(0xFFF9DEDA), Color(0xFFFFB4A9), Color(0xFF8C1D18)),
    "indigo" to Accent(Color(0xFF3F51A6), Color(0xFF001257), Color(0xFFDDE1FF), Color(0xFFB8C3FF), Color(0xFF26378C)),
    "green" to Accent(Color(0xFF3B6A3E), Color(0xFF00210A), Color(0xFFD8EAD3), Color(0xFF9FD49F), Color(0xFF225128)),
    "sumi" to Accent(Color(0xFF4A4740), Color(0xFF1D1B16), Color(0xFFE6E1D5), Color(0xFFCBC6BB), Color(0xFF4A4740)),
)

/** 日色's own palette (styles/tokens.css): paper, ink, and the accent. */
private fun nisshokuScheme(dark: Boolean, accentName: String): ColorScheme {
    val a = accents[accentName] ?: accents.getValue("orange")
    return if (!dark) {
        lightColorScheme(
            primary = a.light,
            onPrimary = Color.White,
            primaryContainer = a.lightContainer,
            onPrimaryContainer = a.onLightContainer,
            secondary = Color(0xFF69645B),
            onSecondary = Color.White,
            secondaryContainer = Color(0xFFEDE7DA),
            onSecondaryContainer = Color(0xFF2C2A26),
            tertiary = Color(0xFF5E6A3A),
            tertiaryContainer = Color(0xFFE2EBC4),
            background = Color(0xFFFAF9F5),
            onBackground = Color(0xFF2C2A26),
            surface = Color(0xFFFAF9F5),
            onSurface = Color(0xFF2C2A26),
            surfaceVariant = Color(0xFFEDE8DC),
            onSurfaceVariant = Color(0xFF69645B),
            surfaceContainerLowest = Color(0xFFFFFFFF),
            surfaceContainerLow = Color(0xFFF5F2EA),
            surfaceContainer = Color(0xFFF1EDE3),
            surfaceContainerHigh = Color(0xFFEBE6DA),
            surfaceContainerHighest = Color(0xFFE6E1D5),
            outline = Color(0xFF9B958B),
            outlineVariant = Color(0xFFE0DACD),
            error = Color(0xFFC2412D),
        )
    } else {
        darkColorScheme(
            primary = a.dark,
            onPrimary = Color(0xFF1F1B16),
            primaryContainer = a.darkContainer,
            onPrimaryContainer = Color(0xFFFFE6D9),
            secondary = Color(0xFFCFC7B8),
            onSecondary = Color(0xFF34302A),
            secondaryContainer = Color(0xFF45413A),
            onSecondaryContainer = Color(0xFFEDE7DA),
            tertiary = Color(0xFFC3CF9F),
            tertiaryContainer = Color(0xFF454F25),
            background = Color(0xFF262522),
            onBackground = Color(0xFFEAE6DD),
            surface = Color(0xFF262522),
            onSurface = Color(0xFFEAE6DD),
            surfaceVariant = Color(0xFF3A3833),
            onSurfaceVariant = Color(0xFFB9B3A7),
            surfaceContainerLowest = Color(0xFF1E1D1A),
            surfaceContainerLow = Color(0xFF2B2A26),
            surfaceContainer = Color(0xFF302E2A),
            surfaceContainerHigh = Color(0xFF3A3833),
            surfaceContainerHighest = Color(0xFF45423C),
            outline = Color(0xFF8A8478),
            outlineVariant = Color(0xFF45423C),
            error = Color(0xFFFFB4A9),
        )
    }
}

/** Titles of notes in the paper's serif. */
val NoteTitleFamily: FontFamily = FontFamily.Serif

private val base = Typography()

private val typography = base.copy(
    displaySmall = base.displaySmall.copy(fontWeight = FontWeight.Medium),
    headlineLarge = base.headlineLarge.copy(fontWeight = FontWeight.Medium),
    headlineMedium = base.headlineMedium.copy(fontWeight = FontWeight.Medium),
    titleLarge = base.titleLarge.copy(fontWeight = FontWeight.SemiBold),
)

/** What the theme follows (the preferences). */
data class ThemeSettings(
    /** `system`, `light` or `dark`. */
    val theme: String = "system",
    val accent: String = "orange",
    /** The wallpaper's colours (Android 12+), instead of 日色's. */
    val dynamicColor: Boolean = true,
)

@Composable
fun isDark(settings: ThemeSettings): Boolean = when (settings.theme) {
    "dark" -> true
    "light" -> false
    else -> isSystemInDarkTheme()
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun NisshokuTheme(settings: ThemeSettings, content: @Composable () -> Unit) {
    val dark = isDark(settings)
    val context = LocalContext.current
    val scheme = if (settings.dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
    } else {
        nisshokuScheme(dark, settings.accent)
    }
    MaterialExpressiveTheme(
        colorScheme = scheme,
        motionScheme = MotionScheme.expressive(),
        typography = typography,
        content = content,
    )
}

/** The style of a note's title in lists. */
fun TextStyle.noteTitle(): TextStyle = copy(fontFamily = NoteTitleFamily)
