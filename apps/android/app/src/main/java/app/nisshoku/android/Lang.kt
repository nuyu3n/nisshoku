package app.nisshoku.android

import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import java.util.Locale

/**
 * The language of the app's screens: the `language` preference (`ja` or
 * `en`), as in the web app and the engine's messages; until it is read, the
 * system's.
 */
object Lang {
    @Volatile var english: Boolean = !Locale.getDefault().language.startsWith("ja")
}

val LocalEnglish = staticCompositionLocalOf { Lang.english }

/** The Japanese or the English text, by the app's language. */
@Composable
@ReadOnlyComposable
fun tr(ja: String, en: String): String = if (LocalEnglish.current) en else ja

/** `tr` outside of composition (notifications, toasts). */
fun trNow(ja: String, en: String): String = if (Lang.english) en else ja
